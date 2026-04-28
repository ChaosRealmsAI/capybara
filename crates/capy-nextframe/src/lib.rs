pub mod adapter;
pub mod compose;
pub mod config;
pub mod doctor;
pub mod error;
pub mod ports;
pub mod validate;

pub use adapter::binary::BinaryAdapter;
pub use compose::{
    ComposePosterFailure, ComposePosterRequest, ComposePosterResult, compose_poster,
};
pub use config::{BinaryDiscovery, NextFrameConfig, ResolvedBinary, ResolvedNextFrameConfig};
pub use doctor::{DoctorReport, doctor};
pub use error::{ErrorBody, NextFrameError, NextFrameErrorCode};
pub use ports::{
    CompileReport, CompositionArtifact, ExportOptions, ExportReport, NextFrameProjectPort,
    NextFrameRecorderPort, SnapshotOptions, SnapshotReport, ValidationReport,
};
pub use validate::{ValidateCompositionRequest, validate_composition};
