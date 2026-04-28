pub mod adapter;
pub mod compile;
pub mod compose;
pub mod config;
pub mod doctor;
pub mod error;
pub mod export;
pub mod ports;
pub mod snapshot;
pub mod validate;

pub use adapter::binary::BinaryAdapter;
pub use compile::{CompileCompositionRequest, CompileError, CompileReport, compile_composition};
pub use compose::{
    ComposePosterFailure, ComposePosterRequest, ComposePosterResult, compose_poster,
};
pub use config::{BinaryDiscovery, NextFrameConfig, ResolvedBinary, ResolvedNextFrameConfig};
pub use doctor::{DoctorReport, doctor};
pub use error::{ErrorBody, NextFrameError, NextFrameErrorCode};
pub use export::{ExportCompositionRequest, ExportError, ExportKind, export_composition};
pub use ports::{
    CompositionArtifact, ExportOptions, ExportReport, NextFrameProjectPort, NextFrameRecorderPort,
    SnapshotOptions, SnapshotReport, ValidationReport,
};
pub use validate::{ValidateCompositionRequest, validate_composition};
