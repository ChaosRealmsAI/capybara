use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRequest {
    pub composition_path: PathBuf,
    pub frame_ms: u64,
    pub out: Option<PathBuf>,
    pub strict_binary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: String,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub frame_ms: u64,
    pub snapshot_mode: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<SnapshotError>,
}

impl SnapshotReport {
    pub fn success(data: SnapshotSuccess) -> Self {
        Self {
            ok: true,
            trace_id: data.trace_id,
            stage: "snapshot".to_string(),
            composition_path: data.composition_path,
            render_source_path: data.render_source_path,
            snapshot_path: data.snapshot_path,
            frame_ms: data.frame_ms,
            snapshot_mode: data.snapshot_mode.as_str().to_string(),
            width: data.width,
            height: data.height,
            byte_size: data.byte_size,
            errors: Vec::new(),
        }
    }

    pub fn failure(data: SnapshotFailure) -> Self {
        Self {
            ok: false,
            trace_id: data.trace_id,
            stage: "snapshot".to_string(),
            composition_path: data.composition_path,
            render_source_path: data.render_source_path,
            snapshot_path: data.snapshot_path,
            frame_ms: data.frame_ms,
            snapshot_mode: String::new(),
            width: 0,
            height: 0,
            byte_size: 0,
            errors: data.errors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotSuccess {
    pub trace_id: String,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub frame_ms: u64,
    pub snapshot_mode: SnapshotMode,
    pub width: u32,
    pub height: u32,
    pub byte_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotFailure {
    pub trace_id: String,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub frame_ms: u64,
    pub errors: Vec<SnapshotError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotMode {
    Binary,
    Embedded,
}

impl SnapshotMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Binary => "binary",
            Self::Embedded => "embedded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotError {
    pub code: String,
    pub path: String,
    pub message: String,
    pub hint: String,
}

impl SnapshotError {
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

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SnapshotError {}
