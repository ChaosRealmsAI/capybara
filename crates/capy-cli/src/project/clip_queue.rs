use std::fs;
use std::path::PathBuf;

use capy_project::{ProjectPackage, ProjectVideoClipQueueManifestV1};
use clap::{Args, Subcommand};
use serde_json::Value;

#[derive(Debug, Args)]
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
    }
}

fn string_json_error(error: impl ToString) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::other(error.to_string()))
}
