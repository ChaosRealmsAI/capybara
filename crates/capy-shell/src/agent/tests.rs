use crate::agent_tools::{
    agent_tool_env, claude_append_system_prompt, codex_developer_instructions,
};
use serde_json::json;

#[test]
fn capy_canvas_tools_extend_claude_and_codex_instructions() -> Result<(), String> {
    let claude_config = json!({
        "appendSystemPrompt": "Existing Claude instruction.",
        "capyCanvasTools": true,
        "capyToolLog": "/tmp/capy-tools.jsonl"
    });
    let prompt = claude_append_system_prompt(
        Some("Existing Claude instruction.".to_string()),
        &claude_config,
    )
    .ok_or_else(|| "claude prompt missing".to_string())?;
    assert!(prompt.contains("Existing Claude instruction."));
    assert!(prompt.contains("target/debug/capy canvas snapshot"));
    assert!(prompt.contains("selected.geometry.x + selected.geometry.w + 48"));

    let codex_config = json!({
        "developerInstructions": "Existing Codex instruction.",
        "capyCanvasTools": true
    });
    let instructions = codex_developer_instructions(
        Some("Existing Codex instruction.".to_string()),
        &codex_config,
    )
    .ok_or_else(|| "codex instructions missing".to_string())?;
    assert!(instructions.contains("Existing Codex instruction."));
    assert!(instructions.contains("target/debug/capy canvas generate-image --dry-run"));

    assert_eq!(
        agent_tool_env(&codex_config),
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
