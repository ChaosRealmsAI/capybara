use std::path::PathBuf;

use serde_json::{Value, json};

use crate::ipc::IpcResponse;

pub(super) fn save_response(req_id: String, params: Value) -> IpcResponse {
    response_from(req_id, save(params))
}

pub(super) fn export_response(req_id: String, params: Value) -> IpcResponse {
    response_from(req_id, export(params))
}

fn save(params: Value) -> Result<Value, String> {
    let document_value = required_document(&params)?;
    let document: capy_poster::PosterDocumentV1 =
        serde_json::from_value(document_value).map_err(|err| err.to_string())?;
    capy_poster::validate_document_v1(&document).map_err(|err| err.to_string())?;
    let path = save_path(&params, &document);
    capy_poster::write_document_json(&path, &document).map_err(|err| err.to_string())?;
    Ok(json!({
        "ok": true,
        "schema": "capy.poster.save.v1",
        "document_id": document.id,
        "path": path.display().to_string()
    }))
}

fn export(params: Value) -> Result<Value, String> {
    let document_value = required_document(&params)?;
    let mut document: capy_poster::PosterDocumentV1 =
        serde_json::from_value(document_value).map_err(|err| err.to_string())?;
    let base_dir = source_base_dir(&params);
    capy_poster::resolve_component_packages(&mut document, &base_dir)
        .map_err(|err| err.to_string())?;
    let out_dir = optional_string(&params, "out_dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_dir().join(slug(&document.id)));
    let formats = formats(&params)?;
    let page = optional_string(&params, "page").filter(|value| value != "all");
    let report = capy_poster::export_document(capy_poster::ExportRequest {
        document,
        out_dir,
        formats,
        page,
    })
    .map_err(|err| err.to_string())?;
    serde_json::to_value(report).map_err(|err| err.to_string())
}

fn source_base_dir(params: &Value) -> PathBuf {
    let Some(path) = optional_string(params, "path") else {
        return default_cwd();
    };
    let resolved = if path.starts_with("/fixtures/") {
        default_cwd().join(path.trim_start_matches('/'))
    } else {
        let raw = PathBuf::from(path);
        if raw.is_absolute() {
            raw
        } else {
            default_cwd().join(raw)
        }
    };
    resolved
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(default_cwd)
}

fn required_document(params: &Value) -> Result<Value, String> {
    params
        .get("document")
        .cloned()
        .ok_or_else(|| "missing required parameter: document".to_string())
}

fn save_path(params: &Value, document: &capy_poster::PosterDocumentV1) -> PathBuf {
    let explicit = optional_string(params, "path");
    let path = explicit
        .filter(|path| !path.starts_with("/fixtures/"))
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_dir().join(format!("{}.json", slug(&document.id))));
    if path.is_absolute() {
        path
    } else {
        default_cwd().join(path)
    }
}

fn formats(params: &Value) -> Result<Vec<capy_poster::ExportFormat>, String> {
    if let Some(list) = params.get("formats").and_then(Value::as_array) {
        return list
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| "formats[] must contain strings".to_string())
                    .and_then(|text| {
                        capy_poster::ExportFormat::parse(text).map_err(|err| err.to_string())
                    })
            })
            .collect();
    }
    let raw =
        optional_string(params, "formats").unwrap_or_else(|| "svg,png,pdf,pptx,json".to_string());
    raw.split(',')
        .map(capy_poster::ExportFormat::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn optional_string(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn default_output_dir() -> PathBuf {
    default_cwd().join("target/capy-poster-workspace")
}

fn default_cwd() -> PathBuf {
    std::env::var_os("CAPY_DEFAULT_CWD")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if slug.trim_matches('-').is_empty() {
        "poster".to_string()
    } else {
        slug
    }
}

fn response_from(req_id: String, result: Result<Value, String>) -> IpcResponse {
    match result {
        Ok(data) => IpcResponse::ok(req_id, data),
        Err(error) => IpcResponse::validation_error(req_id, error),
    }
}
