use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::TimelineError;
use crate::ports::CompositionArtifact;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotOptions {
    pub t_ms: u64,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOptions {
    pub profile: String,
    pub fps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotReport {
    pub ok: bool,
    pub output: PathBuf,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportReport {
    pub ok: bool,
    pub output: PathBuf,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

pub trait TimelineRecorderPort {
    fn snapshot(
        &self,
        source: &Path,
        out: &Path,
        options: SnapshotOptions,
    ) -> Result<SnapshotReport, TimelineError>;

    fn export(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
        options: ExportOptions,
    ) -> Result<ExportReport, TimelineError>;
}
