use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;

use crate::app::ShellEvent;
use crate::store::{Conversation, Provider, Store};

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
    emit(
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
                emit(
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
                emit(
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

    for line in reader.lines() {
        let line = line.map_err(|err| format!("claude stdout failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
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
                emit(
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
        return Err(format!(
            "claude exited with {}: {}",
            output.status,
            stderr.trim()
        ));
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
    let mut child = Command::new("codex")
        .arg("app-server")
        .arg("--listen")
        .arg("stdio://")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
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
        json!({ "threadId": thread_id })
    } else {
        let mut params = json!({
            "cwd": conversation.cwd,
            "serviceName": "capybara",
            "persistExtendedHistory": true
        });
        if let Some(model) = non_empty(conversation.model.as_deref()) {
            params["model"] = json!(model);
        }
        if let Some(policy) = config_str(&conversation.config, "approvalPolicy") {
            params["approvalPolicy"] = json!(policy);
        }
        if let Some(sandbox) = config_str(&conversation.config, "sandbox") {
            params["sandbox"] = json!(codex_sandbox_mode(&sandbox));
        }
        if let Some(tier) = config_str(&conversation.config, "serviceTier") {
            params["serviceTier"] = json!(tier);
        }
        params
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

    let mut turn_params = json!({
        "threadId": thread_id,
        "input": [{ "type": "text", "text": prompt }]
    });
    if let Some(model) = non_empty(conversation.model.as_deref()) {
        turn_params["model"] = json!(model);
    }
    if let Some(effort) = config_str(&conversation.config, "effort") {
        turn_params["effort"] = json!(effort);
    }
    if let Some(policy) = config_str(&conversation.config, "approvalPolicy") {
        turn_params["approvalPolicy"] = json!(policy);
    }
    if let Some(sandbox) = config_str(&conversation.config, "sandbox") {
        turn_params["sandboxPolicy"] = codex_sandbox_policy(&sandbox);
    }
    if let Some(tier) = config_str(&conversation.config, "serviceTier") {
        turn_params["serviceTier"] = json!(tier);
    }
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
                emit(
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

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn claude_args(conversation: &Conversation, prompt: &str, use_resume: bool) -> Vec<String> {
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
    if let Some(system) = config_str(&conversation.config, "appendSystemPrompt") {
        args.push("--append-system-prompt".to_string());
        args.push(system);
    }
    if let Some(budget) = config_str(&conversation.config, "maxBudgetUsd") {
        args.push("--max-budget-usd".to_string());
        args.push(budget);
    }
    if config_bool(&conversation.config, "bare") {
        args.push("--bare".to_string());
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

fn emit(proxy: &EventLoopProxy<ShellEvent>, event: AgentRuntimeEvent) {
    let _send_result = proxy.send_event(ShellEvent::AgentRuntimeEvent { event });
}

#[cfg(test)]
mod tests {
    use super::{claude_args, claude_delta, codex_sandbox_mode, codex_sandbox_policy};
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
                "appendSystemPrompt": "Be concise",
                "maxBudgetUsd": "2.5",
                "bare": true
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
                .any(|window| window == ["--append-system-prompt", "Be concise"])
        );
        assert!(
            args.windows(2)
                .any(|window| window == ["--max-budget-usd", "2.5"])
        );
        assert!(args.contains(&"--bare".to_string()));
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
}
