use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::ShellState;
pub use super::nextframe_state::{NextFrameNodeAction, NextFrameNodeState};
use super::nextframe_state::{NextFrameTransition, iso_now};
use crate::ipc::IpcResponse;

const KIND_NEXTFRAME_COMPOSITION: &str = "nextframe-composition";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: String,
    pub canvas_node_id: u64,
    pub composition_path: String,
    pub node_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipc_socket: Option<String>,
    pub errors: Vec<AttachError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachError {
    pub code: String,
    pub message: String,
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AttachedCanvasNode {
    pub kind: String,
    pub state: NextFrameNodeState,
    pub composition_ref: NextFrameCompositionRef,
    pub history: Vec<NextFrameTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NextFrameCompositionRef {
    pub path: String,
    pub schema_version: String,
    pub track_count: usize,
    pub asset_count: usize,
}

pub fn attach_node(state: &ShellState, params: Value) -> Result<Value, String> {
    let request = AttachRequest::from_params(params)?;
    let path = absolute_path(&request.composition_path)?;
    if !state.has_canvas_node(request.canvas_node_id) {
        return Err(attach_error(
            "CANVAS_NODE_NOT_FOUND",
            format!("canvas node {} was not found", request.canvas_node_id),
            "next step · run capy canvas snapshot",
        ));
    }

    let validation =
        capy_nextframe::validate_composition(capy_nextframe::ValidateCompositionRequest {
            composition_path: path.clone(),
            strict_binary: false,
        });
    let mut attached = AttachedCanvasNode {
        kind: KIND_NEXTFRAME_COMPOSITION.to_string(),
        state: NextFrameNodeState::Draft,
        composition_ref: NextFrameCompositionRef {
            path: path.display().to_string(),
            schema_version: validation.schema_version.clone(),
            track_count: validation.track_count,
            asset_count: validation.asset_count,
        },
        history: Vec::new(),
    };
    if !validation.ok {
        let error = validation.errors.first();
        land_error(
            &mut attached,
            error
                .map(|error| error.code.as_str())
                .unwrap_or("INVALID_COMPOSITION"),
            error
                .map(|error| error.message.as_str())
                .unwrap_or("composition validation failed"),
            error.map(|error| error.hint.clone()).or_else(|| {
                Some("next step · run capy nextframe validate --composition <path>".to_string())
            }),
            "structural validate failed",
        )?;
        state.attach_nextframe_node(request.canvas_node_id, attached)?;
        return Err(attach_error(
            "INVALID_COMPOSITION",
            validation
                .errors
                .first()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "composition validation failed".to_string()),
            "next step · run capy nextframe validate --composition <path>",
        ));
    }
    transition_node(
        &mut attached,
        NextFrameNodeAction::ValidateOk,
        "structural validate ok",
    )?;

    let compile = capy_nextframe::compile_composition(capy_nextframe::CompileCompositionRequest {
        composition_path: path.clone(),
        strict_binary: false,
    });
    if !compile.ok {
        let error = compile.errors.first();
        land_error(
            &mut attached,
            error
                .map(|error| error.code.as_str())
                .unwrap_or("COMPILE_FAILED"),
            error
                .map(|error| error.message.as_str())
                .unwrap_or("composition compile failed"),
            error.map(|error| error.hint.clone()).or_else(|| {
                Some("next step · run capy nextframe compile --composition <path>".to_string())
            }),
            "compile failed",
        )?;
        state.attach_nextframe_node(request.canvas_node_id, attached)?;
        return Err(attach_error(
            "COMPILE_FAILED",
            compile
                .errors
                .first()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "composition compile failed".to_string()),
            "next step · run capy nextframe compile --composition <path>",
        ));
    }
    transition_node(
        &mut attached,
        NextFrameNodeAction::CompileOk,
        "render_source.v1 generated",
    )?;
    transition_node(
        &mut attached,
        NextFrameNodeAction::PreviewReady,
        "v0.13.5 attach marks preview ready",
    )?;
    state.attach_nextframe_node(request.canvas_node_id, attached.clone())?;

    let report = AttachReport {
        ok: true,
        trace_id: trace_id(),
        stage: "attach".to_string(),
        canvas_node_id: request.canvas_node_id,
        composition_path: path.display().to_string(),
        node_state: attached.state.label().to_string(),
        ipc_socket: None,
        errors: Vec::new(),
    };
    Ok(json!({
        "report": report,
        "event": event_detail(request.canvas_node_id, &attached),
        "node": attached
    }))
}

pub fn state_nodes(state: &ShellState, params: Value) -> Result<Value, String> {
    let request = StateRequest::from_params(params)?;
    let attachments = match request.canvas_node_id {
        Some(id) => {
            let node = state
                .nextframe_nodes()?
                .into_iter()
                .find(|(node_id, _)| *node_id == id)
                .map(|(_, node)| node)
                .ok_or_else(|| {
                    attach_error(
                        "CANVAS_NODE_NOT_FOUND",
                        format!("canvas node {id} has no attached NextFrame composition"),
                        "next step · run capy nextframe attach",
                    )
                })?;
            vec![attachment_json(id, &node)]
        }
        None => state
            .nextframe_nodes()?
            .into_iter()
            .map(|(id, node)| attachment_json(id, &node))
            .collect(),
    };
    Ok(json!({
        "ok": true,
        "trace_id": state_trace_id(),
        "stage": "state",
        "attachments": attachments
    }))
}

pub fn open_node(state: &ShellState, params: Value) -> Result<Value, String> {
    let request = OpenRequest::from_params(params)?;
    if !state.has_canvas_node(request.canvas_node_id) {
        return Err(attach_error(
            "CANVAS_NODE_NOT_FOUND",
            format!("canvas node {} was not found", request.canvas_node_id),
            "next step · run capy canvas snapshot",
        ));
    }
    let node = state
        .nextframe_node(request.canvas_node_id)?
        .ok_or_else(|| {
            attach_error(
                "CANVAS_NODE_NOT_FOUND",
                format!(
                    "canvas node {} has no attached NextFrame composition",
                    request.canvas_node_id
                ),
                "next step · run capy nextframe attach",
            )
        })?;
    if node.state != NextFrameNodeState::PreviewReady {
        return Err(attach_error(
            "NOT_PREVIEW_READY",
            format!(
                "canvas node {} NextFrame state is {}",
                request.canvas_node_id,
                node.state.label()
            ),
            "next step · run capy nextframe attach",
        ));
    }
    let preview_url = state.register_nextframe_preview(
        request.canvas_node_id,
        Path::new(&node.composition_ref.path),
    )?;
    Ok(open_report(request.canvas_node_id, preview_url))
}

pub(crate) fn open_response(req_id: String, state: &ShellState, params: Value) -> IpcResponse {
    match open_node(state, params) {
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

pub(crate) fn state_response(req_id: String, state: &ShellState, params: Value) -> IpcResponse {
    match state_nodes(state, params) {
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

fn open_report(canvas_node_id: u64, preview_url: String) -> Value {
    json!({
        "ok": true,
        "trace_id": open_trace_id(),
        "stage": "open",
        "canvas_node_id": canvas_node_id,
        "preview_url": preview_url,
        "selectors": {
            "preview": format!("[data-capy-component-kind='nextframe-composition'][data-canvas-node-id='{canvas_node_id}'] [data-capy-nextframe-preview]"),
            "state": format!("[data-capy-component-kind='nextframe-composition'][data-canvas-node-id='{canvas_node_id}'][data-capy-nextframe-state]")
        }
    })
}

pub(crate) fn event_detail(canvas_node_id: u64, node: &AttachedCanvasNode) -> Value {
    json!({
        "canvas_node_id": canvas_node_id,
        "kind": node.kind,
        "state": node.state,
        "composition_ref": node.composition_ref
    })
}

fn transition_node(
    node: &mut AttachedCanvasNode,
    action: NextFrameNodeAction,
    reason: &str,
) -> Result<(), String> {
    let from = node.state.clone();
    let to = from.transition(action).map_err(|err| {
        attach_error(
            "ILLEGAL_TRANSITION",
            format!("illegal transition from {} via {}", err.from, err.action),
            "next step · inspect nextframe state history",
        )
    })?;
    node.history.push(NextFrameTransition {
        from,
        to: to.clone(),
        at: iso_now(),
        reason: reason.to_string(),
    });
    node.state = to;
    Ok(())
}

fn land_error(
    node: &mut AttachedCanvasNode,
    code: &str,
    message: &str,
    hint: Option<String>,
    reason: &str,
) -> Result<(), String> {
    transition_node(
        node,
        NextFrameNodeAction::Error {
            code: code.to_string(),
            message: message.to_string(),
            hint,
        },
        reason,
    )
}

fn attachment_json(canvas_node_id: u64, node: &AttachedCanvasNode) -> Value {
    json!({
        "canvas_node_id": canvas_node_id,
        "composition_path": node.composition_ref.path,
        "state": node.state,
        "schema_version": node.composition_ref.schema_version,
        "track_count": node.composition_ref.track_count,
        "asset_count": node.composition_ref.asset_count,
        "history": node.history
    })
}

#[derive(Debug)]
struct AttachRequest {
    canvas_node_id: u64,
    composition_path: PathBuf,
}

impl AttachRequest {
    fn from_params(params: Value) -> Result<Self, String> {
        let canvas_node_id = params
            .get("canvas_node_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                attach_error(
                    "IPC_ERROR",
                    "missing required parameter: canvas_node_id",
                    "next step · run capy nextframe attach --help",
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
                    "next step · run capy nextframe attach --help",
                )
            })?;
        Ok(Self {
            canvas_node_id,
            composition_path,
        })
    }
}

#[derive(Debug)]
struct StateRequest {
    canvas_node_id: Option<u64>,
}

impl StateRequest {
    fn from_params(params: Value) -> Result<Self, String> {
        let canvas_node_id = match params.get("canvas_node_id") {
            Some(Value::Null) | None => None,
            Some(value) => Some(value.as_u64().ok_or_else(|| {
                attach_error(
                    "IPC_ERROR",
                    "canvas_node_id must be an unsigned integer",
                    "next step · run capy nextframe state --help",
                )
            })?),
        };
        Ok(Self { canvas_node_id })
    }
}

#[derive(Debug)]
struct OpenRequest {
    canvas_node_id: u64,
}

impl OpenRequest {
    fn from_params(params: Value) -> Result<Self, String> {
        let canvas_node_id = params
            .get("canvas_node_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                attach_error(
                    "IPC_ERROR",
                    "missing required parameter: canvas_node_id",
                    "next step · run capy nextframe open --help",
                )
            })?;
        Ok(Self { canvas_node_id })
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
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
            "next step · run capy nextframe compose-poster",
        ));
    }
    Ok(absolute)
}

fn attach_error(code: &str, message: impl Into<String>, hint: &str) -> String {
    json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    })
    .to_string()
}

fn trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("attach-{millis}-{}", std::process::id())
}

fn state_trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("state-{millis}-{}", std::process::id())
}

fn open_trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("open-{millis}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::{Value, json};

    use super::{open_node, state_nodes};
    use crate::app::ShellState;

    #[test]
    fn open_happy_path_returns_preview_url_and_selectors() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = unique_dir("open-happy")?;
        let path = write_composition(&dir, compilable_composition())?;
        let state = ShellState::default();
        super::attach_node(
            &state,
            json!({"canvas_node_id": 0, "composition_path": path}),
        )?;

        let value = open_node(&state, json!({"canvas_node_id": 0}))?;

        assert_eq!(value["ok"], true);
        assert_eq!(value["stage"], "open");
        assert_eq!(value["canvas_node_id"], 0);
        assert!(
            value["preview_url"]
                .as_str()
                .unwrap_or("")
                .starts_with("http://127.0.0.1:")
        );
        assert_eq!(
            value["selectors"]["preview"],
            "[data-capy-component-kind='nextframe-composition'][data-canvas-node-id='0'] [data-capy-nextframe-preview]"
        );
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn open_rejects_not_preview_ready_node() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("open-not-ready")?;
        let path = write_composition(&dir, json!({"tracks": []}))?;
        let state = ShellState::default();
        let _error = super::attach_node(
            &state,
            json!({"canvas_node_id": 0, "composition_path": path}),
        )
        .expect_err("invalid composition should create error-state attachment");

        let error = open_node(&state, json!({"canvas_node_id": 0}))
            .expect_err("error-state attachment is not preview-ready");
        let value: Value = serde_json::from_str(&error)?;

        assert_eq!(value["code"], "NOT_PREVIEW_READY");
        assert_eq!(
            state_nodes(&state, json!({"canvas_node_id": 0}))?["attachments"][0]["state"]["error"]
                ["code"],
            "COMPOSITION_INVALID"
        );
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn open_reports_canvas_node_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let state = ShellState::default();

        let error = open_node(&state, json!({"canvas_node_id": 99}))
            .expect_err("unknown canvas node should fail");
        let value: Value = serde_json::from_str(&error)?;

        assert_eq!(value["code"], "CANVAS_NODE_NOT_FOUND");
        Ok(())
    }

    fn write_composition(
        dir: &PathBuf,
        value: Value,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        fs::create_dir_all(dir.join("components"))?;
        let path = dir.join("composition.json");
        fs::write(&path, serde_json::to_string_pretty(&value)?)?;
        fs::write(
            dir.join("components").join("html.capy-poster.js"),
            "export function mount(root) { root.textContent = 'ok'; }\nexport function update() {}\n",
        )?;
        Ok(path)
    }

    fn compilable_composition() -> Value {
        json!({
            "schema": "nextframe.composition.v2",
            "schema_version": "capy.composition.v1",
            "id": "poster-snapshot",
            "title": "Poster Snapshot",
            "name": "Poster Snapshot",
            "duration_ms": 1000,
            "duration": "1000ms",
            "viewport": {"w": 1920, "h": 1080, "ratio": "16:9"},
            "theme": "default",
            "tracks": [{
                "id": "track-poster",
                "kind": "component",
                "component": "html.capy-poster",
                "z": 10,
                "time": {"start": "0ms", "end": "1000ms"},
                "duration_ms": 1000,
                "params": {"poster": {
                    "version": "capy-poster-v0.1",
                    "type": "poster",
                    "canvas": {"width": 1920, "height": 1080, "aspectRatio": "16:9", "background": "#fff"},
                    "assets": {},
                    "layers": [{"id": "title", "type": "text", "x": 10, "y": 10, "width": 400, "height": 100, "z": 1, "text": "Hello", "style": {"fontSize": 48, "color": "#111"}}]
                }}
            }],
            "assets": []
        })
    }

    fn unique_dir(label: &str) -> Result<PathBuf, std::time::SystemTimeError> {
        Ok(std::env::temp_dir().join(format!(
            "capy-shell-nextframe-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        )))
    }
}
