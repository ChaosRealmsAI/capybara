use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::Arc;

use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;

use crate::agent_tools::agent_tool_env;
use crate::app::ShellEvent;
use crate::store::{Conversation, CreateRunEvent, Provider, Store};

mod claude;
mod codex;
mod jsonrpc;
mod runtime_event;
mod sdk;
mod tool_path;

use jsonrpc::{read_json_line, read_until_response, send_json};
pub use runtime_event::AgentRuntimeEvent;
use runtime_event::event;
use tool_path::{tool_launch, tool_version};

#[cfg(test)]
use crate::agent_tools::{claude_append_system_prompt, codex_developer_instructions};
#[cfg(test)]
use claude::{args as claude_args, delta as claude_delta};
#[cfg(test)]
use codex::{
    app_server_args as codex_app_server_args, resume_params as codex_resume_params,
    sandbox_mode as codex_sandbox_mode, sandbox_policy as codex_sandbox_policy,
    start_params as codex_start_params, turn_params as codex_turn_params,
};
#[cfg(test)]
use sdk::args as sdk_args;
#[cfg(test)]
use tool_path::{desktop_tool_path_env, resolve_tool_path};

#[derive(Debug)]
struct RunOutput {
    content: String,
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
        let result = if sdk::enabled(&conversation.config) {
            run_sdk(&store, &proxy, &conversation, &run_id, &prompt)
        } else {
            match conversation.provider {
                Provider::Claude => run_claude(&store, &proxy, &conversation, &run_id, &prompt),
                Provider::Codex => run_codex(&store, &proxy, &conversation, &run_id, &prompt),
            }
        };

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
    json!({
        "claude": tool_version("claude", &["--version"]),
        "codex": tool_version("codex", &["--version"]),
        "codex_app_server": tool_version("codex", &["app-server", "--help"])
    })
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
        native_thread_id: output.native_thread_id,
        event_json: output.event_json,
    })
}

fn run_claude(
    store: &Store,
    proxy: &EventLoopProxy<ShellEvent>,
    conversation: &Conversation,
    run_id: &str,
    prompt: &str,
) -> Result<RunOutput, String> {
    let launch = tool_launch("claude");
    let mut command = Command::new(launch.program());
    let use_resume = store
        .messages_for(&conversation.id)?
        .iter()
        .any(|message| message.role == "assistant");
    command.args(claude::args(conversation, prompt, use_resume));
    command.current_dir(&conversation.cwd);
    command.env("PATH", launch.path_env());
    claude::apply_tool_env(&mut command, &conversation.config);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("claude failed to start using {}: {err}", launch.display()))?;
    store.set_run_pid(run_id, child.id())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "claude stdout missing".to_string())?;
    let reader = BufReader::new(stdout);
    let mut content = String::new();
    let mut last_stdout = String::new();

    for line in reader.lines() {
        let line = line.map_err(|err| format!("claude stdout failed: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        last_stdout = line.clone();
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            if value.get("type").and_then(Value::as_str) == Some("result") {
                if content.is_empty() {
                    if let Some(result) = value.get("result").and_then(Value::as_str) {
                        content.push_str(result);
                    }
                }
                continue;
            }
            if let Some(delta) = claude::delta(&value) {
                content.push_str(&delta);
                record_and_emit(
                    store,
                    proxy,
                    event(conversation, run_id, "assistant_delta").with_delta(delta),
                );
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("claude wait failed: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout_detail = claude::result_fallback(&last_stdout);
        let detail = non_empty(Some(stderr.trim()))
            .or_else(|| non_empty(Some(content.trim())))
            .or_else(|| non_empty(Some(stdout_detail.trim())))
            .or_else(|| non_empty(Some(last_stdout.trim())))
            .unwrap_or("no stderr or result content");
        return Err(format!("claude exited with {}: {}", output.status, detail));
    }
    if content.trim().is_empty() {
        content = claude::result_fallback(&String::from_utf8_lossy(&output.stdout));
    }
    Ok(RunOutput {
        content,
        native_thread_id: None,
        event_json: json!({ "provider": conversation.provider.as_str() }),
    })
}

fn run_codex(
    store: &Store,
    proxy: &EventLoopProxy<ShellEvent>,
    conversation: &Conversation,
    run_id: &str,
    prompt: &str,
) -> Result<RunOutput, String> {
    let launch = tool_launch("codex");
    let mut command = Command::new(launch.program());
    command.arg("app-server");
    command.args(codex::app_server_args(&conversation.config));
    command.arg("--listen").arg("stdio://");
    command
        .env("PATH", launch.path_env())
        .envs(agent_tool_env(&conversation.config))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|err| {
        format!(
            "codex app-server failed to start using {}: {err}",
            launch.display()
        )
    })?;
    store.set_run_pid(run_id, child.id())?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "codex stdin missing".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "codex stdout missing".to_string())?;
    let mut reader = BufReader::new(stdout);

    send_json(
        &mut stdin,
        json!({
            "method": "initialize",
            "id": 0,
            "params": {
                "clientInfo": {
                    "name": "capybara",
                    "title": "Capybara",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": { "experimentalApi": true }
            }
        }),
    )?;
    send_json(&mut stdin, json!({ "method": "initialized", "params": {} }))?;

    let start_params = if let Some(thread_id) = conversation.native_thread_id.as_deref() {
        codex::resume_params(conversation, thread_id)
    } else {
        codex::start_params(conversation)
    };
    let start_method = if conversation.native_thread_id.is_some() {
        "thread/resume"
    } else {
        "thread/start"
    };
    send_json(
        &mut stdin,
        json!({ "method": start_method, "id": 1, "params": start_params }),
    )?;
    let thread_id = read_until_response(&mut reader, 1)?
        .get("result")
        .and_then(|result| result.get("thread"))
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| "codex thread response missing thread.id".to_string())?;

    let turn_params = codex::turn_params(conversation, &thread_id, prompt)?;
    send_json(
        &mut stdin,
        json!({ "method": "turn/start", "id": 2, "params": turn_params }),
    )?;

    let mut content = String::new();
    loop {
        let Some(message) = read_json_line(&mut reader)? else {
            break;
        };
        if let Some(error) = message.get("error") {
            return Err(format!("codex error: {error}"));
        }
        if message.get("id").and_then(Value::as_i64) == Some(2) {
            continue;
        }
        if message.get("method").and_then(Value::as_str) == Some("item/agentMessage/delta") {
            if let Some(delta) = message
                .get("params")
                .and_then(|params| params.get("delta"))
                .and_then(Value::as_str)
            {
                content.push_str(delta);
                record_and_emit(
                    store,
                    proxy,
                    event(conversation, run_id, "assistant_delta").with_delta(delta.to_string()),
                );
            }
        }
        if message.get("method").and_then(Value::as_str) == Some("turn/completed") {
            break;
        }
    }
    let _kill_result = child.kill();
    Ok(RunOutput {
        content,
        native_thread_id: Some(thread_id),
        event_json: json!({ "provider": conversation.provider.as_str() }),
    })
}

fn config_str(config: &Value, key: &str) -> Option<String> {
    non_empty(config.get(key).and_then(Value::as_str)).map(ToString::to_string)
}

fn config_bool(config: &Value, key: &str) -> bool {
    config.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn config_array(config: &Value, key: &str) -> Vec<String> {
    config
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|value| non_empty(Some(value)))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
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
