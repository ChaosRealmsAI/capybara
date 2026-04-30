use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportCompositionRequest {
    pub composition_path: PathBuf,
    pub kind: ExportKind,
    pub out: Option<PathBuf>,
    pub fps: u32,
    pub profile: String,
    pub resolution: Option<String>,
    pub parallel: Option<usize>,
    pub strict_recorder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
    Mp4,
}

impl ExportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportStatus {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

impl ExportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: String,
    pub job_id: String,
    pub status: ExportStatus,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub output_path: PathBuf,
    pub kind: String,
    pub duration_ms: u64,
    pub fps: u32,
    pub profile: String,
    pub resolution: String,
    pub parallel: usize,
    pub strict_recorder: bool,
    pub frame_count: u64,
    pub byte_size: u64,
    pub export_mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ExportError>,
}

impl ExportReport {
    pub fn success(data: ExportSuccess) -> Self {
        Self {
            ok: true,
            trace_id: data.trace_id,
            stage: "export".to_string(),
            job_id: data.job_id,
            status: ExportStatus::Done,
            composition_path: data.composition_path,
            render_source_path: data.render_source_path,
            output_path: data.output_path,
            kind: data.kind.as_str().to_string(),
            duration_ms: data.duration_ms,
            fps: data.fps,
            profile: data.profile,
            resolution: data.resolution,
            parallel: data.parallel,
            strict_recorder: data.strict_recorder,
            frame_count: data.frame_count,
            byte_size: data.byte_size,
            export_mode: data.export_mode.as_str().to_string(),
            errors: Vec::new(),
        }
    }

    pub fn failure(data: ExportFailure) -> Self {
        Self {
            ok: false,
            trace_id: data.trace_id,
            stage: "export".to_string(),
            job_id: data.job_id,
            status: ExportStatus::Failed,
            composition_path: data.composition_path,
            render_source_path: data.render_source_path,
            output_path: data.output_path,
            kind: data.kind.as_str().to_string(),
            duration_ms: data.duration_ms,
            fps: data.fps,
            profile: data.profile,
            resolution: data.resolution,
            parallel: data.parallel,
            strict_recorder: data.strict_recorder,
            frame_count: data.frame_count,
            byte_size: 0,
            export_mode: data
                .export_mode
                .map(ExportMode::as_str)
                .unwrap_or("")
                .to_string(),
            errors: data.errors,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSuccess {
    pub trace_id: String,
    pub job_id: String,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub output_path: PathBuf,
    pub kind: ExportKind,
    pub duration_ms: u64,
    pub fps: u32,
    pub profile: String,
    pub resolution: String,
    pub parallel: usize,
    pub strict_recorder: bool,
    pub frame_count: u64,
    pub byte_size: u64,
    pub export_mode: ExportMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFailure {
    pub trace_id: String,
    pub job_id: String,
    pub composition_path: PathBuf,
    pub render_source_path: PathBuf,
    pub output_path: PathBuf,
    pub kind: ExportKind,
    pub duration_ms: u64,
    pub fps: u32,
    pub profile: String,
    pub resolution: String,
    pub parallel: usize,
    pub strict_recorder: bool,
    pub frame_count: u64,
    pub export_mode: Option<ExportMode>,
    pub errors: Vec<ExportError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    Crate,
    Embedded,
}

impl ExportMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crate => "crate",
            Self::Embedded => "embedded",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportError {
    pub code: String,
    pub path: String,
    pub message: String,
    pub hint: String,
}

impl ExportError {
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

impl std::fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ExportError {}

#[cfg(test)]
mod tests {
    use super::ExportMode;

    #[test]
    fn serializes_crate_export_mode() {
        assert_eq!(ExportMode::Crate.as_str(), "crate");
    }
}
