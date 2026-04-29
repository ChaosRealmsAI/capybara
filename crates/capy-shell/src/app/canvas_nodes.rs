use serde_json::{Value, json};

use super::ShellState;
use crate::ipc::IpcResponse;

pub(crate) fn register_response(req_id: String, state: &ShellState, params: Value) -> IpcResponse {
    let ids = params
        .get("ids")
        .or_else(|| params.get("canvas_node_ids"))
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_u64).collect::<Vec<_>>())
        .unwrap_or_default();
    match state.register_canvas_nodes(&ids) {
        Ok(total) => IpcResponse::ok(
            req_id,
            json!({
                "ok": true,
                "registered": ids,
                "known_canvas_nodes": total
            }),
        ),
        Err(message) => IpcResponse {
            req_id,
            ok: false,
            data: None,
            error: Some(json!({
                "code": "CANVAS_NODE_REGISTER_FAILED",
                "message": message
            })),
        },
    }
}
