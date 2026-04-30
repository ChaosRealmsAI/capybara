use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{io::BufRead, io::BufReader, io::Read, thread};

use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;

use super::tool_path::tool_launch;
use super::{event, record_and_emit};
use crate::agent_tools::{
    agent_tool_env, claude_append_system_prompt, codex_developer_instructions,
};
use crate::app::ShellEvent;
use crate::store::{Conversation, Provider, Store};

pub(super) struct SdkRunOutput {
    pub content: String,
    pub native_session_id: Option<String>,
    pub native_thread_id: Option<String>,
    pub event_json: Value,
}

pub(super) fn run(
    store: &Store,
    proxy: &EventLoopProxy<ShellEvent>,
    conversation: &Conversation,
    run_id: &str,
    prompt: &str,
) -> Result<SdkRunOutput, String> {
    let use_resume = store
        .messages_for(&conversation.id)?
        .iter()
        .any(|message| message.role == "assistant");
    let script = sdk_script_path();
    if !script.is_file() {
        return Err(format!(
            "agent SDK bridge script missing: {}",
            script.display()
        ));
    }
    let prompt = sdk_prompt(conversation, prompt);
    let launch = tool_launch("node");
    let mut command = Command::new(launch.program());
    command
        .arg(&script)
        .arg("run-stream")
        .args(stream_args(conversation, &prompt, use_resume))
        .current_dir(repo_root())
        .env("PATH", launch.path_env())
        .envs(agent_tool_env(&conversation.config))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|err| {
        format!(
            "agent SDK bridge failed to start using {} at {}: {err}",
            launch.display(),
            script.display(),
        )
    })?;
    store.set_run_pid(run_id, child.id())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "agent SDK bridge stdout missing".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "agent SDK bridge stderr missing".to_string())?;
    let stderr_handle = thread::spawn(move || {
        let mut reader = stderr;
        let mut output = String::new();
        let _ = reader.read_to_string(&mut output);
        output
    });
    let reader = BufReader::new(stdout);
    let mut final_value = None;
    let mut failure = None;

    for line in reader.lines() {
        let line = line.map_err(|err| format!("agent SDK bridge stdout failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .map_err(|err| format!("agent SDK bridge returned invalid JSONL: {err}: {line}"))?;
        match value.get("type").and_then(Value::as_str) {
            Some("segment") => {
                emit_sdk_event(store, proxy, conversation, run_id, "segment", &value)
            }
            Some("run_completed") => {
                emit_sdk_event(
                    store,
                    proxy,
                    conversation,
                    run_id,
                    "sdk_run_completed",
                    &value,
                );
                final_value = Some(value);
            }
            Some("run_failed") => {
                emit_sdk_event(store, proxy, conversation, run_id, "sdk_run_failed", &value);
                failure = value
                    .get("error")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .or_else(|| Some(value.to_string()));
            }
            _ => emit_sdk_event(store, proxy, conversation, run_id, "sdk_event", &value),
        }
    }

    let status = child
        .wait()
        .map_err(|err| format!("agent SDK bridge wait failed: {err}"))?;
    let stderr = stderr_handle.join().unwrap_or_default();
    if let Some(error) = failure {
        return Err(error);
    }
    if !status.success() {
        return Err(format!(
            "agent SDK bridge exited with {status}: {}",
            stderr.trim()
        ));
    }
    let value = final_value.ok_or_else(|| {
        format!(
            "agent SDK bridge ended without run_completed: {}",
            stderr.trim()
        )
    })?;
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(format!("agent SDK bridge returned failure: {value}"));
    }
    let content = content_from_output(&value);
    let native_session_id = value
        .get("session_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let native_thread_id = value
        .get("thread_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let event_json = json!({
        "provider": conversation.provider.as_str(),
        "runtime": "sdk",
        "segments": value.get("segments").cloned().unwrap_or_else(|| json!([])),
        "raw": value
    });
    Ok(SdkRunOutput {
        content,
        native_session_id,
        native_thread_id,
        event_json,
    })
}

pub(super) fn doctor() -> Value {
    let script = sdk_script_path();
    if !script.is_file() {
        return json!({
            "ok": false,
            "kind": "capy-agent-sdk-doctor",
            "error": format!("agent SDK bridge script missing: {}", script.display())
        });
    }
    let launch = tool_launch("node");
    let output = Command::new(launch.program())
        .arg(&script)
        .arg("doctor")
        .current_dir(repo_root())
        .env("PATH", launch.path_env())
        .output();
    match output {
        Ok(output) if output.status.success() => serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|err| {
                json!({
                    "ok": false,
                    "kind": "capy-agent-sdk-doctor",
                    "error": format!("doctor returned invalid JSON: {err}")
                })
            }),
        Ok(output) => json!({
            "ok": false,
            "kind": "capy-agent-sdk-doctor",
            "status": output.status.code(),
            "stderr": String::from_utf8_lossy(&output.stderr).trim()
        }),
        Err(err) => json!({
            "ok": false,
            "kind": "capy-agent-sdk-doctor",
            "error": format!("node failed to start using {}: {err}", launch.display())
        }),
    }
}

pub(super) fn content_from_output(value: &Value) -> String {
    value
        .get("primary_content")
        .or_else(|| value.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn emit_sdk_event(
    store: &Store,
    proxy: &EventLoopProxy<ShellEvent>,
    conversation: &Conversation,
    run_id: &str,
    kind: &str,
    value: &Value,
) {
    let segment = value.get("segment").unwrap_or(value);
    let status = segment
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| value.get("status").and_then(Value::as_str))
        .unwrap_or("running");
    let content = segment
        .get("summary")
        .or_else(|| segment.get("title"))
        .or_else(|| segment.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("");
    record_and_emit(
        store,
        proxy,
        event(conversation, run_id, kind)
            .with_status(status)
            .with_content(content)
            .with_event(value.clone()),
    );
}

pub(super) fn sdk_prompt(conversation: &Conversation, prompt: &str) -> String {
    let mut instructions = Vec::new();
    match conversation.provider {
        Provider::Claude => {
            push_instruction(
                &mut instructions,
                config_str(&conversation.config, "systemPrompt"),
            );
            push_instruction(
                &mut instructions,
                claude_append_system_prompt(
                    config_str(&conversation.config, "appendSystemPrompt"),
                    &conversation.config,
                ),
            );
        }
        Provider::Codex => {
            push_instruction(
                &mut instructions,
                config_str(&conversation.config, "baseInstructions"),
            );
            push_instruction(
                &mut instructions,
                codex_developer_instructions(
                    config_str(&conversation.config, "developerInstructions"),
                    &conversation.config,
                ),
            );
        }
    }

    if instructions.is_empty() {
        return prompt.to_string();
    }

    format!(
        "Capybara runtime instructions:\n{}\n\nUser prompt:\n{}",
        instructions.join("\n\n"),
        prompt
    )
}

fn push_instruction(instructions: &mut Vec<String>, value: Option<String>) {
    if let Some(value) = value {
        instructions.push(value);
    }
}

pub(super) fn args(
    conversation: &Conversation,
    prompt: &str,
    use_claude_resume: bool,
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--provider".to_string(),
        conversation.provider.as_str().to_string(),
        "--cwd".to_string(),
        conversation.cwd.clone(),
        "--prompt".to_string(),
        prompt.to_string(),
        "--json".to_string(),
    ];
    push_opt(&mut args, "--model", conversation.model.clone());
    push_bool(
        &mut args,
        "--write-code",
        config_bool(&conversation.config, "writeCode"),
    );
    push_opt(
        &mut args,
        "--effort",
        config_str(&conversation.config, "effort"),
    );
    push_opt(
        &mut args,
        "--permission-mode",
        config_str(&conversation.config, "permissionMode"),
    );
    push_opt(
        &mut args,
        "--approval-policy",
        config_str(&conversation.config, "approvalPolicy"),
    );
    push_opt(
        &mut args,
        "--sandbox",
        config_str(&conversation.config, "sandbox"),
    );
    push_many(
        &mut args,
        "--add-dir",
        config_array(&conversation.config, "addDirs"),
    );
    push_opt(
        &mut args,
        "--allowed-tools",
        config_str(&conversation.config, "allowedTools"),
    );
    push_opt(
        &mut args,
        "--disallowed-tools",
        config_str(&conversation.config, "disallowedTools"),
    );
    push_opt(
        &mut args,
        "--tools",
        config_str(&conversation.config, "tools"),
    );
    push_opt(
        &mut args,
        "--mcp-config",
        config_str(&conversation.config, "mcpConfig"),
    );
    let output_schema = config_str(&conversation.config, "outputSchema")
        .or_else(|| config_str(&conversation.config, "jsonSchema"));
    push_opt(&mut args, "--output-schema", output_schema);
    push_opt(
        &mut args,
        "--max-budget-usd",
        config_str(&conversation.config, "maxBudgetUsd"),
    );
    push_opt(
        &mut args,
        "--max-turns",
        config_str(&conversation.config, "maxTurns"),
    );
    push_bool(
        &mut args,
        "--search",
        config_bool(&conversation.config, "search"),
    );
    push_bool(
        &mut args,
        "--no-session-persistence",
        config_bool(&conversation.config, "noSessionPersistence"),
    );
    push_many(
        &mut args,
        "--setting-source",
        config_array(&conversation.config, "settingSources"),
    );
    push_many(
        &mut args,
        "--codex-config",
        config_array(&conversation.config, "codexConfig"),
    );

    match conversation.provider {
        Provider::Codex => {
            push_opt(
                &mut args,
                "--thread-id",
                conversation.native_thread_id.clone(),
            );
        }
        Provider::Claude => {
            if use_claude_resume {
                push_opt(
                    &mut args,
                    "--resume",
                    conversation.native_session_id.clone(),
                );
            } else {
                push_opt(
                    &mut args,
                    "--session-id",
                    conversation.native_session_id.clone(),
                );
            }
        }
    }
    args
}

fn stream_args(conversation: &Conversation, prompt: &str, use_claude_resume: bool) -> Vec<String> {
    args(conversation, prompt, use_claude_resume)
        .into_iter()
        .filter(|value| value != "--json")
        .collect()
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

fn config_str(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
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
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn push_opt(args: &mut Vec<String>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        args.push(key.to_string());
        args.push(value);
    }
}

fn push_many(args: &mut Vec<String>, key: &str, values: Vec<String>) {
    for value in values {
        push_opt(args, key, Some(value));
    }
}

fn push_bool(args: &mut Vec<String>, key: &str, value: bool) {
    if value {
        args.push(key.to_string());
    }
}
