use rusqlite::{OptionalExtension, params};

use super::rows::row_to_run;
use super::util::now_ms;
use super::{RunRecord, Store};

impl Store {
    pub fn get_run(&self, run_id: &str) -> Result<Option<RunRecord>, String> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, conversation_id, pid, status, started_at, ended_at, error FROM runs WHERE id = ?1",
            params![run_id],
            row_to_run,
        )
        .optional()
        .map_err(|err| err.to_string())
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

    pub fn update_native_session(
        &self,
        conversation_id: &str,
        session_id: &str,
    ) -> Result<(), String> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE conversations SET native_session_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![session_id, now_ms(), conversation_id],
        )
        .map_err(|err| format!("update native session failed: {err}"))?;
        Ok(())
    }
}
