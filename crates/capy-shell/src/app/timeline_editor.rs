use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod clip_delivery;
mod model;

use serde_json::{Value, json};

use super::ShellState;
use super::timeline_state::{ExportJob, ExportJobStatus, iso_now};
use crate::ipc::IpcResponse;
use clip_delivery::{write_clip_proposal_composition, write_clip_queue_proposal_composition};
use model::{editor_summary, patch_track_field};

pub(crate) fn open_response(req_id: String, state: &ShellState, params: Value) -> IpcResponse {
    response(req_id, open_composition(state, params))
}

pub(crate) fn state_response(req_id: String, state: &ShellState, params: Value) -> IpcResponse {
    response(req_id, open_composition(state, params))
}

pub(crate) fn patch_response(req_id: String, state: &ShellState, params: Value) -> IpcResponse {
    response(req_id, patch_composition(state, params))
}

pub(crate) fn export_start_response(
    req_id: String,
    state: &ShellState,
    params: Value,
) -> IpcResponse {
    response(req_id, export_start(state, params))
}

fn response(req_id: String, result: Result<Value, String>) -> IpcResponse {
    match result {
        Ok(data) => IpcResponse {
            req_id,
            ok: true,
            data: Some(data),
            error: None,
        },
        Err(error) => IpcResponse {
            req_id,
            ok: false,
            data: None,
            error: serde_json::from_str(&error)
                .ok()
                .or_else(|| Some(json!({ "code": "IPC_ERROR", "message": error }))),
        },
    }
}

fn open_composition(state: &ShellState, params: Value) -> Result<Value, String> {
    let composition_path = composition_path(&params)?;
    let validation =
        capy_timeline::validate_composition(capy_timeline::ValidateCompositionRequest {
            composition_path: composition_path.clone(),
        });
    if !validation.ok {
        return Err(first_timeline_error(
            "INVALID_COMPOSITION",
            "composition validation failed",
            "next step · run capy timeline validate --composition <path>",
            validation.errors.first().map(|error| {
                (
                    error.code.as_str(),
                    error.message.as_str(),
                    error.hint.as_str(),
                )
            }),
        ));
    }

    let compile = capy_timeline::compile_composition(capy_timeline::CompileCompositionRequest {
        composition_path: composition_path.clone(),
    });
    if !compile.ok {
        return Err(first_timeline_error(
            "COMPILE_FAILED",
            "composition compile failed",
            "next step · run capy timeline compile --composition <path>",
            compile.errors.first().map(|error| {
                (
                    error.code.as_str(),
                    error.message.as_str(),
                    error.hint.as_str(),
                )
            }),
        ));
    }

    let composition = read_json(&composition_path)?;
    let render_source_path = sibling_path(&composition_path, "render_source.json");
    let render_source = read_json(&render_source_path)?;
    let preview_url = state.register_timeline_composition_preview(&composition_path)?;
    Ok(json!({
        "ok": true,
        "trace_id": trace_id("composition-open"),
        "stage": "composition-open",
        "composition_path": composition_path.display().to_string(),
        "render_source_path": render_source_path.display().to_string(),
        "preview_url": preview_url,
        "schema_version": validation.schema_version,
        "track_count": validation.track_count,
        "asset_count": validation.asset_count,
        "editor": editor_summary(&composition, Some(&render_source)),
        "render_source": render_source,
    }))
}

fn patch_composition(state: &ShellState, params: Value) -> Result<Value, String> {
    let composition_path = composition_path(&params)?;
    let track_id = required_str(&params, "track_id")?;
    let field = required_str(&params, "field")?;
    let value = params.get("value").cloned().ok_or_else(|| {
        error_json(
            "IPC_ERROR",
            "missing required parameter: value",
            "next step · pass value",
        )
    })?;
    let mut composition = read_json(&composition_path)?;
    let changed = patch_track_field(&mut composition, &track_id, &field, value);
    if !changed {
        return Err(error_json(
            "TRACK_NOT_FOUND",
            format!("track not found: {track_id}"),
            "next step · inspect timeline-composition-state output",
        ));
    }
    write_json(&composition_path, &composition)?;
    open_composition(
        state,
        json!({ "composition_path": composition_path.display().to_string() }),
    )
}

fn export_start(state: &ShellState, params: Value) -> Result<Value, String> {
    let composition_path = composition_path(&params)?;
    let fps = params
        .get("fps")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(30);
    let profile = params
        .get("profile")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("draft")
        .to_string();
    let resolution = params
        .get("resolution")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let parallel = params
        .get("parallel")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0);
    let strict_recorder = params
        .get("strict_recorder")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let job_id = format!("exp-{}-{}", timestamp_millis(), std::process::id());
    let output_path = params
        .get("out")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            composition_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("exports")
                .join(format!("{job_id}.mp4"))
        });
    let export_composition_path = if params.get("queue").is_some() {
        write_clip_queue_proposal_composition(&composition_path, &params, &job_id)?
    } else if params.get("range").is_some() {
        write_clip_proposal_composition(&composition_path, &params, &job_id)?
    } else {
        composition_path.clone()
    };
    let mut job = ExportJob {
        job_id: job_id.clone(),
        status: ExportJobStatus::Running,
        progress: 5,
        output_path: Some(output_path.display().to_string()),
        byte_size: None,
        started_at: iso_now(),
    };
    state.upsert_timeline_editor_job(job.clone())?;

    let compile = capy_timeline::compile_composition(capy_timeline::CompileCompositionRequest {
        composition_path: export_composition_path.clone(),
    });
    if !compile.ok {
        job.status = ExportJobStatus::Failed;
        job.progress = 100;
        state.upsert_timeline_editor_job(job.clone())?;
        return Err(first_timeline_error(
            "COMPILE_FAILED",
            "composition compile failed before export",
            "next step · run capy timeline compile --composition <path>",
            compile.errors.first().map(|error| {
                (
                    error.code.as_str(),
                    error.message.as_str(),
                    error.hint.as_str(),
                )
            }),
        ));
    }

    let export = capy_timeline::export_composition(capy_timeline::ExportCompositionRequest {
        composition_path: export_composition_path.clone(),
        kind: capy_timeline::ExportKind::Mp4,
        out: Some(output_path.clone()),
        fps,
        profile,
        resolution,
        parallel,
        strict_recorder,
    });
    job.status = if export.ok {
        ExportJobStatus::Done
    } else {
        ExportJobStatus::Failed
    };
    job.progress = 100;
    job.byte_size = export
        .output_path
        .as_path()
        .metadata()
        .ok()
        .map(|metadata| metadata.len());
    state.upsert_timeline_editor_job(job.clone())?;
    if !export.ok {
        return Err(first_timeline_error(
            "EXPORT_FAILED",
            "timeline export failed",
            "next step · inspect capy timeline export output",
            export.errors.first().map(|error| {
                (
                    error.code.as_str(),
                    error.message.as_str(),
                    error.hint.as_str(),
                )
            }),
        ));
    }
    Ok(json!({
        "ok": true,
        "trace_id": trace_id("export-start"),
        "stage": "export-start",
        "composition_path": composition_path.display().to_string(),
        "export_composition_path": export_composition_path.display().to_string(),
        "range": params.get("range").cloned(),
        "proposal": params.get("proposal").cloned(),
        "job": job,
        "export": export
    }))
}

fn composition_path(params: &Value) -> Result<PathBuf, String> {
    let raw = params
        .get("composition_path")
        .or_else(|| params.get("composition"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            error_json(
                "IPC_ERROR",
                "missing required parameter: composition_path",
                "next step · pass --composition <path>",
            )
        })?;
    let path = PathBuf::from(raw);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|err| {
                error_json(
                    "IPC_ERROR",
                    format!("read cwd failed: {err}"),
                    "next step · retry from a valid workspace",
                )
            })?
            .join(path)
    };
    if !absolute.is_file() {
        return Err(error_json(
            "COMPOSITION_NOT_FOUND",
            format!("composition not found: {}", absolute.display()),
            "next step · pass an existing composition.json path",
        ));
    }
    Ok(absolute)
}

fn required_str(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            error_json(
                "IPC_ERROR",
                format!("missing required parameter: {key}"),
                "next step · inspect timeline composition help",
            )
        })
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|err| {
        error_json(
            "COMPOSITION_READ_FAILED",
            format!("read JSON failed: {err}"),
            "next step · check file permissions",
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        error_json(
            "COMPOSITION_INVALID",
            format!("JSON parse failed: {err}"),
            "next step · fix composition JSON",
        )
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|err| {
        error_json(
            "COMPOSITION_INVALID",
            format!("serialize JSON failed: {err}"),
            "next step · inspect composition state",
        )
    })?;
    text.push('\n');
    fs::write(path, text).map_err(|err| {
        error_json(
            "COMPOSITION_WRITE_FAILED",
            format!("write JSON failed: {err}"),
            "next step · check file permissions",
        )
    })
}

fn sibling_path(path: &Path, relative: &str) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(relative)
}

fn first_timeline_error(
    default_code: &str,
    default_message: &str,
    default_hint: &str,
    error: Option<(&str, &str, &str)>,
) -> String {
    let (code, message, hint) = error.unwrap_or((default_code, default_message, default_hint));
    error_json(code, message, hint)
}

fn error_json(code: &str, message: impl Into<String>, hint: &str) -> String {
    json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    })
    .to_string()
}

fn trace_id(stage: &str) -> String {
    format!("{stage}-{}-{}", timestamp_millis(), std::process::id())
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
