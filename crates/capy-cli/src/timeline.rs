use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

mod live;
mod report;

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy timeline --help` as the index and `capy timeline help <topic>` for full workflows.
  Common flow: doctor -> compose-poster -> validate -> compile -> snapshot/export -> verify-export.
  Video editor flow: launch shell, then `capy timeline open --composition <composition.json>`.
  Required params: composition commands need --composition; attach needs --canvas-node; open needs --composition or --canvas-node.
  Pitfalls: validate/compile before export; run rebuild after token changes.
  Help topics: `capy timeline help poster-export`, `capy timeline help live`."
)]
pub struct TimelineArgs {
    #[command(subcommand)]
    command: TimelineCommand,
}

#[derive(Debug, Subcommand)]
enum TimelineCommand {
    #[command(about = "Check Timeline binary adapter availability")]
    Doctor(TimelineDoctorArgs),
    #[command(about = "Compose Poster JSON into a Timeline composition project")]
    ComposePoster(TimelineComposePosterArgs),
    #[command(about = "Validate a Timeline composition JSON document")]
    Validate(TimelineValidateArgs),
    #[command(about = "Compile a Timeline composition JSON document")]
    Compile(TimelineCompileArgs),
    #[command(about = "Rebuild a branded Timeline composition when tokens changed")]
    Rebuild(TimelineRebuildArgs),
    #[command(about = "Render a single PNG snapshot from a compiled Timeline composition")]
    Snapshot(TimelineSnapshotArgs),
    #[command(about = "Export MP4 from a compiled Timeline composition")]
    Export(TimelineExportArgs),
    #[command(about = "Run validate, compile, snapshot, export, and write evidence HTML")]
    VerifyExport(TimelineVerifyExportArgs),
    #[command(about = "Attach a Timeline composition to a live canvas node")]
    Attach(TimelineAttachArgs),
    #[command(about = "Read live Timeline attachment state from capy-shell")]
    State(TimelineStateArgs),
    #[command(about = "Read a live Timeline export job from capy-shell")]
    Status(TimelineStatusArgs),
    #[command(about = "Cancel a live Timeline export job tracked by capy-shell")]
    Cancel(TimelineCancelArgs),
    #[command(
        about = "Open a live Timeline composition preview or video editor in the desktop host"
    )]
    Open(TimelineOpenArgs),
    #[command(about = "Show self-contained AI help topics for Timeline")]
    Help(TimelineHelpArgs),
}

#[derive(Debug, Args)]
struct TimelineHelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args)]
struct TimelineDoctorArgs {
    #[arg(long)]
    recorder: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineComposePosterArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    brand_tokens: Option<PathBuf>,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    composition: Option<String>,
    #[arg(long, default_value_t = 1000)]
    duration_ms: u64,
}

#[derive(Debug, Args)]
struct TimelineValidateArgs {
    #[arg(long)]
    composition: PathBuf,
}

#[derive(Debug, Args)]
struct TimelineCompileArgs {
    #[arg(long)]
    composition: PathBuf,
}

#[derive(Debug, Args)]
struct TimelineRebuildArgs {
    #[arg(long)]
    composition: PathBuf,
}

#[derive(Debug, Args)]
struct TimelineSnapshotArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long, default_value_t = 0)]
    frame: u64,
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineExportArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long, default_value = "mp4")]
    kind: String,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long, default_value_t = 30)]
    fps: u32,
    #[arg(long, default_value = "draft")]
    profile: String,
    #[arg(long)]
    resolution: Option<String>,
    #[arg(long)]
    parallel: Option<usize>,
    #[arg(long)]
    strict_recorder: bool,
}

#[derive(Debug, Args)]
struct TimelineVerifyExportArgs {
    #[arg(long)]
    composition: PathBuf,
    #[arg(long)]
    out_html: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineAttachArgs {
    #[arg(long)]
    canvas_node: u64,
    #[arg(long)]
    composition: PathBuf,
    #[arg(long)]
    socket: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct TimelineStateArgs {
    #[arg(long)]
    canvas_node: Option<u64>,
}

#[derive(Debug, Args)]
struct TimelineStatusArgs {
    #[arg(long)]
    job: String,
}

#[derive(Debug, Args)]
struct TimelineCancelArgs {
    #[arg(long)]
    job: String,
}

#[derive(Debug, Args)]
struct TimelineOpenArgs {
    #[arg(long)]
    canvas_node: Option<u64>,
    #[arg(long)]
    composition: Option<PathBuf>,
    #[arg(long)]
    socket: Option<PathBuf>,
}

pub fn handle(args: TimelineArgs) -> Result<(), String> {
    match args.command {
        TimelineCommand::Doctor(args) => doctor(args),
        TimelineCommand::ComposePoster(args) => compose_poster(args),
        TimelineCommand::Validate(args) => validate(args),
        TimelineCommand::Compile(args) => compile(args),
        TimelineCommand::Rebuild(args) => rebuild(args),
        TimelineCommand::Snapshot(args) => snapshot(args),
        TimelineCommand::Export(args) => export(args),
        TimelineCommand::VerifyExport(args) => verify_export(args),
        TimelineCommand::Attach(args) => live::attach(args),
        TimelineCommand::State(args) => live::state(args),
        TimelineCommand::Status(args) => live::status(args),
        TimelineCommand::Cancel(args) => live::cancel(args),
        TimelineCommand::Open(args) => live::open(args),
        TimelineCommand::Help(args) => {
            crate::help_topics::print_timeline_topic(args.topic.as_deref())
        }
    }
}

fn doctor(args: TimelineDoctorArgs) -> Result<(), String> {
    let report = capy_timeline::doctor(capy_timeline::TimelineConfig {
        recorder_bin: args.recorder,
        home: args.home,
    });
    print_json(&report)
}

fn compose_poster(args: TimelineComposePosterArgs) -> Result<(), String> {
    let request = capy_timeline::ComposePosterRequest {
        poster_path: args.input,
        brand_tokens_path: args.brand_tokens,
        project_slug: args.project,
        composition_id: args.composition,
        output_dir: args.out,
        duration_ms: args.duration_ms,
    };
    match capy_timeline::compose_poster(request) {
        Ok(report) => print_json(&report),
        Err(err) => {
            print_json(&capy_timeline::compose::failure(err))?;
            std::process::exit(1);
        }
    }
}

fn rebuild(args: TimelineRebuildArgs) -> Result<(), String> {
    let report = capy_timeline::rebuild(capy_timeline::RebuildRequest {
        composition_path: args.composition,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn validate(args: TimelineValidateArgs) -> Result<(), String> {
    let report = capy_timeline::validate_composition(capy_timeline::ValidateCompositionRequest {
        composition_path: args.composition,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn compile(args: TimelineCompileArgs) -> Result<(), String> {
    let report = capy_timeline::compile_composition(capy_timeline::CompileCompositionRequest {
        composition_path: args.composition,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn snapshot(args: TimelineSnapshotArgs) -> Result<(), String> {
    let report = capy_timeline::snapshot::snapshot(capy_timeline::snapshot::SnapshotRequest {
        composition_path: args.composition,
        frame_ms: args.frame,
        out: args.out,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn export(args: TimelineExportArgs) -> Result<(), String> {
    let kind = match args.kind.as_str() {
        "mp4" => capy_timeline::ExportKind::Mp4,
        _ => {
            let report = report::export_failure(
                "UNSUPPORTED_EXPORT_KIND",
                format!("unsupported export kind: {}", args.kind),
                "next step · pass --kind mp4",
            );
            print_json(&report)?;
            std::process::exit(1);
        }
    };
    let report = capy_timeline::export_composition(capy_timeline::ExportCompositionRequest {
        composition_path: args.composition,
        kind,
        out: args.out,
        fps: args.fps,
        profile: args.profile,
        resolution: args.resolution,
        parallel: args.parallel,
        strict_recorder: args.strict_recorder,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn verify_export(args: TimelineVerifyExportArgs) -> Result<(), String> {
    let report = capy_timeline::verify_export(capy_timeline::VerifyExportRequest {
        composition_path: args.composition,
        out_html: args.out_html,
    });
    print_json(&report)?;
    if report.ok {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

pub(super) fn print_json<T: Serialize>(data: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}

pub(super) fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|err| format!("read cwd failed: {err}"))
}
