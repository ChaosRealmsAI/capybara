use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, ChildStdout};

use serde_json::Value;

pub(super) fn send_json(stdin: &mut ChildStdin, value: Value) -> Result<(), String> {
    let payload = serde_json::to_string(&value).map_err(|err| err.to_string())?;
    stdin
        .write_all(payload.as_bytes())
        .map_err(|err| format!("write JSON-RPC failed: {err}"))?;
    stdin
        .write_all(b"\n")
        .map_err(|err| format!("write JSON-RPC newline failed: {err}"))?;
    stdin
        .flush()
        .map_err(|err| format!("flush JSON-RPC failed: {err}"))
}

pub(super) fn read_until_response(
    reader: &mut BufReader<ChildStdout>,
    id: i64,
) -> Result<Value, String> {
    loop {
        let Some(value) = read_json_line(reader)? else {
            return Err(format!("codex app-server closed before response id {id}"));
        };
        if value.get("id").and_then(Value::as_i64) == Some(id) {
            if let Some(error) = value.get("error") {
                return Err(format!("codex response error: {error}"));
            }
            return Ok(value);
        }
    }
}

pub(super) fn read_json_line(reader: &mut BufReader<ChildStdout>) -> Result<Option<Value>, String> {
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .map_err(|err| format!("read JSON-RPC failed: {err}"))?;
    if bytes == 0 {
        return Ok(None);
    }
    serde_json::from_str::<Value>(line.trim_end())
        .map(Some)
        .map_err(|err| format!("invalid JSON-RPC line: {err}"))
}
