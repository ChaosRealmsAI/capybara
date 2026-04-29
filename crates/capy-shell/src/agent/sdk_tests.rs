use super::{desktop_tool_path_env, resolve_tool_path, sdk::content_from_output, sdk_args};
use crate::store::{Conversation, Provider};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn agent_sdk_runtime_args_preserve_full_auto_provider_options() {
    let codex = Conversation {
        id: "conv".to_string(),
        title: "Test".to_string(),
        provider: Provider::Codex,
        cwd: "/tmp/project".to_string(),
        native_session_id: None,
        native_thread_id: Some("thread-1".to_string()),
        model: Some("gpt-5.5".to_string()),
        config: json!({
            "runtimeBackend": "sdk",
            "writeCode": true,
            "effort": "xhigh",
            "approvalPolicy": "never",
            "sandbox": "danger-full-access",
            "codexConfig": ["model_verbosity=\"low\""]
        }),
        status: "idle".to_string(),
        archived: false,
        created_at: 0,
        updated_at: 0,
    };

    let codex_args = sdk_args(&codex, "edit", false);
    assert!(
        codex_args
            .windows(2)
            .any(|window| window == ["--provider", "codex"])
    );
    assert!(codex_args.contains(&"--write-code".to_string()));
    assert!(
        codex_args
            .windows(2)
            .any(|window| window == ["--approval-policy", "never"])
    );
    assert!(
        codex_args
            .windows(2)
            .any(|window| window == ["--sandbox", "danger-full-access"])
    );
    assert!(
        codex_args
            .windows(2)
            .any(|window| window == ["--thread-id", "thread-1"])
    );

    let claude = Conversation {
        provider: Provider::Claude,
        native_thread_id: None,
        native_session_id: Some("00000000-0000-4000-8000-000000000001".to_string()),
        model: Some("sonnet".to_string()),
        config: json!({
            "runtimeBackend": "sdk",
            "writeCode": true,
            "permissionMode": "bypassPermissions",
            "allowedTools": "Bash,Read",
            "settingSources": ["project"]
        }),
        ..codex
    };

    let claude_args = sdk_args(&claude, "continue", true);
    assert!(
        claude_args
            .windows(2)
            .any(|window| window == ["--provider", "claude"])
    );
    assert!(claude_args.contains(&"--write-code".to_string()));
    assert!(
        claude_args
            .windows(2)
            .any(|window| window == ["--permission-mode", "bypassPermissions"])
    );
    assert!(
        claude_args
            .windows(2)
            .any(|window| window == ["--resume", "00000000-0000-4000-8000-000000000001"])
    );
}

#[test]
fn agent_sdk_runtime_prefers_primary_content_over_hook_tail() {
    let output = json!({
        "ok": true,
        "primary_content": "actual assistant answer",
        "content": "stop hook reminder"
    });

    assert_eq!(content_from_output(&output), "actual assistant answer");
}

#[test]
fn desktop_tool_path_env_includes_common_gui_missing_dirs() {
    let path_env = desktop_tool_path_env();
    let dirs: Vec<PathBuf> = std::env::split_paths(&path_env).collect();

    assert!(dirs.contains(&PathBuf::from("/opt/homebrew/bin")));
    assert!(dirs.contains(&PathBuf::from("/usr/local/bin")));
}

#[test]
fn resolves_tool_from_augmented_path_env() -> Result<(), Box<dyn std::error::Error>> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("time should be available: {err}"))?
        .as_nanos();
    let temp_dir =
        std::env::temp_dir().join(format!("capy-tool-path-{}-{suffix}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;
    let codex = temp_dir.join("codex");
    fs::write(&codex, "#!/bin/sh\n")?;

    let path_env = std::env::join_paths([temp_dir.clone()])?
        .to_string_lossy()
        .into_owned();
    let resolved = resolve_tool_path("codex", &path_env);

    let _cleanup = fs::remove_dir_all(&temp_dir);
    assert_eq!(resolved, Some(codex));
    Ok(())
}
