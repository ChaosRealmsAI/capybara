use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::store::{Conversation, Provider, Store};

pub(super) struct SdkRunOutput {
    pub content: String,
    pub native_thread_id: Option<String>,
}

pub(super) fn enabled(config: &Value) -> bool {
    config
        .get("runtimeBackend")
        .and_then(Value::as_str)
        .map(|value| value.eq_ignore_ascii_case("sdk"))
        .unwrap_or(false)
        || config.get("sdk").and_then(Value::as_bool).unwrap_or(false)
}

pub(super) fn run(
    store: &Store,
    conversation: &Conversation,
    run_id: &str,
    prompt: &str,
) -> Result<SdkRunOutput, String> {
    let use_resume = store
        .messages_for(&conversation.id)?
        .iter()
        .any(|message| message.role == "assistant");
    let script = sdk_script_path();
    let mut command = Command::new("node");
    command
        .arg(&script)
        .args(args(conversation, prompt, use_resume))
        .current_dir(repo_root())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().map_err(|err| {
        format!(
            "agent SDK bridge failed to start at {}: {err}",
            script.display()
        )
    })?;
    store.set_run_pid(run_id, child.id())?;
    let output = child
        .wait_with_output()
        .map_err(|err| format!("agent SDK bridge wait failed: {err}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "agent SDK bridge exited with {}: {}{}",
            output.status,
            stdout.trim(),
            stderr.trim()
        ));
    }
    let value: Value = serde_json::from_str(&stdout)
        .map_err(|err| format!("agent SDK bridge returned invalid JSON: {err}: {stdout}"))?;
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(format!("agent SDK bridge returned failure: {value}"));
    }
    let content = content_from_output(&value);
    let native_thread_id = value
        .get("thread_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(SdkRunOutput {
        content,
        native_thread_id,
    })
}

pub(super) fn content_from_output(value: &Value) -> String {
    value
        .get("primary_content")
        .or_else(|| value.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
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
