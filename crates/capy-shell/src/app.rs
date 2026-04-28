use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{Value, json};
use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowExtMacOS;
use tao::window::{UserAttentionType, Window, WindowBuilder, WindowId};
use tokio::sync::oneshot;

use crate::agent::{self, AgentRuntimeEvent};
use crate::capture;
use crate::ipc::{self, IpcRequest, IpcResponse, error_response, ok_response};
use crate::store::{CreateConversation, Provider, Store};
use crate::webview::ShellBrowser;

pub enum ShellEvent {
    OpenWindow {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    StateQuery {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    DevtoolsQuery {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    DevtoolsEval {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    Screenshot {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    CaptureWindow {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    ConversationRequest {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    AgentRuntimeEvent {
        event: AgentRuntimeEvent,
    },
    IpcFromJs {
        window_id: String,
        body: String,
    },
    Quit {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WindowStatus {
    pub window_id: String,
    pub project: String,
    pub focused: bool,
}

#[derive(Debug, Clone)]
struct WindowMeta {
    project: String,
}

#[derive(Default)]
pub struct ShellState {
    windows: Mutex<Vec<WindowStatus>>,
}

impl ShellState {
    pub fn can_answer_directly(&self, request: &IpcRequest) -> bool {
        request.params.get("query").and_then(Value::as_str) == Some("windows")
            || request.params.get("key").and_then(Value::as_str) == Some("app.ready")
    }

    pub fn state_query(&self, request: IpcRequest) -> IpcResponse {
        if request.params.get("query").and_then(Value::as_str) == Some("windows") {
            let Ok(windows) = self.windows.lock() else {
                return error_response(&request.req_id, "window state lock failed");
            };
            return ok_response(
                &request,
                json!({ "windows": windows.as_slice(), "count": windows.len() }),
            );
        }

        let key = request
            .params
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or("app.ready");
        let value = match key {
            "app.ready" => json!(true),
            _ => Value::Null,
        };
        ok_response(&request, json!({ "key": key, "value": value }))
    }

    fn sync_from_manager(&self, manager: &WindowManager) {
        if let Ok(mut windows) = self.windows.lock() {
            *windows = manager.list();
        }
    }
}

pub fn run() {
    match crate::webview::maybe_run_cef_subprocess() {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            eprintln!("capy-shell CEF subprocess failed: {err}");
            std::process::exit(1);
        }
    }
    let mut cef_runtime = Some(match crate::webview::init_cef_runtime() {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("capy-shell CEF init failed: {err}");
            std::process::exit(1);
        }
    });
    let mut builder = EventLoopBuilder::<ShellEvent>::with_user_event();
    let event_loop = builder.build();
    let proxy = event_loop.create_proxy();
    let state = Arc::new(ShellState::default());
    let store = match Store::open_default() {
        Ok(store) => Arc::new(store),
        Err(err) => {
            eprintln!("capy-shell store failed to start: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = ipc::spawn_server_thread(proxy.clone(), Arc::clone(&state)) {
        eprintln!("capy-shell IPC failed to start: {err}");
        std::process::exit(1);
    }
    ipc::write_ready_event();

    let mut manager = WindowManager::new();
    let mut keepalive_window: Option<Window> = None;

    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                if keepalive_window.is_none() {
                    keepalive_window = WindowBuilder::new()
                        .with_title("Capybara Shell")
                        .with_decorations(false)
                        .with_inner_size(LogicalSize::new(1.0, 1.0))
                        .with_visible(false)
                        .build(target)
                        .ok();
                }
            }
            Event::LoopDestroyed => {
                manager.quit_all();
                let _cleanup_result = std::fs::remove_file(ipc::socket_path());
                drop(cef_runtime.take());
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
                ..
            } => {
                manager.remove_by_tao_window_id(window_id);
                state.sync_from_manager(&manager);
            }
            Event::UserEvent(event) => match event {
                ShellEvent::OpenWindow { request, ack } => {
                    let response = open_window(&mut manager, target, &proxy, request);
                    state.sync_from_manager(&manager);
                    let _send_result = ack.send(response);
                }
                ShellEvent::StateQuery { request, ack } => {
                    state_query(&manager, request, ack);
                }
                ShellEvent::DevtoolsQuery { request, ack } => {
                    devtools_query(&manager, request, ack);
                }
                ShellEvent::DevtoolsEval { request, ack } => {
                    devtools_eval(&manager, request, ack);
                }
                ShellEvent::Screenshot { request, ack } => {
                    screenshot(&manager, request, ack);
                }
                ShellEvent::CaptureWindow { request, ack } => {
                    let response = capture_window(&mut manager, request);
                    let _send_result = ack.send(response);
                }
                ShellEvent::ConversationRequest { request, ack } => {
                    let response = conversation_response(Arc::clone(&store), &proxy, request);
                    let _send_result = ack.send(response);
                }
                ShellEvent::AgentRuntimeEvent { event } => {
                    broadcast_agent_event(&manager, &event);
                }
                ShellEvent::IpcFromJs { window_id, body } => {
                    handle_js_ipc(&manager, Arc::clone(&store), &proxy, &window_id, &body);
                }
                ShellEvent::Quit { request, ack } => {
                    manager.quit_all();
                    state.sync_from_manager(&manager);
                    let _cleanup_result = std::fs::remove_file(ipc::socket_path());
                    let response = ok_response(&request, json!({ "quit": true }));
                    let _send_result = ack.send(response);
                    drop(cef_runtime.take());
                    *control_flow = ControlFlow::Exit;
                }
            },
            _ => {}
        }
    });
}

fn open_window(
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

fn devtools_query(manager: &WindowManager, request: IpcRequest, ack: oneshot::Sender<IpcResponse>) {
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
        let script = devtools_script(&query, get);
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

fn devtools_eval(manager: &WindowManager, request: IpcRequest, ack: oneshot::Sender<IpcResponse>) {
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

fn state_query(manager: &WindowManager, request: IpcRequest, ack: oneshot::Sender<IpcResponse>) {
    let req_id = request.req_id.clone();
    let shared_ack = shared_ack(ack);
    let result = (|| {
        let key = required_string(&request.params, "key")?;
        let window = optional_string(&request.params, "window");
        let (_, webview) = manager.webview_for_target(window.as_deref())?;
        let script = state_script(&key);
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

fn screenshot(manager: &WindowManager, request: IpcRequest, ack: oneshot::Sender<IpcResponse>) {
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
        let (window_id, webview) = manager.webview_for_target(window.as_deref())?;
        let script = screenshot_probe_script(&region);
        let callback_req_id = req_id.clone();
        let window_id = window_id.to_string();
        let callback_ack = Arc::clone(&shared_ack);
        webview
            .evaluate_script_with_callback(&script, move |raw| {
                let response =
                    screenshot_response(&callback_req_id, &window_id, &region, &out, &raw);
                send_shared_response(&callback_ack, response);
            })
            .map_err(|err| format!("screenshot evaluate failed: {err}"))
    })();

    if let Err(error) = result {
        send_shared_response(&shared_ack, error_response(&req_id, error));
    }
}

type SharedAck = Arc<Mutex<Option<oneshot::Sender<IpcResponse>>>>;

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

fn capture_window(manager: &mut WindowManager, request: IpcRequest) -> IpcResponse {
    let result = (|| {
        let out = required_string(&request.params, "out")?;
        let window = optional_string(&request.params, "window");
        let (window_id, window_number) =
            manager.native_window_number_for_target(window.as_deref())?;
        manager.focus(&window_id)?;
        std::thread::sleep(std::time::Duration::from_millis(120));
        let capture = capture::capture_window_by_number(window_number, Path::new(&out))?;
        Ok(json!({
            "out": capture.out.display().to_string(),
            "bytes": capture.bytes,
            "width": capture.width,
            "height": capture.height,
            "window_id": window_id,
            "window_number": window_number
        }))
    })();

    response_from_result(request.req_id, result)
}

fn conversation_response(
    store: Arc<Store>,
    proxy: &EventLoopProxy<ShellEvent>,
    request: IpcRequest,
) -> IpcResponse {
    let result = (|| match request.op.as_str() {
        "conversation-list" => Ok(json!({
            "conversations": store.list_conversations()?,
            "db_path": store.db_path().display().to_string()
        })),
        "conversation-open" => {
            let id = required_string(&request.params, "id")?;
            Ok(serde_json::to_value(store.conversation_detail(&id)?)
                .map_err(|err| err.to_string())?)
        }
        "conversation-events" => {
            let id = required_string(&request.params, "id")?;
            let run_id = optional_string(&request.params, "run_id");
            Ok(json!({
                "events": store.run_events_for(&id, run_id.as_deref())?
            }))
        }
        "conversation-create" => {
            let provider = Provider::parse(
                request
                    .params
                    .get("provider")
                    .and_then(Value::as_str)
                    .unwrap_or("claude"),
            )?;
            let cwd = request
                .params
                .get("cwd")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(default_cwd);
            let model = optional_string(&request.params, "model");
            let config = request
                .params
                .get("config")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let conversation = store.create_conversation(CreateConversation {
                provider,
                cwd,
                model,
                config,
            })?;
            Ok(json!({ "conversation": conversation, "messages": [] }))
        }
        "conversation-send" => {
            let id = required_string(&request.params, "id")?;
            let prompt = required_string(&request.params, "prompt")?;
            let mut conversation = store.get_conversation(&id)?;
            if request.params.get("model").is_some() || request.params.get("config").is_some() {
                let model = if request.params.get("model").is_some() {
                    optional_string(&request.params, "model")
                } else {
                    conversation.model.clone()
                };
                let incoming_config = request
                    .params
                    .get("config")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let config = merge_config(conversation.config.clone(), incoming_config);
                conversation = store.update_config(&id, model, config)?;
            }
            let run_id =
                agent::spawn_turn(Arc::clone(&store), proxy.clone(), conversation, prompt)?;
            Ok(json!({ "run_id": run_id, "status": "running" }))
        }
        "conversation-stop" => {
            let id = required_string(&request.params, "id")?;
            agent::stop_running(&store, &id)
        }
        "conversation-update-config" => {
            let id = required_string(&request.params, "id")?;
            let current = store.get_conversation(&id)?;
            let model = if request.params.get("model").is_some() {
                optional_string(&request.params, "model")
            } else {
                current.model
            };
            let incoming_config = request
                .params
                .get("config")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let config = merge_config(current.config, incoming_config);
            let conversation = store.update_config(&id, model, config)?;
            Ok(json!({ "conversation": conversation }))
        }
        "agent-doctor" => Ok(agent::doctor()),
        _ => Err(format!("unknown conversation op: {}", request.op)),
    })();

    response_from_result(request.req_id, result)
}

fn handle_js_ipc(
    manager: &WindowManager,
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
    let response = conversation_response(store, proxy, request);
    send_frontend_rpc(manager, window_id, response);
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

fn broadcast_agent_event(manager: &WindowManager, event: &AgentRuntimeEvent) {
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

fn default_cwd() -> String {
    std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "/".to_string())
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

fn merge_config(mut current: Value, incoming: Value) -> Value {
    let Some(current_object) = current.as_object_mut() else {
        return incoming;
    };
    if let Some(incoming_object) = incoming.as_object() {
        for (key, value) in incoming_object {
            current_object.insert(key.clone(), value.clone());
        }
        current
    } else {
        incoming
    }
}

fn response_from_result(req_id: String, result: Result<Value, String>) -> IpcResponse {
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

fn devtools_script(query: &str, get: &str) -> String {
    let query_json = json_string(query);
    let get_json = json_string(get);
    format!(
        r#"(function() {{
  function reply(value) {{ return JSON.stringify(value); }}
  const selector = {query_json};
  const get = {get_json};
  const el = document.querySelector(selector);
  if (!el) return reply({{ ok: false, error: "selector not found: " + selector }});
  let value;
  if (get === "bounding-rect") {{
    const rect = el.getBoundingClientRect();
    value = {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }};
  }} else if (get === "outerHTML") {{
    value = el.outerHTML;
  }} else {{
    value = el[get];
  }}
  return reply({{ ok: true, selector, get, value }});
}})()"#
    )
}

fn state_script(key: &str) -> String {
    let key_json = json_string(key);
    format!(
        r#"(function() {{
  function reply(value) {{ return JSON.stringify(value); }}
  const key = {key_json};
  const state = window.CAPYBARA_STATE || {{}};
  let value = null;
  if (key === "canvas.selected-id") value = state.selectedId || null;
  else if (key === "canvas.block-count") value = Array.isArray(state.blocks) ? state.blocks.length : 0;
  else if (key === "planner.status") value = "stub";
  else return reply({{ ok: false, error: "unknown state key: " + key }});
  return reply({{ ok: true, key, value }});
}})()"#
    )
}

fn screenshot_probe_script(region: &str) -> String {
    let selector = match region {
        "canvas" => ".canvas",
        "planner" => ".planner",
        "topbar" => ".topbar",
        _ => "",
    };
    let selector_json = json_string(selector);
    format!(
        r#"(function() {{
  const selector = {selector_json};
  const el = selector ? document.querySelector(selector) : document.documentElement;
  const target = el || document.documentElement;
  const rect = target.getBoundingClientRect();
  const width = Math.max(1, Math.round((selector && el ? rect.width : window.innerWidth) || 1));
  const height = Math.max(1, Math.round((selector && el ? rect.height : window.innerHeight) || 1));
  return {{ ok: true, width, height, dpr: 1, selector, found: !!el }};
}})()"#
    )
}

fn screenshot_response(
    req_id: &str,
    window_id: &str,
    region: &str,
    out: &str,
    raw: &str,
) -> IpcResponse {
    let value = match serde_json::from_str::<Value>(raw) {
        Ok(value) => value,
        Err(err) => return error_response(req_id, format!("invalid screenshot probe JSON: {err}")),
    };
    let width = value
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1)
        .clamp(1, 4096);
    let height = value
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1)
        .clamp(1, 4096);
    let png = encode_stub_png(width, height);
    if let Err(err) = write_png(Path::new(out), &png) {
        return error_response(req_id, format!("write screenshot failed: {err}"));
    }
    IpcResponse {
        req_id: req_id.to_string(),
        ok: true,
        data: Some(json!({
            "window_id": window_id,
            "region": region,
            "out": out,
            "width": width,
            "height": height,
            "bytes": png.len(),
            "format": "png",
            "probe": value
        })),
        error: None,
    }
}

fn write_png(path: &Path, png: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, png)
}

pub fn encode_stub_png(width: u32, height: u32) -> Vec<u8> {
    let row_len = 1usize + width as usize * 4;
    let mut raw = Vec::with_capacity(row_len * height as usize);
    for y in 0..height {
        raw.push(0);
        for x in 0..width {
            let shade = 28u8.saturating_add(((x + y) % 24) as u8);
            raw.extend_from_slice(&[shade, shade, 38, 255]);
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    push_chunk(&mut png, b"IHDR", &ihdr);
    push_chunk(&mut png, b"IDAT", &zlib_store(&raw));
    push_chunk(&mut png, b"IEND", &[]);
    png
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut offset = 0usize;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_len = remaining.min(65_535);
        let final_block = offset + block_len == data.len();
        out.push(if final_block { 0x01 } else { 0x00 });
        let len = block_len as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + block_len]);
        offset += block_len;
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn push_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(kind.len() + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    png.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

struct WindowManager {
    windows: HashMap<String, Window>,
    webviews: HashMap<String, ShellBrowser>,
    id_by_wid: HashMap<WindowId, String>,
    metadata: HashMap<String, WindowMeta>,
    window_numbers: HashMap<String, u32>,
    focused_window_id: Option<String>,
    next_seq: u64,
}

impl WindowManager {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            webviews: HashMap::new(),
            id_by_wid: HashMap::new(),
            metadata: HashMap::new(),
            window_numbers: HashMap::new(),
            focused_window_id: None,
            next_seq: 1,
        }
    }

    fn open(
        &mut self,
        target: &EventLoopWindowTarget<ShellEvent>,
        proxy: &EventLoopProxy<ShellEvent>,
        project: &str,
    ) -> Result<String, String> {
        if let Some(window_id) = self.find_by_project(project, None) {
            self.focus(&window_id)?;
            return Ok(window_id);
        }
        self.open_new(target, proxy, project)
    }

    fn open_new(
        &mut self,
        target: &EventLoopWindowTarget<ShellEvent>,
        proxy: &EventLoopProxy<ShellEvent>,
        project: &str,
    ) -> Result<String, String> {
        let window_id = self.allocate_window_id();
        let (window, webview) =
            crate::webview::create_window(target, proxy.clone(), &window_id, project)?;
        if let Some(window_number) = native_window_number(&window) {
            self.window_numbers.insert(window_id.clone(), window_number);
        }
        self.id_by_wid.insert(window.id(), window_id.clone());
        self.metadata.insert(
            window_id.clone(),
            WindowMeta {
                project: project.to_string(),
            },
        );
        self.webviews.insert(window_id.clone(), webview);
        self.windows.insert(window_id.clone(), window);
        self.focused_window_id = Some(window_id.clone());
        Ok(window_id)
    }

    fn remove_by_tao_window_id(&mut self, tao_id: WindowId) {
        let Some(window_id) = self.id_by_wid.remove(&tao_id) else {
            return;
        };
        self.windows.remove(&window_id);
        self.webviews.remove(&window_id);
        self.metadata.remove(&window_id);
        self.window_numbers.remove(&window_id);
        if self.focused_window_id.as_deref() == Some(window_id.as_str()) {
            self.focused_window_id = None;
        }
    }

    fn quit_all(&mut self) {
        self.webviews.clear();
        self.windows.clear();
        self.id_by_wid.clear();
        self.metadata.clear();
        self.window_numbers.clear();
        self.focused_window_id = None;
    }

    fn list(&self) -> Vec<WindowStatus> {
        let mut statuses: Vec<WindowStatus> = self
            .metadata
            .iter()
            .map(|(window_id, meta)| WindowStatus {
                window_id: window_id.clone(),
                project: meta.project.clone(),
                focused: self.focused_window_id.as_deref() == Some(window_id.as_str()),
            })
            .collect();
        statuses.sort_by(|left, right| left.window_id.cmp(&right.window_id));
        statuses
    }

    fn webview_for_target(
        &self,
        window_id: Option<&str>,
    ) -> Result<(String, &ShellBrowser), String> {
        let target_id = self
            .find_target(window_id)
            .ok_or_else(|| "no open Capybara window".to_string())?;
        let webview = self
            .webviews
            .get(&target_id)
            .ok_or_else(|| format!("webview missing for {target_id}"))?;
        Ok((target_id, webview))
    }

    fn window_by_id(&self, window_id: &str) -> Result<&Window, String> {
        self.windows
            .get(window_id)
            .ok_or_else(|| format!("no such window: {window_id}"))
    }

    fn webview_by_id(&self, window_id: &str) -> Result<&ShellBrowser, String> {
        self.webviews
            .get(window_id)
            .ok_or_else(|| format!("webview missing for {window_id}"))
    }

    fn native_window_number_for_target(
        &self,
        window_id: Option<&str>,
    ) -> Result<(String, u32), String> {
        let target_id = self
            .find_target(window_id)
            .ok_or_else(|| "no open Capybara window".to_string())?;
        let window_number = self
            .window_numbers
            .get(&target_id)
            .copied()
            .ok_or_else(|| format!("native window number missing for {target_id}"))?;
        Ok((target_id, window_number))
    }

    fn focus(&mut self, window_id: &str) -> Result<(), String> {
        let window = self
            .windows
            .get(window_id)
            .ok_or_else(|| format!("no such window: {window_id}"))?;
        activate_current_app();
        window.set_visible(true);
        window.set_focus();
        if let Some(webview) = self.webviews.get(window_id) {
            webview.set_focus(true);
        }
        window.request_user_attention(Some(UserAttentionType::Informational));
        self.focused_window_id = Some(window_id.to_string());
        Ok(())
    }

    fn find_by_project(&self, project: &str, window_id: Option<&str>) -> Option<String> {
        if let Some(window_id) = window_id {
            return self
                .metadata
                .get(window_id)
                .and_then(|meta| (meta.project == project).then(|| window_id.to_string()));
        }
        self.metadata
            .iter()
            .find(|(_, meta)| meta.project == project)
            .map(|(id, _)| id.clone())
    }

    fn find_target(&self, window_id: Option<&str>) -> Option<String> {
        if let Some(window_id) = window_id {
            return self
                .windows
                .contains_key(window_id)
                .then(|| window_id.to_string());
        }
        self.focused_window_id
            .as_ref()
            .filter(|id| self.windows.contains_key(id.as_str()))
            .cloned()
            .or_else(|| self.windows.keys().next().cloned())
    }

    fn allocate_window_id(&mut self) -> String {
        loop {
            let id = format!("w-{}", self.next_seq);
            self.next_seq += 1;
            if !self.windows.contains_key(&id) {
                return id;
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(clashing_extern_declarations)]
fn activate_current_app() {
    use std::ffi::c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const i8) -> *mut c_void;
        fn sel_registerName(name: *const i8) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_id(receiver: *mut c_void, selector: *mut c_void) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_bool(receiver: *mut c_void, selector: *mut c_void, value: i8);
    }

    let app_class = unsafe { objc_getClass(c"NSApplication".as_ptr()) };
    if app_class.is_null() {
        return;
    }
    let shared_selector = unsafe { sel_registerName(c"sharedApplication".as_ptr()) };
    let activate_selector = unsafe { sel_registerName(c"activateIgnoringOtherApps:".as_ptr()) };
    if shared_selector.is_null() || activate_selector.is_null() {
        return;
    }
    let app = unsafe { objc_msg_send_id(app_class, shared_selector) };
    if app.is_null() {
        return;
    }
    unsafe {
        objc_msg_send_bool(app, activate_selector, 1);
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_current_app() {}

#[cfg(target_os = "macos")]
#[allow(clashing_extern_declarations)]
fn native_window_number(window: &Window) -> Option<u32> {
    use std::ffi::c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn sel_registerName(name: *const i8) -> *mut c_void;
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_window_number(receiver: *mut c_void, selector: *mut c_void) -> isize;
    }

    let ns_window = window.ns_window();
    if ns_window.is_null() {
        return None;
    }
    let selector = unsafe { sel_registerName(c"windowNumber".as_ptr()) };
    if selector.is_null() {
        return None;
    }
    let number = unsafe { objc_msg_send_window_number(ns_window.cast(), selector) };
    u32::try_from(number).ok().filter(|number| *number > 0)
}

#[cfg(not(target_os = "macos"))]
fn native_window_number(_window: &Window) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::encode_stub_png;

    #[test]
    fn screenshot_png_has_valid_signature() {
        let png = encode_stub_png(2, 2);

        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(png.windows(4).any(|chunk| chunk == b"IHDR"));
        assert!(png.windows(4).any(|chunk| chunk == b"IDAT"));
        assert!(png.windows(4).any(|chunk| chunk == b"IEND"));
    }
}
