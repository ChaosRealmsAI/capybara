pub mod adapter;
mod clip_first;
pub mod report;
pub mod structural;

use std::path::PathBuf;

pub use report::{BinaryPassthroughResult, ValidationError, ValidationReport, ValidationWarning};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateCompositionRequest {
    pub composition_path: PathBuf,
}

pub fn validate_composition(req: ValidateCompositionRequest) -> ValidationReport {
    let mut report = structural::validate_structure(&req.composition_path);
    adapter::append_crate_passthrough(&mut report);
    report.refresh_ok();
    report
}
