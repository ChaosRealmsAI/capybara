//! whisper forced alignment process helpers
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use super::runtime;
use super::timeline::detect_language;

#[derive(Debug, Deserialize)]
pub(super) struct FfaOutput {
    #[allow(dead_code)]
    pub(super) duration_ms: u64,
    pub(super) language: String,
    pub(super) units: Vec<FfaUnit>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FfaUnit {
    pub(super) text: String,
    pub(super) start_ms: u64,
    pub(super) end_ms: u64,
}

pub(super) fn run_ffa(audio_path: &Path, original_text: &str) -> Result<FfaOutput> {
    let script = runtime::align_script_path()
        .ok_or_else(|| anyhow!("scripts/align_ffa.py not found (set CAPY_TTS_ALIGN_SCRIPT)"))?;
    let language = detect_language(original_text).unwrap_or("en");
    let py_runtime = runtime::existing_runtime(None, None);

    let mut command = Command::new(&py_runtime.python);
    command
        .arg(&script)
        .arg(audio_path.as_os_str())
        .arg(language)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    runtime::apply_runtime_env(&mut command, &py_runtime);

    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to spawn python for whisperX alignment: {}",
            py_runtime.python.display()
        )
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(original_text.as_bytes())
            .context("failed to pipe original text to align script")?;
    }

    let output = child
        .wait_with_output()
        .context("failed to wait on whisperX alignment subprocess")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "whisperX alignment failed (exit {:?}): {}",
            output.status.code(),
            stderr.trim()
        );
    }

    let stdout = std::str::from_utf8(&output.stdout)
        .context("whisperX alignment output is not valid UTF-8")?
        .trim();
    if stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "whisperX alignment produced empty output: {}",
            stderr.trim()
        );
    }

    serde_json::from_str::<FfaOutput>(stdout).with_context(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        format!(
            "failed to parse whisperX alignment JSON: {}; stderr: {}",
            stdout,
            stderr.trim()
        )
    })
}
