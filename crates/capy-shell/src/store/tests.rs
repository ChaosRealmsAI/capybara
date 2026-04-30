use serde_json::json;

use super::{CreateConversation, CreateRunEvent, Provider, Store};

#[test]
fn creates_and_reloads_conversation_messages() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "capy-store-test-{}.sqlite",
        uuid::Uuid::new_v4().simple()
    ));
    let store = Store::open(path.clone())?;
    let conversation = store.create_conversation(CreateConversation {
        provider: Provider::Claude,
        cwd: "/tmp".to_string(),
        model: Some("sonnet".to_string()),
        config: json!({ "effort": "medium" }),
    })?;
    let run = store.create_run(&conversation.id)?;
    assert_eq!(conversation.config["runtimeBackend"], "sdk");
    store.add_message(&conversation.id, "user", "hello", json!({}))?;
    store.add_run_event(CreateRunEvent {
        conversation_id: &conversation.id,
        run_id: &run.id,
        kind: "assistant_delta",
        delta: Some("he"),
        content: None,
        status: None,
        error: None,
        event_json: json!({ "kind": "assistant_delta" }),
    })?;
    store.add_run_event(CreateRunEvent {
        conversation_id: &conversation.id,
        run_id: &run.id,
        kind: "assistant_done",
        delta: None,
        content: Some("hello"),
        status: Some("completed"),
        error: None,
        event_json: json!({ "kind": "assistant_done" }),
    })?;
    let detail = store.conversation_detail(&conversation.id)?;
    assert_eq!(detail.conversation.provider, Provider::Claude);
    assert_eq!(detail.messages.len(), 1);
    let events = store.run_events_for(&conversation.id, Some(&run.id))?;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].delta.as_deref(), Some("he"));
    assert_eq!(events[1].content.as_deref(), Some("hello"));
    store.update_native_session(&conversation.id, "session-from-sdk")?;
    store.update_native_thread(&conversation.id, "thread-from-sdk")?;
    let updated = store.get_conversation(&conversation.id)?;
    assert_eq!(
        updated.native_session_id.as_deref(),
        Some("session-from-sdk")
    );
    assert_eq!(updated.native_thread_id.as_deref(), Some("thread-from-sdk"));
    let _remove_result = std::fs::remove_file(path);
    Ok(())
}
