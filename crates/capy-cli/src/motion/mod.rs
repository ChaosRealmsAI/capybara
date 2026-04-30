mod html;
mod metrics;
mod model;
mod process;
mod prompts;
mod verify;

use std::fs;
use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

use model::{PackagePaths, manifest_json};
use process::{
    command_available, export_videos, extract_frames, normalize_cutout_outputs,
    normalize_frame_count, prepare_output_dir, probe_source, run_cutout_batch, run_self_json,
    write_cutout_manifest, write_json, write_source_contact,
};
use verify::{inspect_manifest, verify_manifest};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy motion cutout --input <mp4> --out <dir> --quality animation --target all --verify --overwrite` for animation-grade transparent motion assets.
  Required params: cutout needs --input and --out; verify/inspect need --manifest; prompt-pack needs --input and --out; preview needs --package.
  Pitfalls: standard H.264 MP4 has no alpha; use the generated PNG sequence, WebM alpha, sprite atlas, or RGB+Alpha dual MP4. Full-video cutout can be slow.
  Help topics: `capy motion help agent`, `capy motion help manifest`, `capy motion help prompt-pack`, `capy motion help qa`, `capy motion help preview`."
)]
pub struct MotionArgs {
    #[command(subcommand)]
    command: MotionCommand,
}

#[derive(Debug, Subcommand)]
enum MotionCommand {
    #[command(about = "Check local motion cutout dependencies")]
    Doctor,
    #[command(about = "Convert MP4 into an animation-grade transparent motion asset package")]
    Cutout(CutoutArgs),
    #[command(about = "Verify a motion asset manifest and QA report")]
    Verify(VerifyArgs),
    #[command(about = "Inspect a motion package for app/game integration")]
    Inspect(VerifyArgs),
    #[command(about = "Write AI handoff, process, QA, and app integration prompts")]
    PromptPack(PromptPackArgs),
    #[command(about = "Serve a motion package preview over local HTTP")]
    Preview(PreviewArgs),
    #[command(about = "Show self-contained motion asset help topics")]
    Help(HelpArgs),
}

#[derive(Debug, Args)]
struct HelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    #[arg(long)]
    manifest: PathBuf,
}

#[derive(Debug, Args)]
struct PromptPackArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(
        long,
        help = "Optional existing motion package root used to embed current QA context"
    )]
    package: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct PreviewArgs {
    #[arg(long, value_name = "DIR")]
    package: PathBuf,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 5332)]
    port: u16,
}

#[derive(Debug, Args)]
struct CutoutArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, value_enum, default_value = "animation")]
    quality: MotionQuality,
    #[arg(long, value_enum, default_value = "all")]
    target: MotionTarget,
    #[arg(long)]
    overwrite: bool,
    #[arg(long)]
    verify: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(
        long,
        help = "Limit extracted frames for smoke runs; omitted means full video"
    )]
    max_frames: Option<u32>,
    #[arg(long, default_value_t = 2048)]
    mask_max_side: u32,
    #[arg(long)]
    full_res_mask: bool,
    #[arg(
        long,
        help = "Reuse existing frames/source, frames/rgba, and masks to rebuild QA, manifest, preview, and exports"
    )]
    reuse_existing: bool,
    #[arg(long, help = "Write PM-readable evidence HTML outside the package")]
    evidence_index: Option<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
enum MotionQuality {
    Animation,
}

#[derive(Debug, Clone, ValueEnum)]
enum MotionTarget {
    All,
}

pub fn handle(args: MotionArgs) -> Result<(), String> {
    match args.command {
        MotionCommand::Doctor => crate::print_json(&doctor()),
        MotionCommand::Cutout(args) => cutout(args).and_then(|value| crate::print_json(&value)),
        MotionCommand::Verify(args) => {
            verify_manifest(&args.manifest).and_then(|value| crate::print_json(&value))
        }
        MotionCommand::Inspect(args) => {
            inspect_manifest(&args.manifest).and_then(|value| crate::print_json(&value))
        }
        MotionCommand::PromptPack(args) => {
            prompt_pack(args).and_then(|value| crate::print_json(&value))
        }
        MotionCommand::Preview(args) => preview(args),
        MotionCommand::Help(args) => crate::help_topics::print_motion_topic(args.topic.as_deref()),
    }
}

fn doctor() -> Value {
    let ffmpeg = command_available("ffmpeg", &["-version"]);
    let ffprobe = command_available("ffprobe", &["-version"]);
    let cutout = run_self_json(&["cutout", "doctor"])
        .unwrap_or_else(|err| json!({ "ok": false, "error": err }));
    json!({
        "ok": ffmpeg["ok"] == true && ffprobe["ok"] == true && cutout.get("ok").and_then(Value::as_bool).unwrap_or(false),
        "schema": "capy.motion.doctor.v1",
        "ffmpeg": ffmpeg,
        "ffprobe": ffprobe,
        "cutout": cutout,
        "commands": ["doctor", "cutout", "verify", "inspect", "prompt-pack", "preview", "help"]
    })
}

fn cutout(args: CutoutArgs) -> Result<Value, String> {
    if args.dry_run {
        return Ok(dry_run_plan(&args));
    }
    if !args.input.is_file() {
        return Err(format!("input video not found: {}", args.input.display()));
    }
    let paths = PackagePaths::new(&args.out);
    if args.reuse_existing {
        if !paths.root.is_dir() {
            return Err(format!(
                "reuse package does not exist: {}",
                paths.root.display()
            ));
        }
    } else {
        prepare_output_dir(&args.out, args.overwrite)?;
    }
    for dir in paths.all_dirs() {
        fs::create_dir_all(dir).map_err(|err| format!("create {} failed: {err}", dir.display()))?;
    }
    let meta = probe_source(&args.input, &paths)?;
    write_source_contact(&args.input, &paths)?;
    let cutout_summary = if args.reuse_existing {
        json!({
            "ok": true,
            "reused_existing": true,
            "source_frames": paths.source_frames_dir,
            "rgba_frames": paths.rgba_frames_dir,
            "masks": paths.masks_dir
        })
    } else {
        extract_frames(&args, &paths)?;
        let frame_count = normalize_frame_count(&paths.source_frames_dir)?;
        let batch_manifest = write_cutout_manifest(&paths, frame_count)?;
        let summary = run_cutout_batch(&args, &paths, &batch_manifest)?;
        normalize_cutout_outputs(&paths, frame_count)?;
        summary
    };
    let frame_count = normalize_frame_count(&paths.source_frames_dir)?;
    let metrics = metrics::analyze_and_build(&paths, meta.fps)?;
    export_videos(&paths, &meta)?;
    html::write_preview(&paths, &metrics.report)?;
    let manifest = manifest_json(
        &args.input,
        &meta,
        &paths,
        &metrics.report,
        &metrics.warnings,
    );
    let prompt_pack = prompts::write_package_prompt_pack(
        &paths,
        &args.input,
        Some(&manifest),
        Some(&metrics.report),
    )?;
    write_json(&paths.root.join("manifest.json"), &manifest)?;
    if args.verify {
        let verify = verify_manifest(&paths.root.join("manifest.json"))?;
        if verify.get("verdict").and_then(Value::as_str) != Some("passed") {
            return Err(format!("motion package verify failed: {verify}"));
        }
    }
    let command_json = json!({
        "ok": true,
        "schema": "capy.motion.cutout.v1",
        "input": args.input,
        "out": paths.root,
        "quality": "animation",
        "target": "all",
        "frame_count": frame_count,
        "manifest": paths.root.join("manifest.json"),
        "preview_html": paths.qa_dir.join("preview.html"),
        "qa_report": paths.qa_dir.join("report.json"),
        "prompt_pack": prompt_pack,
        "cutout_summary": compact_cutout_summary(&cutout_summary, &paths),
        "verdict": metrics.report.get("verdict").cloned().unwrap_or_else(|| json!("draft"))
    });
    if let Some(index_path) = args.evidence_index {
        html::write_evidence_index(
            &index_path,
            &paths.root,
            &args.input,
            &meta,
            &metrics.report,
            &command_json,
        )?;
    }
    Ok(command_json)
}

fn dry_run_plan(args: &CutoutArgs) -> Value {
    json!({
        "ok": true,
        "dry_run": true,
        "schema": "capy.motion.cutout-plan.v1",
        "input": args.input,
        "out": args.out,
        "quality": "animation",
        "target": "all",
        "max_frames": args.max_frames,
        "reuse_existing": args.reuse_existing,
        "files": [
            "source/metadata.json",
            "source/contact.jpg",
            "frames/source/frame_000001.png",
            "frames/rgba/frame_000001.png",
            "frames/cropped/frame_000001.png",
            "masks/frame_000001.png",
            "atlas/walk.png",
            "atlas/walk.json",
            "video/preview.webm",
            "video/rgb.mp4",
            "video/alpha.mp4",
            "qa/preview.html",
            "qa/report.json",
            "prompts/README.md",
            "prompts/process.md",
            "prompts/qa-review.md",
            "prompts/app-integration.md",
            "manifest.json"
        ]
    })
}

fn prompt_pack(args: PromptPackArgs) -> Result<Value, String> {
    prompts::write_standalone_prompt_pack(&args.out, &args.input, args.package.as_deref())
}

fn preview(args: PreviewArgs) -> Result<(), String> {
    let root = args
        .package
        .canonicalize()
        .map_err(|err| format!("motion package not found: {err}"))?;
    let preview = root.join("qa/preview.html");
    if !preview.is_file() {
        return Err(format!(
            "motion preview not found: {}; run `capy motion cutout` first",
            preview.display()
        ));
    }
    println!(
        "capy motion preview http://{}:{}/qa/preview.html -> {}",
        args.host,
        args.port,
        root.display()
    );
    capy_scroll_media::serve_static(capy_scroll_media::ServeOptions {
        root,
        host: args.host,
        port: args.port,
    })
    .map_err(|err| err.to_string())
}

fn compact_cutout_summary(summary: &Value, paths: &PackagePaths) -> Value {
    json!({
        "ok": summary.get("ok").cloned().unwrap_or(Value::Null),
        "engine": summary.get("engine").cloned().unwrap_or(Value::Null),
        "reused_existing": summary.get("reused_existing").cloned().unwrap_or(json!(false)),
        "performance": summary.get("performance").cloned().unwrap_or(Value::Null),
        "report": paths.logs_dir.join("cutout-batch-report.json"),
        "frame_reports": summary
            .get("reports")
            .and_then(Value::as_array)
            .map(|reports| reports.len())
            .unwrap_or(0)
    })
}
