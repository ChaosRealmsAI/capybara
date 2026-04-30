use std::fs;
use std::path::PathBuf;

use capy_project::{ArtifactKind, ProjectGenerateRequestV1, ProjectPackage};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::Value;

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy project --help` as the index and `capy help project` for the full workflow.
  Common commands: init, inspect, workbench, generate, add-design, add-artifact.
  Required params: all commands use --project <dir>; generate needs --artifact, --provider, and --prompt.
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
    #[command(about = "Print the six-card project workbench view")]
    Workbench(ProjectPathArgs),
    #[command(about = "Generate or plan one project artifact through CLI providers")]
    Generate(ProjectGenerateArgs),
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

#[derive(Debug, Clone, ValueEnum)]
enum GenerateProviderArg {
    Fixture,
    Codex,
    Claude,
}

impl GenerateProviderArg {
    fn as_str(&self) -> &'static str {
        match self {
            GenerateProviderArg::Fixture => "fixture",
            GenerateProviderArg::Codex => "codex",
            GenerateProviderArg::Claude => "claude",
        }
    }
}

#[derive(Debug, Args)]
struct ProjectGenerateArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    artifact: String,
    #[arg(long, value_enum, default_value = "fixture")]
    provider: GenerateProviderArg,
    #[arg(long)]
    prompt: String,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    write: bool,
    #[arg(long)]
    out: Option<PathBuf>,
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
        ProjectCommand::Workbench(args) => {
            let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
            serde_json::to_value(package.workbench().map_err(|err| err.to_string())?)
        }
        ProjectCommand::Generate(args) => {
            if args.dry_run && args.write {
                return Err("choose either --dry-run or --write, not both".to_string());
            }
            let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
            let result = package
                .generate(ProjectGenerateRequestV1 {
                    artifact_id: args.artifact,
                    provider: args.provider.as_str().to_string(),
                    prompt: args.prompt,
                    dry_run: !args.write,
                })
                .map_err(|err| err.to_string())?;
            if let Some(out) = args.out.as_ref() {
                write_json_file(out, &result)?;
            }
            serde_json::to_value(result)
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

fn write_json_file<T: serde::Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    fs::write(path, format!("{payload}\n"))
        .map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn print_json(data: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}
