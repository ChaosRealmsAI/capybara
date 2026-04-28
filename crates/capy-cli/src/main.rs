use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};

mod ipc_client;

#[derive(Debug, Parser)]
#[command(
    name = "capy",
    version,
    about = "Capybara CLI for desktop control and AI-friendly verification"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Run the Capybara desktop shell")]
    Shell,
    #[command(about = "Open or focus a Capybara project window")]
    Open(OpenArgs),
    #[command(about = "List running Capybara windows")]
    Ps,
    #[command(about = "Read a UI state key from the active Capybara window")]
    State(StateArgs),
    #[command(about = "Inspect DOM state for AI verification")]
    Devtools(DevtoolsArgs),
    #[command(about = "Capture a DOM probe PNG")]
    Screenshot(ScreenshotArgs),
    #[command(about = "Capture the native macOS window PNG")]
    Capture(CaptureArgs),
    #[command(about = "Run a lightweight Capybara runtime verification")]
    Verify(VerifyArgs),
    #[command(about = "Manage persistent Claude/Codex conversations")]
    Chat(Box<ChatArgs>),
    #[command(about = "Inspect local agent runtimes")]
    Agent(AgentArgs),
    #[command(about = "Quit the Capybara shell")]
    Quit,
}

#[derive(Debug, Args)]
struct OpenArgs {
    #[arg(long, default_value = "demo")]
    project: String,
    #[arg(long)]
    new_window: bool,
}

#[derive(Debug, Args)]
struct StateArgs {
    #[arg(long)]
    key: String,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct DevtoolsArgs {
    #[arg(long)]
    query: Option<String>,
    #[arg(long)]
    eval: Option<String>,
    #[arg(long, default_value = "outerHTML")]
    get: String,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct ScreenshotArgs {
    #[arg(long, default_value = "full")]
    region: String,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct CaptureArgs {
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct ChatArgs {
    #[command(subcommand)]
    command: ChatCommand,
}

#[derive(Debug, Subcommand)]
enum ChatCommand {
    #[command(about = "List persistent conversations")]
    List,
    #[command(about = "Create a persistent conversation")]
    New(ChatNewArgs),
    #[command(about = "Update model and runtime parameters")]
    Configure(ChatConfigureArgs),
    #[command(about = "Open one conversation with messages")]
    Open(ChatOpenArgs),
    #[command(about = "Send a prompt to a conversation")]
    Send(ChatSendArgs),
    #[command(about = "Stop the running turn in one conversation")]
    Stop(ChatOpenArgs),
    #[command(about = "Export one conversation as JSON")]
    Export(ChatOpenArgs),
}

#[derive(Debug, Clone, ValueEnum)]
enum ProviderArg {
    Claude,
    Codex,
}

impl ProviderArg {
    fn as_str(&self) -> &'static str {
        match self {
            ProviderArg::Claude => "claude",
            ProviderArg::Codex => "codex",
        }
    }
}

#[derive(Debug, Args)]
struct ChatNewArgs {
    #[arg(long, value_enum, default_value = "claude")]
    provider: ProviderArg,
    #[arg(long)]
    cwd: Option<PathBuf>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    effort: Option<String>,
    #[arg(long)]
    permission_mode: Option<String>,
    #[arg(long)]
    approval_policy: Option<String>,
    #[arg(long)]
    sandbox: Option<String>,
    #[arg(long)]
    service_tier: Option<String>,
    #[arg(long)]
    add_dir: Vec<String>,
    #[arg(long)]
    allowed_tools: Option<String>,
    #[arg(long)]
    disallowed_tools: Option<String>,
    #[arg(long)]
    mcp_config: Option<String>,
    #[arg(long)]
    append_system_prompt: Option<String>,
    #[arg(long)]
    max_budget_usd: Option<String>,
    #[arg(long)]
    bare: bool,
}

#[derive(Debug, Args)]
struct ChatConfigureArgs {
    #[arg(long)]
    id: String,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    effort: Option<String>,
    #[arg(long)]
    permission_mode: Option<String>,
    #[arg(long)]
    approval_policy: Option<String>,
    #[arg(long)]
    sandbox: Option<String>,
    #[arg(long)]
    service_tier: Option<String>,
    #[arg(long)]
    add_dir: Vec<String>,
    #[arg(long)]
    allowed_tools: Option<String>,
    #[arg(long)]
    disallowed_tools: Option<String>,
    #[arg(long)]
    mcp_config: Option<String>,
    #[arg(long)]
    append_system_prompt: Option<String>,
    #[arg(long)]
    max_budget_usd: Option<String>,
    #[arg(long)]
    bare: bool,
}

#[derive(Debug, Args)]
struct ChatOpenArgs {
    #[arg(long)]
    id: String,
}

#[derive(Debug, Args)]
struct ChatSendArgs {
    #[arg(long)]
    id: String,
    #[arg(required = true)]
    prompt: Vec<String>,
}

#[derive(Debug, Args)]
struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    #[command(about = "Check Claude and Codex runtime availability")]
    Doctor,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Shell => {
            capy_shell::run();
            Ok(())
        }
        Command::Open(args) => send(
            "open-window",
            json!({ "project": args.project, "new_window": args.new_window }),
        ),
        Command::Ps => send("state-query", json!({ "query": "windows" })),
        Command::State(args) => send(
            "state-query",
            json!({ "key": args.key, "window": args.window }),
        ),
        Command::Devtools(args) => {
            if let Some(script) = args.eval {
                send(
                    "devtools-eval",
                    json!({ "eval": script, "window": args.window }),
                )
            } else {
                let query = args
                    .query
                    .ok_or_else(|| "missing --query or --eval for devtools".to_string())?;
                send(
                    "devtools-query",
                    json!({ "query": query, "get": args.get, "window": args.window }),
                )
            }
        }
        Command::Screenshot(args) => send(
            "screenshot",
            json!({
                "region": args.region,
                "out": args.out.display().to_string(),
                "window": args.window
            }),
        ),
        Command::Capture(args) => send(
            "capture",
            json!({ "out": args.out.display().to_string(), "window": args.window }),
        ),
        Command::Verify(args) => send(
            "state-query",
            json!({ "key": "app.ready", "window": args.window, "verify": true }),
        ),
        Command::Chat(args) => match args.command {
            ChatCommand::List => send("conversation-list", json!({})),
            ChatCommand::New(args) => send("conversation-create", chat_new_params(args)?),
            ChatCommand::Configure(args) => {
                send("conversation-update-config", chat_configure_params(args))
            }
            ChatCommand::Open(args) | ChatCommand::Export(args) => {
                send("conversation-open", json!({ "id": args.id }))
            }
            ChatCommand::Send(args) => send(
                "conversation-send",
                json!({ "id": args.id, "prompt": args.prompt.join(" ") }),
            ),
            ChatCommand::Stop(args) => send("conversation-stop", json!({ "id": args.id })),
        },
        Command::Agent(args) => match args.command {
            AgentCommand::Doctor => {
                println!("{}", capy_shell::agent::doctor());
                Ok(())
            }
        },
        Command::Quit => send("quit", json!({})),
    }
}

fn chat_new_params(args: ChatNewArgs) -> Result<Value, String> {
    let cwd = match args.cwd {
        Some(path) => path,
        None => std::env::current_dir().map_err(|err| format!("read cwd failed: {err}"))?,
    };
    let mut config = json!({});
    fill_agent_config(
        &mut config,
        AgentConfigArgs {
            effort: args.effort,
            permission_mode: args.permission_mode,
            approval_policy: args.approval_policy,
            sandbox: args.sandbox,
            service_tier: args.service_tier,
            add_dir: args.add_dir,
            allowed_tools: args.allowed_tools,
            disallowed_tools: args.disallowed_tools,
            mcp_config: args.mcp_config,
            append_system_prompt: args.append_system_prompt,
            max_budget_usd: args.max_budget_usd,
            bare: args.bare,
        },
    );
    Ok(json!({
        "provider": args.provider.as_str(),
        "cwd": cwd.display().to_string(),
        "model": args.model,
        "config": config
    }))
}

fn chat_configure_params(args: ChatConfigureArgs) -> Value {
    let mut config = json!({});
    fill_agent_config(
        &mut config,
        AgentConfigArgs {
            effort: args.effort,
            permission_mode: args.permission_mode,
            approval_policy: args.approval_policy,
            sandbox: args.sandbox,
            service_tier: args.service_tier,
            add_dir: args.add_dir,
            allowed_tools: args.allowed_tools,
            disallowed_tools: args.disallowed_tools,
            mcp_config: args.mcp_config,
            append_system_prompt: args.append_system_prompt,
            max_budget_usd: args.max_budget_usd,
            bare: args.bare,
        },
    );
    let mut params = json!({
        "id": args.id,
        "config": config
    });
    if let Some(model) = args.model {
        params["model"] = json!(model);
    }
    params
}

struct AgentConfigArgs {
    effort: Option<String>,
    permission_mode: Option<String>,
    approval_policy: Option<String>,
    sandbox: Option<String>,
    service_tier: Option<String>,
    add_dir: Vec<String>,
    allowed_tools: Option<String>,
    disallowed_tools: Option<String>,
    mcp_config: Option<String>,
    append_system_prompt: Option<String>,
    max_budget_usd: Option<String>,
    bare: bool,
}

fn fill_agent_config(config: &mut Value, args: AgentConfigArgs) {
    set_opt(config, "effort", args.effort);
    set_opt(config, "permissionMode", args.permission_mode);
    set_opt(config, "approvalPolicy", args.approval_policy);
    set_opt(config, "sandbox", args.sandbox);
    set_opt(config, "serviceTier", args.service_tier);
    set_opt(config, "allowedTools", args.allowed_tools);
    set_opt(config, "disallowedTools", args.disallowed_tools);
    set_opt(config, "mcpConfig", args.mcp_config);
    set_opt(config, "appendSystemPrompt", args.append_system_prompt);
    set_opt(config, "maxBudgetUsd", args.max_budget_usd);
    if !args.add_dir.is_empty() {
        config["addDirs"] = json!(args.add_dir);
    }
    if args.bare {
        config["bare"] = json!(true);
    }
}

fn set_opt(config: &mut Value, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        config[key] = json!(value);
    }
}

fn send(op: &str, params: Value) -> Result<(), String> {
    let request = ipc_client::request(op, params);
    let response = ipc_client::send(request)?;
    if response.ok {
        let data = response.data.unwrap_or(Value::Null);
        println!(
            "{}",
            serde_json::to_string_pretty(&data).map_err(|err| err.to_string())?
        );
        return Ok(());
    }
    Err(response
        .error
        .map(|value| value.to_string())
        .unwrap_or_else(|| "capy IPC request failed".to_string()))
}
