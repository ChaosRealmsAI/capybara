//! CEF off-screen recorder backend.
//!
//! This backend is the performance path for arbitrary browser-rendered export:
//! CEF renders the page off-screen and hands us full-frame pixel buffers via
//! OSR paint callbacks, avoiding the CDP PNG/base64 round trip.

#[cfg(not(feature = "cef-osr"))]
use std::path::Path;

#[cfg(not(feature = "cef-osr"))]
use crate::pipeline::OutputStats;
#[cfg(not(feature = "cef-osr"))]
use crate::record_loop::{RecordConfig, RecordError};

#[derive(Debug, thiserror::Error)]
pub enum CefOsrError {
    #[error("{0}")]
    Unavailable(String),
    #[cfg(feature = "cef-osr")]
    #[error("CEF init failed: {0}")]
    Init(String),
    #[cfg(feature = "cef-osr")]
    #[error("CEF page load failed: {0}")]
    Load(String),
    #[cfg(feature = "cef-osr")]
    #[error("CEF JavaScript bridge failed: {0}")]
    Js(String),
    #[cfg(feature = "cef-osr")]
    #[error("CEF paint timed out: {0}")]
    PaintTimeout(String),
    #[cfg(feature = "cef-osr")]
    #[error("IOSurface bridge failed: {0}")]
    IOSurface(String),
    #[cfg(feature = "cef-osr")]
    #[error("io: {0}")]
    Io(String),
}

#[cfg(not(feature = "cef-osr"))]
pub async fn run(_cfg: RecordConfig) -> Result<OutputStats, RecordError> {
    Err(disabled_record_error())
}

#[cfg(not(feature = "cef-osr"))]
pub async fn probe_duration(_cfg: &RecordConfig) -> Result<u64, RecordError> {
    Err(disabled_record_error())
}

#[cfg(not(feature = "cef-osr"))]
pub async fn snapshot_png(
    _bundle: &Path,
    _t_ms: u64,
    _out: &Path,
    _width: u32,
    _height: u32,
) -> Result<(), CefOsrError> {
    Err(disabled_error())
}

#[cfg(not(feature = "cef-osr"))]
pub fn maybe_run_subprocess() -> Result<bool, CefOsrError> {
    Ok(false)
}

#[cfg(not(feature = "cef-osr"))]
fn disabled_record_error() -> RecordError {
    RecordError::UnsupportedPlatform(disabled_error().to_string())
}

#[cfg(not(feature = "cef-osr"))]
fn disabled_error() -> CefOsrError {
    CefOsrError::Unavailable(
        "cef-osr backend requires building capy-recorder with `--features cef-osr` and a CEF runtime at CEF_ROOT or ~/.cef".into(),
    )
}

#[cfg(feature = "cef-osr")]
mod imp;

#[cfg(feature = "cef-osr")]
pub use imp::{maybe_run_subprocess, probe_duration, run, snapshot_png};
