use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};

mod canvas;
mod canvas_context;
mod chat_context;
mod cutout;
mod desktop_verify;
mod ipc_client;
mod media;

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
    #[command(about = "Cut a fixed-backdrop generated asset into a transparent PNG")]
    Cutout(CutoutArgs),
    #[command(about = "Run a lightweight Capybara runtime verification")]
    Verify(VerifyArgs),
    #[command(about = "Manage persistent Claude/Codex conversations")]
    Chat(Box<ChatArgs>),
    #[command(about = "Operate the live canvas through AI-safe commands")]
    Canvas(canvas::CanvasArgs),
    #[command(about = "Run AI-usable creative generation tools")]
    Image(ImageArgs),
    #[command(about = "Package video clips for scroll-driven HTML pages")]
    Media(media::MediaArgs),
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
struct CutoutArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(
        long,
        default_value = "auto",
        help = "auto or a hex color like #E0E0E0"
    )]
    background: String,
    #[arg(long, default_value_t = 30, help = "Per-channel background tolerance")]
    tolerance: u16,
    #[arg(long, default_value_t = 2, help = "Alpha feather radius in pixels")]
    feather_radius: u32,
    #[arg(
        long,
        default_value_t = 64,
        help = "Drop connected subject islands smaller than this"
    )]
    min_component_area: usize,
    #[arg(
        long,
        default_value_t = 96,
        help = "Cut interior background-like holes at least this large"
    )]
    hole_min_area: usize,
    #[arg(long, help = "Write black/white/deep QA previews to this directory")]
    qa_dir: Option<PathBuf>,
    #[arg(long, help = "Write JSON report to this path")]
    report: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    #[arg(long)]
    window: Option<String>,
    #[arg(long, value_enum, default_value = "readiness")]
    profile: VerifyProfile,
    #[arg(long, help = "Required for --profile desktop")]
    capture_out: Option<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
enum VerifyProfile {
    Readiness,
    Desktop,
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
    #[command(about = "List persisted streaming/runtime events")]
    Events(ChatEventsArgs),
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
    #[command(flatten)]
    runtime: AgentRuntimeOptions,
}

#[derive(Debug, Args, Default, Clone)]
struct AgentRuntimeOptions {
    #[arg(
        long,
        help = "Coding preset: allow provider to edit files in cwd explicitly"
    )]
    write_code: bool,
    #[arg(long, help = "Reasoning effort, e.g. low, medium, high, xhigh")]
    effort: Option<String>,
    #[arg(long, help = "Claude permission mode, e.g. default, plan, acceptEdits")]
    permission_mode: Option<String>,
    #[arg(long, help = "Codex approval policy, e.g. on-request, never")]
    approval_policy: Option<String>,
    #[arg(
        long,
        help = "Codex sandbox mode: read-only, workspace-write, danger-full-access"
    )]
    sandbox: Option<String>,
    #[arg(long, help = "Codex service tier override")]
    service_tier: Option<String>,
    #[arg(long, help = "Extra filesystem root for the provider runtime")]
    add_dir: Vec<String>,
    #[arg(long, help = "Claude allowed tools list")]
    allowed_tools: Option<String>,
    #[arg(long, help = "Claude disallowed tools list")]
    disallowed_tools: Option<String>,
    #[arg(long, help = "MCP config path or JSON")]
    mcp_config: Option<String>,
    #[arg(long, help = "Claude full system prompt override")]
    system_prompt: Option<String>,
    #[arg(long, help = "Prompt appended to the provider's default system prompt")]
    append_system_prompt: Option<String>,
    #[arg(long, help = "Claude maximum run budget in USD")]
    max_budget_usd: Option<String>,
    #[arg(long, help = "Run Claude in bare/minimal mode")]
    bare: bool,
    #[arg(long, help = "Claude fallback model for overloaded print runs")]
    fallback_model: Option<String>,
    #[arg(long, help = "Claude JSON schema string for structured output")]
    json_schema: Option<String>,
    #[arg(long, help = "Claude settings file path or JSON")]
    settings: Option<String>,
    #[arg(long, help = "Claude debug log file path")]
    debug_file: Option<String>,
    #[arg(long, help = "Claude agent name override")]
    agent: Option<String>,
    #[arg(long, help = "Claude custom agents JSON")]
    agents: Option<String>,
    #[arg(long, help = "Claude tools list override")]
    tools: Option<String>,
    #[arg(long, help = "Claude beta header")]
    beta: Vec<String>,
    #[arg(long, help = "Claude plugin directory")]
    plugin_dir: Vec<String>,
    #[arg(long, help = "Use only MCP servers from --mcp-config")]
    strict_mcp_config: bool,
    #[arg(long, help = "Include Claude hook events in stream-json output")]
    include_hook_events: bool,
    #[arg(
        long,
        help = "Disable Claude session persistence for this conversation"
    )]
    no_session_persistence: bool,
    #[arg(long, help = "Allow Claude permission bypass as an explicit option")]
    allow_dangerously_skip_permissions: bool,
    #[arg(long, help = "Explicitly bypass Claude permission checks")]
    dangerously_skip_permissions: bool,
    #[arg(long, help = "Codex model provider override")]
    model_provider: Option<String>,
    #[arg(long, help = "Codex approval reviewer, e.g. user or auto_review")]
    approvals_reviewer: Option<String>,
    #[arg(long, help = "Codex base instructions override")]
    base_instructions: Option<String>,
    #[arg(long, help = "Codex developer instructions override")]
    developer_instructions: Option<String>,
    #[arg(long, help = "Codex reasoning summary: auto, concise, detailed, none")]
    reasoning_summary: Option<String>,
    #[arg(long, help = "Codex output schema JSON or file path")]
    output_schema: Option<String>,
    #[arg(long, help = "Codex personality, e.g. pragmatic, friendly, none")]
    personality: Option<String>,
    #[arg(long, help = "Codex app-server config override key=value")]
    codex_config: Vec<String>,
    #[arg(long, help = "Codex feature flag to enable")]
    codex_enable: Vec<String>,
    #[arg(long, help = "Codex feature flag to disable")]
    codex_disable: Vec<String>,
    #[arg(long, help = "Enable Codex web search through config")]
    search: bool,
    #[arg(long, help = "Start Codex thread as ephemeral")]
    ephemeral: bool,
    #[arg(
        long,
        help = "Inject Capybara Canvas CLI instructions into the agent runtime"
    )]
    capy_canvas_tools: bool,
    #[arg(long, help = "JSONL path for capy canvas tool calls made by the agent")]
    capy_tool_log: Option<String>,
}

#[derive(Debug, Args)]
struct ChatConfigureArgs {
    #[arg(long)]
    id: String,
    #[arg(long)]
    model: Option<String>,
    #[command(flatten)]
    runtime: AgentRuntimeOptions,
}

#[derive(Debug, Args)]
struct ChatOpenArgs {
    #[arg(long)]
    id: String,
}

#[derive(Debug, Args)]
struct ChatEventsArgs {
    #[arg(long)]
    id: String,
    #[arg(long)]
    run_id: Option<String>,
}

#[derive(Debug, Args)]
struct ChatSendArgs {
    #[arg(long)]
    id: String,
    #[arg(long)]
    model: Option<String>,
    #[arg(
        long,
        value_name = "context.json",
        help = "Attach a Canvas Context Packet"
    )]
    canvas_context: Option<PathBuf>,
    #[command(flatten)]
    runtime: AgentRuntimeOptions,
    #[arg(required = true)]
    prompt: Vec<String>,
}

#[derive(Debug, Args)]
struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Debug, Args)]
struct ImageArgs {
    #[command(subcommand)]
    command: ImageCommand,
}

#[derive(Debug, Subcommand)]
enum ImageCommand {
    #[command(about = "List image generation provider options")]
    Providers,
    #[command(about = "Check image generation provider readiness without spending credits")]
    Doctor(ImageProviderArgs),
    #[command(about = "Generate, submit, resume, or dry-run an image request")]
    Generate(ImageGenerateArgs),
    #[command(about = "Check image provider balance")]
    Balance(ImageProviderArgs),
}

#[derive(Debug, Args)]
struct ImageProviderArgs {
    #[arg(long, value_enum, default_value = "apimart-gpt-image-2")]
    provider: ImageProviderArg,
}

#[derive(Debug, Args)]
struct ImageGenerateArgs {
    #[arg(long, value_enum, default_value = "apimart-gpt-image-2")]
    provider: ImageProviderArg,
    #[arg(long, default_value = "1:1")]
    size: String,
    #[arg(long, alias = "aspect-ratio")]
    aspect_ratio: Option<String>,
    #[arg(long, default_value = "1k")]
    resolution: String,
    #[arg(long = "ref")]
    refs: Vec<String>,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    submit_only: bool,
    #[arg(long)]
    resume: Option<String>,
    #[arg(long)]
    no_download: bool,
    #[arg()]
    prompt: Vec<String>,
}

#[derive(Debug, Clone, ValueEnum)]
enum ImageProviderArg {
    #[value(name = "apimart-gpt-image-2")]
    ApimartGptImage2,
}

impl ImageProviderArg {
    fn id(&self) -> capy_image_gen::ImageProviderId {
        match self {
            ImageProviderArg::ApimartGptImage2 => capy_image_gen::ImageProviderId::ApimartGptImage2,
        }
    }
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
        Command::Cutout(args) => {
            let report = cutout::execute(cutout::CutoutRequest {
                input: args.input,
                output: args.output,
                background: args.background,
                tolerance: args.tolerance,
                feather_radius: args.feather_radius,
                min_component_area: args.min_component_area,
                hole_min_area: args.hole_min_area,
                qa_dir: args.qa_dir,
                report: args.report,
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
            );
            Ok(())
        }
        Command::Verify(args) => match args.profile {
            VerifyProfile::Readiness => send(
                "state-query",
                json!({ "key": "app.ready", "window": args.window, "verify": true }),
            ),
            VerifyProfile::Desktop => desktop_verify::verify(args.window, args.capture_out),
        },
        Command::Chat(args) => match args.command {
            ChatCommand::List => send("conversation-list", json!({})),
            ChatCommand::New(args) => send("conversation-create", chat_new_params(args)?),
            ChatCommand::Configure(args) => {
                send("conversation-update-config", chat_configure_params(args))
            }
            ChatCommand::Open(args) | ChatCommand::Export(args) => {
                send("conversation-open", json!({ "id": args.id }))
            }
            ChatCommand::Events(args) => send(
                "conversation-events",
                json!({ "id": args.id, "run_id": args.run_id }),
            ),
            ChatCommand::Send(args) => send("conversation-send", chat_send_params(args)?),
            ChatCommand::Stop(args) => send("conversation-stop", json!({ "id": args.id })),
        },
        Command::Canvas(args) => canvas::handle(args),
        Command::Image(args) => handle_image_command(args),
        Command::Media(args) => media::handle(args),
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
    fill_agent_config(&mut config, args.runtime);
    Ok(json!({
        "provider": args.provider.as_str(),
        "cwd": cwd.display().to_string(),
        "model": args.model,
        "config": config
    }))
}

fn chat_configure_params(args: ChatConfigureArgs) -> Value {
    let mut config = json!({});
    fill_agent_config(&mut config, args.runtime);
    let mut params = json!({
        "id": args.id,
        "config": config
    });
    if let Some(model) = args.model {
        params["model"] = json!(model);
    }
    params
}

fn chat_send_params(args: ChatSendArgs) -> Result<Value, String> {
    let mut config = json!({});
    fill_agent_config(&mut config, args.runtime);
    let canvas_context = args
        .canvas_context
        .map(chat_context::load_canvas_context_packet)
        .transpose()?;
    let prompt = if let Some(context) = canvas_context.as_ref() {
        chat_context::prompt_with_canvas_context(&args.prompt.join(" "), context)
    } else {
        args.prompt.join(" ")
    };
    let mut params = json!({
        "id": args.id,
        "prompt": prompt,
        "config": config
    });
    if let Some(model) = args.model {
        params["model"] = json!(model);
    }
    if let Some(context) = canvas_context {
        params["canvas_context"] = context;
    }
    Ok(params)
}

fn fill_agent_config(config: &mut Value, args: AgentRuntimeOptions) {
    set_opt(config, "effort", args.effort);
    set_opt(config, "permissionMode", args.permission_mode);
    set_opt(config, "approvalPolicy", args.approval_policy);
    set_opt(config, "sandbox", args.sandbox);
    set_opt(config, "serviceTier", args.service_tier);
    set_opt(config, "allowedTools", args.allowed_tools);
    set_opt(config, "disallowedTools", args.disallowed_tools);
    set_opt(config, "mcpConfig", args.mcp_config);
    set_opt(config, "systemPrompt", args.system_prompt);
    set_opt(config, "appendSystemPrompt", args.append_system_prompt);
    set_opt(config, "maxBudgetUsd", args.max_budget_usd);
    set_opt(config, "fallbackModel", args.fallback_model);
    set_opt(config, "jsonSchema", args.json_schema);
    set_opt(config, "settings", args.settings);
    set_opt(config, "debugFile", args.debug_file);
    set_opt(config, "agent", args.agent);
    set_opt(config, "agents", args.agents);
    set_opt(config, "tools", args.tools);
    set_opt(config, "modelProvider", args.model_provider);
    set_opt(config, "approvalsReviewer", args.approvals_reviewer);
    set_opt(config, "baseInstructions", args.base_instructions);
    set_opt(config, "developerInstructions", args.developer_instructions);
    set_opt(config, "reasoningSummary", args.reasoning_summary);
    set_opt(config, "outputSchema", args.output_schema);
    set_opt(config, "personality", args.personality);
    set_opt(config, "capyToolLog", args.capy_tool_log);
    if !args.add_dir.is_empty() {
        config["addDirs"] = json!(args.add_dir);
    }
    if !args.beta.is_empty() {
        config["betas"] = json!(args.beta);
    }
    if !args.plugin_dir.is_empty() {
        config["pluginDirs"] = json!(args.plugin_dir);
    }
    if !args.codex_config.is_empty() {
        config["codexConfig"] = json!(args.codex_config);
    }
    if !args.codex_enable.is_empty() {
        config["codexEnable"] = json!(args.codex_enable);
    }
    if !args.codex_disable.is_empty() {
        config["codexDisable"] = json!(args.codex_disable);
    }
    if args.bare {
        config["bare"] = json!(true);
    }
    if args.strict_mcp_config {
        config["strictMcpConfig"] = json!(true);
    }
    if args.include_hook_events {
        config["includeHookEvents"] = json!(true);
    }
    if args.no_session_persistence {
        config["noSessionPersistence"] = json!(true);
    }
    if args.allow_dangerously_skip_permissions {
        config["allowDangerouslySkipPermissions"] = json!(true);
    }
    if args.dangerously_skip_permissions {
        config["dangerouslySkipPermissions"] = json!(true);
    }
    if args.search {
        config["search"] = json!(true);
    }
    if args.ephemeral {
        config["ephemeral"] = json!(true);
    }
    if args.capy_canvas_tools {
        config["capyCanvasTools"] = json!(true);
    }
    if args.write_code {
        config["writeCode"] = json!(true);
        set_default(config, "approvalPolicy", "never");
        set_default(config, "sandbox", "danger-full-access");
        set_default(config, "permissionMode", "bypassPermissions");
        if config.get("allowDangerouslySkipPermissions").is_none() {
            config["allowDangerouslySkipPermissions"] = json!(true);
        }
        if config.get("dangerouslySkipPermissions").is_none() {
            config["dangerouslySkipPermissions"] = json!(true);
        }
    }
}

fn set_opt(config: &mut Value, key: &str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        config[key] = json!(value);
    }
}

fn set_default(config: &mut Value, key: &str, value: &str) {
    if config.get(key).is_none() {
        config[key] = json!(value);
    }
}

fn send(op: &str, params: Value) -> Result<(), String> {
    let data = request_data(op, params)?;
    print_json(&data)
}

fn print_json(data: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn handle_image_command(args: ImageArgs) -> Result<(), String> {
    let data = match args.command {
        ImageCommand::Providers => json!({
            "ok": true,
            "providers": capy_image_gen::providers()
        }),
        ImageCommand::Doctor(args) => {
            serde_json::to_value(capy_image_gen::doctor(args.provider.id()))
                .map_err(|err| err.to_string())?
        }
        ImageCommand::Balance(args) => {
            capy_image_gen::balance(args.provider.id()).map_err(|err| err.to_string())?
        }
        ImageCommand::Generate(args) => {
            let request = image_generate_request(args)?;
            capy_image_gen::generate_image(request).map_err(|err| err.to_string())?
        }
    };
    print_json(&data)
}

fn image_generate_request(
    args: ImageGenerateArgs,
) -> Result<capy_image_gen::GenerateImageRequest, String> {
    if args.dry_run && args.submit_only {
        return Err("--dry-run and --submit-only cannot be used together".to_string());
    }
    if args.resume.is_some() && (args.dry_run || args.submit_only || !args.prompt.is_empty()) {
        return Err(
            "--resume cannot be combined with prompt, --dry-run, or --submit-only".to_string(),
        );
    }
    let mode = if args.resume.is_some() {
        capy_image_gen::ImageGenerateMode::Resume
    } else if args.dry_run {
        capy_image_gen::ImageGenerateMode::DryRun
    } else if args.submit_only {
        capy_image_gen::ImageGenerateMode::SubmitOnly
    } else {
        capy_image_gen::ImageGenerateMode::Generate
    };
    let prompt = if args.prompt.is_empty() {
        None
    } else {
        Some(args.prompt.join(" "))
    };
    Ok(capy_image_gen::GenerateImageRequest {
        provider: args.provider.id(),
        mode,
        prompt,
        size: args.aspect_ratio.unwrap_or(args.size),
        resolution: args.resolution,
        refs: args.refs,
        output_dir: args.out,
        name: args.name,
        download: !args.no_download,
        task_id: args.resume,
    })
}

fn request_data(op: &str, params: Value) -> Result<Value, String> {
    let request = ipc_client::request(op, params);
    let response = ipc_client::send(request)?;
    if response.ok {
        return Ok(response.data.unwrap_or(Value::Null));
    }
    Err(response
        .error
        .map(|value| value.to_string())
        .unwrap_or_else(|| "capy IPC request failed".to_string()))
}
