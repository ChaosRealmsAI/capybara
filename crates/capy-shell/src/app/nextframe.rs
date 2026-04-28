use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::ShellState;

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
    pub state: String,
    pub composition_ref: NextFrameCompositionRef,
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
    let document = capy_nextframe::compose::CompositionDocument::load(&path).map_err(|err| {
        attach_error(
            "INVALID_COMPOSITION",
            err,
            "next step · rerun capy nextframe validate",
        )
    })?;
    let validation =
        capy_nextframe::validate_composition(capy_nextframe::ValidateCompositionRequest {
            composition_path: path.clone(),
            strict_binary: false,
        });
    if !validation.ok {
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
    if !state.has_canvas_node(request.canvas_node_id) {
        return Err(attach_error(
            "CANVAS_NODE_NOT_FOUND",
            format!("canvas node {} was not found", request.canvas_node_id),
            "next step · run capy canvas snapshot",
        ));
    }

    let node_state = "preview-ready".to_string();
    let attached = AttachedCanvasNode {
        kind: "nextframe-composition".to_string(),
        state: node_state.clone(),
        composition_ref: NextFrameCompositionRef {
            path: path.display().to_string(),
            schema_version: document.schema_version,
            track_count: document.tracks.len(),
            asset_count: document.assets.len(),
        },
    };
    state.attach_nextframe_node(request.canvas_node_id, attached.clone())?;

    let report = AttachReport {
        ok: true,
        trace_id: trace_id(),
        stage: "attach".to_string(),
        canvas_node_id: request.canvas_node_id,
        composition_path: path.display().to_string(),
        node_state,
        ipc_socket: None,
        errors: Vec::new(),
    };
    Ok(json!({
        "report": report,
        "event": event_detail(request.canvas_node_id, &attached),
        "node": attached
    }))
}

pub(crate) fn event_detail(canvas_node_id: u64, node: &AttachedCanvasNode) -> Value {
    json!({
        "canvas_node_id": canvas_node_id,
        "kind": node.kind,
        "state": node.state,
        "composition_ref": node.composition_ref
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use serde_json::{Value, json};

    use super::attach_node;
    use crate::app::ShellState;

    #[test]
    fn attach_happy_path_marks_node_preview_ready() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("happy")?;
        let path = write_composition(&dir, valid_composition())?;
        let state = ShellState::default();

        let value = attach_node(
            &state,
            json!({"canvas_node_id": 0, "composition_path": path}),
        )?;

        assert_eq!(value["report"]["ok"], true);
        assert_eq!(value["report"]["node_state"], "preview-ready");
        assert_eq!(value["node"]["kind"], "nextframe-composition");
        assert_eq!(value["node"]["composition_ref"]["track_count"], 1);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn attach_reports_canvas_node_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("missing-node")?;
        let path = write_composition(&dir, valid_composition())?;
        let state = ShellState::default();

        let error = attach_node(
            &state,
            json!({"canvas_node_id": 42, "composition_path": path}),
        )
        .expect_err("missing node should fail");
        let value: Value = serde_json::from_str(&error)?;

        assert_eq!(value["code"], "CANVAS_NODE_NOT_FOUND");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn attach_reports_invalid_composition() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("invalid")?;
        let path = write_composition(&dir, json!({"tracks": []}))?;
        let state = ShellState::default();

        let error = attach_node(
            &state,
            json!({"canvas_node_id": 0, "composition_path": path}),
        )
        .expect_err("invalid composition should fail");
        let value: Value = serde_json::from_str(&error)?;

        assert_eq!(value["code"], "INVALID_COMPOSITION");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    fn write_composition(
        dir: &PathBuf,
        value: Value,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        fs::create_dir_all(dir)?;
        let path = dir.join("composition.json");
        fs::write(&path, serde_json::to_string_pretty(&value)?)?;
        Ok(path)
    }

    fn valid_composition() -> Value {
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
                "params": {"poster": {"type": "poster"}}
            }],
            "assets": []
        })
    }

    fn unique_dir(label: &str) -> Result<PathBuf, std::time::SystemTimeError> {
        Ok(std::env::temp_dir().join(format!(
            "capy-shell-nextframe-{label}-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
        )))
    }

    use std::time::{SystemTime, UNIX_EPOCH};
}
