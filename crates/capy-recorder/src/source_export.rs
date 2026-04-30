use std::process::ExitCode;

use capy_recorder::events::{emit, Event};
use capy_recorder::record_loop::RecordError;
use capy_recorder::{ExportOpts, ExportResolution, RecorderBackend};

pub(crate) struct SourceExportArgs {
    pub(crate) source: std::path::PathBuf,
    pub(crate) output: std::path::PathBuf,
    pub(crate) profile: String,
    pub(crate) diagnostics: Option<std::path::PathBuf>,
    pub(crate) events: bool,
    pub(crate) resolution: Option<String>,
    pub(crate) fps: Option<u32>,
    pub(crate) parallel: Option<usize>,
}

pub(crate) fn dispatch_validate_source(source: std::path::PathBuf) -> ExitCode {
    match capy_recorder::validate_render_source_file(&source) {
        Ok(summary) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ok": true,
                    "source": source.display().to_string(),
                    "summary": summary
                }))
                .unwrap_or_else(|_| "{\"ok\":true}".to_string())
            );
            ExitCode::from(0)
        }
        Err(message) => {
            emit(Event::Error {
                code: "SOURCE_INVALID".into(),
                message,
            });
            ExitCode::from(2)
        }
    }
}

pub(crate) async fn dispatch_source_export(args: SourceExportArgs) -> ExitCode {
    let preset = match source_export_profile(
        &args.profile,
        args.resolution.as_deref(),
        args.fps,
        args.parallel,
    ) {
        Ok(preset) => preset,
        Err(message) => {
            emit(Event::Error {
                code: "CLI_INVALID".into(),
                message,
            });
            return ExitCode::from(1);
        }
    };
    let summary = match capy_recorder::validate_render_source_file(&args.source) {
        Ok(summary) => summary,
        Err(message) => {
            emit(Event::Error {
                code: "SOURCE_INVALID".into(),
                message,
            });
            return ExitCode::from(2);
        }
    };
    if let Some(parent) = args
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        if let Err(err) = std::fs::create_dir_all(parent) {
            emit(Event::Error {
                code: "IO_FAILED".into(),
                message: format!("create output dir: {err}"),
            });
            return ExitCode::from(1);
        }
    }
    if !args.events {
        capy_recorder::events::set_quiet(true);
    }
    let capture = args
        .diagnostics
        .as_ref()
        .map(|_| capy_recorder::events::start_capture());
    let result = capy_recorder::run_export_from_source(
        &args.source,
        &args.output,
        ExportOpts {
            duration_s: summary.duration_ms as f64 / 1000.0,
            fps: preset.fps,
            bitrate_bps: preset.resolution.bitrate_bps(),
            resolution_override: Some(preset.resolution),
            parallel: Some(preset.parallel),
            ..Default::default()
        },
    )
    .await;
    let captured_events = capture
        .map(capy_recorder::events::EventCaptureGuard::finish)
        .unwrap_or_default();
    if !args.events {
        capy_recorder::events::set_quiet(false);
    }
    let stats = match result {
        Ok(stats) => stats,
        Err(err) => {
            emit(Event::Error {
                code: err.code_str().to_string(),
                message: err.to_string(),
            });
            return ExitCode::from(exit_code_u8(&err));
        }
    };
    let audio_muxed = summary.audio_tracks > 0;
    if let Some(path) = args.diagnostics {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            if let Err(err) = std::fs::create_dir_all(parent) {
                emit(Event::Error {
                    code: "IO_FAILED".into(),
                    message: format!("create diagnostics dir: {err}"),
                });
                return ExitCode::from(1);
            }
        }
        let report = serde_json::json!({
            "schema_version": "nf.recorder_diagnostics.v1",
            "source": args.source.display().to_string(),
            "out": args.output.display().to_string(),
            "profile": preset.name,
            "resolution": preset.resolution.as_str(),
            "fps": preset.fps,
            "parallel": preset.parallel,
            "backend": RecorderBackend::only().as_str(),
            "audio_muxed": audio_muxed,
            "stats": {
                "bytes": stats.size_bytes,
                "frames": stats.frames,
                "duration_ms": stats.duration_ms,
                "moov_front": stats.moov_front
            },
            "source_summary": summary,
            "events": captured_events
        });
        if let Err(err) = std::fs::write(
            &path,
            serde_json::to_vec_pretty(&report).unwrap_or_else(|_| b"{}".to_vec()),
        ) {
            emit(Event::Error {
                code: "IO_FAILED".into(),
                message: format!("write diagnostics: {err}"),
            });
            return ExitCode::from(1);
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "out": args.output.display().to_string(),
            "source": args.source.display().to_string(),
            "profile": preset.name,
            "resolution": preset.resolution.as_str(),
            "fps": preset.fps,
            "parallel": preset.parallel,
            "backend": RecorderBackend::only().as_str(),
            "bytes": stats.size_bytes,
            "frames": stats.frames,
            "duration_ms": stats.duration_ms,
            "audio_muxed": audio_muxed
        }))
        .unwrap_or_else(|_| "{\"ok\":true}".to_string())
    );
    ExitCode::from(0)
}

pub(crate) async fn dispatch_snapshot_source(
    source: std::path::PathBuf,
    t_ms: u64,
    output: std::path::PathBuf,
    resolution: Option<String>,
) -> ExitCode {
    let resolution = match resolution.as_deref() {
        Some(raw) => match ExportResolution::parse_str(raw) {
            Some(value) => Some(value),
            None => {
                emit(Event::Error {
                    code: "CLI_INVALID".into(),
                    message: format!("--resolution must be 720p, 1080p or 4k (got '{raw}')"),
                });
                return ExitCode::from(1);
            }
        },
        None => None,
    };
    match capy_recorder::snapshot_from_source(&source, &output, t_ms, resolution).await {
        Ok(()) => {
            emit(Event::SnapshotDone {
                bundle: source.display().to_string(),
                t_ms,
                out: output.display().to_string(),
            });
            ExitCode::from(0)
        }
        Err(e) => {
            let code = e.code_str().to_string();
            let message = e.to_string();
            emit(Event::Error { code, message });
            ExitCode::from(e.exit_code())
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SourceExportProfile {
    name: &'static str,
    resolution: ExportResolution,
    fps: u32,
    parallel: usize,
}

fn source_export_profile(
    profile: &str,
    resolution: Option<&str>,
    fps: Option<u32>,
    parallel: Option<usize>,
) -> Result<SourceExportProfile, String> {
    let mut preset = match profile.trim().to_ascii_lowercase().as_str() {
        "draft" => SourceExportProfile {
            name: "draft",
            resolution: ExportResolution::P720,
            fps: 30,
            parallel: 1,
        },
        "standard" => SourceExportProfile {
            name: "standard",
            resolution: ExportResolution::P1080,
            fps: 30,
            parallel: 1,
        },
        "final" => SourceExportProfile {
            name: "final",
            resolution: ExportResolution::P1080,
            fps: 60,
            parallel: 1,
        },
        "final-fast" | "fast-final" => SourceExportProfile {
            name: "final-fast",
            resolution: ExportResolution::P1080,
            fps: 60,
            parallel: 2,
        },
        other => {
            return Err(format!(
                "--profile must be draft, standard, final or final-fast (got '{other}')"
            ));
        }
    };
    if let Some(raw) = resolution {
        preset.resolution = ExportResolution::parse_str(raw)
            .ok_or_else(|| format!("--resolution must be 720p, 1080p or 4k (got '{raw}')"))?;
    }
    if let Some(value) = fps {
        if value != 30 && value != 60 {
            return Err(format!("--fps must be 30 or 60 (got {value})"));
        }
        preset.fps = value;
    }
    if let Some(value) = parallel {
        if value == 0 || value > 4 {
            return Err(format!("--parallel must be between 1 and 4 (got {value})"));
        }
        preset.parallel = value;
    }
    validate_source_export_combo(preset)?;
    Ok(preset)
}

fn validate_source_export_combo(preset: SourceExportProfile) -> Result<(), String> {
    if preset.resolution == ExportResolution::K4 && preset.fps != 30 {
        return Err(format!(
            "4k export supports 30fps only (got {}fps)",
            preset.fps
        ));
    }
    if preset.resolution == ExportResolution::K4 && preset.parallel > 2 {
        return Err(format!(
            "4k export supports parallel 1 or 2 only (got x{})",
            preset.parallel
        ));
    }
    Ok(())
}

fn exit_code_u8(e: &RecordError) -> u8 {
    e.exit_code()
}
