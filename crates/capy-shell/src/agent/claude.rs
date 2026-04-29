use std::process::Command;

use serde_json::Value;

use crate::agent_tools::{agent_tool_env, claude_append_system_prompt};
use crate::store::Conversation;

use super::{config_array, config_bool, config_str, non_empty};

pub(super) fn delta(value: &Value) -> Option<String> {
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
            if item.get("type").and_then(Value::as_str) == Some("text")
                && let Some(chunk) = item.get("text").and_then(Value::as_str)
            {
                text.push_str(chunk);
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

pub(super) fn result_fallback(raw: &str) -> String {
    raw.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| delta(&value))
        .collect::<Vec<_>>()
        .join("")
}

pub(super) fn args(conversation: &Conversation, prompt: &str, use_resume: bool) -> Vec<String> {
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
    for (flag, key) in [
        ("--allowedTools", "allowedTools"),
        ("--disallowedTools", "disallowedTools"),
        ("--mcp-config", "mcpConfig"),
        ("--system-prompt", "systemPrompt"),
        ("--max-budget-usd", "maxBudgetUsd"),
        ("--fallback-model", "fallbackModel"),
        ("--json-schema", "jsonSchema"),
        ("--settings", "settings"),
        ("--debug-file", "debugFile"),
        ("--agent", "agent"),
        ("--agents", "agents"),
        ("--tools", "tools"),
    ] {
        if let Some(value) = config_str(&conversation.config, key) {
            args.push(flag.to_string());
            args.push(value);
        }
    }
    if let Some(system) = claude_append_system_prompt(
        config_str(&conversation.config, "appendSystemPrompt"),
        &conversation.config,
    ) {
        args.push("--append-system-prompt".to_string());
        args.push(system);
    }
    for beta in config_array(&conversation.config, "betas") {
        args.push("--betas".to_string());
        args.push(beta);
    }
    for plugin_dir in config_array(&conversation.config, "pluginDirs") {
        args.push("--plugin-dir".to_string());
        args.push(plugin_dir);
    }
    for (enabled, flag) in [
        (config_bool(&conversation.config, "bare"), "--bare"),
        (
            config_bool(&conversation.config, "strictMcpConfig"),
            "--strict-mcp-config",
        ),
        (
            config_bool(&conversation.config, "includeHookEvents"),
            "--include-hook-events",
        ),
        (
            config_bool(&conversation.config, "noSessionPersistence"),
            "--no-session-persistence",
        ),
        (
            config_bool(&conversation.config, "allowDangerouslySkipPermissions") || write_code,
            "--allow-dangerously-skip-permissions",
        ),
        (
            config_bool(&conversation.config, "dangerouslySkipPermissions") || write_code,
            "--dangerously-skip-permissions",
        ),
    ] {
        if enabled {
            args.push(flag.to_string());
        }
    }
    args.push("--".to_string());
    args.push(prompt.to_string());
    args
}

pub(super) fn apply_tool_env(command: &mut Command, config: &Value) {
    for (key, value) in agent_tool_env(config) {
        command.env(key, value);
    }
}
