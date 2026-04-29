use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

#[derive(Debug)]
pub(super) struct AttachRequest {
    pub(super) canvas_node_id: u64,
    pub(super) composition_path: PathBuf,
}

impl AttachRequest {
    pub(super) fn from_params(params: Value) -> Result<Self, String> {
        let canvas_node_id = params
            .get("canvas_node_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                attach_error(
                    "IPC_ERROR",
                    "missing required parameter: canvas_node_id",
                    "next step · run capy timeline attach --help",
                )
            })?;
        let composition_path = params
            .get("composition_path")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                attach_error(
                    "IPC_ERROR",
                    "missing required parameter: composition_path",
                    "next step · run capy timeline attach --help",
                )
            })?;
        Ok(Self {
            canvas_node_id,
            composition_path,
        })
    }
}

#[derive(Debug)]
pub(super) struct StateRequest {
    pub(super) canvas_node_id: Option<u64>,
}

impl StateRequest {
    pub(super) fn from_params(params: Value) -> Result<Self, String> {
        let canvas_node_id = match params.get("canvas_node_id") {
            Some(Value::Null) | None => None,
            Some(value) => Some(value.as_u64().ok_or_else(|| {
                attach_error(
                    "IPC_ERROR",
                    "canvas_node_id must be an unsigned integer",
                    "next step · run capy timeline state --help",
                )
            })?),
        };
        Ok(Self { canvas_node_id })
    }
}

#[derive(Debug)]
pub(super) struct OpenRequest {
    pub(super) canvas_node_id: u64,
}

impl OpenRequest {
    pub(super) fn from_params(params: Value) -> Result<Self, String> {
        let canvas_node_id = params
            .get("canvas_node_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                attach_error(
                    "IPC_ERROR",
                    "missing required parameter: canvas_node_id",
                    "next step · run capy timeline open --help",
                )
            })?;
        Ok(Self { canvas_node_id })
    }
}

#[derive(Debug)]
pub(super) struct ExportJobRequest {
    pub(super) job_id: String,
}

impl ExportJobRequest {
    pub(super) fn from_params(params: Value, command: &str) -> Result<Self, String> {
        let job_id = params
            .get("job_id")
            .or_else(|| params.get("job"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
            .ok_or_else(|| {
                attach_error(
                    "IPC_ERROR",
                    "missing required parameter: job_id",
                    format!("next step · run capy timeline {command} --job <job_id>").as_str(),
                )
            })?;
        Ok(Self { job_id })
    }
}

pub(super) fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| {
                attach_error(
                    "IPC_ERROR",
                    format!("read cwd failed: {err}"),
                    "next step · retry from a valid workspace",
                )
            })?
            .join(path)
    };
    if !absolute.exists() {
        return Err(attach_error(
            "COMPOSITION_NOT_FOUND",
            format!("composition not found: {}", absolute.display()),
            "next step · run capy timeline compose-poster",
        ));
    }
    Ok(absolute)
}

pub(super) fn attach_error(code: &str, message: impl Into<String>, hint: &str) -> String {
    json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    })
    .to_string()
}

pub(super) fn trace_id() -> String {
    trace_with_prefix("attach")
}

pub(super) fn state_trace_id() -> String {
    trace_with_prefix("state")
}

pub(super) fn open_trace_id() -> String {
    trace_with_prefix("open")
}

pub(super) fn export_status_trace_id() -> String {
    trace_with_prefix("export-status")
}

pub(super) fn export_cancel_trace_id() -> String {
    trace_with_prefix("export-cancel")
}

fn trace_with_prefix(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{prefix}-{millis}-{}", std::process::id())
}
