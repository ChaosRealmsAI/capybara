use std::path::Path;
use std::process::Command;

mod runtime;

use crate::error::{TimelineError, TimelineErrorCode};
use crate::ports::{
    CompositionArtifact, ExportOptions, ExportReport, SnapshotOptions, SnapshotReport,
    TimelineRecorderPort,
};

use runtime::{
    capy_shell_helper_path_for_recorder, ensure_parent, recorder_export_command,
    recorder_snapshot_command, render_source_path, stderr_tail, stdout_tail,
};

#[derive(Debug, Clone, Default)]
pub struct RecorderAdapter;

impl TimelineRecorderPort for RecorderAdapter {
    fn snapshot(
        &self,
        source: &Path,
        out: &Path,
        options: SnapshotOptions,
    ) -> Result<SnapshotReport, TimelineError> {
        snapshot_with_recorder_crate(source, out, options)
    }

    fn export(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
        options: ExportOptions,
    ) -> Result<ExportReport, TimelineError> {
        export_with_recorder_crate(artifact, out, options)
    }
}

#[cfg(target_os = "macos")]
fn snapshot_with_recorder_crate(
    source: &Path,
    out: &Path,
    options: SnapshotOptions,
) -> Result<SnapshotReport, TimelineError> {
    ensure_parent(out, TimelineErrorCode::SnapshotFailed)?;
    let source_path = source.to_path_buf();
    let out_path = out.to_path_buf();
    let resolution = options
        .resolution
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(parse_resolution)
        .transpose()?;
    let t_ms = options.t_ms;
    run_recorder_thread(move || {
        runtime::recorder_runtime()?
            .block_on(capy_recorder::snapshot_from_source(
                &source_path,
                &out_path,
                t_ms,
                resolution,
            ))
            .map_err(|err| {
                TimelineError::new(
                    TimelineErrorCode::SnapshotFailed,
                    format!("capy-recorder crate snapshot failed: {err}"),
                    "next step · rerun capy timeline snapshot",
                )
            })
    })
    .map(|_| ())?;

    Ok(SnapshotReport {
        ok: true,
        output: out.to_path_buf(),
        command: recorder_snapshot_command(source, out, options.t_ms),
        stdout: String::new(),
        stderr: String::new(),
    })
}

#[cfg(target_os = "macos")]
fn run_recorder_thread<T, F>(job: F) -> Result<T, TimelineError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, TimelineError> + Send + 'static,
{
    std::thread::spawn(job).join().map_err(|_| {
        TimelineError::new(
            TimelineErrorCode::TimelineNotFound,
            "capy-recorder worker thread panicked",
            "next step · rerun capy timeline export and inspect logs",
        )
    })?
}

#[cfg(not(target_os = "macos"))]
fn snapshot_with_recorder_crate(
    _source: &Path,
    _out: &Path,
    _options: SnapshotOptions,
) -> Result<SnapshotReport, TimelineError> {
    Err(TimelineError::new(
        TimelineErrorCode::TimelineNotFound,
        "capy-recorder crate snapshot is only available on macOS",
        "embedded mode required",
    ))
}

#[cfg(target_os = "macos")]
fn export_with_recorder_crate(
    artifact: &CompositionArtifact,
    out: &Path,
    options: ExportOptions,
) -> Result<ExportReport, TimelineError> {
    let source = render_source_path(&artifact.composition_path);
    ensure_parent(out, TimelineErrorCode::ExportFailed)?;
    capy_recorder::validate_render_source_file(&source).map_err(|err| {
        TimelineError::new(
            TimelineErrorCode::ExportFailed,
            format!("capy-recorder source validation failed: {err}"),
            "next step · rerun capy timeline compile",
        )
    })?;
    let command = recorder_export_command(&source, out, &options)?;
    let mut process = Command::new(&command[0]);
    process.args(&command[1..]);
    if let Some(helper) = capy_shell_helper_path_for_recorder(Path::new(&command[0]))? {
        process.env("NF_CEF_HELPER", helper);
    }
    let output = process.output().map_err(|err| {
        TimelineError::new(
            TimelineErrorCode::ExportFailed,
            format!("spawn capy-recorder export failed: {err}"),
            "next step · build capy-recorder with cef-osr and stage the app bundle",
        )
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(TimelineError::new(
            TimelineErrorCode::ExportFailed,
            format!(
                "capy-recorder export failed with status {}: {}{}",
                output
                    .status
                    .code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "signal".to_string()),
                stderr_tail(&stderr),
                stdout_tail(&stdout),
            ),
            "next step · inspect recorder stderr and rerun strict export",
        ));
    }

    Ok(ExportReport {
        ok: true,
        output: out.to_path_buf(),
        command,
        stdout,
        stderr,
    })
}

#[cfg(not(target_os = "macos"))]
fn export_with_recorder_crate(
    _artifact: &CompositionArtifact,
    _out: &Path,
    _options: ExportOptions,
) -> Result<ExportReport, TimelineError> {
    Err(TimelineError::new(
        TimelineErrorCode::TimelineNotFound,
        "capy-recorder crate export is only available on macOS",
        "embedded mode required",
    ))
}

#[cfg(target_os = "macos")]
fn parse_resolution(raw: &str) -> Result<capy_recorder::ExportResolution, TimelineError> {
    capy_recorder::ExportResolution::parse_str(raw).ok_or_else(|| {
        TimelineError::new(
            TimelineErrorCode::SnapshotFailed,
            format!("unsupported snapshot resolution: {raw}"),
            "next step · pass resolution 720p, 1080p, or 4k",
        )
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::ports::ExportOptions;

    use super::runtime::{build_recorder_export_command, recorder_snapshot_command};

    #[test]
    fn recorder_snapshot_command_targets_source_api() {
        let command = recorder_snapshot_command(
            Path::new("/tmp/render_source.json"),
            Path::new("/tmp/frame.png"),
            42,
        );

        assert_eq!(command[0], "capy-recorder");
        assert_eq!(command[1], "snapshot-source");
        assert!(command.contains(&"--source".to_string()));
        assert!(command.contains(&"--t-ms".to_string()));
        assert!(command.contains(&"42".to_string()));
        assert!(command.contains(&"/tmp/frame.png".to_string()));
    }

    #[test]
    fn recorder_export_command_carries_fps() {
        let command = build_recorder_export_command(
            PathBuf::from("capy-recorder"),
            Path::new("/tmp/render_source.json"),
            Path::new("/tmp/out.mp4"),
            &ExportOptions {
                profile: "final".to_string(),
                fps: 30,
                resolution: Some("4k".to_string()),
                parallel: Some(2),
                strict_recorder: true,
            },
        );

        assert_eq!(command[0], "capy-recorder");
        assert_eq!(command[1], "export");
        assert!(command.contains(&"--source".to_string()));
        assert!(command.contains(&"--fps".to_string()));
        assert!(command.contains(&"30".to_string()));
        assert!(command.contains(&"--resolution".to_string()));
        assert!(command.contains(&"4k".to_string()));
        assert!(command.contains(&"--parallel".to_string()));
        assert!(command.contains(&"2".to_string()));
        assert!(command.contains(&"/tmp/out.mp4".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unit_test_binary_is_not_treated_as_app_bundle() {
        assert!(!super::runtime::current_exe_is_macos_app_bundle());
    }
}
