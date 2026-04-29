use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::ShellState;
use super::timeline::AttachedCanvasNode;
use super::timeline_state::{ExportJob, TimelineTransition, iso_from_unix};
use crate::ipc::IpcResponse;

pub(crate) fn state_detail_response(
    req_id: String,
    state: &ShellState,
    params: Value,
) -> IpcResponse {
    match state_detail(state, params) {
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

pub(crate) fn state_detail(state: &ShellState, params: Value) -> Result<Value, String> {
    let request = StateDetailRequest::from_params(params)?;
    let node = state
        .timeline_node(request.canvas_node_id)?
        .ok_or_else(|| {
            detail_error(
                "CANVAS_NODE_NOT_FOUND",
                format!(
                    "canvas node {} has no attached Timeline composition",
                    request.canvas_node_id
                ),
                "next step · run capy timeline attach",
            )
        })?;
    Ok(json!({
        "ok": true,
        "trace_id": state_trace_id(),
        "stage": "state-detail",
        "attachment": attachment_detail_json(request.canvas_node_id, &node)
    }))
}

fn attachment_detail_json(canvas_node_id: u64, node: &AttachedCanvasNode) -> Value {
    let composition_path = Path::new(&node.composition_ref.path);
    let composition = read_json(composition_path);
    json!({
        "canvas_node_id": canvas_node_id,
        "kind": node.kind,
        "composition_path": node.composition_ref.path,
        "state": node.state,
        "schema_version": node.composition_ref.schema_version,
        "track_count": node.composition_ref.track_count,
        "asset_count": node.composition_ref.asset_count,
        "source": source_refs(composition.as_ref()),
        "composition": composition_detail(composition_path),
        "compile": compile_detail(composition_path, &node.history),
        "export_jobs": export_jobs_detail(&node.export_jobs),
        "evidence": evidence_detail(composition_path),
        "history": node.history
    })
}

fn read_json(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn source_refs(composition: Option<&Value>) -> Value {
    let Some(composition) = composition else {
        return json!({"poster_refs": [], "scroll_media_refs": [], "brand_tokens": null});
    };
    let poster_refs = source_assets(composition, &["poster", "image"]);
    let scroll_media_refs = source_assets(composition, &["scroll", "video", "scroll-media"]);
    let brand_tokens = composition
        .get("theme")
        .and_then(Value::as_object)
        .map(|theme| {
            json!({
                "tokens_ref": theme.get("tokens_ref").and_then(Value::as_str),
                "source_path": theme.get("source_path").and_then(Value::as_str),
                "hash": theme.get("hash").and_then(Value::as_str)
            })
        });
    json!({
        "poster_refs": poster_refs,
        "scroll_media_refs": scroll_media_refs,
        "brand_tokens": brand_tokens
    })
}

fn source_assets(composition: &Value, needles: &[&str]) -> Vec<Value> {
    composition
        .get("assets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|asset| asset_matches(asset, needles))
        .map(|asset| {
            json!({
                "id": asset.get("id").and_then(Value::as_str),
                "type": asset.get("type").and_then(Value::as_str),
                "kind": asset.get("kind").and_then(Value::as_str),
                "source_kind": asset.get("source_kind").and_then(Value::as_str),
                "source_path": asset.get("source_path").and_then(Value::as_str),
                "original_path": asset.get("original_path").and_then(Value::as_str),
                "materialized_path": asset.get("materialized_path").and_then(Value::as_str),
                "src": asset.get("src").and_then(Value::as_str),
                "byte_size": asset.get("byte_size").and_then(Value::as_u64)
            })
        })
        .collect()
}

fn asset_matches(asset: &Value, needles: &[&str]) -> bool {
    ["type", "kind", "source_kind", "src"]
        .iter()
        .filter_map(|key| asset.get(key).and_then(Value::as_str))
        .map(str::to_lowercase)
        .any(|value| needles.iter().any(|needle| value.contains(needle)))
}

fn composition_detail(path: &Path) -> Value {
    let preview_lines = fs::read_to_string(path)
        .map(|text| text.lines().take(5).map(str::to_string).collect::<Vec<_>>())
        .unwrap_or_default();
    json!({
        "path": path.display().to_string(),
        "preview_lines": preview_lines
    })
}

fn compile_detail(composition_path: &Path, history: &[TimelineTransition]) -> Value {
    let render_source_path = sibling_path(composition_path, "render_source.json");
    let timestamp = fs::metadata(&render_source_path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(system_time_iso);
    let compile_mode = read_json(&render_source_path)
        .and_then(|source| {
            source
                .get("compile_mode")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string());
    let status = if render_source_path.is_file() {
        "ready"
    } else if history.iter().any(|item| item.to.label() == "error") {
        "error"
    } else {
        "missing"
    };
    json!({
        "render_source_path": render_source_path.display().to_string(),
        "status": status,
        "compile_mode": compile_mode,
        "timestamp": timestamp
    })
}

fn export_jobs_detail(jobs: &[ExportJob]) -> Vec<Value> {
    jobs.iter()
        .map(|job| {
            let byte_size = job.byte_size.or_else(|| {
                job.output_path
                    .as_deref()
                    .and_then(|path| fs::metadata(path).ok())
                    .map(|metadata| metadata.len())
            });
            json!({
                "job_id": job.job_id,
                "status": job.status,
                "progress": job.progress,
                "output_path": job.output_path,
                "byte_size": byte_size,
                "started_at": job.started_at
            })
        })
        .collect()
}

fn evidence_detail(composition_path: &Path) -> Value {
    let path = sibling_path(composition_path, "evidence/index.html");
    json!({
        "index_html": path.display().to_string(),
        "exists": path.is_file()
    })
}

fn sibling_path(composition_path: &Path, relative: &str) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(relative)
}

fn system_time_iso(time: SystemTime) -> Option<String> {
    let seconds = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(iso_from_unix(seconds))
}

fn detail_error(code: &str, message: impl Into<String>, hint: &str) -> String {
    json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    })
    .to_string()
}

fn state_trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("state-detail-{millis}-{}", std::process::id())
}

#[derive(Debug)]
struct StateDetailRequest {
    canvas_node_id: u64,
}

impl StateDetailRequest {
    fn from_params(params: Value) -> Result<Self, String> {
        let canvas_node_id = params
            .get("canvas_node_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                detail_error(
                    "IPC_ERROR",
                    "missing required parameter: canvas_node_id",
                    "next step · run capy timeline state --canvas-node <id>",
                )
            })?;
        Ok(Self { canvas_node_id })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::{Value, json};

    use super::state_detail;
    use crate::app::ShellState;
    use crate::app::timeline_state::{ExportJob, ExportJobStatus};

    #[test]
    fn state_detail_reports_single_attachment_chain() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("state-detail")?;
        let path = write_composition(&dir, compilable_composition())?;
        let export_path = dir.join("exports").join("export.mp4");
        fs::create_dir_all(dir.join("exports"))?;
        fs::write(&export_path, b"mp4")?;
        fs::create_dir_all(dir.join("evidence"))?;
        fs::write(dir.join("evidence").join("index.html"), "<html></html>")?;
        let state = ShellState::default();
        crate::app::timeline::attach_node(
            &state,
            json!({"canvas_node_id": 0, "composition_path": path}),
        )?;
        let mut node = state
            .timeline_node(0)?
            .ok_or("attached node should be present")?;
        node.export_jobs.push(ExportJob {
            job_id: "exp-detail".to_string(),
            status: ExportJobStatus::Done,
            progress: 100,
            output_path: Some(export_path.display().to_string()),
            byte_size: None,
            started_at: "1970-01-01T00:00:00Z".to_string(),
        });
        state.attach_timeline_node(0, node)?;

        let value = state_detail(&state, json!({"canvas_node_id": 0}))?;
        let attachment = &value["attachment"];

        assert_eq!(value["stage"], "state-detail");
        assert_eq!(attachment["canvas_node_id"], 0);
        assert_eq!(
            attachment["composition"]["preview_lines"]
                .as_array()
                .map(Vec::len),
            Some(5)
        );
        assert_eq!(attachment["compile"]["status"], "ready");
        assert_eq!(attachment["export_jobs"][0]["byte_size"], 3);
        assert_eq!(attachment["evidence"]["exists"], true);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    fn write_composition(dir: &Path, value: Value) -> Result<PathBuf, Box<dyn std::error::Error>> {
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
            "schema": "capy.timeline.composition.v1",
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
            "capy-shell-timeline-detail-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        )))
    }
}
