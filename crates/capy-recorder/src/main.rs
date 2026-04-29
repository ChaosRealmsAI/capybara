//! `capy-recorder` binary entry point · CEF OSR record / snapshot / verify dispatch.
//!
//! Single-threaded tokio runtime keeps CEF OSR and macOS helpers on the process
//! main thread. The entrypoint only routes CLI subcommands; source export policy
//! lives in `source_export`.

use std::process::ExitCode;

use capy_recorder::cli::{self, Command};
use capy_recorder::events::{Event, emit};
use capy_recorder::orchestrator;
use capy_recorder::record_loop::RecordError;
use capy_recorder::verify_mp4;

mod source_export;

use source_export::{
    SourceExportArgs, dispatch_snapshot_source, dispatch_source_export, dispatch_validate_source,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    if let Err(err) = capy_recorder::cef_osr::maybe_run_subprocess() {
        emit(Event::Error {
            code: "CEF_SUBPROCESS_FAILED".into(),
            message: err.to_string(),
        });
        return ExitCode::from(3);
    }

    let parsed = cli::parse();

    match parsed.command {
        Some(Command::ValidateSource { source }) => dispatch_validate_source(source),
        Some(Command::Export {
            source,
            profile,
            output,
            diagnostics,
            events,
            resolution,
            fps,
            parallel,
        }) => {
            dispatch_source_export(SourceExportArgs {
                source,
                profile,
                output,
                diagnostics,
                events,
                resolution,
                fps,
                parallel,
            })
            .await
        }
        Some(Command::SnapshotSource {
            source,
            t_ms,
            output,
            resolution,
        }) => dispatch_snapshot_source(source, t_ms, output, resolution).await,
        Some(Command::Snapshot {
            bundle,
            t_ms,
            output,
            viewport,
        }) => dispatch_snapshot(bundle, t_ms, output, &viewport).await,
        Some(Command::Verify {
            file,
            expect_fps,
            expect_bitrate,
            json: _json,
        }) => dispatch_verify(file, expect_fps, expect_bitrate),
        None => dispatch_record(parsed).await,
    }
}

async fn dispatch_record(parsed: cli::Cli) -> ExitCode {
    let is_subprocess = parsed.frame_range.is_some();
    let cfg = match cli::to_config(&parsed) {
        Ok(c) => c,
        Err(msg) => {
            emit(Event::Error {
                code: "CLI_INVALID".to_string(),
                message: msg.clone(),
            });
            eprintln!("capy-recorder: {msg}");
            return ExitCode::from(1);
        }
    };
    let parallel =
        match orchestrator::resolve_requested_parallel(parsed.parallel, cfg.width, cfg.height) {
            Ok(n) => n,
            Err(e) => {
                let code = e.code_str().to_string();
                let message = e.to_string();
                emit(Event::Error { code, message });
                return ExitCode::from(exit_code_u8(&e));
            }
        };

    if parallel > 1 && !is_subprocess {
        return match orchestrator::run_parallel(cfg, parallel).await {
            Ok(()) => ExitCode::from(0),
            Err(e) => {
                let code = e.code_str().to_string();
                let message = e.to_string();
                emit(Event::Error { code, message });
                ExitCode::from(exit_code_u8(&e))
            }
        };
    }

    match capy_recorder::export_api::run_record_config(cfg).await {
        Ok(_stats) => ExitCode::from(0),
        Err(e) => {
            let code = e.code_str().to_string();
            let message = e.to_string();
            emit(Event::Error { code, message });
            ExitCode::from(exit_code_u8(&e))
        }
    }
}

fn exit_code_u8(e: &RecordError) -> u8 {
    e.exit_code()
}

async fn dispatch_snapshot(
    bundle: std::path::PathBuf,
    t_ms: u64,
    output: std::path::PathBuf,
    viewport: &str,
) -> ExitCode {
    let (w, h) = match cli::parse_viewport(viewport) {
        Ok(wh) => wh,
        Err(msg) => {
            emit(Event::Error {
                code: "CLI_INVALID".into(),
                message: msg.clone(),
            });
            eprintln!("capy-recorder: {msg}");
            return ExitCode::from(1);
        }
    };

    match capy_recorder::cef_osr::snapshot_png(&bundle, t_ms, &output, w, h).await {
        Ok(()) => {
            emit(Event::SnapshotDone {
                bundle: bundle.display().to_string(),
                t_ms,
                out: output.display().to_string(),
            });
            ExitCode::from(0)
        }
        Err(e) => {
            emit(Event::Error {
                code: "CEF_OSR_SNAPSHOT_FAILED".into(),
                message: e.to_string(),
            });
            ExitCode::from(3)
        }
    }
}

fn dispatch_verify(
    file: std::path::PathBuf,
    expect_fps: u32,
    expect_bitrate: Option<u32>,
) -> ExitCode {
    match verify_mp4::verify(&file, expect_fps, expect_bitrate) {
        Ok((verdict, asserts)) => {
            let all_pass = asserts.iter().all(|a| a.pass);
            let status = if all_pass { "PASS" } else { "FAIL" };
            let asserts_json = asserts
                .iter()
                .map(|a| serde_json::to_value(a).unwrap_or(serde_json::Value::Null))
                .collect();

            emit(Event::VerifyResult {
                file: verdict.file.clone(),
                status: status.into(),
                moov_front: verdict.moov_front,
                codec: verdict.codec.clone(),
                frame_rate: verdict.frame_rate,
                bit_rate: verdict.bit_rate,
                color_primaries: verdict.color_primaries.clone(),
                transfer: verdict.transfer.clone(),
                has_b_frames: verdict.has_b_frames,
                duration_ms: verdict.duration_ms,
                asserts: asserts_json,
            });

            if all_pass {
                ExitCode::from(0)
            } else {
                ExitCode::from(4)
            }
        }
        Err(e) => {
            emit(Event::Error {
                code: "VERIFY_FAILED".into(),
                message: format!("{e}"),
            });
            ExitCode::from(2)
        }
    }
}
