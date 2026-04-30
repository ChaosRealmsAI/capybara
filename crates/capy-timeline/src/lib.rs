pub mod adapters;
pub mod asset;
pub mod brand;
pub mod compile;
pub mod compose;
pub mod config;
pub mod doctor;
pub mod error;
pub mod export;
pub mod ports;
pub mod snapshot;
pub mod validate;
pub mod verify;
mod video_source;

pub use adapters::timeline_engine::TimelineAdapter;
pub use brand::{RebuildReport, RebuildRequest, rebuild};
pub use compile::{CompileCompositionRequest, CompileError, CompileReport, compile_composition};
pub use compose::{
    ComposePosterFailure, ComposePosterRequest, ComposePosterResult, compose_poster,
};
pub use config::{BinaryDiscovery, ResolvedBinary, ResolvedTimelineConfig, TimelineConfig};
pub use doctor::{DoctorReport, doctor};
pub use error::{ErrorBody, TimelineError, TimelineErrorCode};
pub use export::{ExportCompositionRequest, ExportError, ExportKind, export_composition};
pub use ports::{
    CompositionArtifact, ExportOptions, ExportReport, SnapshotOptions, SnapshotReport,
    TimelineProjectPort, TimelineRecorderPort, ValidationReport,
};
pub use validate::{ValidateCompositionRequest, validate_composition};
pub use verify::{VerifyError, VerifyExportRequest, VerifyReport, verify_export};
