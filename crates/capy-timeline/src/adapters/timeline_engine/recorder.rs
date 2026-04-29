use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{TimelineError, TimelineErrorCode};
use crate::ports::{
    CompositionArtifact, ExportOptions, ExportReport, SnapshotOptions, SnapshotReport,
    TimelineRecorderPort,
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
    let resolution = options
        .resolution
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(parse_resolution)
        .transpose()?;
    recorder_runtime()?
        .block_on(capy_recorder::snapshot_from_source(
            source,
            out,
            options.t_ms,
            resolution,
        ))
        .map_err(|err| {
            TimelineError::new(
                TimelineErrorCode::SnapshotFailed,
                format!("capy-recorder crate snapshot failed: {err}"),
                "next step · rerun capy timeline snapshot",
            )
        })?;

    Ok(SnapshotReport {
        ok: true,
        output: out.to_path_buf(),
        command: recorder_snapshot_command(source, out, options.t_ms),
        stdout: String::new(),
        stderr: String::new(),
    })
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
    let summary = capy_recorder::validate_render_source_file(&source).map_err(|err| {
        TimelineError::new(
            TimelineErrorCode::ExportFailed,
            format!("capy-recorder source validation failed: {err}"),
            "next step · rerun capy timeline compile",
        )
    })?;
    let fps = options.fps.max(1);
    let stats = recorder_runtime()?
        .block_on(capy_recorder::run_export_from_source(
            &source,
            out,
            capy_recorder::ExportOpts {
                duration_s: summary.duration_ms as f64 / 1000.0,
                viewport: summary.viewport,
                fps,
                ..Default::default()
            },
        ))
        .map_err(|err| {
            TimelineError::new(
                TimelineErrorCode::ExportFailed,
                format!("capy-recorder crate export failed: {err}"),
                "next step · rerun capy timeline export",
            )
        })?;

    Ok(ExportReport {
        ok: true,
        output: out.to_path_buf(),
        command: recorder_export_command(&source, out, fps),
        stdout: serde_json::to_string(&serde_json::json!({
            "path": stats.path,
            "frames": stats.frames,
            "duration_ms": stats.duration_ms,
            "size_bytes": stats.size_bytes,
            "moov_front": stats.moov_front
        }))
        .unwrap_or_default(),
        stderr: String::new(),
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

fn render_source_path(composition_path: &Path) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("render_source.json")
}

fn recorder_snapshot_command(source: &Path, out: &Path, t_ms: u64) -> Vec<String> {
    vec![
        "capy-recorder".to_string(),
        "snapshot-source".to_string(),
        "--source".to_string(),
        source.display().to_string(),
        "--t-ms".to_string(),
        t_ms.to_string(),
        "--output".to_string(),
        out.display().to_string(),
    ]
}

fn recorder_export_command(source: &Path, out: &Path, fps: u32) -> Vec<String> {
    vec![
        "capy-recorder".to_string(),
        "export".to_string(),
        "--source".to_string(),
        source.display().to_string(),
        "--profile".to_string(),
        "draft".to_string(),
        "--output".to_string(),
        out.display().to_string(),
        "--fps".to_string(),
        fps.to_string(),
    ]
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

#[cfg(target_os = "macos")]
fn recorder_runtime() -> Result<tokio::runtime::Runtime, TimelineError> {
    if !current_exe_is_macos_app_bundle() {
        return Err(TimelineError::new(
            TimelineErrorCode::TimelineNotFound,
            "capy-recorder crate mode requires a macOS app bundle CEF runtime",
            "embedded mode required",
        ));
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| {
            TimelineError::new(
                TimelineErrorCode::TimelineNotFound,
                format!("create capy-recorder runtime failed: {err}"),
                "embedded mode required",
            )
        })
}

#[cfg(target_os = "macos")]
fn current_exe_is_macos_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            let contents_dir = exe.parent()?.parent()?;
            let app_dir = contents_dir.parent()?;
            let has_contents = contents_dir.file_name()?.to_str()? == "Contents";
            let has_app_extension = app_dir.extension()?.to_str()? == "app";
            Some(has_contents && has_app_extension)
        })
        .unwrap_or(false)
}

fn ensure_parent(path: &Path, code: TimelineErrorCode) -> Result<(), TimelineError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            TimelineError::new(
                code,
                format!("create output parent failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{recorder_export_command, recorder_snapshot_command};

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
        let command = recorder_export_command(
            Path::new("/tmp/render_source.json"),
            Path::new("/tmp/out.mp4"),
            24,
        );

        assert_eq!(command[0], "capy-recorder");
        assert_eq!(command[1], "export");
        assert!(command.contains(&"--source".to_string()));
        assert!(command.contains(&"--fps".to_string()));
        assert!(command.contains(&"24".to_string()));
        assert!(command.contains(&"/tmp/out.mp4".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unit_test_binary_is_not_treated_as_app_bundle() {
        assert!(!super::current_exe_is_macos_app_bundle());
    }
}
