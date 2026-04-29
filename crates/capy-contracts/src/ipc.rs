use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcRequest {
    pub req_id: String,
    pub op: String,
    #[serde(default)]
    pub params: Value,
}

impl IpcRequest {
    pub fn new(req_id: impl Into<String>, op: impl Into<String>, params: Value) -> Self {
        Self {
            req_id: req_id.into(),
            op: op.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcResponse {
    pub req_id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

impl IpcResponse {
    pub fn ok(req_id: impl Into<String>, data: Value) -> Self {
        Self {
            req_id: req_id.into(),
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn validation_error(req_id: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            req_id: req_id.into(),
            ok: false,
            data: None,
            error: Some(json!({
                "error": "validation failed",
                "detail": detail.into(),
                "hint": "run `capy <cmd> --help` for expected format",
                "exit_code": 2
            })),
        }
    }

    pub fn socket_error(
        req_id: impl Into<String>,
        detail: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            req_id: req_id.into(),
            ok: false,
            data: None,
            error: Some(json!({
                "error": "socket failed",
                "detail": detail.into(),
                "hint": hint.into(),
                "exit_code": 1
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IpcRequest, IpcResponse};
    use serde_json::json;

    #[test]
    fn ipc_request_keeps_existing_wire_shape() -> Result<(), serde_json::Error> {
        let request = IpcRequest::new("req-1", "state-query", json!({"key": "app.ready"}));
        let value = serde_json::to_value(&request)?;

        assert_eq!(
            value,
            json!({
                "req_id": "req-1",
                "op": "state-query",
                "params": {"key": "app.ready"}
            })
        );
        Ok(())
    }

    #[test]
    fn validation_error_keeps_existing_error_shape() -> Result<(), serde_json::Error> {
        let response = IpcResponse::validation_error("req-2", "missing key");
        let value = serde_json::to_value(&response)?;

        assert_eq!(value["req_id"], "req-2");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["error"], "validation failed");
        assert_eq!(value["error"]["exit_code"], 2);
        Ok(())
    }
}
