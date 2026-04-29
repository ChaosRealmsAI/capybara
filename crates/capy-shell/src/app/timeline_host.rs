use serde_json::{Value, json};

use super::window::WindowManager;
use super::{ShellState, timeline};
use crate::ipc::{IpcRequest, IpcResponse};

pub(crate) fn attach(
    manager: &WindowManager,
    state: &ShellState,
    request: IpcRequest,
) -> IpcResponse {
    let req_id = request.req_id.clone();
    match timeline::attach_node(state, request.params) {
        Ok(data) => {
            if let Some(event) = data.get("event") {
                broadcast(manager, "capy:canvas-node-attached", event);
            }
            IpcResponse {
                req_id,
                ok: true,
                data: data.get("report").cloned().or(Some(data)),
                error: None,
            }
        }
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

pub(crate) fn open(
    manager: &WindowManager,
    state: &ShellState,
    request: IpcRequest,
) -> IpcResponse {
    let req_id = request.req_id.clone();
    match timeline::open_node(state, request.params) {
        Ok(data) => {
            broadcast(manager, "capy:timeline-opened", &data);
            IpcResponse {
                req_id,
                ok: true,
                data: Some(data),
                error: None,
            }
        }
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

fn broadcast(manager: &WindowManager, event_name: &str, event: &Value) {
    let Ok(payload) = serde_json::to_string(event) else {
        return;
    };
    let script =
        format!("window.dispatchEvent(new CustomEvent('{event_name}', {{ detail: {payload} }}));");
    for webview in manager.webviews.values() {
        let _eval_result = webview.evaluate_script(&script);
    }
}
