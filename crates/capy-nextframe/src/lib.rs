pub mod adapter;
pub mod config;
pub mod doctor;
pub mod error;
pub mod ports;

pub use adapter::binary::BinaryAdapter;
pub use config::{BinaryDiscovery, NextFrameConfig, ResolvedBinary, ResolvedNextFrameConfig};
pub use doctor::{DoctorReport, doctor};
pub use error::{ErrorBody, NextFrameError, NextFrameErrorCode};
pub use ports::{
    CompileReport, CompositionArtifact, ExportOptions, ExportReport, NextFrameProjectPort,
    NextFrameRecorderPort, SnapshotOptions, SnapshotReport, ValidationReport,
};
