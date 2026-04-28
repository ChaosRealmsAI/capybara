use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::compile::CompileReport;
use crate::export::ExportReport;
use crate::snapshot::SnapshotReport;
use crate::validate::ValidationReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyExportRequest {
    pub composition_path: PathBuf,
    pub out_html: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    pub ok: bool,
    pub trace_id: String,
    pub stage: String,
    pub composition_path: PathBuf,
    pub evidence_root: PathBuf,
    pub evidence_index_html: PathBuf,
    pub stages: VerifyStages,
    pub verdict: String,
    pub duration_ms: u128,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<VerifyError>,
}

impl VerifyReport {
    pub fn new(
        trace_id: String,
        composition_path: PathBuf,
        evidence_root: PathBuf,
        evidence_index_html: PathBuf,
        stages: VerifyStages,
        duration_ms: u128,
        title: String,
    ) -> Self {
        let ok = stages.ok();
        Self {
            ok,
            trace_id,
            stage: "verify-export".to_string(),
            composition_path,
            evidence_root,
            evidence_index_html,
            stages,
            verdict: verdict(ok),
            duration_ms,
            title,
            errors: Vec::new(),
        }
    }

    pub fn push_error(&mut self, error: VerifyError) {
        self.ok = false;
        self.verdict = verdict(false);
        self.errors.push(error);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyStages {
    pub validate: ValidationReport,
    pub compile: CompileReport,
    pub snapshot: SnapshotReport,
    pub export: ExportReport,
}

impl VerifyStages {
    pub fn ok(&self) -> bool {
        self.validate.ok && self.compile.ok && self.snapshot.ok && self.export.ok
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyError {
    pub code: String,
    pub path: String,
    pub message: String,
    pub hint: String,
}

impl VerifyError {
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

impl std::fmt::Display for VerifyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for VerifyError {}

fn verdict(ok: bool) -> String {
    if ok {
        "passed".to_string()
    } else {
        "failed".to_string()
    }
}
