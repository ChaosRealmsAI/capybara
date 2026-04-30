use std::path::PathBuf;

use capy_project::{ArtifactKind, ProjectPackage};
use clap::{Args, Subcommand};
use serde_json::Value;

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy project --help` as the index and `capy help project` for the full workflow.
  Common commands: init, inspect, add-design, add-artifact.
  Required params: all commands use --project <dir>; add-design/add-artifact need --path and --title.
  Pitfalls: paths must live inside the project root; this command writes only the local `.capy` file package.
  Help topic: `capy help project`."
)]
pub struct ProjectArgs {
    #[command(subcommand)]
    command: ProjectCommand,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    #[command(about = "Create a .capy project package")]
    Init(ProjectInitArgs),
    #[command(about = "Inspect a .capy project package")]
    Inspect(ProjectPathArgs),
    #[command(about = "Register an AI-readable design-language asset")]
    AddDesign(ProjectAddDesignArgs),
    #[command(about = "Register a source artifact")]
    AddArtifact(ProjectAddArtifactArgs),
}

#[derive(Debug, Args)]
struct ProjectPathArgs {
    #[arg(long)]
    project: PathBuf,
}

#[derive(Debug, Args)]
struct ProjectInitArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    name: Option<String>,
}

#[derive(Debug, Args)]
struct ProjectAddDesignArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    kind: String,
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: Option<String>,
}

#[derive(Debug, Args)]
struct ProjectAddArtifactArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    kind: ArtifactKind,
    #[arg(long)]
    title: String,
    #[arg(long = "design-ref")]
    design_refs: Vec<String>,
}

pub fn handle(args: ProjectArgs) -> Result<(), String> {
    let data = match args.command {
        ProjectCommand::Init(args) => {
            let package =
                ProjectPackage::init(args.project, args.name).map_err(|err| err.to_string())?;
            serde_json::to_value(package.inspect().map_err(|err| err.to_string())?)
        }
        ProjectCommand::Inspect(args) => {
            let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
            serde_json::to_value(package.inspect().map_err(|err| err.to_string())?)
        }
        ProjectCommand::AddDesign(args) => {
            let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
            serde_json::to_value(
                package
                    .add_design_asset(args.kind, args.path, args.title, args.description)
                    .map_err(|err| err.to_string())?,
            )
        }
        ProjectCommand::AddArtifact(args) => {
            let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
            serde_json::to_value(
                package
                    .add_artifact(args.kind, args.path, args.title, args.design_refs)
                    .map_err(|err| err.to_string())?,
            )
        }
    }
    .map_err(|err| err.to_string())?;
    print_json(&data)
}

fn print_json(data: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}
