use std::sync::Arc;

use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;

use crate::agent;
use crate::app::ShellEvent;
use crate::ipc::{IpcRequest, IpcResponse};
use crate::store::{CreateConversation, Provider, Store};

pub(super) fn response(
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
            Ok(json!({ "events": store.run_events_for(&id, run_id.as_deref())? }))
        }
        "conversation-create" => create(store, request.params),
        "conversation-send" => send(store, proxy, request.params),
        "conversation-stop" => {
            let id = required_string(&request.params, "id")?;
            agent::stop_running(&store, &id)
        }
        "conversation-update-config" => update_config(store, request.params),
        "agent-doctor" => Ok(agent::doctor()),
        _ => Err(format!("unknown conversation op: {}", request.op)),
    })();

    super::ipc_handlers::response_from_result(request.req_id, result)
}

fn create(store: Arc<Store>, params: Value) -> Result<Value, String> {
    let provider = Provider::parse(
        params
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("claude"),
    )?;
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(default_cwd);
    let model = optional_string(&params, "model");
    let config = params.get("config").cloned().unwrap_or_else(|| json!({}));
    let conversation = store.create_conversation(CreateConversation {
        provider,
        cwd,
        model,
        config,
    })?;
    Ok(json!({ "conversation": conversation, "messages": [] }))
}

fn send(
    store: Arc<Store>,
    proxy: &EventLoopProxy<ShellEvent>,
    params: Value,
) -> Result<Value, String> {
    let id = required_string(&params, "id")?;
    let prompt = required_string(&params, "prompt")?;
    let mut conversation = store.get_conversation(&id)?;
    if params.get("model").is_some() || params.get("config").is_some() {
        let model = if params.get("model").is_some() {
            optional_string(&params, "model")
        } else {
            conversation.model.clone()
        };
        let incoming_config = params.get("config").cloned().unwrap_or_else(|| json!({}));
        let config = merge_config(conversation.config.clone(), incoming_config);
        conversation = store.update_config(&id, model, config)?;
    }
    let canvas_context = params.get("canvas_context").cloned().unwrap_or(Value::Null);
    let run_id = agent::spawn_turn(
        Arc::clone(&store),
        proxy.clone(),
        conversation,
        prompt,
        canvas_context,
    )?;
    Ok(json!({ "run_id": run_id, "status": "running" }))
}

fn update_config(store: Arc<Store>, params: Value) -> Result<Value, String> {
    let id = required_string(&params, "id")?;
    let current = store.get_conversation(&id)?;
    let model = if params.get("model").is_some() {
        optional_string(&params, "model")
    } else {
        current.model
    };
    let incoming_config = params.get("config").cloned().unwrap_or_else(|| json!({}));
    let config = merge_config(current.config, incoming_config);
    let conversation = store.update_config(&id, model, config)?;
    Ok(json!({ "conversation": conversation }))
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
