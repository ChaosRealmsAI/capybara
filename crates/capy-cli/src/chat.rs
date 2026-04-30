use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

use crate::chat_context;

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy chat --help` as the index and `capy chat help <topic>` for full workflows.
  Common commands: list, new, send, events, open, stop, export.
  Required params: send/open/events/stop/export need --id; send also needs a prompt.
  Pitfalls: --write-code grants broad edit permissions; use --capy-canvas-tools for canvas-aware agents.
  Help topics: `capy chat help agent`, `capy chat help canvas-tools`."
)]
pub struct ChatArgs {
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
    #[command(about = "Show self-contained AI help topics for chat")]
    Help(ChatHelpArgs),
}

#[derive(Debug, Args)]
struct ChatHelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
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
    #[arg(long, help = "Compatibility no-op: SDK is always the chat runtime")]
    sdk: bool,
    #[arg(long, help = "Agent runtime backend; only sdk is supported")]
    runtime_backend: Option<String>,
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
    #[arg(long, help = "Codex SDK config override key=value")]
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

pub fn handle(args: Box<ChatArgs>) -> Result<(), String> {
    match args.command {
        ChatCommand::List => crate::send("conversation-list", json!({})),
        ChatCommand::New(args) => crate::send("conversation-create", chat_new_params(args)?),
        ChatCommand::Configure(args) => {
            crate::send("conversation-update-config", chat_configure_params(args)?)
        }
        ChatCommand::Open(args) | ChatCommand::Export(args) => {
            crate::send("conversation-open", json!({ "id": args.id }))
        }
        ChatCommand::Events(args) => crate::send(
            "conversation-events",
            json!({ "id": args.id, "run_id": args.run_id }),
        ),
        ChatCommand::Send(args) => crate::send("conversation-send", chat_send_params(args)?),
        ChatCommand::Stop(args) => crate::send("conversation-stop", json!({ "id": args.id })),
        ChatCommand::Help(args) => crate::help_topics::print_chat_topic(args.topic.as_deref()),
    }
}

fn chat_new_params(args: ChatNewArgs) -> Result<Value, String> {
    let cwd = match args.cwd {
        Some(path) => path,
        None => std::env::current_dir().map_err(|err| format!("read cwd failed: {err}"))?,
    };
    let mut config = json!({});
    fill_agent_config(&mut config, args.runtime)?;
    Ok(json!({
        "provider": args.provider.as_str(),
        "cwd": cwd.display().to_string(),
        "model": args.model,
        "config": config
    }))
}

fn chat_configure_params(args: ChatConfigureArgs) -> Result<Value, String> {
    let mut config = json!({});
    fill_agent_config(&mut config, args.runtime)?;
    let mut params = json!({
        "id": args.id,
        "config": config
    });
    if let Some(model) = args.model {
        params["model"] = json!(model);
    }
    Ok(params)
}

fn chat_send_params(args: ChatSendArgs) -> Result<Value, String> {
    let mut config = json!({});
    fill_agent_config(&mut config, args.runtime)?;
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

fn fill_agent_config(config: &mut Value, args: AgentRuntimeOptions) -> Result<(), String> {
    config["runtimeBackend"] = json!("sdk");
    if let Some(backend) = args
        .runtime_backend
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if !backend.eq_ignore_ascii_case("sdk") {
            return Err(format!(
                "agent runtime backend is SDK-only; --runtime-backend={backend} was removed"
            ));
        }
    }
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
    let _sdk_compat_flag = args.sdk;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{AgentRuntimeOptions, fill_agent_config};
    use serde_json::json;

    #[test]
    fn chat_runtime_defaults_to_sdk() -> Result<(), String> {
        let mut config = json!({});
        fill_agent_config(&mut config, AgentRuntimeOptions::default())?;

        assert_eq!(config["runtimeBackend"], "sdk");
        Ok(())
    }

    #[test]
    fn chat_runtime_rejects_removed_cli_backend() -> Result<(), String> {
        let mut config = json!({});
        let result = fill_agent_config(
            &mut config,
            AgentRuntimeOptions {
                runtime_backend: Some("cli".to_string()),
                ..AgentRuntimeOptions::default()
            },
        );
        let Err(err) = result else {
            return Err("cli backend should be rejected".to_string());
        };

        assert!(err.contains("SDK-only"));
        Ok(())
    }

    #[test]
    fn chat_runtime_accepts_sdk_backend_compat_flag() -> Result<(), String> {
        let mut config = json!({});
        fill_agent_config(
            &mut config,
            AgentRuntimeOptions {
                sdk: true,
                runtime_backend: Some("sdk".to_string()),
                ..AgentRuntimeOptions::default()
            },
        )?;

        assert_eq!(config["runtimeBackend"], "sdk");
        Ok(())
    }
}
