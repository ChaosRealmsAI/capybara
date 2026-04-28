use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileCompositionRequest {
    pub composition_path: PathBuf,
    pub strict_binary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompileReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: String,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub render_source_schema: String,
    pub duration_ms: u128,
    pub track_count: usize,
    pub compile_mode: String,
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<CompileError>,
}

impl CompileReport {
    pub fn success(data: CompileSuccess) -> Self {
        Self {
            ok: true,
            trace_id: data.trace_id,
            stage: "compile".to_string(),
            composition_path: data.composition_path,
            render_source_path: data.render_source_path,
            render_source_schema: data.render_source_schema,
            duration_ms: data.duration_ms,
            track_count: data.track_count,
            compile_mode: data.compile_mode.as_str().to_string(),
            warnings: data.warnings,
            errors: Vec::new(),
        }
    }

    pub fn failure(
        trace_id: String,
        composition_path: PathBuf,
        render_source_path: PathBuf,
        duration_ms: u128,
        errors: Vec<CompileError>,
    ) -> Self {
        Self {
            ok: false,
            trace_id,
            stage: "compile".to_string(),
            composition_path,
            render_source_path,
            render_source_schema: String::new(),
            duration_ms,
            track_count: 0,
            compile_mode: String::new(),
            warnings: Vec::new(),
            errors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileSuccess {
    pub trace_id: String,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub render_source_schema: String,
    pub duration_ms: u128,
    pub track_count: usize,
    pub compile_mode: CompileMode,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileMode {
    Binary,
    Crate,
    Embedded,
}

impl CompileMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Crate => "crate",
            Self::Embedded => "embedded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileError {
    pub code: String,
    pub path: String,
    pub message: String,
    pub hint: String,
}

impl CompileError {
    pub fn new(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            path: path.into(),
            message: message.into(),
            hint: hint.into(),
        }
    }
}
