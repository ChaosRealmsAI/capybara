use serde_json::json;

use super::{Conversation, Message, Provider, RunEvent, RunRecord};

pub(super) fn row_to_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
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

pub(super) fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
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

pub(super) fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
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

pub(super) fn row_to_run_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunEvent> {
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

pub(super) fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, String> {
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(|err| err.to_string())?);
    }
    Ok(values)
}
