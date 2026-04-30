use std::fs;
use std::path::PathBuf;

use capy_project::{ProjectPackage, ProjectVideoClipQueueManifestV1};
use clap::{Args, Subcommand};
use serde_json::Value;

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use when: AI or desktop verification needs to inspect, seed, or locally suggest the project-level linear video clip queue.
  Required params: every command needs --project <dir>; write also needs --manifest <queue.json>.
  Outputs: inspect/write return capy.project-video-clip-queue.v1; analyze/semantics return capy.project-video-clip-semantics.v1; suggest returns capy.project-video-clip-suggestion.v1 with ordered items and semantic reasons when analysis exists.
  Pitfalls: this is a linear edit queue, not a multi-track NLE; analyze and suggest are no-spend deterministic local planners and must not call paid providers; all paths must live inside the project root.
  Next step: run analyze, then suggest, then adopt through desktop UI or write a queue manifest; verify with semantics/inspect and a real export.")]
pub(crate) struct ProjectClipQueueArgs {
    #[command(subcommand)]
    command: ProjectClipQueueCommand,
}

#[derive(Debug, Subcommand)]
enum ProjectClipQueueCommand {
    #[command(about = "Inspect .capy/video-clip-queue.json")]
    Inspect(ProjectClipQueuePathArgs),
    #[command(about = "Write .capy/video-clip-queue.json from a manifest JSON file")]
    Write(ProjectClipQueueWriteArgs),
    #[command(
        about = "Analyze clip queue semantics and persist them in Project Core",
        after_help = "AI quick start:
  Use when: the video workspace needs per-clip Chinese summaries, tags, rhythm, use-case, and recommendation reasons before suggestion.
  Required params: --project <dir>.
  Output: capy.project-video-clip-semantics.v1 JSON written to .capy/video-clip-semantics.json.
  State effects: writes project metadata through Project Core and touches project.json updated_at.
  Do not: call paid model providers, analyze files outside the project root, or expand the queue into a full NLE.
  Verify: run project clip-queue semantics --project <dir>, then project clip-queue suggest --project <dir> and check semantic_reason fields."
    )]
    Analyze(ProjectClipQueuePathArgs),
    #[command(
        about = "Inspect persisted clip semantic analysis",
        after_help = "AI quick start:
  Use when: AI needs to confirm Project Core has persisted video clip semantic results after analysis or reopen.
  Required params: --project <dir>.
  Output: capy.project-video-clip-semantics.v1 JSON; missing analysis returns an empty manifest.
  State effects: read-only.
  Do not: treat an empty manifest as analyzed; run analyze first when items[] is empty.
  Verify: items[] should contain summary_zh, tags, rhythm, use_case, and recommendation for each queue segment."
    )]
    Semantics(ProjectClipQueuePathArgs),
    #[command(
        about = "Suggest an explainable linear queue from project videos, semantics, and the persisted queue",
        after_help = "AI quick start:
  Use when: a no-spend local planner should propose an explainable clip order before the user adopts it.
  Required params: --project <dir>.
  Output: capy.project-video-clip-suggestion.v1 JSON with suggestion_id, rationale, source_video_count, existing_queue_count, and items[] containing source, range, duration, reason, and semantic_reason when analysis exists.
  State effects: read-only; it does not mutate .capy/video-clip-queue.json.
  Do not: treat this as creative model output, call provider SDKs, or add transitions/subtitles/audio mixing.
  Verify: run project clip-queue analyze first, confirm semantic_reason in suggestions, inspect before and after adoption, then export the adopted queue."
    )]
    Suggest(ProjectClipQueuePathArgs),
}

#[derive(Debug, Args)]
struct ProjectClipQueuePathArgs {
    #[arg(long)]
    project: PathBuf,
}

#[derive(Debug, Args)]
struct ProjectClipQueueWriteArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    manifest: PathBuf,
}

pub(crate) fn handle_clip_queue(args: ProjectClipQueueArgs) -> Result<Value, serde_json::Error> {
    match args.command {
        ProjectClipQueueCommand::Inspect(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(package.video_clip_queue().map_err(string_json_error)?)
        }
        ProjectClipQueueCommand::Write(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            let raw = fs::read_to_string(&args.manifest).map_err(string_json_error)?;
            let manifest = serde_json::from_str::<ProjectVideoClipQueueManifestV1>(&raw)?;
            serde_json::to_value(
                package
                    .write_video_clip_queue(manifest.items)
                    .map_err(string_json_error)?,
            )
        }
        ProjectClipQueueCommand::Analyze(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(
                package
                    .analyze_video_clip_semantics()
                    .map_err(string_json_error)?,
            )
        }
        ProjectClipQueueCommand::Semantics(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(package.video_clip_semantics().map_err(string_json_error)?)
        }
        ProjectClipQueueCommand::Suggest(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(
                package
                    .suggest_video_clip_queue()
                    .map_err(string_json_error)?,
            )
        }
    }
}

fn string_json_error(error: impl ToString) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::other(error.to_string()))
}
