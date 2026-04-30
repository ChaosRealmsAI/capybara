use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use capy_agent_runtime::{AgentSdkRunRequest, run_sdk_json};
use capy_project::{
    ArtifactKind, GENERATE_RUN_SCHEMA_VERSION, ProjectGenerateRequestV1, ProjectGenerateRunV1,
    ProjectPackage, parse_project_ai_response,
};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy project --help` as the index and `capy help project` for the full workflow.
  Common commands: init, inspect, design-language inspect, design-language validate, workbench, generate, add-design, add-artifact.
  Required params: all commands use --project <dir>; generate needs --artifact, --provider, and --prompt; live SDK generation also needs --live.
  Pitfalls: paths must live inside the project root; validate design language before live generation; live codex/claude generation calls the Agent SDK, then Capybara applies a patch.
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
        help = "Call the real Claude/Codex SDK and turn its JSON output into a patch"
    )]
    live: bool,
    #[arg(long)]
    dry_run: bool,
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
        ProjectCommand::Generate(args) => {
            if args.dry_run && args.write {
                return Err("choose either --dry-run or --write, not both".to_string());
            }
            let out = args.out.clone();
            let live = args.live || args.sdk_response.is_some();
            if args.save_prompt.is_some() && !live {
                return Err("--save-prompt requires --live or --sdk-response".to_string());
            }
            let result = if live {
                generate_live(args)?
            } else {
                let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
                package
                    .generate(ProjectGenerateRequestV1 {
                        artifact_id: args.artifact,
                        provider: args.provider.as_str().to_string(),
                        prompt: args.prompt,
                        dry_run: !args.write,
                    })
                    .map_err(|err| err.to_string())?
            };
            if let Some(out) = out.as_ref() {
                write_json_file(out, &result)?;
            }
            serde_json::to_value(result)
        }
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

fn generate_live(
    args: ProjectGenerateArgs,
) -> Result<capy_project::ProjectGenerateResultV1, String> {
    let provider = args.provider.as_str();
    if provider == "fixture" {
        return Err("--live requires --provider codex or --provider claude".to_string());
    }
    let project_root = fs::canonicalize(&args.project).map_err(|err| {
        format!(
            "canonicalize project {} failed: {err}",
            args.project.display()
        )
    })?;
    let package = ProjectPackage::open(&project_root).map_err(|err| err.to_string())?;
    let request = ProjectGenerateRequestV1 {
        artifact_id: args.artifact.clone(),
        provider: provider.to_string(),
        prompt: args.prompt.clone(),
        dry_run: !args.write,
    };
    let prompt = package
        .build_ai_prompt(&request)
        .map_err(|err| err.to_string())?;
    if let Some(save_prompt) = args.save_prompt.as_ref() {
        write_json_file(save_prompt, &prompt)?;
    }
    let sdk_output = run_sdk_json(AgentSdkRunRequest {
        provider: provider.to_string(),
        cwd: project_root.clone(),
        prompt: prompt.prompt.clone(),
        output_schema: prompt.output_schema.clone(),
        model: args.model,
        effort: args.effort,
        fake_response: args
            .sdk_response
            .or_else(|| std::env::var_os("CAPY_PROJECT_AI_RESPONSE_FIXTURE").map(PathBuf::from)),
    })
    .map_err(|err| err.to_string())?;
    let ai_response = parse_project_ai_response(&sdk_output).map_err(|err| err.to_string())?;
    let patch = package
        .patch_from_ai_response(
            &request.artifact_id,
            Some(prompt.context_id.clone()),
            format!("project-ai:{provider}"),
            ai_response.clone(),
        )
        .map_err(|err| err.to_string())?;
    let patch_result = package
        .apply_patch(patch.clone(), None, request.dry_run)
        .map_err(|err| err.to_string())?;
    let preview_source = ai_response
        .artifacts
        .first()
        .map(|artifact| artifact.new_source.clone());
    let inspection = package.inspect().map_err(|err| err.to_string())?;
    let run = ProjectGenerateRunV1 {
        schema_version: GENERATE_RUN_SCHEMA_VERSION.to_string(),
        id: new_id("gen"),
        project_id: inspection.manifest.id,
        artifact_id: request.artifact_id.clone(),
        provider: provider.to_string(),
        prompt: request.prompt,
        status: if request.dry_run {
            "planned"
        } else {
            "completed"
        }
        .to_string(),
        trace_id: new_id("trace"),
        dry_run: request.dry_run,
        design_language_ref: Some(prompt.design_language_ref.clone()),
        design_language_summary: Some(prompt.design_language_summary.clone()),
        command_preview: live_command_preview(provider, &project_root, &request.artifact_id),
        changed_artifact_refs: patch_result.run.changed_artifact_refs.clone(),
        evidence_refs: vec![patch_result.run_path.clone()],
        output: Some(json!({
            "mode": "live",
            "context_id": prompt.context_id,
            "design_language_ref": prompt.design_language_ref,
            "design_language_summary": prompt.design_language_summary,
            "summary_zh": ai_response.summary_zh,
            "verify_notes": ai_response.verify_notes,
            "patch_run": patch_result,
            "patch": patch,
            "sdk": summarize_sdk_output(&sdk_output)
        })),
        error: None,
        generated_at: now_ms(),
    };
    package
        .record_external_generate_run(run, preview_source, !request.dry_run)
        .map_err(|err| err.to_string())
}

fn live_command_preview(
    provider: &str,
    project_root: &std::path::Path,
    artifact: &str,
) -> Vec<String> {
    vec![
        "target/debug/capy".to_string(),
        "agent".to_string(),
        "sdk".to_string(),
        "run".to_string(),
        "--provider".to_string(),
        provider.to_string(),
        "--cwd".to_string(),
        project_root.display().to_string(),
        "--output-schema".to_string(),
        "capy.project-ai-response.v1".to_string(),
        "--prompt".to_string(),
        format!("Project artifact {artifact} generation prompt"),
    ]
}

fn summarize_sdk_output(value: &Value) -> Value {
    json!({
        "ok": value.get("ok").and_then(Value::as_bool),
        "provider": value.get("provider").and_then(Value::as_str),
        "thread_id": value.get("thread_id").and_then(Value::as_str),
        "session_id": value.get("session_id").and_then(Value::as_str),
        "usage": value.get("usage").cloned().unwrap_or(Value::Null),
        "total_cost_usd": value.get("total_cost_usd").cloned().unwrap_or(Value::Null)
    })
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
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
