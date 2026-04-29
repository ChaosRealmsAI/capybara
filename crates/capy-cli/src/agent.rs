use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Args, Subcommand, ValueEnum};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy agent --help` as the index and `capy agent help doctor` for the full runtime check workflow.
  Common command: `capy agent doctor`.
  SDK command: `capy agent sdk doctor`; full-auto smoke: `capy agent sdk run --provider codex --write-code --prompt ...`.
  Required params: none.
  Pitfalls: check runtime availability before starting long chat runs.
  Help topics: `capy agent help doctor`, `capy agent help sdk`."
)]
pub struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    #[command(about = "Check Claude and Codex runtime availability")]
    Doctor,
    #[command(about = "Operate the standalone Claude/Codex SDK runtime")]
    Sdk(SdkArgs),
    #[command(about = "Show self-contained AI help topics for agent runtime")]
    Help(AgentHelpArgs),
}

#[derive(Debug, Args)]
struct AgentHelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args)]
struct SdkArgs {
    #[command(subcommand)]
    command: SdkCommand,
}

#[derive(Debug, Subcommand)]
enum SdkCommand {
    #[command(about = "Check SDK packages and local provider runtimes")]
    Doctor,
    #[command(about = "Print normalized SDK runtime options as JSON")]
    Normalize(SdkRuntimeArgs),
    #[command(about = "Run one prompt through Claude Agent SDK or Codex SDK")]
    Run(SdkRunArgs),
}

#[derive(Debug, Clone, ValueEnum)]
enum SdkProviderArg {
    Claude,
    Codex,
}

impl SdkProviderArg {
    fn as_str(&self) -> &'static str {
        match self {
            SdkProviderArg::Claude => "claude",
            SdkProviderArg::Codex => "codex",
        }
    }
}

#[derive(Debug, Args, Clone)]
struct SdkRuntimeArgs {
    #[arg(long, value_enum)]
    provider: SdkProviderArg,
    #[arg(long)]
    cwd: Option<PathBuf>,
    #[arg(long)]
    model: Option<String>,
    #[arg(
        long,
        help = "Reasoning effort: minimal, low, medium, high, xhigh, max"
    )]
    effort: Option<String>,
    #[arg(long, help = "Enable full-auto local coding authority")]
    write_code: bool,
    #[arg(long, help = "Claude permission mode")]
    permission_mode: Option<String>,
    #[arg(long, help = "Codex approval policy")]
    approval_policy: Option<String>,
    #[arg(long, help = "Codex sandbox mode")]
    sandbox: Option<String>,
    #[arg(long, help = "Extra filesystem root for provider runtime")]
    add_dir: Vec<String>,
    #[arg(long, help = "Claude allowed tools list")]
    allowed_tools: Option<String>,
    #[arg(long, help = "Claude disallowed tools list")]
    disallowed_tools: Option<String>,
    #[arg(long, help = "Claude tools list or claude_code preset")]
    tools: Option<String>,
    #[arg(long, help = "MCP config JSON or path")]
    mcp_config: Option<String>,
    #[arg(long, help = "Structured output JSON schema or path")]
    output_schema: Option<String>,
    #[arg(long, help = "Maximum Claude turns")]
    max_turns: Option<u32>,
    #[arg(long, help = "Claude maximum run budget in USD")]
    max_budget_usd: Option<String>,
    #[arg(long, help = "Claude setting source: user, project, local")]
    setting_source: Vec<String>,
    #[arg(long, help = "Existing Codex SDK thread id")]
    thread_id: Option<String>,
    #[arg(long, help = "Claude SDK session id for new session")]
    session_id: Option<String>,
    #[arg(long, help = "Claude SDK session id to resume")]
    resume: Option<String>,
    #[arg(long, help = "Disable Claude session persistence")]
    no_session_persistence: bool,
    #[arg(long, help = "Enable Codex web search")]
    search: bool,
    #[arg(long, help = "Skip Codex git repository check")]
    skip_git_repo_check: bool,
    #[arg(long, help = "Codex SDK config override key=value")]
    codex_config: Vec<String>,
    #[arg(long, help = "Codex CLI path override for the SDK")]
    codex_path: Option<String>,
    #[arg(long, help = "Claude executable path override for the SDK")]
    claude_path: Option<String>,
}

#[derive(Debug, Args)]
struct SdkRunArgs {
    #[command(flatten)]
    runtime: SdkRuntimeArgs,
    #[arg(long)]
    prompt: Option<String>,
    #[arg(
        long,
        help = "Keep JSON output; currently the SDK bridge always emits JSON"
    )]
    json: bool,
    #[arg(long, help = "Include raw SDK items/messages in JSON output")]
    raw: bool,
    #[arg(value_name = "PROMPT")]
    prompt_tail: Vec<String>,
}

pub fn handle(args: AgentArgs) -> Result<(), String> {
    match args.command {
        AgentCommand::Doctor => {
            println!("{}", capy_shell::agent::doctor());
            Ok(())
        }
        AgentCommand::Sdk(args) => handle_sdk(args),
        AgentCommand::Help(args) => crate::help_topics::print_agent_topic(args.topic.as_deref()),
    }
}

fn handle_sdk(args: SdkArgs) -> Result<(), String> {
    match args.command {
        SdkCommand::Doctor => run_node_sdk(["doctor"]),
        SdkCommand::Normalize(args) => {
            let mut node_args = vec!["normalize".to_string()];
            append_runtime_args(&mut node_args, args)?;
            run_node_sdk(node_args)
        }
        SdkCommand::Run(args) => {
            let prompt = args
                .prompt
                .or_else(|| {
                    if args.prompt_tail.is_empty() {
                        None
                    } else {
                        Some(args.prompt_tail.join(" "))
                    }
                })
                .ok_or_else(|| "missing --prompt or positional prompt".to_string())?;
            let mut node_args = vec!["run".to_string()];
            append_runtime_args(&mut node_args, args.runtime)?;
            node_args.push("--prompt".to_string());
            node_args.push(prompt);
            if args.json {
                node_args.push("--json".to_string());
            }
            if args.raw {
                node_args.push("--raw".to_string());
            }
            run_node_sdk(node_args)
        }
    }
}

fn append_runtime_args(target: &mut Vec<String>, args: SdkRuntimeArgs) -> Result<(), String> {
    target.push("--provider".to_string());
    target.push(args.provider.as_str().to_string());
    let cwd = match args.cwd {
        Some(path) => path,
        None => std::env::current_dir().map_err(|err| format!("read cwd failed: {err}"))?,
    };
    target.push("--cwd".to_string());
    target.push(cwd.display().to_string());
    push_opt(target, "--model", args.model);
    push_opt(target, "--effort", args.effort);
    push_flag(target, "--write-code", args.write_code);
    push_opt(target, "--permission-mode", args.permission_mode);
    push_opt(target, "--approval-policy", args.approval_policy);
    push_opt(target, "--sandbox", args.sandbox);
    push_many(target, "--add-dir", args.add_dir);
    push_opt(target, "--allowed-tools", args.allowed_tools);
    push_opt(target, "--disallowed-tools", args.disallowed_tools);
    push_opt(target, "--tools", args.tools);
    push_opt(target, "--mcp-config", args.mcp_config);
    push_opt(target, "--output-schema", args.output_schema);
    push_opt(
        target,
        "--max-turns",
        args.max_turns.map(|value| value.to_string()),
    );
    push_opt(target, "--max-budget-usd", args.max_budget_usd);
    push_many(target, "--setting-source", args.setting_source);
    push_opt(target, "--thread-id", args.thread_id);
    push_opt(target, "--session-id", args.session_id);
    push_opt(target, "--resume", args.resume);
    push_flag(
        target,
        "--no-session-persistence",
        args.no_session_persistence,
    );
    push_flag(target, "--search", args.search);
    push_flag(target, "--skip-git-repo-check", args.skip_git_repo_check);
    push_many(target, "--codex-config", args.codex_config);
    push_opt(target, "--codex-path", args.codex_path);
    push_opt(target, "--claude-path", args.claude_path);
    Ok(())
}

fn run_node_sdk<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let script = sdk_script_path();
    let status = Command::new("node")
        .arg(&script)
        .args(args.into_iter().map(|arg| arg.as_ref().to_string()))
        .current_dir(repo_root())
        .status()
        .map_err(|err| {
            format!(
                "node SDK bridge failed to start at {}: {err}",
                display(&script)
            )
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("node SDK bridge exited with {status}"))
    }
}

fn sdk_script_path() -> PathBuf {
    repo_root().join("tools/capy-agent-sdk/src/cli.mjs")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn push_opt(target: &mut Vec<String>, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        target.push(key.to_string());
        target.push(value);
    }
}

fn push_many(target: &mut Vec<String>, key: &str, values: Vec<String>) {
    for value in values {
        push_opt(target, key, Some(value));
    }
}

fn push_flag(target: &mut Vec<String>, key: &str, enabled: bool) {
    if enabled {
        target.push(key.to_string());
    }
}

fn display(path: &Path) -> String {
    path.display().to_string()
}
