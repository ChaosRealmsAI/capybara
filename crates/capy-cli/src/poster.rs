use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Args)]
pub struct PosterArgs {
    #[command(subcommand)]
    command: PosterCommand,
}

#[derive(Debug, Subcommand)]
enum PosterCommand {
    #[command(about = "Validate a Capybara Poster JSON document")]
    Validate(PosterValidateArgs),
    #[command(about = "Compile Poster JSON into NextFrame render_source.v1")]
    Compile(PosterCompileArgs),
    #[command(about = "Compile Poster JSON and snapshot it through nf-recorder")]
    Snapshot(PosterSnapshotArgs),
}

#[derive(Debug, Args)]
struct PosterValidateArgs {
    #[arg(long)]
    input: PathBuf,
}

#[derive(Debug, Args)]
struct PosterCompileArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = 1000)]
    duration_ms: u64,
}

#[derive(Debug, Args)]
struct PosterSnapshotArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    render_source_out: Option<PathBuf>,
    #[arg(long, default_value_t = 1000)]
    duration_ms: u64,
    #[arg(long, default_value_t = 0)]
    t_ms: u64,
    #[arg(long)]
    resolution: Option<String>,
    #[arg(long)]
    recorder: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct CommandSummary {
    command: Vec<String>,
    status: i32,
    stdout: String,
    stderr: String,
}

pub fn handle(args: PosterArgs) -> Result<(), String> {
    match args.command {
        PosterCommand::Validate(args) => validate(args),
        PosterCommand::Compile(args) => compile(args),
        PosterCommand::Snapshot(args) => snapshot(args),
    }
}

fn validate(args: PosterValidateArgs) -> Result<(), String> {
    let document = capy_poster::read_document(&args.input).map_err(|err| err.to_string())?;
    print_json(&json!({
        "ok": true,
        "input": args.input,
        "version": document.version,
        "canvas": document.canvas,
        "layers": document.layers.len(),
        "assets": document.assets.len(),
        "generated_assets": document.assets.values().filter(|asset| asset.provenance.is_some()).count()
    }))
}

fn compile(args: PosterCompileArgs) -> Result<(), String> {
    let report = compile_to_file(PosterCompileArgs {
        input: args.input.clone(),
        out: args.out.clone(),
        duration_ms: args.duration_ms,
    })?;
    print_json(&json!({
        "ok": true,
        "input": args.input,
        "out": args.out,
        "schema_version": report.render_source_schema,
        "duration_ms": report.duration_ms,
        "render_source_path": report.render_source_path,
        "compile_mode": report.compile_mode,
        "deprecated": "capy poster compile forwards to capy nextframe compile"
    }))
}

fn snapshot(args: PosterSnapshotArgs) -> Result<(), String> {
    let render_source = args
        .render_source_out
        .clone()
        .unwrap_or_else(|| default_render_source_path(&args.out));
    let compile_args = PosterCompileArgs {
        input: args.input.clone(),
        out: render_source.clone(),
        duration_ms: args.duration_ms,
    };
    compile_to_file(compile_args)?;

    if let Some(parent) = args
        .out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create output directory {}: {err}", parent.display()))?;
    }

    let recorder = resolve_recorder(args.recorder);
    let validate = run_validate_source(&recorder, &render_source)?;
    let snapshot = run_snapshot_source(
        &recorder,
        &render_source,
        &args.out,
        args.t_ms,
        args.resolution.as_deref(),
    )?;
    print_json(&json!({
        "ok": true,
        "input": args.input,
        "render_source": render_source,
        "output": args.out,
        "recorder": recorder,
        "validate": validate,
        "snapshot": snapshot
    }))
}

fn compile_to_file(args: PosterCompileArgs) -> Result<capy_nextframe::CompileReport, String> {
    let output = legacy_render_source_path(&args.out);
    let composition_dir = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let compose = capy_nextframe::compose_poster(capy_nextframe::ComposePosterRequest {
        poster_path: args.input,
        project_slug: None,
        composition_id: None,
        output_dir: Some(composition_dir),
        duration_ms: args.duration_ms,
    })
    .map_err(|err| err.body.message)?;
    let report = capy_nextframe::compile_composition(capy_nextframe::CompileCompositionRequest {
        composition_path: compose.composition_path,
        strict_binary: false,
    });
    if !report.ok {
        let message = report
            .errors
            .first()
            .map(|err| format!("{}: {} · {}", err.code, err.message, err.hint))
            .unwrap_or_else(|| {
                "COMPILE_FAILED: compile failed · next step · rerun capy nextframe validate"
                    .to_string()
            });
        return Err(message);
    }
    if report.render_source_path != output {
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create output directory {}: {err}", parent.display()))?;
        }
        std::fs::copy(&report.render_source_path, &output).map_err(|err| {
            format!(
                "copy render_source {} to {}: {err}",
                report.render_source_path.display(),
                output.display()
            )
        })?;
    }
    Ok(report)
}

fn legacy_render_source_path(out: &Path) -> PathBuf {
    if out.extension().is_some() {
        out.to_path_buf()
    } else {
        out.join("render_source.json")
    }
}

fn default_render_source_path(output: &Path) -> PathBuf {
    output.with_extension("render_source.json")
}

fn resolve_recorder(arg: Option<PathBuf>) -> PathBuf {
    arg.or_else(|| std::env::var_os("CAPY_NF_RECORDER").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("nf-recorder"))
}

fn run_validate_source(recorder: &Path, source: &Path) -> Result<CommandSummary, String> {
    run_command(
        recorder,
        &[
            "validate-source".to_string(),
            "--source".to_string(),
            source.display().to_string(),
        ],
    )
}

fn run_snapshot_source(
    recorder: &Path,
    source: &Path,
    output: &Path,
    t_ms: u64,
    resolution: Option<&str>,
) -> Result<CommandSummary, String> {
    let mut args = vec![
        "snapshot-source".to_string(),
        "--source".to_string(),
        source.display().to_string(),
        "--t-ms".to_string(),
        t_ms.to_string(),
        "--output".to_string(),
        output.display().to_string(),
    ];
    if let Some(resolution) = resolution.filter(|value| !value.trim().is_empty()) {
        args.push("--resolution".to_string());
        args.push(resolution.to_string());
    }
    run_command(recorder, &args)
}

fn run_command(program: &Path, args: &[String]) -> Result<CommandSummary, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("spawn {}: {err}", program.display()))?;
    let summary = command_summary(program, args, output);
    if summary.status != 0 {
        return Err(format!(
            "command failed ({}): {}\nstdout: {}\nstderr: {}",
            summary.status,
            summary.command.join(" "),
            summary.stdout,
            summary.stderr
        ));
    }
    Ok(summary)
}

fn command_summary(program: &Path, args: &[String], output: Output) -> CommandSummary {
    let mut command = vec![program.display().to_string()];
    command.extend(args.iter().cloned());
    CommandSummary {
        command,
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    }
}

fn print_json<T: Serialize>(data: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}
