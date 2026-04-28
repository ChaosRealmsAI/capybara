use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;

use crate::app::ShellEvent;
use crate::store::{Conversation, CreateRunEvent, Provider, Store};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentRuntimeEvent {
    pub conversation_id: String,
    pub run_id: String,
    pub provider: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub event: Value,
}

#[derive(Debug)]
struct RunOutput {
    content: String,
    native_thread_id: Option<String>,
}

pub fn spawn_turn(
    store: Arc<Store>,
    proxy: EventLoopProxy<ShellEvent>,
    conversation: Conversation,
    prompt: String,
) -> Result<String, String> {
    if store
        .running_run_for_conversation(&conversation.id)?
        .is_some()
    {
        return Err("conversation already has a running turn".to_string());
    }
    let run = store.create_run(&conversation.id)?;
    store.add_message(
        &conversation.id,
        "user",
        &prompt,
        json!({ "source": "capybara" }),
    )?;
    store.update_title_if_default(&conversation.id, &prompt)?;
    store.update_status(&conversation.id, "running")?;
    record_and_emit(
        &store,
        &proxy,
        event(&conversation, &run.id, "run_status")
            .with_status("running")
            .with_content("Agent run started"),
    );

    let run_id = run.id.clone();
    std::thread::spawn(move || {
        let result = match conversation.provider {
            Provider::Claude => run_claude(&store, &proxy, &conversation, &run_id, &prompt),
            Provider::Codex => run_codex(&store, &proxy, &conversation, &run_id, &prompt),
        };

        match result {
            Ok(output) => {
                if !output.content.trim().is_empty() {
                    let _message_result = store.add_message(
                        &conversation.id,
                        "assistant",
                        output.content.trim_end(),
                        json!({ "provider": conversation.provider.as_str() }),
                    );
                }
                if let Some(thread_id) = output.native_thread_id {
                    let _thread_result = store.update_native_thread(&conversation.id, &thread_id);
                }
                let _status_result = store.update_status(&conversation.id, "idle");
                let _run_result = store.finish_run(&run_id, "completed", None);
                record_and_emit(
                    &store,
                    &proxy,
                    event(&conversation, &run_id, "assistant_done")
                        .with_status("completed")
                        .with_content(output.content),
                );
            }
            Err(error) => {
                let _status_result = store.update_status(&conversation.id, "error");
                let _run_result = store.finish_run(&run_id, "failed", Some(&error));
                let _message_result = store.add_message(
                    &conversation.id,
                    "system",
                    &error,
                    json!({ "level": "error" }),
                );
                record_and_emit(
                    &store,
                    &proxy,
                    event(&conversation, &run_id, "error")
                        .with_status("failed")
                        .with_error(error),
                );
            }
        }
    });

    Ok(run.id)
}

pub fn stop_running(store: &Store, conversation_id: &str) -> Result<Value, String> {
    let Some(run) = store.running_run_for_conversation(conversation_id)? else {
        return Ok(json!({ "stopped": false, "reason": "no running run" }));
    };
    let Some(pid) = run.pid else {
        store.finish_run(&run.id, "stopped", Some("run had no recorded pid"))?;
        return Ok(json!({ "stopped": false, "run_id": run.id, "reason": "pid missing" }));
    };
    let status = Command::new("kill")
        .arg(pid.to_string())
        .status()
        .map_err(|err| format!("kill failed: {err}"))?;
    store.finish_run(&run.id, "stopped", Some("stopped by user"))?;
    store.update_status(conversation_id, "idle")?;
    Ok(json!({
        "stopped": status.success(),
        "run_id": run.id,
        "pid": pid
    }))
}

pub fn doctor() -> Value {
    json!({
        "claude": tool_version("claude", &["--version"]),
        "codex": tool_version("codex", &["--version"]),
        "codex_app_server": tool_version("codex", &["app-server", "--help"])
    })
}

fn run_claude(
    store: &Store,
    proxy: &EventLoopProxy<ShellEvent>,
    conversation: &Conversation,
    run_id: &str,
    prompt: &str,
) -> Result<RunOutput, String> {
    let mut command = Command::new("claude");
    let use_resume = store
        .messages_for(&conversation.id)?
        .iter()
        .any(|message| message.role == "assistant");
    command.args(claude_args(conversation, prompt, use_resume));
    command.current_dir(&conversation.cwd);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("claude failed to start: {err}"))?;
    store.set_run_pid(run_id, child.id())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "claude stdout missing".to_string())?;
    let reader = BufReader::new(stdout);
    let mut content = String::new();
    let mut last_stdout = String::new();

    for line in reader.lines() {
        let line = line.map_err(|err| format!("claude stdout failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        last_stdout = line.clone();
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if value.get("type").and_then(Value::as_str) == Some("result") {
                if content.is_empty() {
                    if let Some(result) = value.get("result").and_then(Value::as_str) {
                        content.push_str(result);
                    }
                }
                continue;
            }
            if let Some(delta) = claude_delta(&value) {
                content.push_str(&delta);
                record_and_emit(
                    store,
                    proxy,
                    event(conversation, run_id, "assistant_delta").with_delta(delta),
                );
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("claude wait failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout_detail = claude_result_fallback(&last_stdout);
        let detail = non_empty(Some(stderr.trim()))
            .or_else(|| non_empty(Some(content.trim())))
            .or_else(|| non_empty(Some(stdout_detail.trim())))
            .or_else(|| non_empty(Some(last_stdout.trim())))
            .unwrap_or("no stderr or result content");
        return Err(format!("claude exited with {}: {}", output.status, detail));
    }
    if content.trim().is_empty() {
        content = claude_result_fallback(&String::from_utf8_lossy(&output.stdout));
    }
    Ok(RunOutput {
        content,
        native_thread_id: None,
    })
}

fn run_codex(
    store: &Store,
    proxy: &EventLoopProxy<ShellEvent>,
    conversation: &Conversation,
    run_id: &str,
    prompt: &str,
) -> Result<RunOutput, String> {
    let mut command = Command::new("codex");
    command.arg("app-server");
    command.args(codex_app_server_args(&conversation.config));
    command.arg("--listen").arg("stdio://");
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|err| format!("codex app-server failed to start: {err}"))?;
    store.set_run_pid(run_id, child.id())?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "codex stdin missing".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "codex stdout missing".to_string())?;
    let mut reader = BufReader::new(stdout);

    send_json(
        &mut stdin,
        json!({
            "method": "initialize",
            "id": 0,
            "params": {
                "clientInfo": {
                    "name": "capybara",
                    "title": "Capybara",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": { "experimentalApi": true }
            }
        }),
    )?;
    send_json(&mut stdin, json!({ "method": "initialized", "params": {} }))?;

    let start_params = if let Some(thread_id) = conversation.native_thread_id.as_deref() {
        codex_resume_params(conversation, thread_id)
    } else {
        codex_start_params(conversation)
    };
    let start_method = if conversation.native_thread_id.is_some() {
        "thread/resume"
    } else {
        "thread/start"
    };
    send_json(
        &mut stdin,
        json!({ "method": start_method, "id": 1, "params": start_params }),
    )?;
    let thread_id = read_until_response(&mut reader, 1)?
        .get("result")
        .and_then(|result| result.get("thread"))
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| "codex thread response missing thread.id".to_string())?;

    let turn_params = codex_turn_params(conversation, &thread_id, prompt)?;
    send_json(
        &mut stdin,
        json!({ "method": "turn/start", "id": 2, "params": turn_params }),
    )?;

    let mut content = String::new();
    loop {
        let Some(message) = read_json_line(&mut reader)? else {
            break;
        };
        if let Some(error) = message.get("error") {
            return Err(format!("codex error: {error}"));
        }
        if message.get("id").and_then(Value::as_i64) == Some(2) {
            continue;
        }
        if message.get("method").and_then(Value::as_str) == Some("item/agentMessage/delta") {
            if let Some(delta) = message
                .get("params")
                .and_then(|params| params.get("delta"))
                .and_then(Value::as_str)
            {
                content.push_str(delta);
                record_and_emit(
                    store,
                    proxy,
                    event(conversation, run_id, "assistant_delta").with_delta(delta.to_string()),
                );
            }
        }
        if message.get("method").and_then(Value::as_str) == Some("turn/completed") {
            break;
        }
    }
    let _kill_result = child.kill();
    Ok(RunOutput {
        content,
        native_thread_id: Some(thread_id),
    })
}

fn codex_app_server_args(config: &Value) -> Vec<String> {
    let mut args = Vec::new();
    for value in config_array(config, "codexConfig") {
        args.push("-c".to_string());
        args.push(value);
    }
    for value in config_array(config, "codexEnable") {
        args.push("--enable".to_string());
        args.push(value);
    }
    for value in config_array(config, "codexDisable") {
        args.push("--disable".to_string());
        args.push(value);
    }
    args
}

fn codex_start_params(conversation: &Conversation) -> Value {
    let mut params = json!({
        "cwd": conversation.cwd,
        "serviceName": "capybara",
        "persistExtendedHistory": true
    });
    apply_codex_thread_overrides(&mut params, conversation);
    params
}

fn codex_resume_params(conversation: &Conversation, thread_id: &str) -> Value {
    let mut params = json!({ "threadId": thread_id });
    apply_codex_thread_overrides(&mut params, conversation);
    params
}

fn apply_codex_thread_overrides(params: &mut Value, conversation: &Conversation) {
    if let Some(model) = non_empty(conversation.model.as_deref()) {
        params["model"] = json!(model);
    }
    set_config_str_param(params, &conversation.config, "approvalsReviewer");
    set_config_str_param(params, &conversation.config, "baseInstructions");
    set_config_str_param(params, &conversation.config, "developerInstructions");
    set_config_str_param(params, &conversation.config, "modelProvider");
    set_config_str_param(params, &conversation.config, "personality");
    set_config_str_param(params, &conversation.config, "serviceTier");
    if let Some(approval_policy) = codex_approval_policy(&conversation.config) {
        params["approvalPolicy"] = json!(approval_policy);
    }
    if let Some(sandbox) = codex_sandbox_setting(&conversation.config) {
        params["sandbox"] = json!(codex_sandbox_mode(&sandbox));
    }
    if config_bool(&conversation.config, "ephemeral") {
        params["ephemeral"] = json!(true);
    }
    if config_bool(&conversation.config, "search") {
        params["config"] = json!({ "web_search": true });
    }
}

fn codex_turn_params(
    conversation: &Conversation,
    thread_id: &str,
    prompt: &str,
) -> Result<Value, String> {
    let mut params = json!({
        "threadId": thread_id,
        "input": [{ "type": "text", "text": prompt }]
    });
    if let Some(model) = non_empty(conversation.model.as_deref()) {
        params["model"] = json!(model);
    }
    if let Some(approval_policy) = codex_approval_policy(&conversation.config) {
        params["approvalPolicy"] = json!(approval_policy);
    }
    set_config_str_param(&mut params, &conversation.config, "approvalsReviewer");
    set_config_str_param(&mut params, &conversation.config, "effort");
    set_config_str_param(&mut params, &conversation.config, "personality");
    set_config_str_param_rename(
        &mut params,
        &conversation.config,
        "reasoningSummary",
        "summary",
    );
    set_config_str_param(&mut params, &conversation.config, "serviceTier");
    if let Some(sandbox) = codex_sandbox_setting(&conversation.config) {
        params["sandboxPolicy"] = codex_sandbox_policy(&sandbox);
    }
    if let Some(output_schema) = config_str(&conversation.config, "outputSchema") {
        params["outputSchema"] = json_or_file(&output_schema)?;
    }
    Ok(params)
}

fn set_config_str_param(params: &mut Value, config: &Value, key: &str) {
    set_config_str_param_rename(params, config, key, key);
}

fn set_config_str_param_rename(params: &mut Value, config: &Value, key: &str, target: &str) {
    if let Some(value) = config_str(config, key) {
        params[target] = json!(value);
    }
}

fn json_or_file(value: &str) -> Result<Value, String> {
    let trimmed = value.trim();
    let source = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        trimmed.to_string()
    } else if Path::new(trimmed).exists() {
        std::fs::read_to_string(trimmed)
            .map_err(|err| format!("read JSON schema failed: {trimmed}: {err}"))?
    } else {
        trimmed.to_string()
    };
    serde_json::from_str(&source).map_err(|err| format!("invalid JSON schema: {err}"))
}

fn send_json(stdin: &mut std::process::ChildStdin, value: Value) -> Result<(), String> {
    let payload = serde_json::to_string(&value).map_err(|err| err.to_string())?;
    stdin
        .write_all(payload.as_bytes())
        .map_err(|err| format!("write JSON-RPC failed: {err}"))?;
    stdin
        .write_all(b"\n")
        .map_err(|err| format!("write JSON-RPC newline failed: {err}"))?;
    stdin
        .flush()
        .map_err(|err| format!("flush JSON-RPC failed: {err}"))
}

fn read_until_response(
    reader: &mut BufReader<std::process::ChildStdout>,
    id: i64,
) -> Result<Value, String> {
    loop {
        let Some(value) = read_json_line(reader)? else {
            return Err(format!("codex app-server closed before response id {id}"));
        };
        if value.get("id").and_then(Value::as_i64) == Some(id) {
            if let Some(error) = value.get("error") {
                return Err(format!("codex response error: {error}"));
            }
            return Ok(value);
        }
    }
}

fn read_json_line(
    reader: &mut BufReader<std::process::ChildStdout>,
) -> Result<Option<Value>, String> {
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .map_err(|err| format!("read JSON-RPC failed: {err}"))?;
    if bytes == 0 {
        return Ok(None);
    }
    serde_json::from_str::<Value>(line.trim_end())
        .map(Some)
        .map_err(|err| format!("invalid JSON-RPC line: {err}"))
}

fn claude_delta(value: &Value) -> Option<String> {
    if value.get("type").and_then(Value::as_str) == Some("content_block_delta") {
        return value
            .get("delta")
            .and_then(|delta| delta.get("text"))
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    if value.get("type").and_then(Value::as_str) == Some("assistant") {
        let content = value
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(Value::as_array)?;
        let mut text = String::new();
        for item in content {
            if item.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(chunk) = item.get("text").and_then(Value::as_str) {
                    text.push_str(chunk);
                }
            }
        }
        return (!text.is_empty()).then_some(text);
    }
    if value.get("type").and_then(Value::as_str) == Some("result") {
        return value
            .get("result")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    None
}

fn claude_result_fallback(raw: &str) -> String {
    raw.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| claude_delta(&value))
        .collect::<Vec<_>>()
        .join("")
}

fn config_str(config: &Value, key: &str) -> Option<String> {
    non_empty(config.get(key).and_then(Value::as_str)).map(ToString::to_string)
}

fn config_bool(config: &Value, key: &str) -> bool {
    config.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn config_array(config: &Value, key: &str) -> Vec<String> {
    config
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|value| non_empty(Some(value)))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn codex_approval_policy(config: &Value) -> Option<String> {
    config_str(config, "approvalPolicy")
        .or_else(|| config_bool(config, "writeCode").then(|| "never".to_string()))
}

fn codex_sandbox_setting(config: &Value) -> Option<String> {
    config_str(config, "sandbox")
        .or_else(|| config_bool(config, "writeCode").then(|| "danger-full-access".to_string()))
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn claude_args(conversation: &Conversation, prompt: &str, use_resume: bool) -> Vec<String> {
    let write_code = config_bool(&conversation.config, "writeCode");
    let mut args = vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--include-partial-messages".to_string(),
    ];
    if let Some(session_id) = conversation.native_session_id.as_deref() {
        args.push(if use_resume {
            "--resume".to_string()
        } else {
            "--session-id".to_string()
        });
        args.push(session_id.to_string());
    }
    if let Some(model) = non_empty(conversation.model.as_deref()) {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    if let Some(effort) = config_str(&conversation.config, "effort") {
        args.push("--effort".to_string());
        args.push(effort);
    }
    if let Some(mode) = config_str(&conversation.config, "permissionMode") {
        args.push("--permission-mode".to_string());
        args.push(mode);
    } else if write_code {
        args.push("--permission-mode".to_string());
        args.push("bypassPermissions".to_string());
    }
    for dir in config_array(&conversation.config, "addDirs") {
        args.push("--add-dir".to_string());
        args.push(dir);
    }
    if let Some(tools) = config_str(&conversation.config, "allowedTools") {
        args.push("--allowedTools".to_string());
        args.push(tools);
    }
    if let Some(tools) = config_str(&conversation.config, "disallowedTools") {
        args.push("--disallowedTools".to_string());
        args.push(tools);
    }
    if let Some(mcp) = config_str(&conversation.config, "mcpConfig") {
        args.push("--mcp-config".to_string());
        args.push(mcp);
    }
    if let Some(system) = config_str(&conversation.config, "systemPrompt") {
        args.push("--system-prompt".to_string());
        args.push(system);
    }
    if let Some(system) = config_str(&conversation.config, "appendSystemPrompt") {
        args.push("--append-system-prompt".to_string());
        args.push(system);
    }
    if let Some(budget) = config_str(&conversation.config, "maxBudgetUsd") {
        args.push("--max-budget-usd".to_string());
        args.push(budget);
    }
    if let Some(model) = config_str(&conversation.config, "fallbackModel") {
        args.push("--fallback-model".to_string());
        args.push(model);
    }
    if let Some(schema) = config_str(&conversation.config, "jsonSchema") {
        args.push("--json-schema".to_string());
        args.push(schema);
    }
    if let Some(settings) = config_str(&conversation.config, "settings") {
        args.push("--settings".to_string());
        args.push(settings);
    }
    if let Some(debug_file) = config_str(&conversation.config, "debugFile") {
        args.push("--debug-file".to_string());
        args.push(debug_file);
    }
    if let Some(agent) = config_str(&conversation.config, "agent") {
        args.push("--agent".to_string());
        args.push(agent);
    }
    if let Some(agents) = config_str(&conversation.config, "agents") {
        args.push("--agents".to_string());
        args.push(agents);
    }
    if let Some(tools) = config_str(&conversation.config, "tools") {
        args.push("--tools".to_string());
        args.push(tools);
    }
    for beta in config_array(&conversation.config, "betas") {
        args.push("--betas".to_string());
        args.push(beta);
    }
    for plugin_dir in config_array(&conversation.config, "pluginDirs") {
        args.push("--plugin-dir".to_string());
        args.push(plugin_dir);
    }
    if config_bool(&conversation.config, "bare") {
        args.push("--bare".to_string());
    }
    if config_bool(&conversation.config, "strictMcpConfig") {
        args.push("--strict-mcp-config".to_string());
    }
    if config_bool(&conversation.config, "includeHookEvents") {
        args.push("--include-hook-events".to_string());
    }
    if config_bool(&conversation.config, "noSessionPersistence") {
        args.push("--no-session-persistence".to_string());
    }
    if config_bool(&conversation.config, "allowDangerouslySkipPermissions") || write_code {
        args.push("--allow-dangerously-skip-permissions".to_string());
    }
    if config_bool(&conversation.config, "dangerouslySkipPermissions") || write_code {
        args.push("--dangerously-skip-permissions".to_string());
    }
    args.push("--".to_string());
    args.push(prompt.to_string());
    args
}

fn codex_sandbox_mode(value: &str) -> &str {
    match value {
        "read-only" | "readOnly" => "read-only",
        "danger-full-access" | "dangerFullAccess" => "danger-full-access",
        _ => "workspace-write",
    }
}

fn codex_sandbox_policy(value: &str) -> Value {
    match value {
        "read-only" | "readOnly" => json!({ "type": "readOnly" }),
        "danger-full-access" | "dangerFullAccess" => json!({ "type": "dangerFullAccess" }),
        _ => json!({ "type": "workspaceWrite" }),
    }
}

fn tool_version(bin: &str, args: &[&str]) -> Value {
    match Command::new(bin).args(args).output() {
        Ok(output) => json!({
            "available": output.status.success(),
            "version": String::from_utf8_lossy(&output.stdout).trim(),
            "error": String::from_utf8_lossy(&output.stderr).trim()
        }),
        Err(err) => json!({ "available": false, "error": err.to_string() }),
    }
}

fn event(conversation: &Conversation, run_id: &str, kind: &str) -> AgentRuntimeEvent {
    AgentRuntimeEvent {
        conversation_id: conversation.id.clone(),
        run_id: run_id.to_string(),
        provider: conversation.provider.as_str().to_string(),
        kind: kind.to_string(),
        delta: None,
        content: None,
        status: None,
        error: None,
        event: Value::Null,
    }
}

impl AgentRuntimeEvent {
    fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    fn with_delta(mut self, delta: impl Into<String>) -> Self {
        self.delta = Some(delta.into());
        self
    }
}

fn record_and_emit(store: &Store, proxy: &EventLoopProxy<ShellEvent>, event: AgentRuntimeEvent) {
    let event_json = serde_json::to_value(&event).unwrap_or(Value::Null);
    let _event_result = store.add_run_event(CreateRunEvent {
        conversation_id: &event.conversation_id,
        run_id: &event.run_id,
        kind: &event.kind,
        delta: event.delta.as_deref(),
        content: event.content.as_deref(),
        status: event.status.as_deref(),
        error: event.error.as_deref(),
        event_json,
    });
    emit(proxy, event);
}

fn emit(proxy: &EventLoopProxy<ShellEvent>, event: AgentRuntimeEvent) {
    let _send_result = proxy.send_event(ShellEvent::AgentRuntimeEvent { event });
}

#[cfg(test)]
mod tests {
    use super::{
        claude_args, claude_delta, codex_app_server_args, codex_resume_params, codex_sandbox_mode,
        codex_sandbox_policy, codex_start_params, codex_turn_params,
    };
    use crate::store::{Conversation, Provider};
    use serde_json::json;

    #[test]
    fn extracts_claude_assistant_text() {
        let value = json!({
            "type": "assistant",
            "message": { "content": [{ "type": "text", "text": "hello" }] }
        });
        assert_eq!(claude_delta(&value).as_deref(), Some("hello"));
    }

    #[test]
    fn builds_claude_cli_args_with_streaming_and_runtime_options() {
        let conversation = Conversation {
            id: "conv".to_string(),
            title: "Test".to_string(),
            provider: Provider::Claude,
            cwd: "/tmp".to_string(),
            native_session_id: Some("00000000-0000-4000-8000-000000000001".to_string()),
            native_thread_id: None,
            model: Some("sonnet".to_string()),
            config: json!({
                "effort": "medium",
                "permissionMode": "plan",
                "addDirs": ["/tmp/extra"],
                "allowedTools": "Bash(git *) Read",
                "disallowedTools": "Write",
                "mcpConfig": "/tmp/mcp.json",
                "systemPrompt": "You are Capybara",
                "appendSystemPrompt": "Be concise",
                "maxBudgetUsd": "2.5",
                "fallbackModel": "sonnet",
                "jsonSchema": "{\"type\":\"object\"}",
                "settings": "/tmp/settings.json",
                "debugFile": "/tmp/claude-debug.log",
                "agent": "reviewer",
                "agents": "{\"reviewer\":{\"description\":\"Reviews\",\"prompt\":\"Review\"}}",
                "tools": "Bash,Read",
                "betas": ["beta-a", "beta-b"],
                "pluginDirs": ["/tmp/plugin"],
                "bare": true,
                "strictMcpConfig": true,
                "includeHookEvents": true,
                "noSessionPersistence": true,
                "allowDangerouslySkipPermissions": true,
                "dangerouslySkipPermissions": true
            }),
            status: "idle".to_string(),
            archived: false,
            created_at: 0,
            updated_at: 0,
        };

        let args = claude_args(&conversation, "hello", false);

        assert!(
            args.windows(3)
                .any(|window| window == ["-p", "--output-format", "stream-json"])
        );
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--include-partial-messages".to_string()));
        assert!(
            args.windows(2)
                .any(|window| window == ["--session-id", "00000000-0000-4000-8000-000000000001"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--model", "sonnet"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--effort", "medium"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--permission-mode", "plan"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--add-dir", "/tmp/extra"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--allowedTools", "Bash(git *) Read"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--disallowedTools", "Write"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--mcp-config", "/tmp/mcp.json"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--system-prompt", "You are Capybara"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--append-system-prompt", "Be concise"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--max-budget-usd", "2.5"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--fallback-model", "sonnet"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--json-schema", "{\"type\":\"object\"}"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--settings", "/tmp/settings.json"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--debug-file", "/tmp/claude-debug.log"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--agent", "reviewer"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--tools", "Bash,Read"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--betas", "beta-a"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--plugin-dir", "/tmp/plugin"])
        );
        assert!(args.contains(&"--bare".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        assert!(args.contains(&"--include-hook-events".to_string()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--allow-dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert_eq!(args.get(args.len() - 2).map(String::as_str), Some("--"));
        assert_eq!(args.last().map(String::as_str), Some("hello"));
    }

    #[test]
    fn builds_claude_resume_args_for_later_turns() {
        let conversation = Conversation {
            id: "conv".to_string(),
            title: "Test".to_string(),
            provider: Provider::Claude,
            cwd: "/tmp".to_string(),
            native_session_id: Some("00000000-0000-4000-8000-000000000001".to_string()),
            native_thread_id: None,
            model: None,
            config: json!({}),
            status: "idle".to_string(),
            archived: false,
            created_at: 0,
            updated_at: 0,
        };

        let args = claude_args(&conversation, "continue", true);

        assert!(
            args.windows(2)
                .any(|window| window == ["--resume", "00000000-0000-4000-8000-000000000001"])
        );
        assert!(!args.contains(&"--session-id".to_string()));
    }

    #[test]
    fn write_code_preset_maps_to_claude_permission_bypass() {
        let conversation = Conversation {
            id: "conv".to_string(),
            title: "Test".to_string(),
            provider: Provider::Claude,
            cwd: "/tmp".to_string(),
            native_session_id: None,
            native_thread_id: None,
            model: None,
            config: json!({ "writeCode": true }),
            status: "idle".to_string(),
            archived: false,
            created_at: 0,
            updated_at: 0,
        };

        let args = claude_args(&conversation, "edit files", false);

        assert!(
            args.windows(2)
                .any(|window| window == ["--permission-mode", "bypassPermissions"])
        );
        assert!(args.contains(&"--allow-dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn maps_codex_sandbox_shapes_for_thread_and_turn_params() {
        assert_eq!(codex_sandbox_mode("workspace-write"), "workspace-write");
        assert_eq!(codex_sandbox_mode("workspaceWrite"), "workspace-write");
        assert_eq!(codex_sandbox_mode("readOnly"), "read-only");
        assert_eq!(codex_sandbox_mode("dangerFullAccess"), "danger-full-access");

        assert_eq!(
            codex_sandbox_policy("workspace-write"),
            json!({ "type": "workspaceWrite" })
        );
        assert_eq!(
            codex_sandbox_policy("read-only"),
            json!({ "type": "readOnly" })
        );
        assert_eq!(
            codex_sandbox_policy("danger-full-access"),
            json!({ "type": "dangerFullAccess" })
        );
    }

    #[test]
    fn builds_codex_app_server_and_thread_turn_params() -> Result<(), String> {
        let conversation = Conversation {
            id: "conv".to_string(),
            title: "Test".to_string(),
            provider: Provider::Codex,
            cwd: "/tmp/project".to_string(),
            native_session_id: None,
            native_thread_id: None,
            model: Some("gpt-5.2".to_string()),
            config: json!({
                "approvalPolicy": "never",
                "approvalsReviewer": "user",
                "sandbox": "workspace-write",
                "serviceTier": "flex",
                "effort": "high",
                "reasoningSummary": "concise",
                "modelProvider": "openai",
                "baseInstructions": "Base",
                "developerInstructions": "Dev",
                "outputSchema": "{\"type\":\"object\",\"properties\":{\"ok\":{\"type\":\"boolean\"}}}",
                "personality": "pragmatic",
                "codexConfig": ["model_verbosity=\"low\""],
                "codexEnable": ["experimental_feature"],
                "codexDisable": ["legacy_feature"],
                "search": true,
                "ephemeral": true
            }),
            status: "idle".to_string(),
            archived: false,
            created_at: 0,
            updated_at: 0,
        };

        let app_args = codex_app_server_args(&conversation.config);
        assert!(
            app_args
                .windows(2)
                .any(|window| window == ["-c", "model_verbosity=\"low\""])
        );
        assert!(
            app_args
                .windows(2)
                .any(|window| window == ["--enable", "experimental_feature"])
        );
        assert!(
            app_args
                .windows(2)
                .any(|window| window == ["--disable", "legacy_feature"])
        );

        let start = codex_start_params(&conversation);
        assert_eq!(start["cwd"], json!("/tmp/project"));
        assert_eq!(start["model"], json!("gpt-5.2"));
        assert_eq!(start["approvalPolicy"], json!("never"));
        assert_eq!(start["approvalsReviewer"], json!("user"));
        assert_eq!(start["sandbox"], json!("workspace-write"));
        assert_eq!(start["serviceTier"], json!("flex"));
        assert_eq!(start["modelProvider"], json!("openai"));
        assert_eq!(start["baseInstructions"], json!("Base"));
        assert_eq!(start["developerInstructions"], json!("Dev"));
        assert_eq!(start["personality"], json!("pragmatic"));
        assert_eq!(start["config"], json!({ "web_search": true }));
        assert_eq!(start["ephemeral"], json!(true));

        let resume = codex_resume_params(&conversation, "thread-1");
        assert_eq!(resume["threadId"], json!("thread-1"));
        assert_eq!(resume["model"], json!("gpt-5.2"));

        let turn = codex_turn_params(&conversation, "thread-1", "hello")?;
        assert_eq!(turn["threadId"], json!("thread-1"));
        assert_eq!(turn["input"], json!([{ "type": "text", "text": "hello" }]));
        assert_eq!(turn["effort"], json!("high"));
        assert_eq!(turn["summary"], json!("concise"));
        assert_eq!(turn["sandboxPolicy"], json!({ "type": "workspaceWrite" }));
        assert_eq!(
            turn["outputSchema"],
            json!({ "type": "object", "properties": { "ok": { "type": "boolean" } } })
        );

        Ok(())
    }

    #[test]
    fn write_code_preset_maps_to_codex_unrestricted_workspace() -> Result<(), String> {
        let conversation = Conversation {
            id: "conv".to_string(),
            title: "Test".to_string(),
            provider: Provider::Codex,
            cwd: "/tmp/project".to_string(),
            native_session_id: None,
            native_thread_id: None,
            model: None,
            config: json!({ "writeCode": true }),
            status: "idle".to_string(),
            archived: false,
            created_at: 0,
            updated_at: 0,
        };

        let start = codex_start_params(&conversation);
        assert_eq!(start["approvalPolicy"], json!("never"));
        assert_eq!(start["sandbox"], json!("danger-full-access"));

        let turn = codex_turn_params(&conversation, "thread-1", "edit")?;
        assert_eq!(turn["approvalPolicy"], json!("never"));
        assert_eq!(turn["sandboxPolicy"], json!({ "type": "dangerFullAccess" }));

        Ok(())
    }
}
