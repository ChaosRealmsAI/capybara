//! `snapshot` · product-internal single-frame PNG sampling.
//!
//! Historical: v1.14 T-18 snapshot command.
//!
//! **Why this exists (self-verification rule)**: VP-4 needs pixel-level diff
//! between "mp4 frame at t_ms" and "snapshot at t_ms"; both must come from
//! the **same CARenderer / IOSurface path** — any external tool (playwright /
//! chromium) samples a different pipeline and destroys diff validity.
//!
//! ## Flow (aligned with `record_loop::run`)
//! 1. `MacHeadlessShell::new_headless` at `(width, height)` · `dpr=1`.
//! 2. `load_bundle` · blocks until navigation finished.
//! 3. `callAsync("document.body.dataset.mode='record'; return true;")` — flips
//!    the runtime into record mode (RAF off · determinism on · per ADR-041).
//! 4. `callAsync("return await window.__nf.seek(t_ms);")` — awaits
//!    `{ t, frameReady: true, seq }` (T-12 contract).
//! 5. `shell.snapshot()` → `IOSurfaceHandle` (zero-copy CARenderer sample).
//! 6. `iosurface_to_png` locks BGRA pixels · swaps to RGBA · encodes.
//! 7. Write PNG to `out`.
//!
//! ## Key pitfall (T-12 lesson)
//! `callAsyncJavaScript` bridges JS Numbers back as `f64` (via NSNumber). If
//! we pulled `t` we'd need `as_f64().and_then(|f| if f.fract()==0.0 { Some(f as u64) })`.
//! Here we only care about `frameReady: bool`, so `as_bool()` is enough —
//! but we still validate the contract shape for visibility.

use std::path::Path;

use self::png::{
    rgba_png_looks_black, sample_until_committed, FALLBACK_HEIGHT, FALLBACK_WIDTH,
    MAX_SAFE_SNAPSHOT_PIXELS,
};
use capy_shell_mac::{DesktopShell, MacHeadlessShell, ShellConfig};
use readiness::seek_runtime;

mod png;
mod readiness;

pub(crate) use self::png::iosurface_to_png;

const EXPORT_SEEK_SETTLE: std::time::Duration = std::time::Duration::from_millis(12);

/// Errors returned by `snapshot` · mapped 1-to-1 onto exit codes in `main`.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    /// Shell init / load / snapshot bubble.
    #[error("shell: {0}")]
    Shell(String),
    /// `callAsyncJavaScript` itself failed or timed out.
    #[error("js call: {0}")]
    JsCall(String),
    /// Runtime did not return `frameReady: true` for the requested `t_ms`.
    #[error("frameReady=false at t_ms={t_ms}")]
    FrameNotReady { t_ms: u64 },
    /// Runtime returned a non-object / missing-field payload.
    #[error("frameReady contract violation: {0}")]
    FrameReadyContract(String),
    /// `IOSurfaceLock(ReadOnly)` returned non-zero.
    #[error("IOSurfaceLock failed: {0}")]
    IoSurfaceLock(i32),
    /// `png` encoder failure (buffer write or header).
    #[error("png encode: {0}")]
    PngEncode(String),
    /// Disk write failure.
    #[error("io: {0}")]
    Io(String),
    /// Derived code for `events::Event::Error` emission.
    #[error("bundle load failed: {0}")]
    BundleLoad(String),
}

impl SnapshotError {
    /// Enum-string code for the stdout `error` event.
    #[must_use]
    pub fn code_str(&self) -> &'static str {
        match self {
            Self::Shell(_) => "SHELL_ERROR",
            Self::JsCall(_) => "JS_CALL_FAILED",
            Self::FrameNotReady { .. } => "FRAME_READY_FALSE",
            Self::FrameReadyContract(_) => "FRAME_READY_CONTRACT",
            Self::IoSurfaceLock(_) => "IOSURFACE_LOCK_FAILED",
            Self::PngEncode(_) => "PNG_ENCODE_FAILED",
            Self::Io(_) => "IO_ERROR",
            Self::BundleLoad(_) => "BUNDLE_LOAD_FAILED",
        }
    }

    /// Process exit code · `1` for user error (bundle-load) · `2` for internal.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::BundleLoad(_) => 1,
            _ => 2,
        }
    }
}

/// Snapshot one frame · produces a PNG at `out`.
///
/// Runs on the current thread — the caller (capy-recorder main) uses a
/// `tokio::runtime::Builder::new_current_thread()` runtime so `call_async`
/// can pump the macOS main run loop.
pub async fn snapshot(
    bundle: &Path,
    t_ms: u64,
    out: &Path,
    width: u32,
    height: u32,
) -> Result<(), SnapshotError> {
    let mut png = snapshot_once(bundle, t_ms, width, height).await?;
    let pixel_count = u64::from(width) * u64::from(height);
    if pixel_count > MAX_SAFE_SNAPSHOT_PIXELS
        && rgba_png_looks_black(&png)
        && (width != FALLBACK_WIDTH || height != FALLBACK_HEIGHT)
    {
        png = snapshot_once(bundle, t_ms, FALLBACK_WIDTH, FALLBACK_HEIGHT).await?;
    }

    std::fs::write(out, &png).map_err(|e| SnapshotError::Io(e.to_string()))?;
    Ok(())
}

async fn snapshot_once(
    bundle: &Path,
    t_ms: u64,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, SnapshotError> {
    // 1. Boot headless shell (NSWindow orderOut · WKWebView child · CARenderer sampler).
    let shell = MacHeadlessShell::new_headless(ShellConfig {
        viewport: (width, height),
        device_pixel_ratio: 1.0,
        bundle_url: bundle.to_path_buf(),
    })
    .map_err(|e| SnapshotError::Shell(format!("{e}")))?;

    // 2. Load bundle · blocks until navigation finished (DEFAULT_TIMEOUT = 15s).
    shell
        .load_bundle(bundle)
        .map_err(|e| SnapshotError::BundleLoad(format!("{e}")))?;

    // 3. Flip runtime into record mode (deterministic · RAF off · per ADR-041).
    //    `call_async` returns `serde_json::Value` — `true` confirms the flip.
    let mode_flip = shell
        .call_async("document.body.dataset.mode = 'record'; return true;")
        .await
        .map_err(|e| SnapshotError::JsCall(format!("{e}")))?;
    if mode_flip.as_bool() != Some(true) {
        return Err(SnapshotError::FrameReadyContract(format!(
            "mode flip returned non-true: {mode_flip}"
        )));
    }

    // 4. Seek + await frameReady. Runtime contract: `{ t, frameReady, seq }`.
    let v = seek_runtime(&shell, t_ms).await?;

    // Validate frameReady. Missing / wrong type / false all count as failure.
    let ready = v
        .get("frameReady")
        .and_then(|x| x.as_bool())
        .ok_or_else(|| {
            SnapshotError::FrameReadyContract(format!(
                "missing frameReady at t_ms={t_ms} · got {v}"
            ))
        })?;
    if !ready {
        return Err(SnapshotError::FrameNotReady { t_ms });
    }

    // 4.1 Wait one more visual turn before takeSnapshot. Off-screen WebKit may
    // throttle RAF, so keep a setTimeout fallback.
    let paint_barrier = r#"
      return await new Promise(resolve => {
        let done = false;
        function finish(v) {
          if (done) return;
          done = true;
          resolve(v);
        }
        try {
          if (typeof requestAnimationFrame === 'function') {
            requestAnimationFrame(() => requestAnimationFrame(() => finish('raf')));
          }
        } catch (_e) {}
        setTimeout(() => finish('timeout'), 34);
      });
    "#;
    let _ = shell
        .call_async(paint_barrier)
        .await
        .map_err(|e| SnapshotError::JsCall(format!("paint barrier: {e}")))?;

    // 5. Sample CARenderer → IOSurface (zero-copy · same path as record).
    //
    // WKWebView layer commit is not synchronous with `window.__nf.seek(t)`
    // resolving — WebContent process paints the frame slightly after the JS
    // promise fires · CoreAnimation then commits to the IOSurface the next
    // time we `setLayer / render`. carenderer_sample test converges within
    // ≤ 30 iterations by pumping the run loop + CATransaction flush between
    // `sample` calls (POC-04B observation).
    //
    // In production record_loop this is hidden because 60fps sampling has
    // throwaway first frames until layer commits catch up. For single-frame
    // snapshot we loop until center pixel reads non-transparent or we hit
    // the max budget. Each iteration pumps the main run loop via `call_async`
    // (cheap noop) so WebContent has a chance to deliver its commit.
    let surface = sample_until_committed(&shell).await?;

    // 6. Encode PNG from BGRA pixels (swap → RGBA).
    let png = iosurface_to_png(&surface)?;
    Ok(png)
}
