pub mod project;
pub mod recorder;

use std::path::Path;

use crate::config::TimelineConfig;
use crate::error::TimelineError;
use crate::ports::{
    CompileReport, CompositionArtifact, ExportOptions, ExportReport, SnapshotOptions,
    SnapshotReport, TimelineProjectPort, TimelineRecorderPort, ValidationReport,
};

pub use project::ProjectAdapter;
pub use recorder::RecorderAdapter;

#[derive(Debug, Clone)]
pub struct TimelineAdapter {
    project: ProjectAdapter,
    recorder: RecorderAdapter,
}

impl TimelineAdapter {
    pub fn new(config: TimelineConfig) -> Self {
        Self {
            project: ProjectAdapter::new(config),
            recorder: RecorderAdapter,
        }
    }
}

impl Default for TimelineAdapter {
    fn default() -> Self {
        Self::new(TimelineConfig::default())
    }
}

impl TimelineProjectPort for TimelineAdapter {
    fn validate(&self, artifact: &CompositionArtifact) -> Result<ValidationReport, TimelineError> {
        self.project.validate(artifact)
    }

    fn compile(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
    ) -> Result<CompileReport, TimelineError> {
        self.project.compile(artifact, out)
    }
}

impl TimelineRecorderPort for TimelineAdapter {
    fn snapshot(
        &self,
        source: &Path,
        out: &Path,
        options: SnapshotOptions,
    ) -> Result<SnapshotReport, TimelineError> {
        self.recorder.snapshot(source, out, options)
    }

    fn export(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
        options: ExportOptions,
    ) -> Result<ExportReport, TimelineError> {
        self.recorder.export(artifact, out, options)
    }
}
