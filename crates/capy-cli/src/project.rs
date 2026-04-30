use std::fs;
use std::path::PathBuf;

use capy_project::{
    ArtifactKind, ProjectCampaignRequestV1, ProjectGenerateRequestV1, ProjectPackage,
};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::Value;

mod clip_queue;
mod live;

use clip_queue::{ProjectClipQueueArgs, handle_clip_queue};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy project --help` as the index and `capy help project` for the full workflow.
  Common commands: init, inspect, design-language inspect, design-language validate, workbench, import-video, clip-queue inspect|write|suggest, generate, campaign, run, add-design, add-artifact.
  Required params: all commands use --project <dir>; generate needs --artifact, --provider, and --prompt; selected target context uses --selector, --json-pointer, or --canvas-node; campaign needs --brief; clip-queue write needs --manifest; clip-queue suggest is read-only; live SDK generation also needs --live; run decisions need a run id.
  Pitfalls: paths must live inside the project root; import-video uses local ffprobe/ffmpeg only; clip-queue is a linear manifest, not an NLE; validate design language before live generation; use --review for AI proposals; accept is the only review action that mutates source files.
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
    #[command(about = "Inspect or validate the active project design-language package")]
    DesignLanguage(ProjectDesignLanguageArgs),
    #[command(about = "Print the six-card project workbench view")]
    Workbench(ProjectPathArgs),
    #[command(about = "Import one local project video and materialize preview metadata")]
    ImportVideo(ProjectImportVideoArgs),
    #[command(about = "Inspect, write, or suggest the project-level video clip queue")]
    ClipQueue(ProjectClipQueueArgs),
    #[command(about = "Generate or plan one project artifact through CLI providers")]
    Generate(ProjectGenerateArgs),
    #[command(about = "Review, accept, reject, retry, or undo AI project runs")]
    Run(ProjectRunArgs),
    #[command(about = "Plan, generate, and inspect multi-artifact campaigns")]
    Campaign(ProjectCampaignArgs),
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
struct ProjectDesignLanguageArgs {
    #[command(subcommand)]
    command: ProjectDesignLanguageCommand,
}

#[derive(Debug, Subcommand)]
enum ProjectDesignLanguageCommand {
    #[command(about = "Inspect the active project design-language package")]
    Inspect(ProjectPathArgs),
    #[command(about = "Validate design-language refs and local asset paths")]
    Validate(ProjectPathArgs),
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
    role: Option<String>,
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

#[derive(Debug, Args)]
struct ProjectImportVideoArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    path: PathBuf,
    #[arg(long)]
    title: Option<String>,
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
    #[arg(
        long,
        help = "CSS selector or stable DOM selector for selected target context"
    )]
    selector: Option<String>,
    #[arg(
        long = "json-pointer",
        help = "JSON Pointer for selected target context"
    )]
    json_pointer: Option<String>,
    #[arg(
        long = "canvas-node",
        help = "Visible canvas/surface node id for selected context"
    )]
    canvas_node: Option<String>,
    #[arg(
        long,
        help = "Call the real Claude/Codex SDK and turn its JSON output into a patch"
    )]
    live: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(
        long,
        help = "Record a proposed AI change for review without mutating source"
    )]
    review: bool,
    #[arg(long)]
    write: bool,
    #[arg(long, help = "Provider model override for live SDK generation")]
    model: Option<String>,
    #[arg(long, help = "Reasoning effort for live SDK generation")]
    effort: Option<String>,
    #[arg(
        long = "sdk-response",
        help = "Read an SDK JSON response fixture instead of calling the provider"
    )]
    sdk_response: Option<PathBuf>,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long = "save-prompt", help = "Write the live AI prompt packet to JSON")]
    save_prompt: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ProjectRunArgs {
    #[command(subcommand)]
    command: ProjectRunCommand,
}

#[derive(Debug, Args)]
struct ProjectCampaignArgs {
    #[command(subcommand)]
    command: ProjectCampaignCommand,
}

#[derive(Debug, Subcommand)]
enum ProjectCampaignCommand {
    #[command(about = "Plan campaign artifact targets from a brief")]
    Plan(ProjectCampaignPlanArgs),
    #[command(about = "Generate fixture review proposals for a campaign")]
    Generate(ProjectCampaignPlanArgs),
    #[command(about = "Show one campaign run JSON")]
    Show(ProjectCampaignShowArgs),
}

#[derive(Debug, Args)]
struct ProjectCampaignPlanArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    brief: String,
    #[arg(long = "artifact")]
    artifacts: Vec<String>,
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct ProjectCampaignShowArgs {
    #[arg(long)]
    project: PathBuf,
    run_id: String,
}

#[derive(Debug, Subcommand)]
enum ProjectRunCommand {
    #[command(about = "List project AI/generation runs")]
    List(ProjectPathArgs),
    #[command(about = "Show one project run JSON")]
    Show(ProjectRunIdArgs),
    #[command(about = "Accept a proposed review run and mutate the artifact source")]
    Accept(ProjectRunDecisionArgs),
    #[command(about = "Reject a proposed review run without mutating source")]
    Reject(ProjectRunDecisionArgs),
    #[command(about = "Create a linked proposal from a previous proposed or rejected run")]
    Retry(ProjectRunDecisionArgs),
    #[command(about = "Undo an accepted run by restoring the prior artifact source")]
    Undo(ProjectRunDecisionArgs),
}

#[derive(Debug, Args)]
struct ProjectRunIdArgs {
    #[arg(long)]
    project: PathBuf,
    run_id: String,
}

#[derive(Debug, Args)]
struct ProjectRunDecisionArgs {
    #[arg(long)]
    project: PathBuf,
    run_id: String,
    #[arg(long, default_value = "cli")]
    actor: String,
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
        ProjectCommand::DesignLanguage(args) => match args.command {
            ProjectDesignLanguageCommand::Inspect(args) => {
                let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
                serde_json::to_value(
                    package
                        .inspect_design_language()
                        .map_err(|err| err.to_string())?,
                )
            }
            ProjectDesignLanguageCommand::Validate(args) => {
                let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
                serde_json::to_value(
                    package
                        .validate_design_language()
                        .map_err(|err| err.to_string())?,
                )
            }
        },
        ProjectCommand::Workbench(args) => {
            let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
            serde_json::to_value(package.workbench().map_err(|err| err.to_string())?)
        }
        ProjectCommand::ImportVideo(args) => {
            let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
            serde_json::to_value(
                package
                    .import_video_artifact(args.path, args.title)
                    .map_err(|err| err.to_string())?,
            )
        }
        ProjectCommand::ClipQueue(args) => handle_clip_queue(args),
        ProjectCommand::Generate(args) => {
            if [args.dry_run, args.write, args.review]
                .into_iter()
                .filter(|enabled| *enabled)
                .count()
                > 1
            {
                return Err("choose only one of --dry-run, --write, or --review".to_string());
            }
            let out = args.out.clone();
            let live = args.live || args.sdk_response.is_some();
            if args.save_prompt.is_some() && !live {
                return Err("--save-prompt requires --live or --sdk-response".to_string());
            }
            let result = if live {
                live::generate_live(args)?
            } else {
                let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
                package
                    .generate(ProjectGenerateRequestV1 {
                        artifact_id: args.artifact,
                        provider: args.provider.as_str().to_string(),
                        prompt: args.prompt,
                        dry_run: !args.write,
                        review: args.review,
                        selector: args.selector,
                        canvas_node: args.canvas_node,
                        json_pointer: args.json_pointer,
                    })
                    .map_err(|err| err.to_string())?
            };
            if let Some(out) = out.as_ref() {
                write_json_file(out, &result)?;
            }
            serde_json::to_value(result)
        }
        ProjectCommand::Run(args) => project_run(args),
        ProjectCommand::Campaign(args) => project_campaign(args),
        ProjectCommand::AddDesign(args) => {
            let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
            serde_json::to_value(
                package
                    .add_design_asset(
                        args.kind,
                        args.role,
                        args.path,
                        args.title,
                        args.description,
                    )
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

fn project_campaign(args: ProjectCampaignArgs) -> Result<serde_json::Value, serde_json::Error> {
    match args.command {
        ProjectCampaignCommand::Plan(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            let plan = package
                .campaign_plan(ProjectCampaignRequestV1 {
                    brief: args.brief,
                    artifact_ids: args.artifacts,
                })
                .map_err(string_json_error)?;
            if let Some(out) = args.out.as_ref() {
                write_json_file(out, &plan).map_err(string_json_error)?;
            }
            serde_json::to_value(plan)
        }
        ProjectCampaignCommand::Generate(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            let result = package
                .campaign_generate(ProjectCampaignRequestV1 {
                    brief: args.brief,
                    artifact_ids: args.artifacts,
                })
                .map_err(string_json_error)?;
            if let Some(out) = args.out.as_ref() {
                write_json_file(out, &result).map_err(string_json_error)?;
            }
            serde_json::to_value(result)
        }
        ProjectCampaignCommand::Show(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(
                package
                    .campaign_show(&args.run_id)
                    .map_err(string_json_error)?,
            )
        }
    }
}

fn project_run(args: ProjectRunArgs) -> Result<serde_json::Value, serde_json::Error> {
    let data = match args.command {
        ProjectRunCommand::List(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(package.list_project_runs().map_err(string_json_error)?)?
        }
        ProjectRunCommand::Show(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(
                package
                    .show_project_run(&args.run_id)
                    .map_err(string_json_error)?,
            )?
        }
        ProjectRunCommand::Accept(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(
                package
                    .accept_review_run(&args.run_id, &args.actor)
                    .map_err(string_json_error)?,
            )?
        }
        ProjectRunCommand::Reject(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(
                package
                    .reject_review_run(&args.run_id, &args.actor)
                    .map_err(string_json_error)?,
            )?
        }
        ProjectRunCommand::Retry(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(
                package
                    .retry_review_run(&args.run_id, &args.actor)
                    .map_err(string_json_error)?,
            )?
        }
        ProjectRunCommand::Undo(args) => {
            let package = ProjectPackage::open(args.project).map_err(string_json_error)?;
            serde_json::to_value(
                package
                    .undo_review_run(&args.run_id, &args.actor)
                    .map_err(string_json_error)?,
            )?
        }
    };
    Ok(data)
}

fn string_json_error(error: impl ToString) -> serde_json::Error {
    serde_json::Error::io(std::io::Error::other(error.to_string()))
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
