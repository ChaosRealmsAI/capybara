use rusqlite::{OptionalExtension, params};

use super::rows::row_to_run;
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
}
