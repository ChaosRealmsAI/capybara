use std::fs;
use std::path::Path;

use capy_poster::{PosterDocument, PosterError, validate_document};
use serde_json::Value;

use crate::error::{NextFrameError, NextFrameErrorCode};

#[derive(Debug, Clone)]
pub struct PosterInput {
    pub document: PosterDocument,
    pub raw: Value,
}

impl PosterInput {
    pub fn id(&self) -> Option<&str> {
        string_field(&self.raw, "id")
    }

    pub fn title(&self) -> Option<&str> {
        string_field(&self.raw, "title")
    }
}

pub fn read_poster(path: &Path) -> Result<PosterInput, NextFrameError> {
    let text = fs::read_to_string(path).map_err(|err| {
        let code = if err.kind() == std::io::ErrorKind::NotFound {
            NextFrameErrorCode::PosterNotFound
        } else {
            NextFrameErrorCode::PosterInvalid
        };
        NextFrameError::new(
            code,
            format!("read poster {} failed: {err}", path.display()),
            format!(
                "next step · verify --input points to a readable Poster JSON: {}",
                path.display()
            ),
        )
    })?;
    let raw: Value = serde_json::from_str(&text).map_err(invalid_json_error)?;
    let document: PosterDocument =
        serde_json::from_value(raw.clone()).map_err(invalid_json_error)?;
    validate_document(&document).map_err(invalid_poster_error)?;
    Ok(PosterInput { document, raw })
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn invalid_json_error(err: serde_json::Error) -> NextFrameError {
    NextFrameError::new(
        NextFrameErrorCode::PosterInvalid,
        format!("poster JSON is invalid: {err}"),
        "next step · run capy poster validate --input <poster.json>",
    )
}

fn invalid_poster_error(err: PosterError) -> NextFrameError {
    NextFrameError::new(
        NextFrameErrorCode::PosterInvalid,
        format!("poster document is invalid: {err}"),
        "next step · run capy poster validate --input <poster.json>",
    )
}
