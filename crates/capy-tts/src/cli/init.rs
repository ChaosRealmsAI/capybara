//! TTS whisperX runtime initialization commands.

use anyhow::Result;
use serde_json::{Value, json};

use crate::cli::args::{DoctorArgs, InitArgs};
use crate::output::write_stdout_line;
use crate::whisper::runtime;

pub(crate) fn doctor(args: DoctorArgs) -> Result<()> {
    let languages = runtime::resolve_languages(&args.languages, args.all)?;
    let selected = runtime::existing_runtime(
        args.runtime.cache_dir.as_deref(),
        args.runtime.python.as_deref(),
    );
    let status = runtime::status_or_error(&selected, &languages);

    write_stdout_line(format_args!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": status.get("ok").and_then(Value::as_bool).unwrap_or(false),
            "kind": "tts-doctor",
            "default_backend": "edge",
            "config_path": crate::config::TtsConfig::config_path(),
            "align_script": align_script_status(),
            "runtime": runtime::runtime_json(&selected),
            "alignment": status,
            "providers": {
                "edge": {
                    "available": true,
                    "spend": false,
                },
                "volcengine": {
                    "configured": volcengine_configured(),
                    "spend": true,
                }
            },
            "commands": ["doctor", "init", "synth", "batch", "play", "preview", "voices", "concat", "config"],
        }))?
    ));
    Ok(())
}

pub(crate) fn run(args: InitArgs) -> Result<()> {
    let languages = runtime::resolve_languages(&args.languages, args.all)?;
    let existing = runtime::existing_runtime(
        args.runtime.cache_dir.as_deref(),
        args.runtime.python.as_deref(),
    );
    let before = runtime::status_or_error(&existing, &languages);
    let existing_packages_ready = runtime::packages_ready(&before);
    let fixed_python = runtime::fixed_python_requested(args.runtime.python.as_deref());

    let selected = if existing_packages_ready || fixed_python || args.skip_pip {
        existing.clone()
    } else {
        runtime::managed_runtime(args.runtime.cache_dir.as_deref())
    };

    let mut actions = Vec::new();
    if selected.source == runtime::RuntimeSource::ManagedVenv && !selected.python.is_file() {
        actions.push("create-venv");
    }

    let selected_before = if selected.python == existing.python
        && selected.hf_home == existing.hf_home
        && selected.source == existing.source
    {
        before.clone()
    } else {
        runtime::status_or_error(&selected, &languages)
    };
    let selected_packages_ready = runtime::packages_ready(&selected_before);

    if !args.skip_pip && !selected_packages_ready {
        actions.push("install-python-packages");
    }

    let missing_before = runtime::missing_languages(&selected_before);
    if !missing_before.is_empty() {
        actions.push("download-align-models");
    }

    if args.dry_run {
        write_stdout_line(format_args!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "kind": "tts-init",
                "dry_run": true,
                "languages": languages,
                "runtime": runtime::runtime_json(&selected),
                "before": selected_before,
                "missing_languages": missing_before,
                "planned_actions": actions,
                "packages": runtime::PYTHON_PACKAGES,
            }))?
        ));
        return Ok(());
    }

    let mut created_venv = false;
    if selected.source == runtime::RuntimeSource::ManagedVenv {
        created_venv = runtime::create_venv(&selected)?;
    }

    let mut installed_packages = false;
    let after_venv = runtime::status_or_error(&selected, &languages);
    if !args.skip_pip && !runtime::packages_ready(&after_venv) {
        runtime::install_packages(&selected)?;
        installed_packages = true;
    }

    let before_download = runtime::status_or_error(&selected, &languages);
    let missing = runtime::missing_languages(&before_download);
    let download = if missing.is_empty() {
        json!({
            "ok": true,
            "kind": "tts-align-download",
            "skipped": true,
            "reason": "all requested align models already cached",
            "languages": languages,
        })
    } else {
        runtime::download(&selected, &missing)?
    };

    let after = runtime::status_or_error(&selected, &languages);
    let ok = after.get("ok").and_then(Value::as_bool).unwrap_or(false);
    write_stdout_line(format_args!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": ok,
            "kind": "tts-init",
            "dry_run": false,
            "languages": languages,
            "runtime": runtime::runtime_json(&selected),
            "created_venv": created_venv,
            "installed_packages": installed_packages,
            "packages": runtime::PYTHON_PACKAGES,
            "before": before,
            "before_download": before_download,
            "download": download,
            "after": after,
        }))?
    ));
    Ok(())
}

fn align_script_status() -> Value {
    let align_env = std::env::var("CAPY_TTS_ALIGN_SCRIPT").ok();
    let bundled_align =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/align_ffa.py");
    let path = align_env
        .clone()
        .unwrap_or_else(|| bundled_align.display().to_string());
    json!({
        "path": path,
        "exists": std::path::Path::new(&path).is_file(),
        "env": align_env,
    })
}

fn volcengine_configured() -> bool {
    std::env::var("VOLCENGINE_APP_ID").is_ok() && std::env::var("VOLCENGINE_ACCESS_TOKEN").is_ok()
}
