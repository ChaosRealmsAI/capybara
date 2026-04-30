use serde::Serialize;
use serde_json::Value;

use crate::store::Conversation;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AgentRuntimeEvent {
    pub conversation_id: String,
    pub run_id: String,
    pub provider: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub event: Value,
}

pub(super) fn event(conversation: &Conversation, run_id: &str, kind: &str) -> AgentRuntimeEvent {
    AgentRuntimeEvent {
        conversation_id: conversation.id.clone(),
        run_id: run_id.to_string(),
        provider: conversation.provider.as_str().to_string(),
        kind: kind.to_string(),
        delta: None,
        content: None,
        status: None,
        error: None,
        event: Value::Null,
    }
}

impl AgentRuntimeEvent {
    pub(super) fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }

    pub(super) fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub(super) fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    pub(super) fn with_event(mut self, event: Value) -> Self {
        self.event = event;
        self
    }
}
