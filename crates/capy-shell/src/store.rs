use std::env;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

mod schema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Codex => "codex",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "claude" => Ok(Provider::Claude),
            "codex" => Ok(Provider::Codex),
            _ => Err(format!("unsupported provider: {value}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub provider: Provider,
    pub cwd: String,
    pub native_session_id: Option<String>,
    pub native_thread_id: Option<String>,
    pub model: Option<String>,
    pub config: Value,
    pub status: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub event_json: Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub conversation_id: String,
    pub pid: Option<i64>,
    pub status: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEvent {
    pub id: String,
    pub conversation_id: String,
    pub run_id: String,
    pub kind: String,
    pub delta: Option<String>,
    pub content: Option<String>,
    pub status: Option<String>,
    pub error: Option<String>,
    pub event_json: Value,
    pub created_at: i64,
}

#[derive(Debug)]
pub struct CreateRunEvent<'a> {
    pub conversation_id: &'a str,
    pub run_id: &'a str,
    pub kind: &'a str,
    pub delta: Option<&'a str>,
    pub content: Option<&'a str>,
    pub status: Option<&'a str>,
    pub error: Option<&'a str>,
    pub event_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationDetail {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConversation {
    pub provider: Provider,
    pub cwd: String,
    pub model: Option<String>,
    #[serde(default)]
    pub config: Value,
}

pub struct Store {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

impl Store {
    pub fn open_default() -> Result<Self, String> {
        let dir = app_support_dir()?;
        std::fs::create_dir_all(&dir)
            .map_err(|err| format!("create app data dir failed: {err}"))?;
        Self::open(dir.join("capybara.sqlite"))
    }

    pub fn open(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create database parent failed: {err}"))?;
        }
        let conn = Connection::open(&path).map_err(|err| format!("open database failed: {err}"))?;
        let store = Self {
            conn: Mutex::new(conn),
            db_path: path,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.lock()?;
        schema::migrate(&conn)
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>, String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, title, provider, cwd, native_session_id, native_thread_id,
                       model, config_json, status, archived, created_at, updated_at
                FROM conversations
                WHERE archived = 0
                ORDER BY updated_at DESC
                "#,
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], row_to_conversation)
            .map_err(|err| err.to_string())?;
        collect_rows(rows)
    }

    pub fn conversation_detail(&self, id: &str) -> Result<ConversationDetail, String> {
        let conversation = self.get_conversation(id)?;
        let messages = self.messages_for(id)?;
        Ok(ConversationDetail {
            conversation,
            messages,
        })
    }

    pub fn create_conversation(&self, input: CreateConversation) -> Result<Conversation, String> {
        let id = new_id("conv");
        let now = now_ms();
        let title = format!("New {}", input.provider.as_str());
        let native_session_id =
            (input.provider == Provider::Claude).then(|| Uuid::new_v4().to_string());
        let config = normalize_config(input.config);
        let config_json = serde_json::to_string(&config).map_err(|err| err.to_string())?;
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT INTO conversations
                (id, title, provider, cwd, native_session_id, native_thread_id,
                 model, config_json, status, archived, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, 'idle', 0, ?8, ?8)
            "#,
            params![
                id,
                title,
                input.provider.as_str(),
                input.cwd,
                native_session_id,
                input.model,
                config_json,
                now
            ],
        )
        .map_err(|err| format!("create conversation failed: {err}"))?;
        drop(conn);
        self.get_conversation(&id)
    }

    pub fn get_conversation(&self, id: &str) -> Result<Conversation, String> {
        let conn = self.lock()?;
        conn.query_row(
            r#"
            SELECT id, title, provider, cwd, native_session_id, native_thread_id,
                   model, config_json, status, archived, created_at, updated_at
            FROM conversations
            WHERE id = ?1
            "#,
            params![id],
            row_to_conversation,
        )
        .optional()
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("conversation not found: {id}"))
    }

    pub fn messages_for(&self, conversation_id: &str) -> Result<Vec<Message>, String> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, role, content, event_json, created_at
                FROM messages
                WHERE conversation_id = ?1
                ORDER BY created_at ASC
                "#,
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![conversation_id], row_to_message)
            .map_err(|err| err.to_string())?;
        collect_rows(rows)
    }

    pub fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        event_json: Value,
    ) -> Result<Message, String> {
        let id = new_id("msg");
        let now = now_ms();
        let event = serde_json::to_string(&event_json).map_err(|err| err.to_string())?;
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT INTO messages (id, conversation_id, role, content, event_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![id, conversation_id, role, content, event, now],
        )
        .map_err(|err| format!("insert message failed: {err}"))?;
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now, conversation_id],
        )
        .map_err(|err| format!("touch conversation failed: {err}"))?;
        Ok(Message {
            id,
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            event_json,
            created_at: now,
        })
    }

    pub fn create_run(&self, conversation_id: &str) -> Result<RunRecord, String> {
        let id = new_id("run");
        let now = now_ms();
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT INTO runs (id, conversation_id, pid, status, started_at, ended_at, error)
            VALUES (?1, ?2, NULL, 'running', ?3, NULL, NULL)
            "#,
            params![id, conversation_id, now],
        )
        .map_err(|err| format!("create run failed: {err}"))?;
        Ok(RunRecord {
            id,
            conversation_id: conversation_id.to_string(),
            pid: None,
            status: "running".to_string(),
            started_at: now,
            ended_at: None,
            error: None,
        })
    }

    pub fn set_run_pid(&self, run_id: &str, pid: u32) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE runs SET pid = ?1 WHERE id = ?2",
            params![i64::from(pid), run_id],
        )
        .map_err(|err| format!("update run pid failed: {err}"))?;
        Ok(())
    }

    pub fn finish_run(
        &self,
        run_id: &str,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        let now = now_ms();
        let conn = self.lock()?;
        conn.execute(
            "UPDATE runs SET status = ?1, ended_at = ?2, error = ?3 WHERE id = ?4",
            params![status, now, error, run_id],
        )
        .map_err(|err| format!("finish run failed: {err}"))?;
        Ok(())
    }

    pub fn add_run_event(&self, input: CreateRunEvent<'_>) -> Result<RunEvent, String> {
        let id = new_id("evt");
        let now = now_ms();
        let event = serde_json::to_string(&input.event_json).map_err(|err| err.to_string())?;
        let conn = self.lock()?;
        conn.execute(
            r#"
            INSERT INTO run_events
                (id, conversation_id, run_id, kind, delta, content, status, error, event_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                id,
                input.conversation_id,
                input.run_id,
                input.kind,
                input.delta,
                input.content,
                input.status,
                input.error,
                event,
                now
            ],
        )
        .map_err(|err| format!("insert run event failed: {err}"))?;
        Ok(RunEvent {
            id,
            conversation_id: input.conversation_id.to_string(),
            run_id: input.run_id.to_string(),
            kind: input.kind.to_string(),
            delta: input.delta.map(ToString::to_string),
            content: input.content.map(ToString::to_string),
            status: input.status.map(ToString::to_string),
            error: input.error.map(ToString::to_string),
            event_json: input.event_json,
            created_at: now,
        })
    }

    pub fn run_events_for(
        &self,
        conversation_id: &str,
        run_id: Option<&str>,
    ) -> Result<Vec<RunEvent>, String> {
        let conn = self.lock()?;
        let (sql, params): (&str, Vec<&str>) = if let Some(run_id) = run_id {
            (
                r#"
                SELECT id, conversation_id, run_id, kind, delta, content, status, error, event_json, created_at
                FROM run_events
                WHERE conversation_id = ?1 AND run_id = ?2
                ORDER BY created_at ASC
                "#,
                vec![conversation_id, run_id],
            )
        } else {
            (
                r#"
                SELECT id, conversation_id, run_id, kind, delta, content, status, error, event_json, created_at
                FROM run_events
                WHERE conversation_id = ?1
                ORDER BY created_at ASC
                "#,
                vec![conversation_id],
            )
        };
        let mut stmt = conn.prepare(sql).map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params), row_to_run_event)
            .map_err(|err| err.to_string())?;
        collect_rows(rows)
    }

    pub fn running_run_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<RunRecord>, String> {
        let conn = self.lock()?;
        conn.query_row(
            r#"
            SELECT id, conversation_id, pid, status, started_at, ended_at, error
            FROM runs
            WHERE conversation_id = ?1 AND status = 'running'
            ORDER BY started_at DESC
            LIMIT 1
            "#,
            params![conversation_id],
            row_to_run,
        )
        .optional()
        .map_err(|err| err.to_string())
    }

    pub fn update_status(&self, conversation_id: &str, status: &str) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE conversations SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now_ms(), conversation_id],
        )
        .map_err(|err| format!("update status failed: {err}"))?;
        Ok(())
    }

    pub fn update_title_if_default(
        &self,
        conversation_id: &str,
        prompt: &str,
    ) -> Result<(), String> {
        let title = title_from_prompt(prompt);
        let conn = self.lock()?;
        conn.execute(
            r#"
            UPDATE conversations
            SET title = ?1, updated_at = ?2
            WHERE id = ?3 AND title LIKE 'New %'
            "#,
            params![title, now_ms(), conversation_id],
        )
        .map_err(|err| format!("update title failed: {err}"))?;
        Ok(())
    }

    pub fn update_native_thread(
        &self,
        conversation_id: &str,
        thread_id: &str,
    ) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE conversations SET native_thread_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![thread_id, now_ms(), conversation_id],
        )
        .map_err(|err| format!("update native thread failed: {err}"))?;
        Ok(())
    }

    pub fn update_config(
        &self,
        conversation_id: &str,
        model: Option<String>,
        config: Value,
    ) -> Result<Conversation, String> {
        let normalized = normalize_config(config);
        let config_json = serde_json::to_string(&normalized).map_err(|err| err.to_string())?;
        let conn = self.lock()?;
        conn.execute(
            "UPDATE conversations SET model = ?1, config_json = ?2, updated_at = ?3 WHERE id = ?4",
            params![model, config_json, now_ms(), conversation_id],
        )
        .map_err(|err| format!("update config failed: {err}"))?;
        drop(conn);
        self.get_conversation(conversation_id)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|_| "database lock poisoned".to_string())
    }
}

fn row_to_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    let provider: String = row.get(2)?;
    let config_json: String = row.get(7)?;
    let config = serde_json::from_str(&config_json).unwrap_or_else(|_| json!({}));
    Ok(Conversation {
        id: row.get(0)?,
        title: row.get(1)?,
        provider: Provider::parse(&provider).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(err)),
            )
        })?,
        cwd: row.get(3)?,
        native_session_id: row.get(4)?,
        native_thread_id: row.get(5)?,
        model: row.get(6)?,
        config,
        status: row.get(8)?,
        archived: row.get::<_, i64>(9)? != 0,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    let event_json: String = row.get(4)?;
    Ok(Message {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        event_json: serde_json::from_str(&event_json).unwrap_or_else(|_| json!({})),
        created_at: row.get(5)?,
    })
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
    Ok(RunRecord {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        pid: row.get(2)?,
        status: row.get(3)?,
        started_at: row.get(4)?,
        ended_at: row.get(5)?,
        error: row.get(6)?,
    })
}

fn row_to_run_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunEvent> {
    let event_json: String = row.get(8)?;
    Ok(RunEvent {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        run_id: row.get(2)?,
        kind: row.get(3)?,
        delta: row.get(4)?,
        content: row.get(5)?,
        status: row.get(6)?,
        error: row.get(7)?,
        event_json: serde_json::from_str(&event_json).unwrap_or_else(|_| json!({})),
        created_at: row.get(9)?,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, String> {
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(|err| err.to_string())?);
    }
    Ok(values)
}

fn app_support_dir() -> Result<PathBuf, String> {
    if cfg!(target_os = "macos") {
        let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
        return Ok(PathBuf::from(home).join("Library/Application Support/Capybara"));
    }
    let home = env::var_os("HOME").ok_or_else(|| "HOME is not set".to_string())?;
    Ok(PathBuf::from(home).join(".capybara"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn normalize_config(value: Value) -> Value {
    if value.is_object() { value } else { json!({}) }
}

fn title_from_prompt(prompt: &str) -> String {
    let mut title = prompt
        .split_whitespace()
        .take(10)
        .collect::<Vec<_>>()
        .join(" ");
    if title.chars().count() > 60 {
        title = title.chars().take(60).collect();
    }
    if title.is_empty() {
        "Untitled conversation".to_string()
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::{CreateConversation, CreateRunEvent, Provider, Store};
    use serde_json::json;

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
        let _remove_result = std::fs::remove_file(path);
        Ok(())
    }
}
