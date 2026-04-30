use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

const FOCUS_MODEL_FILES: [&str; 4] = [
    "depth_anything_v2_vits_slim.onnx",
    "isnet.onnx",
    "focus_matting_1.0.0.onnx",
    "focus_refiner_1.0.0.onnx",
];

const PYTHON_PACKAGES: [&str; 5] = [
    "withoutbg==1.0.3",
    "onnxruntime==1.25.1",
    "pillow==12.2.0",
    "numpy==2.4.4",
    "huggingface_hub==1.12.0",
];

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy cutout --help` as the index and `capy cutout help <topic>` for full workflows.
  Common commands: `capy cutout doctor`, `capy cutout init`, `capy cutout run ...`, `capy cutout batch ...`.
  Required params: run needs --input and --output; batch needs --manifest and --out-dir.
  Pitfalls: first run needs a local Focus runtime/model cache; source images should use neutral gray with clear subject/background separation, and QA previews are required for PM-visible assets.
  Help topics: `capy cutout help agent`, `capy cutout help manifest`."
)]
pub struct CutoutCliArgs {
    #[command(subcommand)]
    command: CutoutCommand,
}

#[derive(Debug, Subcommand)]
enum CutoutCommand {
    #[command(about = "Check Focus cutout runtime without downloading models")]
    Doctor(CutoutDoctorArgs),
    #[command(about = "Create the local Focus runtime and download model weights")]
    Init(CutoutInitArgs),
    #[command(about = "Cut one generated asset with withoutbg/focus")]
    Run(CutoutRunArgs),
    #[command(about = "Cut a manifest of generated assets with withoutbg/focus")]
    Batch(CutoutBatchArgs),
    #[command(about = "Show self-contained AI help topics for cutout")]
    Help(CutoutHelpArgs),
}

#[derive(Debug, Args)]
struct CutoutHelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args, Clone)]
struct CutoutRuntimeArgs {
    #[arg(
        long,
        help = "Override Focus cache root; defaults to a user cache directory"
    )]
    cache_dir: Option<PathBuf>,
    #[arg(
        long,
        help = "Override Python executable; defaults to cache venv Python"
    )]
    python: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CutoutDoctorArgs {
    #[command(flatten)]
    runtime: CutoutRuntimeArgs,
}

#[derive(Debug, Args)]
struct CutoutInitArgs {
    #[command(flatten)]
    runtime: CutoutRuntimeArgs,
    #[arg(long, help = "Skip pip install and only download/check model files")]
    skip_pip: bool,
}

#[derive(Debug, Args)]
struct CutoutRunArgs {
    #[command(flatten)]
    runtime: CutoutRuntimeArgs,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, help = "Write the alpha mask PNG")]
    mask_out: Option<PathBuf>,
    #[arg(long, help = "Write black/white/deep QA previews to this directory")]
    qa_dir: Option<PathBuf>,
    #[arg(long, help = "Write JSON report to this path")]
    report: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = 2048,
        help = "Resize longest side for mask inference before applying alpha to original RGB"
    )]
    mask_max_side: u32,
    #[arg(long, help = "Run mask inference at full source resolution")]
    full_res_mask: bool,
}

#[derive(Debug, Args)]
struct CutoutBatchArgs {
    #[command(flatten)]
    runtime: CutoutRuntimeArgs,
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    out_dir: PathBuf,
    #[arg(long, help = "Write batch summary JSON to this path")]
    report: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = 2048,
        help = "Resize longest side for mask inference before applying alpha to original RGB"
    )]
    mask_max_side: u32,
    #[arg(long, help = "Run mask inference at full source resolution")]
    full_res_mask: bool,
}

#[derive(Debug, Clone)]
struct RuntimePaths {
    cache_dir: PathBuf,
    venv_dir: PathBuf,
    hf_cache_dir: PathBuf,
    python: PathBuf,
    runner: PathBuf,
}

pub fn handle(args: CutoutCliArgs) -> Result<(), String> {
    let output = match args.command {
        CutoutCommand::Doctor(args) => doctor(&args.runtime)?,
        CutoutCommand::Init(args) => init(&args)?,
        CutoutCommand::Run(args) => run_one(&args)?,
        CutoutCommand::Batch(args) => run_batch(&args)?,
        CutoutCommand::Help(args) => {
            crate::help_topics::print_cutout_topic(args.topic.as_deref())?;
            return Ok(());
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn doctor(args: &CutoutRuntimeArgs) -> Result<Value, String> {
    let paths = runtime_paths(args)?;
    if !paths.python.is_file() {
        return Ok(json!({
            "ok": false,
            "engine": "withoutbg/focus",
            "kind": "cutout-doctor",
            "cache_dir": paths.cache_dir,
            "python": {
                "ok": false,
                "path": paths.python,
                "hint": "run `capy cutout init` or pass --python"
            },
            "runner": path_status(&paths.runner),
            "model_files": model_file_status(&paths.hf_cache_dir)
        }));
    }
    run_runner(
        &paths,
        "doctor",
        json!({
            "cache_dir": paths.cache_dir,
            "hf_cache_dir": paths.hf_cache_dir,
            "model_files": FOCUS_MODEL_FILES,
        }),
    )
}

fn init(args: &CutoutInitArgs) -> Result<Value, String> {
    let paths = runtime_paths(&args.runtime)?;
    std::fs::create_dir_all(&paths.cache_dir).map_err(|err| {
        format!(
            "create cutout cache directory failed: {}: {err}",
            paths.cache_dir.display()
        )
    })?;
    std::fs::create_dir_all(&paths.hf_cache_dir).map_err(|err| {
        format!(
            "create model cache directory failed: {}: {err}",
            paths.hf_cache_dir.display()
        )
    })?;

    let bootstrap_python = args
        .runtime
        .python
        .clone()
        .unwrap_or_else(|| PathBuf::from("python3"));
    if args.runtime.python.is_none() && !paths.python.is_file() {
        run_status(
            Command::new(&bootstrap_python)
                .arg("-m")
                .arg("venv")
                .arg(&paths.venv_dir),
            "create Focus Python venv",
        )?;
    }

    if !args.skip_pip {
        let mut pip = Command::new(&paths.python);
        pip.arg("-m")
            .arg("pip")
            .arg("install")
            .arg("--upgrade")
            .arg("pip")
            .args(PYTHON_PACKAGES);
        run_status(&mut pip, "install Focus Python packages")?;
    }

    let download = run_runner(
        &paths,
        "download",
        json!({
            "cache_dir": paths.cache_dir,
            "hf_cache_dir": paths.hf_cache_dir,
            "model_files": FOCUS_MODEL_FILES,
        }),
    )?;
    Ok(json!({
        "ok": download.get("ok").and_then(Value::as_bool).unwrap_or(false),
        "engine": "withoutbg/focus",
        "kind": "cutout-init",
        "cache_dir": paths.cache_dir,
        "python": paths.python,
        "packages": PYTHON_PACKAGES,
        "download": download,
    }))
}

fn run_one(args: &CutoutRunArgs) -> Result<Value, String> {
    let paths = runtime_paths(&args.runtime)?;
    require_python(&paths)?;
    let mask_max_side = mask_max_side(args.mask_max_side, args.full_res_mask);
    run_runner(
        &paths,
        "cut",
        json!({
            "cache_dir": paths.cache_dir,
            "hf_cache_dir": paths.hf_cache_dir,
            "model_files": FOCUS_MODEL_FILES,
            "input": args.input,
            "output": args.output,
            "mask_out": args.mask_out,
            "qa_dir": args.qa_dir,
            "report": args.report,
            "mask_max_side": mask_max_side,
        }),
    )
}

fn run_batch(args: &CutoutBatchArgs) -> Result<Value, String> {
    let paths = runtime_paths(&args.runtime)?;
    require_python(&paths)?;
    let mask_max_side = mask_max_side(args.mask_max_side, args.full_res_mask);
    run_runner(
        &paths,
        "batch",
        json!({
            "cache_dir": paths.cache_dir,
            "hf_cache_dir": paths.hf_cache_dir,
            "model_files": FOCUS_MODEL_FILES,
            "manifest": args.manifest,
            "out_dir": args.out_dir,
            "report": args.report,
            "mask_max_side": mask_max_side,
        }),
    )
}

fn mask_max_side(value: u32, full_res: bool) -> u32 {
    if full_res { 0 } else { value.max(256) }
}

fn runtime_paths(args: &CutoutRuntimeArgs) -> Result<RuntimePaths, String> {
    let cache_dir = args.cache_dir.clone().unwrap_or_else(default_cache_dir);
    let venv_dir = cache_dir.join("venv");
    let hf_cache_dir = cache_dir.join("hf");
    let python = args
        .python
        .clone()
        .unwrap_or_else(|| venv_python(&venv_dir));
    let runner = runner_path()?;
    Ok(RuntimePaths {
        cache_dir,
        venv_dir,
        hf_cache_dir,
        python,
        runner,
    })
}

fn runner_path() -> Result<PathBuf, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| "cannot resolve repository root from CARGO_MANIFEST_DIR".to_string())?;
    Ok(root.join("scripts/capy-focus-cutout.py"))
}

#[cfg(windows)]
fn venv_python(venv_dir: &Path) -> PathBuf {
    venv_dir.join("Scripts").join("python.exe")
}

#[cfg(not(windows))]
fn venv_python(venv_dir: &Path) -> PathBuf {
    venv_dir.join("bin").join("python")
}

fn default_cache_dir() -> PathBuf {
    if let Some(value) = env::var_os("CAPY_CUTOUT_CACHE") {
        return PathBuf::from(value);
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Caches")
                .join("capybara")
                .join("cutout")
                .join("focus");
        }
    }
    #[cfg(windows)]
    {
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local)
                .join("Capybara")
                .join("cutout")
                .join("focus");
        }
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("capybara")
            .join("cutout")
            .join("focus");
    }
    PathBuf::from(".capybara-cache")
        .join("cutout")
        .join("focus")
}

fn require_python(paths: &RuntimePaths) -> Result<(), String> {
    if paths.python.is_file() {
        return Ok(());
    }
    Err(format!(
        "Focus runtime missing: {}. Run `capy cutout init` first or pass --python.",
        paths.python.display()
    ))
}

fn run_runner(paths: &RuntimePaths, operation: &str, input: Value) -> Result<Value, String> {
    let mut child = Command::new(&paths.python)
        .arg(&paths.runner)
        .arg(operation)
        .env("HF_HUB_DISABLE_XET", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            format!(
                "spawn Focus cutout runner failed: python={} runner={} error={err}",
                paths.python.display(),
                paths.runner.display()
            )
        })?;
    let input_text = serde_json::to_string(&input).map_err(|err| err.to_string())?;
    let Some(stdin) = child.stdin.as_mut() else {
        return Err("Focus cutout runner stdin unavailable".to_string());
    };
    stdin
        .write_all(input_text.as_bytes())
        .map_err(|err| format!("write Focus cutout runner input failed: {err}"))?;
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .map_err(|err| format!("wait for Focus cutout runner failed: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(format!(
            "Focus cutout runner failed: {}",
            if stderr.is_empty() { stdout } else { stderr }
        ));
    }
    serde_json::from_str(&stdout)
        .map_err(|err| format!("parse Focus cutout runner JSON failed: {err}; stdout={stdout}"))
}

fn run_status(command: &mut Command, label: &str) -> Result<(), String> {
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("{label} failed to start: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(format!(
        "{label} failed: {}",
        if stderr.is_empty() { stdout } else { stderr }
    ))
}

fn path_status(path: &Path) -> Value {
    json!({
        "ok": path.exists(),
        "path": path,
        "is_file": path.is_file(),
        "is_dir": path.is_dir(),
    })
}

fn model_file_status(cache_dir: &Path) -> Vec<Value> {
    FOCUS_MODEL_FILES
        .iter()
        .map(|name| {
            json!({
                "name": name,
                "found": find_file(cache_dir, name).map(|path| path.display().to_string()),
            })
        })
        .collect()
}

fn find_file(root: &Path, name: &str) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry_result in entries {
        let entry = entry_result.ok()?;
        let path = entry.path();
        if path.file_name().and_then(|value| value.to_str()) == Some(name) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        }
    }
    None
}
