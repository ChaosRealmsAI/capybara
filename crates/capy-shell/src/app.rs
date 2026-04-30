use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tao::dpi::LogicalSize;
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Window, WindowBuilder};
use tokio::sync::oneshot;

use crate::agent::AgentRuntimeEvent;
use crate::ipc::{self, IpcRequest, IpcResponse, error_response, ok_response};
use crate::store::Store;

mod canvas_nodes;
mod canvas_tool;
mod conversation;
mod ipc_handlers;
mod probes;
pub mod timeline;
mod timeline_detail;
mod timeline_editor;
mod timeline_host;
mod timeline_preview;
mod timeline_state;
mod window;

use window::{WindowManager, WindowStatus};

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
    TimelineAttach {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    TimelineOpen {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    TimelineCompositionOpen {
        request: IpcRequest,
        ack: oneshot::Sender<IpcResponse>,
    },
    AgentRuntimeEvent {
        event: AgentRuntimeEvent,
    },
    CanvasToolEvent {
        window_id: String,
        event: Value,
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

pub struct ShellState {
    windows: Mutex<Vec<WindowStatus>>,
    canvas_nodes: Mutex<BTreeSet<u64>>,
    timeline_nodes: Mutex<BTreeMap<u64, timeline::AttachedCanvasNode>>,
    timeline_editor_jobs: Mutex<BTreeMap<String, timeline_state::ExportJob>>,
    timeline_preview: timeline_preview::TimelinePreviewServer,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            windows: Mutex::new(Vec::new()),
            canvas_nodes: Mutex::new(BTreeSet::from([0])),
            timeline_nodes: Mutex::new(BTreeMap::new()),
            timeline_editor_jobs: Mutex::new(BTreeMap::new()),
            timeline_preview: timeline_preview::TimelinePreviewServer::start(),
        }
    }
}

impl ShellState {
    pub fn can_answer_directly(&self, request: &IpcRequest) -> bool {
        request.params.get("query").and_then(Value::as_str) == Some("windows")
            || request.params.get("key").and_then(Value::as_str) == Some("app.ready")
            || request.params.get("key").and_then(Value::as_str) == Some("timeline.attachments")
            || request.params.get("key").and_then(Value::as_str) == Some("timeline.editorJobs")
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
            "timeline.attachments" => {
                let Ok(nodes) = self.timeline_nodes.lock() else {
                    return error_response(&request.req_id, "timeline state lock failed");
                };
                json!(nodes.clone())
            }
            "timeline.editorJobs" => {
                let Ok(jobs) = self.timeline_editor_jobs.lock() else {
                    return error_response(
                        &request.req_id,
                        "timeline editor job state lock failed",
                    );
                };
                json!(jobs.values().cloned().collect::<Vec<_>>())
            }
            _ => Value::Null,
        };
        ok_response(&request, json!({ "key": key, "value": value }))
    }

    pub fn timeline_state_query(&self, request: IpcRequest) -> IpcResponse {
        timeline::state_response(request.req_id.clone(), self, request.params)
    }

    pub fn timeline_export_status_query(&self, request: IpcRequest) -> IpcResponse {
        timeline::export_status_response(request.req_id.clone(), self, request.params)
    }

    pub fn timeline_export_cancel_query(&self, request: IpcRequest) -> IpcResponse {
        timeline::export_cancel_response(request.req_id.clone(), self, request.params)
    }

    pub fn timeline_composition_state_query(&self, request: IpcRequest) -> IpcResponse {
        timeline_editor::state_response(request.req_id.clone(), self, request.params)
    }

    pub fn timeline_composition_patch_query(&self, request: IpcRequest) -> IpcResponse {
        timeline_editor::patch_response(request.req_id.clone(), self, request.params)
    }

    pub fn timeline_export_start_query(&self, request: IpcRequest) -> IpcResponse {
        timeline_editor::export_start_response(request.req_id.clone(), self, request.params)
    }

    pub(crate) fn has_canvas_node(&self, id: u64) -> bool {
        self.canvas_nodes
            .lock()
            .map(|nodes| nodes.contains(&id))
            .unwrap_or(false)
    }

    pub(crate) fn register_canvas_nodes(&self, ids: &[u64]) -> Result<usize, String> {
        let mut nodes = self
            .canvas_nodes
            .lock()
            .map_err(|_| "canvas node state lock failed".to_string())?;
        for id in ids {
            nodes.insert(*id);
        }
        Ok(nodes.len())
    }

    pub(crate) fn attach_timeline_node(
        &self,
        id: u64,
        node: timeline::AttachedCanvasNode,
    ) -> Result<(), String> {
        let mut nodes = self
            .timeline_nodes
            .lock()
            .map_err(|_| "timeline state lock failed".to_string())?;
        nodes.insert(id, node);
        Ok(())
    }

    pub(crate) fn timeline_nodes(
        &self,
    ) -> Result<Vec<(u64, timeline::AttachedCanvasNode)>, String> {
        let nodes = self
            .timeline_nodes
            .lock()
            .map_err(|_| "timeline state lock failed".to_string())?;
        Ok(nodes.iter().map(|(id, node)| (*id, node.clone())).collect())
    }

    pub(crate) fn timeline_node(
        &self,
        id: u64,
    ) -> Result<Option<timeline::AttachedCanvasNode>, String> {
        let nodes = self
            .timeline_nodes
            .lock()
            .map_err(|_| "timeline state lock failed".to_string())?;
        Ok(nodes.get(&id).cloned())
    }

    pub(crate) fn register_timeline_preview(
        &self,
        canvas_node_id: u64,
        composition_path: &Path,
    ) -> Result<String, String> {
        self.timeline_preview
            .register(canvas_node_id, composition_path)
    }

    pub(crate) fn register_timeline_composition_preview(
        &self,
        composition_path: &Path,
    ) -> Result<String, String> {
        self.timeline_preview
            .register(stable_preview_id(composition_path), composition_path)
    }

    pub(crate) fn upsert_timeline_editor_job(
        &self,
        job: timeline_state::ExportJob,
    ) -> Result<(), String> {
        let mut jobs = self
            .timeline_editor_jobs
            .lock()
            .map_err(|_| "timeline editor job state lock failed".to_string())?;
        jobs.insert(job.job_id.clone(), job);
        Ok(())
    }

    pub(crate) fn timeline_editor_job(
        &self,
        job_id: &str,
    ) -> Result<Option<timeline_state::ExportJob>, String> {
        let jobs = self
            .timeline_editor_jobs
            .lock()
            .map_err(|_| "timeline editor job state lock failed".to_string())?;
        Ok(jobs.get(job_id).cloned())
    }

    pub(crate) fn cancel_timeline_editor_job(
        &self,
        job_id: &str,
    ) -> Result<Option<timeline_state::ExportJob>, String> {
        let mut jobs = self
            .timeline_editor_jobs
            .lock()
            .map_err(|_| "timeline editor job state lock failed".to_string())?;
        let Some(job) = jobs.get_mut(job_id) else {
            return Ok(None);
        };
        job.status = timeline_state::ExportJobStatus::Cancelled;
        job.progress = job.progress.min(99);
        Ok(Some(job.clone()))
    }

    fn sync_from_manager(&self, manager: &WindowManager) {
        if let Ok(mut windows) = self.windows.lock() {
            *windows = manager.list();
        }
    }
}

fn stable_preview_id(path: &Path) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for byte in path.display().to_string().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

pub fn run() {
    match crate::browser::maybe_run_cef_subprocess() {
        Ok(true) => return,
        Ok(false) => {}
        Err(err) => {
            eprintln!("capy-shell CEF subprocess failed: {err}");
            std::process::exit(1);
        }
    }
    let mut builder = EventLoopBuilder::<ShellEvent>::with_user_event();
    let event_loop = builder.build();
    let proxy = event_loop.create_proxy();
    let mut cef_runtime = Some(match crate::browser::init_cef_runtime() {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("capy-shell CEF init failed: {err}");
            std::process::exit(1);
        }
    });
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
    let initial_project = std::env::var("CAPY_OPEN_ON_START")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let mut did_open_initial_project = false;

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
                if !did_open_initial_project {
                    did_open_initial_project = true;
                    if let Some(project) = initial_project.as_deref() {
                        if let Err(err) = manager.open(target, &proxy, project) {
                            eprintln!("capy-shell initial open failed: {err}");
                        }
                        state.sync_from_manager(&manager);
                    }
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
                    let response = ipc_handlers::open_window(&mut manager, target, &proxy, request);
                    state.sync_from_manager(&manager);
                    let _send_result = ack.send(response);
                }
                ShellEvent::StateQuery { request, ack } => {
                    ipc_handlers::state_query(&manager, request, ack);
                }
                ShellEvent::DevtoolsQuery { request, ack } => {
                    ipc_handlers::devtools_query(&manager, request, ack);
                }
                ShellEvent::DevtoolsEval { request, ack } => {
                    ipc_handlers::devtools_eval(&manager, request, ack);
                }
                ShellEvent::Screenshot { request, ack } => {
                    ipc_handlers::screenshot(&mut manager, request, ack);
                }
                ShellEvent::CaptureWindow { request, ack } => {
                    ipc_handlers::capture_window(&mut manager, request, ack);
                }
                ShellEvent::ConversationRequest { request, ack } => {
                    let response = conversation::response(Arc::clone(&store), &proxy, request);
                    let _send_result = ack.send(response);
                }
                ShellEvent::TimelineAttach { request, ack } => {
                    let response = timeline_host::attach(&manager, &state, request);
                    let _send_result = ack.send(response);
                }
                ShellEvent::TimelineOpen { request, ack } => {
                    let response = timeline_host::open(&manager, &state, request);
                    let _send_result = ack.send(response);
                }
                ShellEvent::TimelineCompositionOpen { request, ack } => {
                    let response = timeline_host::composition_open(&manager, &state, request);
                    let _send_result = ack.send(response);
                }
                ShellEvent::AgentRuntimeEvent { event } => {
                    ipc_handlers::broadcast_agent_event(&manager, &event);
                }
                ShellEvent::CanvasToolEvent { window_id, event } => {
                    ipc_handlers::send_canvas_tool_event(&manager, &window_id, event);
                }
                ShellEvent::IpcFromJs { window_id, body } => {
                    ipc_handlers::handle_js_ipc(
                        &manager,
                        Arc::clone(&state),
                        Arc::clone(&store),
                        &proxy,
                        &window_id,
                        &body,
                    );
                }
                ShellEvent::Quit { request, ack } => {
                    manager.quit_all();
                    state.sync_from_manager(&manager);
                    let _cleanup_result = std::fs::remove_file(ipc::socket_path());
                    let response = ipc_handlers::quit_response(request);
                    let _send_result = ack.send(response);
                    drop(cef_runtime.take());
                    *control_flow = ControlFlow::Exit;
                }
            },
            _ => {}
        }
    });
}
