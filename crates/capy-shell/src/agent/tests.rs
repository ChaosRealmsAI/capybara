use super::{
    agent_tool_env, claude_append_system_prompt, claude_args, claude_delta, codex_app_server_args,
    codex_developer_instructions, codex_resume_params, codex_sandbox_mode, codex_sandbox_policy,
    codex_start_params, codex_turn_params,
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
fn capy_canvas_tools_extend_claude_and_codex_instructions() -> Result<(), String> {
    let claude = Conversation {
        id: "conv".to_string(),
        title: "Test".to_string(),
        provider: Provider::Claude,
        cwd: "/tmp".to_string(),
        native_session_id: None,
        native_thread_id: None,
        model: None,
        config: json!({
            "appendSystemPrompt": "Existing Claude instruction.",
            "capyCanvasTools": true,
            "capyToolLog": "/tmp/capy-tools.jsonl"
        }),
        status: "idle".to_string(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    };
    let prompt = claude_append_system_prompt(
        Some("Existing Claude instruction.".to_string()),
        &claude.config,
    )
    .ok_or_else(|| "claude prompt missing".to_string())?;
    assert!(prompt.contains("Existing Claude instruction."));
    assert!(prompt.contains("target/debug/capy canvas snapshot"));
    assert!(prompt.contains("selected.geometry.x + selected.geometry.w + 48"));

    let codex = Conversation {
        provider: Provider::Codex,
        config: json!({
            "developerInstructions": "Existing Codex instruction.",
            "capyCanvasTools": true
        }),
        ..claude
    };
    let instructions = codex_developer_instructions(
        Some("Existing Codex instruction.".to_string()),
        &codex.config,
    )
    .ok_or_else(|| "codex instructions missing".to_string())?;
    assert!(instructions.contains("Existing Codex instruction."));
    assert!(instructions.contains("target/debug/capy canvas generate-image --dry-run"));

    assert_eq!(
        agent_tool_env(&codex.config),
        Vec::<(String, String)>::new(),
        "tool log env only appears when capyToolLog is configured"
    );
    assert_eq!(
        agent_tool_env(&json!({ "capyToolLog": "/tmp/capy-tools.jsonl" })),
        vec![(
            "CAPY_TOOL_CALL_LOG".to_string(),
            "/tmp/capy-tools.jsonl".to_string()
        )]
    );
    Ok(())
}

#[test]
fn codex_project_instructions_extend_developer_instructions() -> Result<(), String> {
    let conversation = Conversation {
        id: "conv".to_string(),
        title: "Test".to_string(),
        provider: Provider::Codex,
        cwd: "/tmp/project".to_string(),
        native_session_id: None,
        native_thread_id: None,
        model: None,
        config: json!({
            "developerInstructions": "Existing Codex instruction.",
            "capyProjectInstructions": true
        }),
        status: "idle".to_string(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    };

    let instructions = codex_developer_instructions(
        Some("Existing Codex instruction.".to_string()),
        &conversation.config,
    )
    .ok_or_else(|| "codex project instructions missing".to_string())?;
    assert!(instructions.starts_with("Existing Codex instruction."));
    assert!(instructions.contains("Capybara desktop communication contract."));
    assert!(instructions.contains("fenced `html` block"));
    assert!(instructions.contains("semantic body fragment"));
    assert!(instructions.contains("Do not include `<style>`, inline `style=\"\"`, scripts"));
    assert!(instructions.contains("compact enough for the right-side chat"));
    assert!(instructions.contains("capy-card"));

    let start = codex_start_params(&conversation);
    assert!(
        start["developerInstructions"]
            .as_str()
            .unwrap_or_default()
            .contains("Capybara desktop communication contract.")
    );
    Ok(())
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
