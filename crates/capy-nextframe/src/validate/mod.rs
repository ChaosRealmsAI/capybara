pub mod adapter;
pub mod report;
pub mod structural;

use std::path::PathBuf;

pub use report::{BinaryPassthroughResult, ValidationError, ValidationReport, ValidationWarning};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateCompositionRequest {
    pub composition_path: PathBuf,
    pub strict_binary: bool,
}

pub fn validate_composition(req: ValidateCompositionRequest) -> ValidationReport {
    let mut report = structural::validate_structure(&req.composition_path);
    adapter::append_binary_passthrough(&mut report, req.strict_binary);
    report.refresh_ok();
    report
}
