use std::path::Path;

use serde_json::{Value, json};

use crate::agent_tools::codex_developer_instructions;
use crate::store::Conversation;

use super::{config_array, config_bool, config_str, non_empty};

pub(super) fn app_server_args(config: &Value) -> Vec<String> {
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

pub(super) fn start_params(conversation: &Conversation) -> Value {
    let mut params = json!({
        "cwd": conversation.cwd,
        "serviceName": "capybara",
        "persistExtendedHistory": true
    });
    apply_thread_overrides(&mut params, conversation);
    params
}

pub(super) fn resume_params(conversation: &Conversation, thread_id: &str) -> Value {
    let mut params = json!({ "threadId": thread_id });
    apply_thread_overrides(&mut params, conversation);
    params
}

pub(super) fn turn_params(
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
    if let Some(approval_policy) = approval_policy(&conversation.config) {
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
    if let Some(sandbox) = sandbox_setting(&conversation.config) {
        params["sandboxPolicy"] = sandbox_policy(&sandbox);
    }
    if let Some(output_schema) = config_str(&conversation.config, "outputSchema") {
        params["outputSchema"] = json_or_file(&output_schema)?;
    }
    Ok(params)
}

fn apply_thread_overrides(params: &mut Value, conversation: &Conversation) {
    if let Some(model) = non_empty(conversation.model.as_deref()) {
        params["model"] = json!(model);
    }
    set_config_str_param(params, &conversation.config, "approvalsReviewer");
    set_config_str_param(params, &conversation.config, "baseInstructions");
    if let Some(instructions) = codex_developer_instructions(
        config_str(&conversation.config, "developerInstructions"),
        &conversation.config,
    ) {
        params["developerInstructions"] = json!(instructions);
    }
    set_config_str_param(params, &conversation.config, "modelProvider");
    set_config_str_param(params, &conversation.config, "personality");
    set_config_str_param(params, &conversation.config, "serviceTier");
    if let Some(approval_policy) = approval_policy(&conversation.config) {
        params["approvalPolicy"] = json!(approval_policy);
    }
    if let Some(sandbox) = sandbox_setting(&conversation.config) {
        params["sandbox"] = json!(sandbox_mode(&sandbox));
    }
    if config_bool(&conversation.config, "ephemeral") {
        params["ephemeral"] = json!(true);
    }
    if config_bool(&conversation.config, "search") {
        params["config"] = json!({ "web_search": true });
    }
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

fn approval_policy(config: &Value) -> Option<String> {
    config_str(config, "approvalPolicy")
        .or_else(|| config_bool(config, "writeCode").then(|| "never".to_string()))
}

fn sandbox_setting(config: &Value) -> Option<String> {
    config_str(config, "sandbox")
        .or_else(|| config_bool(config, "writeCode").then(|| "danger-full-access".to_string()))
}

pub(super) fn sandbox_mode(value: &str) -> &str {
    match value {
        "read-only" | "readOnly" => "read-only",
        "danger-full-access" | "dangerFullAccess" => "danger-full-access",
        _ => "workspace-write",
    }
}

pub(super) fn sandbox_policy(value: &str) -> Value {
    match value {
        "read-only" | "readOnly" => json!({ "type": "readOnly" }),
        "danger-full-access" | "dangerFullAccess" => json!({ "type": "dangerFullAccess" }),
        _ => json!({ "type": "workspaceWrite" }),
    }
}
