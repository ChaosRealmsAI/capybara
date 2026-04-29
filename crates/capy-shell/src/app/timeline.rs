use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use capy_contracts::timeline::KIND_TIMELINE_COMPOSITION;

use super::ShellState;
use super::timeline_state::{ExportJob, ExportJobStatus, TimelineTransition, iso_now};
pub use super::timeline_state::{TimelineNodeAction, TimelineNodeState};
use crate::ipc::IpcResponse;

mod requests;
#[cfg(test)]
mod tests;

use requests::{
    AttachRequest, ExportJobRequest, OpenRequest, StateRequest, absolute_path, attach_error,
    export_cancel_trace_id, export_status_trace_id, open_trace_id, state_trace_id, trace_id,
};

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
    pub state: TimelineNodeState,
    pub composition_ref: TimelineCompositionRef,
    pub export_jobs: Vec<ExportJob>,
    pub history: Vec<TimelineTransition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TimelineCompositionRef {
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
        capy_timeline::validate_composition(capy_timeline::ValidateCompositionRequest {
            composition_path: path.clone(),
        });
    let mut attached = AttachedCanvasNode {
        kind: KIND_TIMELINE_COMPOSITION.to_string(),
        state: TimelineNodeState::Draft,
        composition_ref: TimelineCompositionRef {
            path: path.display().to_string(),
            schema_version: validation.schema_version.clone(),
            track_count: validation.track_count,
            asset_count: validation.asset_count,
        },
        export_jobs: Vec::new(),
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
                Some("next step · run capy timeline validate --composition <path>".to_string())
            }),
            "structural validate failed",
        )?;
        state.attach_timeline_node(request.canvas_node_id, attached)?;
        return Err(attach_error(
            "INVALID_COMPOSITION",
            validation
                .errors
                .first()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "composition validation failed".to_string()),
            "next step · run capy timeline validate --composition <path>",
        ));
    }
    transition_node(
        &mut attached,
        TimelineNodeAction::ValidateOk,
        "structural validate ok",
    )?;

    let compile = capy_timeline::compile_composition(capy_timeline::CompileCompositionRequest {
        composition_path: path.clone(),
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
                Some("next step · run capy timeline compile --composition <path>".to_string())
            }),
            "compile failed",
        )?;
        state.attach_timeline_node(request.canvas_node_id, attached)?;
        return Err(attach_error(
            "COMPILE_FAILED",
            compile
                .errors
                .first()
                .map(|error| error.message.clone())
                .unwrap_or_else(|| "composition compile failed".to_string()),
            "next step · run capy timeline compile --composition <path>",
        ));
    }
    transition_node(
        &mut attached,
        TimelineNodeAction::CompileOk,
        "render_source.v1 generated",
    )?;
    transition_node(
        &mut attached,
        TimelineNodeAction::PreviewReady,
        "v0.13.5 attach marks preview ready",
    )?;
    state.attach_timeline_node(request.canvas_node_id, attached.clone())?;

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
                .timeline_nodes()?
                .into_iter()
                .find(|(node_id, _)| *node_id == id)
                .map(|(_, node)| node)
                .ok_or_else(|| {
                    attach_error(
                        "CANVAS_NODE_NOT_FOUND",
                        format!("canvas node {id} has no attached Timeline composition"),
                        "next step · run capy timeline attach",
                    )
                })?;
            vec![attachment_json(id, &node)]
        }
        None => state
            .timeline_nodes()?
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
        .timeline_node(request.canvas_node_id)?
        .ok_or_else(|| {
            attach_error(
                "CANVAS_NODE_NOT_FOUND",
                format!(
                    "canvas node {} has no attached Timeline composition",
                    request.canvas_node_id
                ),
                "next step · run capy timeline attach",
            )
        })?;
    if node.state != TimelineNodeState::PreviewReady {
        return Err(attach_error(
            "NOT_PREVIEW_READY",
            format!(
                "canvas node {} Timeline state is {}",
                request.canvas_node_id,
                node.state.label()
            ),
            "next step · run capy timeline attach",
        ));
    }
    let preview_url = state.register_timeline_preview(
        request.canvas_node_id,
        Path::new(&node.composition_ref.path),
    )?;
    Ok(open_report(request.canvas_node_id, preview_url))
}

pub fn export_status(state: &ShellState, params: Value) -> Result<Value, String> {
    let request = ExportJobRequest::from_params(params, "status")?;
    let job = find_export_job(state, &request.job_id)?.ok_or_else(|| {
        attach_error(
            "EXPORT_JOB_NOT_FOUND",
            format!("export job {} was not found", request.job_id),
            "next step · run capy timeline export",
        )
    })?;
    Ok(json!({
        "ok": true,
        "trace_id": export_status_trace_id(),
        "stage": "status",
        "job": job
    }))
}

pub fn export_cancel(state: &ShellState, params: Value) -> Result<Value, String> {
    let request = ExportJobRequest::from_params(params, "cancel")?;
    let mut nodes = state.timeline_nodes()?;
    let mut cancelled = None;
    for (canvas_node_id, mut node) in nodes.drain(..) {
        if let Some(job) = node
            .export_jobs
            .iter_mut()
            .find(|job| job.job_id == request.job_id)
        {
            job.status = ExportJobStatus::Cancelled;
            job.progress = job.progress.min(99);
            cancelled = Some(job.clone());
            state.attach_timeline_node(canvas_node_id, node)?;
            break;
        }
    }
    let job = cancelled.ok_or_else(|| {
        attach_error(
            "EXPORT_JOB_NOT_FOUND",
            format!("export job {} was not found", request.job_id),
            "next step · run capy timeline export",
        )
    })?;
    Ok(json!({
        "ok": true,
        "trace_id": export_cancel_trace_id(),
        "stage": "cancel",
        "job": job
    }))
}

pub(crate) fn export_status_response(
    req_id: String,
    state: &ShellState,
    params: Value,
) -> IpcResponse {
    match export_status(state, params) {
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

pub(crate) fn export_cancel_response(
    req_id: String,
    state: &ShellState,
    params: Value,
) -> IpcResponse {
    match export_cancel(state, params) {
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
            "preview": format!("[data-capy-component-kind='timeline-composition'][data-canvas-node-id='{canvas_node_id}'] [data-capy-timeline-preview]"),
            "state": format!("[data-capy-component-kind='timeline-composition'][data-canvas-node-id='{canvas_node_id}'][data-capy-timeline-state]")
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
    action: TimelineNodeAction,
    reason: &str,
) -> Result<(), String> {
    let from = node.state.clone();
    let to = from.transition(action).map_err(|err| {
        attach_error(
            "ILLEGAL_TRANSITION",
            format!("illegal transition from {} via {}", err.from, err.action),
            "next step · inspect timeline state history",
        )
    })?;
    node.history.push(TimelineTransition {
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
        TimelineNodeAction::Error {
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
        "export_jobs": node.export_jobs,
        "history": node.history
    })
}

fn find_export_job(state: &ShellState, job_id: &str) -> Result<Option<ExportJob>, String> {
    Ok(state
        .timeline_nodes()?
        .into_iter()
        .flat_map(|(_, node)| node.export_jobs)
        .find(|job| job.job_id == job_id))
}
