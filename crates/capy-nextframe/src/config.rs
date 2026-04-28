use std::path::PathBuf;

use serde::Serialize;

use crate::error::{NextFrameError, NextFrameErrorCode};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NextFrameConfig {
    pub nf_bin: Option<PathBuf>,
    pub recorder_bin: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub mode: Option<NextFrameMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NextFrameMode {
    Crate,
    Binary,
}

impl NextFrameMode {
    pub fn resolve(flag: Option<Self>) -> Result<Self, NextFrameError> {
        if let Some(mode) = flag {
            return Ok(mode);
        }
        match std::env::var("CAPY_NEXTFRAME_MODE") {
            Ok(raw) => Self::parse(&raw),
            Err(std::env::VarError::NotPresent) => Ok(Self::Crate),
            Err(err) => Err(NextFrameError::new(
                NextFrameErrorCode::NextframeVersionUnsupported,
                format!("read CAPY_NEXTFRAME_MODE failed: {err}"),
                "next step · set CAPY_NEXTFRAME_MODE=crate or CAPY_NEXTFRAME_MODE=binary",
            )),
        }
    }

    pub fn parse(raw: &str) -> Result<Self, NextFrameError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "crate" => Ok(Self::Crate),
            "binary" => Ok(Self::Binary),
            value => Err(NextFrameError::new(
                NextFrameErrorCode::NextframeVersionUnsupported,
                format!("unsupported CAPY_NEXTFRAME_MODE: {value}"),
                "next step · set CAPY_NEXTFRAME_MODE=crate or CAPY_NEXTFRAME_MODE=binary",
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crate => "crate",
            Self::Binary => "binary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BinaryDiscovery {
    Flag,
    Env,
    Path,
    Missing,
}

impl BinaryDiscovery {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "FLAG",
            Self::Env => "ENV",
            Self::Path => "PATH",
            Self::Missing => "MISSING",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedBinary {
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub discovery: BinaryDiscovery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNextFrameConfig {
    pub nf: ResolvedBinary,
    pub recorder: ResolvedBinary,
    pub home: Option<PathBuf>,
    pub mode: NextFrameMode,
}

impl NextFrameConfig {
    pub fn resolve(&self) -> Result<ResolvedNextFrameConfig, NextFrameError> {
        let mode = NextFrameMode::resolve(self.mode)?;
        let nf = resolve_binary(self.nf_bin.clone(), "CAPY_NF", "nf")?;
        let recorder =
            resolve_binary(self.recorder_bin.clone(), "CAPY_NF_RECORDER", "nf-recorder")?;
        Ok(ResolvedNextFrameConfig {
            nf,
            recorder,
            home: self
                .home
                .clone()
                .or_else(|| std::env::var_os("CAPY_NEXTFRAME_HOME").map(PathBuf::from)),
            mode,
        })
    }
}

pub fn resolve_binary(
    flag: Option<PathBuf>,
    env_key: &str,
    path_name: &str,
) -> Result<ResolvedBinary, NextFrameError> {
    if let Some(path) = flag {
        return Ok(binary_from_path(path, BinaryDiscovery::Flag));
    }
    if let Some(path) = std::env::var_os(env_key).map(PathBuf::from) {
        return Ok(binary_from_path(path, BinaryDiscovery::Env));
    }
    match which::which(path_name) {
        Ok(path) => Ok(ResolvedBinary {
            found: true,
            path: Some(path),
            version: None,
            discovery: BinaryDiscovery::Path,
        }),
        Err(err) => {
            if matches!(err, which::Error::CannotFindBinaryPath) {
                Ok(ResolvedBinary {
                    found: false,
                    path: None,
                    version: None,
                    discovery: BinaryDiscovery::Missing,
                })
            } else {
                Err(NextFrameError::not_found(format!(
                    "resolve {path_name} failed: {err}"
                )))
            }
        }
    }
}

fn binary_from_path(path: PathBuf, discovery: BinaryDiscovery) -> ResolvedBinary {
    ResolvedBinary {
        found: path.is_file(),
        path: Some(path),
        version: None,
        discovery,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{BinaryDiscovery, NextFrameConfig, NextFrameMode, resolve_binary};

    #[test]
    fn resolves_flag_before_any_other_source() -> Result<(), crate::NextFrameError> {
        let path = PathBuf::from("/definitely/not/a/real/nf");
        let resolved = resolve_binary(Some(path.clone()), "CAPY_NF", "nf")?;

        assert_eq!(resolved.discovery, BinaryDiscovery::Flag);
        assert_eq!(resolved.path, Some(path));
        assert!(!resolved.found);
        Ok(())
    }

    #[test]
    fn missing_path_reports_missing_without_error() -> Result<(), crate::NextFrameError> {
        let resolved = resolve_binary(None, "CAPY_TEST_NO_SUCH_ENV", "capy-no-such-nf-binary")?;

        assert_eq!(resolved.discovery, BinaryDiscovery::Missing);
        assert_eq!(resolved.path, None);
        assert!(!resolved.found);
        Ok(())
    }

    #[test]
    fn explicit_home_is_preserved_in_resolved_config() -> Result<(), crate::NextFrameError> {
        let resolved = NextFrameConfig {
            nf_bin: None,
            recorder_bin: None,
            home: Some(PathBuf::from("/tmp/capy-nextframe-test-home")),
            mode: None,
        }
        .resolve()?;

        assert_eq!(
            resolved.home,
            Some(PathBuf::from("/tmp/capy-nextframe-test-home"))
        );
        Ok(())
    }

    #[test]
    fn parses_empty_mode_as_crate() -> Result<(), crate::NextFrameError> {
        let mode = NextFrameMode::parse("")?;

        assert_eq!(mode, NextFrameMode::Crate);
        Ok(())
    }

    #[test]
    fn parses_binary_mode() -> Result<(), crate::NextFrameError> {
        let mode = NextFrameMode::parse("binary")?;

        assert_eq!(mode, NextFrameMode::Binary);
        Ok(())
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = match NextFrameMode::parse("process") {
            Ok(_) => return,
            Err(err) => err,
        };

        assert_eq!(err.body.code, "NEXTFRAME_VERSION_UNSUPPORTED");
    }
}
