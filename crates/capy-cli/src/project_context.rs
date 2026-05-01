use std::fs;
use std::path::PathBuf;

use capy_project::{ContextBuildRequest, ProjectPackage};
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy context build --project <dir> --artifact <id>` to produce a capy.context.v1 JSON packet.
  Required params: build needs --project and --artifact; --selector, --json-pointer, or --canvas-node add precise selection context; --out writes the packet to disk.
  Video projects: the same build command adds read-only video_project_context when source media, clip queue, or proposal history exists.
  Pitfalls: context building does not call models, mutate queue, or apply proposal history; it packages project/design/artifact/selection state for a later AI run.
  Help topic: `capy help context`."
)]
pub struct ContextArgs {
    #[command(subcommand)]
    command: ContextCommand,
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    #[command(about = "Build a capy.context.v1 packet")]
    Build(ContextBuildArgs),
}

#[derive(Debug, Args)]
struct ContextBuildArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    artifact: String,
    #[arg(long)]
    selector: Option<String>,
    #[arg(long = "canvas-node")]
    canvas_node: Option<String>,
    #[arg(long = "json-pointer")]
    json_pointer: Option<String>,
    #[arg(long)]
    out: Option<PathBuf>,
}

pub fn handle(args: ContextArgs) -> Result<(), String> {
    let ContextCommand::Build(args) = args.command;
    let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
    let context = package
        .build_context(ContextBuildRequest {
            artifact_id: args.artifact,
            selector: args.selector,
            canvas_node: args.canvas_node,
            json_pointer: args.json_pointer,
        })
        .map_err(|err| err.to_string())?;
    let payload = serde_json::to_string_pretty(&context).map_err(|err| err.to_string())?;
    if let Some(out) = args.out {
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create context output dir failed: {err}"))?;
        }
        fs::write(&out, format!("{payload}\n"))
            .map_err(|err| format!("write context output failed: {err}"))?;
    }
    println!("{payload}");
    Ok(())
}
