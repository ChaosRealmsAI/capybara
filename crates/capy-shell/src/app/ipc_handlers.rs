use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tokio::sync::oneshot;

use capy_contracts::canvas::OP_CANVAS_NODES_REGISTER;
use capy_contracts::timeline::{
    OP_TIMELINE_ATTACH, OP_TIMELINE_COMPOSITION_OPEN, OP_TIMELINE_COMPOSITION_PATCH,
    OP_TIMELINE_COMPOSITION_STATE, OP_TIMELINE_EXPORT_CANCEL, OP_TIMELINE_EXPORT_START,
    OP_TIMELINE_EXPORT_STATUS, OP_TIMELINE_OPEN, OP_TIMELINE_STATE, OP_TIMELINE_STATE_DETAIL,
};

use crate::agent::AgentRuntimeEvent;
use crate::ipc::{IpcRequest, IpcResponse, error_response, ok_response};
use crate::store::Store;

use super::window::WindowManager;
use super::{
    ShellEvent, ShellState, canvas_nodes, canvas_tool, conversation, probes, timeline_detail,
    timeline_host,
};

type SharedAck = Arc<Mutex<Option<oneshot::Sender<IpcResponse>>>>;

pub(super) fn open_window(
    manager: &mut WindowManager,
    target: &EventLoopWindowTarget<ShellEvent>,
    proxy: &EventLoopProxy<ShellEvent>,
    request: IpcRequest,
) -> IpcResponse {
    let result = (|| {
        let project = required_string(&request.params, "project").unwrap_or_else(|_| "demo".into());
        let new_window = request
            .params
            .get("new_window")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let window_id = if new_window {
            manager.open_new(target, proxy, &project)?
        } else {
            manager.open(target, proxy, &project)?
        };
        Ok(json!({
            "window_id": window_id,
            "project": project,
            "pid": std::process::id()
        }))
    })();

    response_from_result(request.req_id, result)
}

pub(super) fn devtools_query(
    manager: &WindowManager,
    request: IpcRequest,
    ack: oneshot::Sender<IpcResponse>,
) {
    let req_id = request.req_id.clone();
    let shared_ack = shared_ack(ack);
    let result = (|| {
        let query = required_string(&request.params, "query")?;
        let get = request
            .params
            .get("get")
            .and_then(Value::as_str)
            .unwrap_or("outerHTML");
        let window = optional_string(&request.params, "window");
        let (_, webview) = manager.webview_for_target(window.as_deref())?;
        let script = probes::devtools_script(&query, get);
        let callback_ack = Arc::clone(&shared_ack);
        let callback_req_id = req_id.clone();
        webview
            .evaluate_script_with_callback(&script, move |raw| {
                send_shared_response(&callback_ack, js_callback_response(&callback_req_id, &raw));
            })
            .map_err(|err| format!("devtools evaluate failed: {err}"))
    })();

    if let Err(error) = result {
        send_shared_response(&shared_ack, error_response(&req_id, error));
    }
}

pub(super) fn devtools_eval(
    manager: &WindowManager,
    request: IpcRequest,
    ack: oneshot::Sender<IpcResponse>,
) {
    let req_id = request.req_id.clone();
    let shared_ack = shared_ack(ack);
    let result = (|| {
        let script = required_string(&request.params, "eval")?;
        let window = optional_string(&request.params, "window");
        let (_, webview) = manager.webview_for_target(window.as_deref())?;
        let callback_ack = Arc::clone(&shared_ack);
        let callback_req_id = req_id.clone();
        webview
            .evaluate_script_with_callback(&script, move |raw| {
                send_shared_response(&callback_ack, js_callback_response(&callback_req_id, &raw));
            })
            .map_err(|err| format!("devtools eval failed: {err}"))
    })();

    if let Err(error) = result {
        send_shared_response(&shared_ack, error_response(&req_id, error));
    }
}

pub(super) fn state_query(
    manager: &WindowManager,
    request: IpcRequest,
    ack: oneshot::Sender<IpcResponse>,
) {
    let req_id = request.req_id.clone();
    let shared_ack = shared_ack(ack);
    let result = (|| {
        let key = required_string(&request.params, "key")?;
        let window = optional_string(&request.params, "window");
        let (_, webview) = manager.webview_for_target(window.as_deref())?;
        let script = probes::state_script(&key);
        let callback_ack = Arc::clone(&shared_ack);
        let callback_req_id = req_id.clone();
        webview
            .evaluate_script_with_callback(&script, move |raw| {
                send_shared_response(&callback_ack, js_callback_response(&callback_req_id, &raw));
            })
            .map_err(|err| format!("state evaluate failed: {err}"))
    })();

    if let Err(error) = result {
        send_shared_response(&shared_ack, error_response(&req_id, error));
    }
}

pub(super) fn screenshot(
    manager: &mut WindowManager,
    request: IpcRequest,
    ack: oneshot::Sender<IpcResponse>,
) {
    let req_id = request.req_id.clone();
    let shared_ack = shared_ack(ack);
    let result = (|| {
        let region = request
            .params
            .get("region")
            .and_then(Value::as_str)
            .unwrap_or("full")
            .to_string();
        let out = required_string(&request.params, "out")?;
        let window = optional_string(&request.params, "window");
        let (window_id, _) = manager.webview_for_target(window.as_deref())?;
        manager.focus(&window_id)?;
        std::thread::sleep(std::time::Duration::from_millis(120));
        let (_, webview) = manager.webview_for_target(Some(&window_id))?;
        let script = probes::screenshot_probe_script(&region);
        let callback_req_id = req_id.clone();
        let window_id = window_id.to_string();
        let callback_ack = Arc::clone(&shared_ack);
        webview
            .evaluate_script_with_callback(&script, move |raw| {
                let response =
                    probes::screenshot_response(&callback_req_id, &window_id, &region, &out, &raw);
                send_shared_response(&callback_ack, response);
            })
            .map_err(|err| format!("screenshot evaluate failed: {err}"))
    })();

    if let Err(error) = result {
        send_shared_response(&shared_ack, error_response(&req_id, error));
    }
}

pub(super) fn capture_window(
    manager: &mut WindowManager,
    request: IpcRequest,
    ack: oneshot::Sender<IpcResponse>,
) {
    let req_id = request.req_id.clone();
    let shared_ack = shared_ack(ack);
    let result = (|| {
        let out = required_string(&request.params, "out")?;
        let window = optional_string(&request.params, "window");
        let (window_id, _) = manager.webview_for_target(window.as_deref())?;
        manager.focus(&window_id)?;
        std::thread::sleep(std::time::Duration::from_millis(120));
        let (_, webview) = manager.webview_for_target(Some(&window_id))?;
        let script = probes::screenshot_probe_script("full");
        let callback_req_id = req_id.clone();
        let callback_ack = Arc::clone(&shared_ack);
        webview
            .evaluate_script_with_callback(&script, move |raw| {
                let response =
                    probes::screenshot_response(&callback_req_id, &window_id, "full", &out, &raw);
                send_shared_response(&callback_ack, response);
            })
            .map_err(|err| format!("capture evaluate failed: {err}"))
    })();

    if let Err(error) = result {
        send_shared_response(&shared_ack, error_response(&req_id, error));
    }
}

pub(super) fn handle_js_ipc(
    manager: &WindowManager,
    state: Arc<ShellState>,
    store: Arc<Store>,
    proxy: &EventLoopProxy<ShellEvent>,
    window_id: &str,
    body: &str,
) {
    let trimmed = body.trim();
    if let Ok(window) = manager.window_by_id(window_id) {
        if trimmed == "drag_window" {
            let _drag_result = window.drag_window();
            return;
        } else if trimmed == "maximize_toggle" {
            window.set_maximized(!window.is_maximized());
            return;
        }
    }
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return;
    };
    if value.get("type").and_then(Value::as_str) == Some("console") {
        eprintln!("CAPYCONSOLE {trimmed}");
        return;
    }
    if value.get("kind").and_then(Value::as_str) != Some("rpc") {
        return;
    }
    let req_id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("frontend-rpc")
        .to_string();
    let Some(op) = value.get("op").and_then(Value::as_str) else {
        send_frontend_rpc(
            manager,
            window_id,
            IpcResponse {
                req_id,
                ok: false,
                data: None,
                error: Some(json!({ "error": "missing op" })),
            },
        );
        return;
    };
    let request = IpcRequest {
        req_id,
        op: op.to_string(),
        params: value.get("params").cloned().unwrap_or_else(|| json!({})),
    };
    let response = if op == "canvas-generate-image" {
        response_from_result(
            request.req_id.clone(),
            canvas_tool::start_image_generation(
                proxy.clone(),
                window_id.to_string(),
                request.params,
            ),
        )
    } else if op == OP_CANVAS_NODES_REGISTER {
        canvas_nodes::register_response(request.req_id.clone(), &state, request.params)
    } else if op == OP_TIMELINE_ATTACH {
        timeline_host::attach(manager, &state, request)
    } else if op == OP_TIMELINE_STATE {
        state.timeline_state_query(request)
    } else if op == OP_TIMELINE_STATE_DETAIL {
        timeline_detail::state_detail_response(request.req_id.clone(), &state, request.params)
    } else if op == OP_TIMELINE_COMPOSITION_OPEN {
        timeline_host::composition_open(manager, &state, request)
    } else if op == OP_TIMELINE_COMPOSITION_STATE {
        state.timeline_composition_state_query(request)
    } else if op == OP_TIMELINE_COMPOSITION_PATCH {
        state.timeline_composition_patch_query(request)
    } else if op == OP_TIMELINE_EXPORT_START {
        state.timeline_export_start_query(request)
    } else if op == OP_TIMELINE_EXPORT_STATUS {
        state.timeline_export_status_query(request)
    } else if op == OP_TIMELINE_EXPORT_CANCEL {
        state.timeline_export_cancel_query(request)
    } else if op == OP_TIMELINE_OPEN {
        timeline_host::open(manager, &state, request)
    } else {
        conversation::response(store, proxy, request)
    };
    send_frontend_rpc(manager, window_id, response);
}

pub(super) fn send_canvas_tool_event(manager: &WindowManager, window_id: &str, event: Value) {
    let Ok(webview) = manager.webview_by_id(window_id) else {
        return;
    };
    let Ok(payload) = serde_json::to_string(&event) else {
        return;
    };
    let script = format!(
        "window.dispatchEvent(new CustomEvent('capy:canvas-tool-event', {{ detail: {payload} }}));"
    );
    let _eval_result = webview.evaluate_script(&script);
}

pub(super) fn broadcast_agent_event(manager: &WindowManager, event: &AgentRuntimeEvent) {
    let Ok(payload) = serde_json::to_string(event) else {
        return;
    };
    let script = format!(
        "window.dispatchEvent(new CustomEvent('capy:agent-event', {{ detail: {payload} }}));"
    );
    for webview in manager.webviews.values() {
        let _eval_result = webview.evaluate_script(&script);
    }
}

fn shared_ack(ack: oneshot::Sender<IpcResponse>) -> SharedAck {
    Arc::new(Mutex::new(Some(ack)))
}

fn send_shared_response(shared_ack: &SharedAck, response: IpcResponse) {
    let Ok(mut guard) = shared_ack.lock() else {
        return;
    };
    if let Some(ack) = guard.take() {
        let _send_result = ack.send(response);
    }
}

fn send_frontend_rpc(manager: &WindowManager, window_id: &str, response: IpcResponse) {
    let Ok(webview) = manager.webview_by_id(window_id) else {
        return;
    };
    let Ok(payload) = serde_json::to_string(&response) else {
        return;
    };
    let script = format!("window.__capyReceive && window.__capyReceive({payload});");
    let _eval_result = webview.evaluate_script(&script);
}

fn required_string(params: &Value, key: &str) -> Result<String, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing required parameter: {key}"))
}

fn optional_string(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn response_from_result(req_id: String, result: Result<Value, String>) -> IpcResponse {
    match result {
        Ok(data) => IpcResponse {
            req_id,
            ok: true,
            data: Some(data),
            error: None,
        },
        Err(error) => error_response(&req_id, error),
    }
}

fn js_callback_response(req_id: &str, raw: &str) -> IpcResponse {
    let parsed = serde_json::from_str::<Value>(raw).and_then(|value| {
        if let Some(inner) = value.as_str() {
            serde_json::from_str::<Value>(inner)
        } else {
            Ok(value)
        }
    });
    match parsed {
        Ok(value) if value.get("ok").and_then(Value::as_bool) == Some(false) => error_response(
            req_id,
            value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("JavaScript operation failed"),
        ),
        Ok(value) => IpcResponse {
            req_id: req_id.to_string(),
            ok: true,
            data: Some(value),
            error: None,
        },
        Err(err) => error_response(req_id, format!("invalid JavaScript callback JSON: {err}")),
    }
}

pub(super) fn quit_response(request: IpcRequest) -> IpcResponse {
    ok_response(&request, json!({ "quit": true }))
}
