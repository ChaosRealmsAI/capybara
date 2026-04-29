use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimelineErrorCode {
    TimelineNotFound,
    TimelineVersionUnsupported,
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

impl TimelineErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TimelineNotFound => "TIMELINE_NOT_FOUND",
            Self::TimelineVersionUnsupported => "TIMELINE_VERSION_UNSUPPORTED",
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
        code: TimelineErrorCode,
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
pub struct TimelineError {
    pub body: ErrorBody,
}

impl TimelineError {
    pub fn new(
        code: TimelineErrorCode,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            body: ErrorBody::new(code, message, hint),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(
            TimelineErrorCode::TimelineNotFound,
            message,
            timeline_setup_hint(),
        )
    }
}

pub fn timeline_setup_hint() -> String {
    "run capy timeline doctor, then rerun with crate adapter inputs available".to_string()
}

#[cfg(test)]
mod tests {
    use super::{ErrorBody, TimelineErrorCode};

    #[test]
    fn serializes_error_code_as_contract_string() -> Result<(), serde_json::Error> {
        let body = ErrorBody::new(
            TimelineErrorCode::TimelineNotFound,
            "timeline engine missing",
            "rerun capy timeline doctor",
        );
        let value = serde_json::to_value(body)?;

        assert_eq!(value["code"], "TIMELINE_NOT_FOUND");
        assert_eq!(value["message"], "timeline engine missing");
        assert_eq!(value["hint"], "rerun capy timeline doctor");
        Ok(())
    }
}
