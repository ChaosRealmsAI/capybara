use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NextFrameErrorCode {
    NextframeNotFound,
    NextframeVersionUnsupported,
    PosterInvalid,
    PosterNotFound,
    OutDirWriteFailed,
    CompositionInvalid,
    ComponentMissing,
    ComponentAbiInvalid,
    AssetMissing,
    BrandTokenMissing,
    CompileFailed,
    RecorderValidateFailed,
    SnapshotFailed,
    ExportFailed,
    VerifyExportFailed,
    DesktopHostFailed,
}

impl NextFrameErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NextframeNotFound => "NEXTFRAME_NOT_FOUND",
            Self::NextframeVersionUnsupported => "NEXTFRAME_VERSION_UNSUPPORTED",
            Self::PosterInvalid => "POSTER_INVALID",
            Self::PosterNotFound => "POSTER_NOT_FOUND",
            Self::OutDirWriteFailed => "OUT_DIR_WRITE_FAILED",
            Self::CompositionInvalid => "COMPOSITION_INVALID",
            Self::ComponentMissing => "COMPONENT_MISSING",
            Self::ComponentAbiInvalid => "COMPONENT_ABI_INVALID",
            Self::AssetMissing => "ASSET_MISSING",
            Self::BrandTokenMissing => "BRAND_TOKEN_MISSING",
            Self::CompileFailed => "COMPILE_FAILED",
            Self::RecorderValidateFailed => "RECORDER_VALIDATE_FAILED",
            Self::SnapshotFailed => "SNAPSHOT_FAILED",
            Self::ExportFailed => "EXPORT_FAILED",
            Self::VerifyExportFailed => "VERIFY_EXPORT_FAILED",
            Self::DesktopHostFailed => "DESKTOP_HOST_FAILED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub hint: String,
}

impl ErrorBody {
    pub fn new(
        code: NextFrameErrorCode,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            code: code.as_str().to_string(),
            message: message.into(),
            hint: hint.into(),
        }
    }
}

#[derive(Debug, Error)]
#[error("{body:?}")]
pub struct NextFrameError {
    pub body: ErrorBody,
}

impl NextFrameError {
    pub fn new(
        code: NextFrameErrorCode,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            body: ErrorBody::new(code, message, hint),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(
            NextFrameErrorCode::NextframeNotFound,
            message,
            nextframe_setup_hint(),
        )
    }
}

pub fn nextframe_setup_hint() -> String {
    "run capy nextframe doctor, then rerun with crate adapter inputs available".to_string()
}

#[cfg(test)]
mod tests {
    use super::{ErrorBody, NextFrameErrorCode};

    #[test]
    fn serializes_error_code_as_contract_string() -> Result<(), serde_json::Error> {
        let body = ErrorBody::new(
            NextFrameErrorCode::NextframeNotFound,
            "nf missing",
            "rerun capy nextframe doctor",
        );
        let value = serde_json::to_value(body)?;

        assert_eq!(value["code"], "NEXTFRAME_NOT_FOUND");
        assert_eq!(value["message"], "nf missing");
        assert_eq!(value["hint"], "rerun capy nextframe doctor");
        Ok(())
    }
}
