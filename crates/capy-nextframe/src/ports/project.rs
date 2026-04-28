use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::NextFrameError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionArtifact {
    pub project_slug: String,
    pub composition_id: String,
    pub project_root: PathBuf,
    pub composition_path: PathBuf,
    pub component_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub ok: bool,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompileReport {
    pub ok: bool,
    pub output: PathBuf,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

pub trait NextFrameProjectPort {
    fn validate(&self, artifact: &CompositionArtifact) -> Result<ValidationReport, NextFrameError>;
    fn compile(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
    ) -> Result<CompileReport, NextFrameError>;
}
