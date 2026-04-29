use std::path::PathBuf;

use serde::Serialize;

use crate::error::TimelineError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimelineConfig {
    pub recorder_bin: Option<PathBuf>,
    pub home: Option<PathBuf>,
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
pub struct ResolvedTimelineConfig {
    pub recorder: ResolvedBinary,
    pub home: Option<PathBuf>,
}

impl TimelineConfig {
    pub fn resolve(&self) -> Result<ResolvedTimelineConfig, TimelineError> {
        let recorder = resolve_binary(self.recorder_bin.clone(), "CAPY_RECORDER", "capy-recorder")?;
        Ok(ResolvedTimelineConfig {
            recorder,
            home: self
                .home
                .clone()
                .or_else(|| std::env::var_os("CAPY_TIMELINE_HOME").map(PathBuf::from)),
        })
    }
}

pub fn resolve_binary(
    flag: Option<PathBuf>,
    env_key: &str,
    path_name: &str,
) -> Result<ResolvedBinary, TimelineError> {
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
                Err(TimelineError::not_found(format!(
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

    use super::{BinaryDiscovery, TimelineConfig, resolve_binary};

    #[test]
    fn resolves_flag_before_any_other_source() -> Result<(), crate::TimelineError> {
        let path = PathBuf::from("/definitely/not/a/real/capy-recorder");
        let resolved = resolve_binary(Some(path.clone()), "CAPY_RECORDER", "capy-recorder")?;

        assert_eq!(resolved.discovery, BinaryDiscovery::Flag);
        assert_eq!(resolved.path, Some(path));
        assert!(!resolved.found);
        Ok(())
    }

    #[test]
    fn missing_path_reports_missing_without_error() -> Result<(), crate::TimelineError> {
        let resolved = resolve_binary(
            None,
            "CAPY_TEST_NO_SUCH_ENV",
            "capy-no-such-recorder-binary",
        )?;

        assert_eq!(resolved.discovery, BinaryDiscovery::Missing);
        assert_eq!(resolved.path, None);
        assert!(!resolved.found);
        Ok(())
    }

    #[test]
    fn explicit_home_is_preserved_in_resolved_config() -> Result<(), crate::TimelineError> {
        let resolved = TimelineConfig {
            recorder_bin: None,
            home: Some(PathBuf::from("/tmp/capy-timeline-test-home")),
        }
        .resolve()?;

        assert_eq!(
            resolved.home,
            Some(PathBuf::from("/tmp/capy-timeline-test-home"))
        );
        Ok(())
    }
}
