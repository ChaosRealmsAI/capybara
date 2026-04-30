use serde_json::{Value, json};

use capy_contracts::timeline::{
    OP_TIMELINE_ATTACH, OP_TIMELINE_COMPOSITION_OPEN, OP_TIMELINE_EXPORT_CANCEL,
    OP_TIMELINE_EXPORT_STATUS, OP_TIMELINE_OPEN, OP_TIMELINE_STATE,
};

use crate::ipc_client;

use super::report::{attach_failure, job_failure, open_failure, state_failure};
use super::{
    TimelineAttachArgs, TimelineCancelArgs, TimelineOpenArgs, TimelineStateArgs,
    TimelineStatusArgs, absolute_path, print_json,
};

pub(super) fn attach(args: TimelineAttachArgs) -> Result<(), String> {
    let composition_path = absolute_path(args.composition)?;
    let socket = args.socket.unwrap_or_else(ipc_client::socket_path);
    let request = ipc_client::request(
        OP_TIMELINE_ATTACH,
        json!({
            "canvas_node_id": args.canvas_node,
            "composition_path": composition_path.display().to_string()
        }),
    );
    match ipc_client::send_to(request, socket.clone()) {
        Ok(response) if response.ok => {
            let mut report = response.data.unwrap_or(Value::Null);
            report["ipc_socket"] = json!(socket.display().to_string());
            print_json(&report)
        }
        Ok(response) => {
            let report = attach_failure(
                args.canvas_node,
                &composition_path,
                &socket,
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("IPC_ERROR"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("capy-shell timeline attach failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy timeline attach --help"),
            );
            print_json(&report)?;
            std::process::exit(1);
        }
        Err(error) => {
            let report = attach_failure(
                args.canvas_node,
                &composition_path,
                &socket,
                "SHELL_UNAVAILABLE",
                error,
                "next step · run capy shell",
            );
            print_json(&report)?;
            std::process::exit(1);
        }
    }
}

pub(super) fn state(args: TimelineStateArgs) -> Result<(), String> {
    let socket = ipc_client::socket_path();
    let request = ipc_client::request(
        OP_TIMELINE_STATE,
        json!({
            "canvas_node_id": args.canvas_node
        }),
    );
    match ipc_client::send_to(request, socket) {
        Ok(response) if response.ok => print_json(&response.data.unwrap_or(Value::Null)),
        Ok(response) => {
            let report = state_failure(
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("IPC_ERROR"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("capy-shell timeline state failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy timeline state --help"),
            );
            print_json(&report)?;
            std::process::exit(1);
        }
        Err(error) => {
            let report = state_failure("SHELL_UNAVAILABLE", error, "next step · run capy shell");
            print_json(&report)?;
            std::process::exit(1);
        }
    }
}

pub(super) fn status(args: TimelineStatusArgs) -> Result<(), String> {
    timeline_job_ipc(
        OP_TIMELINE_EXPORT_STATUS,
        &args.job,
        "status",
        "next step · run capy shell",
    )
}

pub(super) fn cancel(args: TimelineCancelArgs) -> Result<(), String> {
    timeline_job_ipc(
        OP_TIMELINE_EXPORT_CANCEL,
        &args.job,
        "cancel",
        "next step · run capy shell",
    )
}

fn timeline_job_ipc(op: &str, job_id: &str, stage: &str, shell_hint: &str) -> Result<(), String> {
    let socket = ipc_client::socket_path();
    let request = ipc_client::request(op, json!({ "job_id": job_id }));
    match ipc_client::send_to(request, socket.clone()) {
        Ok(response) if response.ok => print_json(&response.data.unwrap_or(Value::Null)),
        Ok(response) => {
            let report = job_failure(
                stage,
                job_id,
                &socket,
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("IPC_ERROR"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("capy-shell timeline export job op failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy timeline status --help"),
            );
            print_json(&report)?;
            std::process::exit(1);
        }
        Err(error) => {
            let report = job_failure(
                stage,
                job_id,
                &socket,
                "SHELL_UNAVAILABLE",
                error,
                shell_hint,
            );
            print_json(&report)?;
            std::process::exit(1);
        }
    }
}

pub(super) fn open(args: TimelineOpenArgs) -> Result<(), String> {
    let socket = args.socket.unwrap_or_else(ipc_client::socket_path);
    let (request, canvas_node) = if let Some(composition) = args.composition {
        let composition_path = absolute_path(composition)?;
        (
            ipc_client::request(
                OP_TIMELINE_COMPOSITION_OPEN,
                json!({
                    "composition_path": composition_path.display().to_string()
                }),
            ),
            None,
        )
    } else {
        let canvas_node = args.canvas_node.ok_or_else(|| {
            "missing --canvas-node or --composition for capy timeline open".to_string()
        })?;
        (
            ipc_client::request(
                OP_TIMELINE_OPEN,
                json!({
                    "canvas_node_id": canvas_node
                }),
            ),
            Some(canvas_node),
        )
    };
    match ipc_client::send_to(request, socket.clone()) {
        Ok(response) if response.ok => {
            let mut report = response.data.unwrap_or(Value::Null);
            report["ipc_socket"] = json!(socket.display().to_string());
            print_json(&report)
        }
        Ok(response) => {
            let report = open_failure(
                canvas_node.unwrap_or(0),
                &socket,
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("code"))
                    .and_then(Value::as_str)
                    .unwrap_or("IPC_ERROR"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("message"))
                    .and_then(Value::as_str)
                    .unwrap_or("capy-shell timeline open failed"),
                response
                    .error
                    .as_ref()
                    .and_then(|error| error.get("hint"))
                    .and_then(Value::as_str)
                    .unwrap_or("next step · run capy timeline open --help"),
            );
            print_json(&report)?;
            std::process::exit(1);
        }
        Err(error) => {
            let report = open_failure(
                canvas_node.unwrap_or(0),
                &socket,
                "SHELL_UNAVAILABLE",
                error,
                "next step · run capy shell",
            );
            print_json(&report)?;
            std::process::exit(1);
        }
    }
}
