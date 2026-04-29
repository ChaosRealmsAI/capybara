use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

pub(super) fn state_failure(code: &str, message: impl Into<String>, hint: &str) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": trace_id("state"),
        "stage": "state",
        "code": code,
        "errors": [error]
    })
}

pub(super) fn attach_failure(
    canvas_node_id: u64,
    composition_path: &std::path::Path,
    socket: &std::path::Path,
    code: &str,
    message: impl Into<String>,
    hint: &str,
) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": trace_id("attach"),
        "stage": "attach",
        "canvas_node_id": canvas_node_id,
        "composition_path": composition_path.display().to_string(),
        "node_state": "error",
        "ipc_socket": socket.display().to_string(),
        "code": code,
        "errors": [error]
    })
}

pub(super) fn open_failure(
    canvas_node_id: u64,
    socket: &std::path::Path,
    code: &str,
    message: impl Into<String>,
    hint: &str,
) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": trace_id("open"),
        "stage": "open",
        "canvas_node_id": canvas_node_id,
        "ipc_socket": socket.display().to_string(),
        "code": code,
        "errors": [error]
    })
}

pub(super) fn export_failure(code: &str, message: impl Into<String>, hint: &str) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": trace_id("export"),
        "stage": "export",
        "status": "failed",
        "code": code,
        "errors": [error]
    })
}

pub(super) fn job_failure(
    stage: &str,
    job_id: &str,
    socket: &std::path::Path,
    code: &str,
    message: impl Into<String>,
    hint: &str,
) -> Value {
    let error = json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    });
    json!({
        "ok": false,
        "trace_id": trace_id(stage),
        "stage": stage,
        "job_id": job_id,
        "ipc_socket": socket.display().to_string(),
        "code": code,
        "errors": [error]
    })
}

fn trace_id(stage: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{stage}-{millis}-{}", std::process::id())
}
