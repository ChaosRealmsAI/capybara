use std::process::Command;
use std::sync::Arc;

use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;

use crate::app::ShellEvent;
use crate::store::{Conversation, CreateRunEvent, Store};

mod runtime_event;
mod sdk;
mod tool_path;

pub use runtime_event::AgentRuntimeEvent;
use runtime_event::event;

#[derive(Debug)]
struct RunOutput {
    content: String,
    native_session_id: Option<String>,
    native_thread_id: Option<String>,
    event_json: Value,
}

pub fn spawn_turn(
    store: Arc<Store>,
    proxy: EventLoopProxy<ShellEvent>,
    conversation: Conversation,
    prompt: String,
    canvas_context: Value,
) -> Result<String, String> {
    if store
        .running_run_for_conversation(&conversation.id)?
        .is_some()
    {
        return Err("conversation already has a running turn".to_string());
    }
    let run = store.create_run(&conversation.id)?;
    store.add_message(
        &conversation.id,
        "user",
        &prompt,
        message_event_json(&canvas_context),
    )?;
    store.update_title_if_default(&conversation.id, &prompt)?;
    store.update_status(&conversation.id, "running")?;
    record_and_emit(
        &store,
        &proxy,
        event(&conversation, &run.id, "run_status")
            .with_status("running")
            .with_content("Agent run started"),
    );

    let run_id = run.id.clone();
    std::thread::spawn(move || {
        let result = run_sdk(&store, &proxy, &conversation, &run_id, &prompt);

        match result {
            Ok(output) => {
                if !output.content.trim().is_empty() {
                    let _message_result = store.add_message(
                        &conversation.id,
                        "assistant",
                        output.content.trim_end(),
                        output.event_json,
                    );
                }
                if let Some(session_id) = output.native_session_id {
                    let _session_result =
                        store.update_native_session(&conversation.id, &session_id);
                }
                if let Some(thread_id) = output.native_thread_id {
                    let _thread_result = store.update_native_thread(&conversation.id, &thread_id);
                }
                let _status_result = store.update_status(&conversation.id, "idle");
                let _run_result = store.finish_run(&run_id, "completed", None);
                record_and_emit(
                    &store,
                    &proxy,
                    event(&conversation, &run_id, "assistant_done")
                        .with_status("completed")
                        .with_content(output.content),
                );
            }
            Err(error) => {
                let was_stopped = store
                    .get_run(&run_id)
                    .ok()
                    .flatten()
                    .map(|run| run.status == "stopped")
                    .unwrap_or(false);
                if was_stopped {
                    let _status_result = store.update_status(&conversation.id, "idle");
                    record_and_emit(
                        &store,
                        &proxy,
                        event(&conversation, &run_id, "run_status")
                            .with_status("stopped")
                            .with_content("Agent run stopped"),
                    );
                    return;
                }
                let _status_result = store.update_status(&conversation.id, "error");
                let _run_result = store.finish_run(&run_id, "failed", Some(&error));
                let _message_result = store.add_message(
                    &conversation.id,
                    "system",
                    &error,
                    json!({ "level": "error" }),
                );
                record_and_emit(
                    &store,
                    &proxy,
                    event(&conversation, &run_id, "error")
                        .with_status("failed")
                        .with_error(error),
                );
            }
        }
    });

    Ok(run.id)
}

fn message_event_json(canvas_context: &Value) -> Value {
    if canvas_context.is_null() {
        json!({ "source": "capybara" })
    } else {
        json!({
            "source": "capybara",
            "canvas_context": canvas_context
        })
    }
}

pub fn stop_running(store: &Store, conversation_id: &str) -> Result<Value, String> {
    let Some(run) = store.running_run_for_conversation(conversation_id)? else {
        return Ok(json!({ "stopped": false, "reason": "no running run" }));
    };
    let Some(pid) = run.pid else {
        store.finish_run(&run.id, "stopped", Some("run had no recorded pid"))?;
        return Ok(json!({ "stopped": false, "run_id": run.id, "reason": "pid missing" }));
    };
    let status = Command::new("kill")
        .arg(pid.to_string())
        .status()
        .map_err(|err| format!("kill failed: {err}"))?;
    store.finish_run(&run.id, "stopped", Some("stopped by user"))?;
    store.update_status(conversation_id, "idle")?;
    Ok(json!({
        "stopped": status.success(),
        "run_id": run.id,
        "pid": pid
    }))
}

pub fn doctor() -> Value {
    sdk::doctor()
}

fn run_sdk(
    store: &Store,
    proxy: &EventLoopProxy<ShellEvent>,
    conversation: &Conversation,
    run_id: &str,
    prompt: &str,
) -> Result<RunOutput, String> {
    let output = sdk::run(store, proxy, conversation, run_id, prompt)?;
    Ok(RunOutput {
        content: output.content,
        native_session_id: output.native_session_id,
        native_thread_id: output.native_thread_id,
        event_json: output.event_json,
    })
}

fn record_and_emit(store: &Store, proxy: &EventLoopProxy<ShellEvent>, event: AgentRuntimeEvent) {
    let event_json = serde_json::to_value(&event).unwrap_or(Value::Null);
    let _event_result = store.add_run_event(CreateRunEvent {
        conversation_id: &event.conversation_id,
        run_id: &event.run_id,
        kind: &event.kind,
        delta: event.delta.as_deref(),
        content: event.content.as_deref(),
        status: event.status.as_deref(),
        error: event.error.as_deref(),
        event_json,
    });
    emit(proxy, event);
}

fn emit(proxy: &EventLoopProxy<ShellEvent>, event: AgentRuntimeEvent) {
    let _send_result = proxy.send_event(ShellEvent::AgentRuntimeEvent { event });
}

#[cfg(test)]
mod sdk_tests;
#[cfg(test)]
mod tests;
