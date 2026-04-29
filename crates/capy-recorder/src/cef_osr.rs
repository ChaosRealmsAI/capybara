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
mod imp {
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use capy_shell_mac::webview::pump_main_run_loop;
    use capy_shell_mac::IOSurfaceHandle;
    use serde_json::Value;
    use wef::{Browser, BrowserHandler, FuncRegistry, PaintElementType, Settings};

    use crate::cef_osr::CefOsrError;
    use crate::events::{emit, Event};
    use crate::pipeline::h264::PipelineH264_1080p;
    use crate::pipeline::hevc::PipelineHevcMain;
    use crate::pipeline::{
        ColorSpec, OutputStats, PipelineError, RecordOpts, RecordPipeline, VideoCodec,
    };
    use crate::record_loop::{verify_frame_ready, RecordConfig, RecordError};

    const PROGRESS_EVERY: u64 = 30;
    const PAGE_READY_TIMEOUT: Duration = Duration::from_secs(20);
    const FRAME_READY_TIMEOUT: Duration = Duration::from_secs(5);
    const PAINT_TIMEOUT: Duration = Duration::from_secs(5);
    const RUN_LOOP_TICK: Duration = Duration::from_millis(8);

    const DETERMINISTIC_CLOCK_SCRIPT: &str = r#"
(() => {
  if (window.__nf_export_clock_installed) return;
  window.__nf_export_clock_installed = true;
  let now = 0;
  let rafId = 1;
  let rafQueue = [];
  const callbacks = new Map();
  Object.defineProperty(window, "__nf_external_now", {
    configurable: true,
    get() { return now; },
    set(v) { now = Number(v) || 0; }
  });
  window.requestAnimationFrame = function(cb) {
    const id = rafId++;
    callbacks.set(id, cb);
    rafQueue.push(id);
    return id;
  };
  window.cancelAnimationFrame = function(id) {
    callbacks.delete(id);
  };
  Date.now = function() { return Math.round(now); };
  try {
    Object.defineProperty(window.performance, "now", {
      configurable: true,
      value: function() { return now; }
    });
  } catch (_e) {}
  window.__nf_flush_raf = function(rounds) {
    const count = Math.max(1, Number(rounds) || 1);
    for (let pass = 0; pass < count; pass += 1) {
      const q = rafQueue;
      rafQueue = [];
      for (const id of q) {
        const cb = callbacks.get(id);
        callbacks.delete(id);
        if (typeof cb === "function") cb(now);
      }
    }
  };
  window.__nf_sync_document_animations = function(t) {
    try {
      const animations = document.getAnimations ? document.getAnimations({ subtree: true }) : [];
      for (const animation of animations) {
        try {
          animation.pause();
          animation.currentTime = Number(t) || 0;
        } catch (_e) {}
      }
    } catch (_e) {}
  };
  window.__nf_export_prepare_frame = async function(t) {
    const target = Number(t) || 0;
    window.__nf_external_now = target;
    window.__nf_sync_document_animations(target);
    if (typeof window.__nf_seek_export === "function") {
      window.__nf_seek_export(target);
    } else if (window.__nf && typeof window.__nf.seek === "function") {
      await window.__nf.seek(target);
    } else {
      throw new Error("Timeline export seek bridge missing");
    }
    window.__nf_flush_raf(3);
    window.__nf_sync_document_animations(target);
    if (typeof window.__nf_wait_media_export === "function") {
      await window.__nf_wait_media_export(target);
    }
    window.__nf_flush_raf(1);
    const raw = typeof window.__nf_read_seek_export === "function"
      ? window.__nf_read_seek_export()
      : JSON.stringify({ t: target, frameReady: true, seq: 1 });
    const payload = JSON.parse(String(raw || "null"));
    return payload || { t: target, frameReady: true, seq: 1 };
  };
})();
"#;

    enum ActivePipeline {
        H264(PipelineH264_1080p),
        Hevc(PipelineHevcMain),
    }

    impl ActivePipeline {
        fn new(cfg: &RecordConfig) -> Result<Self, PipelineError> {
            let opts = RecordOpts {
                width: cfg.width,
                height: cfg.height,
                fps: cfg.fps,
                bitrate_bps: cfg.bitrate_bps,
                codec: cfg.codec,
                output: cfg.output.clone(),
                color: ColorSpec::BT709_SDR_8bit,
            };
            match cfg.codec {
                VideoCodec::H264 => Ok(Self::H264(PipelineH264_1080p::new(opts)?)),
                VideoCodec::HevcMain8 => Ok(Self::Hevc(PipelineHevcMain::new(opts)?)),
            }
        }

        fn push_frame(
            &mut self,
            surface: IOSurfaceHandle,
            pts_ms: u64,
        ) -> Result<(), PipelineError> {
            match self {
                Self::H264(p) => p.push_frame(surface, pts_ms),
                Self::Hevc(p) => p.push_frame(surface, pts_ms),
            }
        }

        fn finish(self) -> Result<OutputStats, PipelineError> {
            match self {
                Self::H264(p) => p.finish(),
                Self::Hevc(p) => p.finish(),
            }
        }
    }

    #[derive(Clone)]
    struct CefHandler {
        state: Arc<Mutex<CefState>>,
    }

    impl BrowserHandler for CefHandler {
        fn on_created(&mut self) {
            if let Ok(mut state) = self.state.lock() {
                state.created = true;
            }
        }

        fn on_load_end(&mut self, frame: wef::Frame) {
            if frame.is_main() {
                frame.execute_javascript(DETERMINISTIC_CLOCK_SCRIPT);
                if let Ok(mut state) = self.state.lock() {
                    state.loaded = true;
                }
            }
        }

        fn on_load_error(&mut self, _frame: wef::Frame, error_text: &str, failed_url: &str) {
            if let Ok(mut state) = self.state.lock() {
                state.load_error = Some(format!("{error_text}: {failed_url}"));
            }
        }

        fn on_paint(
            &mut self,
            type_: PaintElementType,
            _dirty_rects: &wef::DirtyRects,
            image_buffer: wef::ImageBuffer<'_>,
        ) {
            if type_ != PaintElementType::View {
                return;
            }
            let bytes = image_buffer.into_raw();
            if let Ok(mut state) = self.state.lock() {
                state.paint_seq = state.paint_seq.saturating_add(1);
                state.latest_bgra.clear();
                state.latest_bgra.extend_from_slice(bytes);
            }
        }
    }

    #[derive(Default)]
    struct CefState {
        created: bool,
        loaded: bool,
        load_error: Option<String>,
        paint_seq: u64,
        latest_bgra: Vec<u8>,
        bridge_events: VecDeque<Value>,
    }

    struct CefRuntime {
        #[cfg(target_os = "macos")]
        _loader: wef::FrameworkLoader,
        cache_dir: PathBuf,
    }

    impl CefRuntime {
        fn init() -> Result<Self, CefOsrError> {
            let cache_dir = create_temp_dir("nf-cef-osr").map_err(CefOsrError::Io)?;
            #[cfg(target_os = "macos")]
            let loader = wef::FrameworkLoader::load_in_main()
                .map_err(|err| CefOsrError::Init(err.to_string()))?;

            let mut settings = Settings::new()
                .root_cache_path(path_to_string(&cache_dir)?)
                .cache_path(path_to_string(&cache_dir.join("profile"))?);
            if let Some(helper) = browser_subprocess_path()? {
                settings = settings.browser_subprocess_path(helper);
            }
            wef::init(settings).map_err(|err| CefOsrError::Init(err.to_string()))?;
            Ok(Self {
                #[cfg(target_os = "macos")]
                _loader: loader,
                cache_dir,
            })
        }
    }

    pub fn maybe_run_subprocess() -> Result<bool, CefOsrError> {
        if !std::env::args().any(|arg| arg.starts_with("--type=") || arg == "--type") {
            return Ok(false);
        }
        #[cfg(target_os = "macos")]
        let _sandbox =
            wef::SandboxContext::new().map_err(|err| CefOsrError::Init(err.to_string()))?;
        #[cfg(target_os = "macos")]
        let _loader = wef::FrameworkLoader::load_in_helper()
            .map_err(|err| CefOsrError::Init(err.to_string()))?;
        wef::exec_process().map_err(|err| CefOsrError::Init(err.to_string()))
    }

    impl Drop for CefRuntime {
        fn drop(&mut self) {
            wef::shutdown();
            let _ = std::fs::remove_dir_all(&self.cache_dir);
        }
    }

    struct CefPage {
        browser: Browser,
        state: Arc<Mutex<CefState>>,
        _runtime: CefRuntime,
        width: u32,
        height: u32,
        next_token: u64,
    }

    impl CefPage {
        fn open(bundle: &Path, width: u32, height: u32, fps: u32) -> Result<Self, CefOsrError> {
            let runtime = CefRuntime::init()?;
            let state = Arc::new(Mutex::new(CefState::default()));
            let bridge_state = state.clone();
            let registry = FuncRegistry::builder()
                .register("nfRecorderEvent", move |raw: String| -> bool {
                    if let Ok(value) = serde_json::from_str::<Value>(&raw) {
                        if let Ok(mut state) = bridge_state.lock() {
                            state.bridge_events.push_back(value);
                        }
                    }
                    true
                })
                .build();
            let browser = Browser::builder()
                .size(width, height)
                .device_scale_factor(1.0)
                .frame_rate(fps.clamp(1, 90))
                .url(file_url(bundle)?)
                .handler(CefHandler {
                    state: state.clone(),
                })
                .func_registry(registry)
                .build();

            let mut page = Self {
                browser,
                state,
                _runtime: runtime,
                width,
                height,
                next_token: 1,
            };
            page.wait_for_page_ready()?;
            Ok(page)
        }

        fn wait_for_page_ready(&mut self) -> Result<(), CefOsrError> {
            wait_until(PAGE_READY_TIMEOUT, || {
                if let Ok(state) = self.state.lock() {
                    if let Some(err) = &state.load_error {
                        return Err(CefOsrError::Load(err.clone()));
                    }
                    Ok(state.created && state.loaded)
                } else {
                    Ok(false)
                }
            })?;

            let started = Instant::now();
            let mut last_event = Value::Null;
            while started.elapsed() < PAGE_READY_TIMEOUT {
                let token = self.next_bridge_token();
                let script = format!(
                    r#"(async () => {{
  const payload = {{
    kind: "ready",
    token: {token},
    ready: document.readyState,
    hasSeek: !!window.__nf_seek_export,
    hasPrepare: !!window.__nf_export_prepare_frame,
    hasDuration: !!(window.__nf && window.__nf.getDuration)
  }};
  await window.jsBridge.nfRecorderEvent(JSON.stringify(payload));
}})();"#
                );
                self.execute(&script)?;
                if let Ok(event) =
                    self.wait_bridge_event("ready", token, Duration::from_millis(500))
                {
                    let complete = event.get("ready").and_then(Value::as_str) == Some("complete");
                    let has_seek = event.get("hasSeek").and_then(Value::as_bool) == Some(true);
                    let has_prepare =
                        event.get("hasPrepare").and_then(Value::as_bool) == Some(true);
                    let has_duration =
                        event.get("hasDuration").and_then(Value::as_bool) == Some(true);
                    if complete && has_seek && has_prepare && has_duration {
                        return Ok(());
                    }
                    last_event = event;
                }
            }
            Err(CefOsrError::Load(format!(
                "export bridge not ready: {last_event}"
            )))
        }

        fn duration_ms(&mut self, max_duration_s: u32) -> Result<u64, RecordError> {
            let token = self.next_bridge_token();
            let script = format!(
                r#"(async () => {{
  let value = null;
  try {{
    value = (window.__nf && typeof window.__nf.getDuration === "function") ? window.__nf.getDuration() : null;
  }} catch (e) {{
    await window.jsBridge.nfRecorderEvent(JSON.stringify({{ kind: "duration", token: {token}, error: String(e && e.message || e) }}));
    return;
  }}
  await window.jsBridge.nfRecorderEvent(JSON.stringify({{ kind: "duration", token: {token}, value }}));
}})();"#
            );
            self.execute(&script)?;
            let event = self.wait_bridge_event("duration", token, FRAME_READY_TIMEOUT)?;
            if let Some(error) = event.get("error").and_then(Value::as_str) {
                return Err(RecordError::ShellError(error.to_string()));
            }
            let max_cap_ms = u64::from(max_duration_s).saturating_mul(1000);
            let duration_ms = match js_number_as_u64(event.get("value")) {
                Some(0) | None => max_cap_ms,
                Some(d) => d.min(max_cap_ms),
            };
            Ok(duration_ms)
        }

        fn prepare_frame(&mut self, t_ms: f64) -> Result<Value, RecordError> {
            let token = self.next_bridge_token();
            let script = format!(
                r#"(async () => {{
  try {{
    const payload = await window.__nf_export_prepare_frame({t_ms:.6});
    await window.jsBridge.nfRecorderEvent(JSON.stringify({{ kind: "frame", token: {token}, payload }}));
  }} catch (e) {{
    await window.jsBridge.nfRecorderEvent(JSON.stringify({{ kind: "frame", token: {token}, error: String(e && e.message || e) }}));
  }}
}})();"#
            );
            self.execute(&script)?;
            let event = self.wait_bridge_event("frame", token, FRAME_READY_TIMEOUT)?;
            if let Some(error) = event.get("error").and_then(Value::as_str) {
                return Err(RecordError::ShellError(error.to_string()));
            }
            Ok(event.get("payload").cloned().unwrap_or(Value::Null))
        }

        fn wait_paint_after(&mut self, paint_seq: u64) -> Result<Vec<u8>, CefOsrError> {
            wait_until(PAINT_TIMEOUT, || {
                if let Ok(state) = self.state.lock() {
                    if state.paint_seq > paint_seq
                        && state.latest_bgra.len()
                            == usize::try_from(u64::from(self.width) * u64::from(self.height) * 4)
                                .unwrap_or(usize::MAX)
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            })?;
            let state = self
                .state
                .lock()
                .map_err(|_| CefOsrError::PaintTimeout("state mutex poisoned".into()))?;
            Ok(state.latest_bgra.clone())
        }

        fn paint_seq(&self) -> u64 {
            self.state.lock().map(|state| state.paint_seq).unwrap_or(0)
        }

        fn execute(&self, script: &str) -> Result<(), CefOsrError> {
            let frame = self
                .browser
                .main_frame()
                .ok_or_else(|| CefOsrError::Js("main frame unavailable".into()))?;
            frame.execute_javascript(script);
            Ok(())
        }

        fn wait_bridge_event(
            &self,
            kind: &str,
            token: u64,
            timeout: Duration,
        ) -> Result<Value, CefOsrError> {
            let mut found = None;
            wait_until(timeout, || {
                if let Ok(mut state) = self.state.lock() {
                    if let Some(pos) = state.bridge_events.iter().position(|value| {
                        value.get("kind").and_then(Value::as_str) == Some(kind)
                            && value.get("token").and_then(Value::as_u64) == Some(token)
                    }) {
                        found = state.bridge_events.remove(pos);
                        return Ok(true);
                    }
                }
                Ok(false)
            })?;
            found.ok_or_else(|| CefOsrError::Js(format!("{kind} event missing token={token}")))
        }

        fn next_bridge_token(&mut self) -> u64 {
            let token = self.next_token;
            self.next_token = self.next_token.saturating_add(1);
            token
        }
    }

    pub async fn run(cfg: RecordConfig) -> Result<OutputStats, RecordError> {
        let mut page = CefPage::open(&cfg.bundle, cfg.width, cfg.height, cfg.fps)?;
        let duration_ms = page.duration_ms(cfg.max_duration_s)?;
        if duration_ms == 0 {
            return Err(RecordError::BundleLoadFailed(
                "duration resolves to 0 (check --max-duration and bundle getDuration)".into(),
            ));
        }

        let mut pipeline = ActivePipeline::new(&cfg)?;
        emit(Event::RecordStart {
            bundle: cfg.bundle.display().to_string(),
            out: cfg.output.display().to_string(),
            fps: cfg.fps,
            bitrate_bps: cfg.bitrate_bps,
            viewport: [cfg.width, cfg.height],
        });

        let frame_dur_ms = 1000.0_f64 / f64::from(cfg.fps);
        let total_frames = ((duration_ms as f64) / frame_dur_ms).round() as u64;
        if total_frames == 0 {
            return Err(RecordError::NoFrames);
        }
        let (range_start, range_end) = match cfg.frame_range {
            Some((s, e)) => (s.min(total_frames), e.min(total_frames)),
            None => (0, total_frames),
        };
        if range_end <= range_start {
            return Err(RecordError::NoFrames);
        }

        let mut frames_encoded = 0_u64;
        for seq in range_start..range_end {
            let t_exact_ms = seq as f64 * 1000.0 / f64::from(cfg.fps);
            let t_ms = t_exact_ms.round() as u64;
            let frame_start = Instant::now();
            let paint_seq = page.paint_seq();
            let ready = page.prepare_frame(t_exact_ms)?;
            verify_frame_ready(&ready, t_exact_ms, None)?;
            let bgra = page.wait_paint_after(paint_seq)?;
            let surface = IOSurfaceHandle::from_bgra_bytes(cfg.width, cfg.height, &bgra)
                .map_err(|err| CefOsrError::IOSurface(err.to_string()))?;
            pipeline.push_frame(surface, t_ms)?;
            frames_encoded = frames_encoded.saturating_add(1);

            emit(Event::RecordFrame {
                t_ms,
                t_exact_ms,
                seq,
                encode_ms: frame_start.elapsed().as_secs_f64() * 1000.0,
            });

            if seq > 0 && seq.is_multiple_of(PROGRESS_EVERY) {
                let percent = (seq as f64) / (total_frames as f64) * 100.0;
                emit(Event::RecordEncodeProgress {
                    frames_encoded: seq,
                    total_frames,
                    percent,
                });
            }
        }

        if frames_encoded == 0 {
            return Err(RecordError::NoFrames);
        }
        let stats = pipeline.finish()?;
        emit(Event::RecordDone {
            out: stats.path.clone(),
            duration_ms: stats.duration_ms,
            size_bytes: stats.size_bytes,
            moov_front: stats.moov_front,
        });
        Ok(stats)
    }

    pub async fn probe_duration(cfg: &RecordConfig) -> Result<u64, RecordError> {
        let mut page = CefPage::open(&cfg.bundle, cfg.width, cfg.height, cfg.fps)?;
        page.duration_ms(cfg.max_duration_s)
    }

    pub async fn snapshot_png(
        bundle: &Path,
        t_ms: u64,
        out: &Path,
        width: u32,
        height: u32,
    ) -> Result<(), CefOsrError> {
        let mut page = CefPage::open(bundle, width, height, 30)?;
        let paint_seq = page.paint_seq();
        let ready = page
            .prepare_frame(t_ms as f64)
            .map_err(|err| CefOsrError::Js(err.to_string()))?;
        verify_frame_ready(&ready, t_ms as f64, None)
            .map_err(|err| CefOsrError::Js(err.to_string()))?;
        let bgra = page.wait_paint_after(paint_seq)?;
        let surface = IOSurfaceHandle::from_bgra_bytes(width, height, &bgra)
            .map_err(|err| CefOsrError::IOSurface(err.to_string()))?;
        let png = crate::snapshot::iosurface_to_png(&surface)
            .map_err(|err| CefOsrError::Io(err.to_string()))?;
        std::fs::write(out, png).map_err(|err| CefOsrError::Io(err.to_string()))?;
        Ok(())
    }

    fn wait_until<F>(timeout: Duration, mut predicate: F) -> Result<(), CefOsrError>
    where
        F: FnMut() -> Result<bool, CefOsrError>,
    {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if predicate()? {
                return Ok(());
            }
            pump_main_run_loop(RUN_LOOP_TICK);
        }
        Err(CefOsrError::PaintTimeout(format!(
            "timed out after {}ms",
            timeout.as_millis()
        )))
    }

    fn js_number_as_u64(v: Option<&Value>) -> Option<u64> {
        let v = v?;
        if let Some(u) = v.as_u64() {
            return Some(u);
        }
        if let Some(i) = v.as_i64() {
            if i >= 0 {
                return Some(i as u64);
            }
        }
        if let Some(f) = v.as_f64() {
            if f.is_finite() && f >= 0.0 && f.fract() == 0.0 && f <= u64::MAX as f64 {
                return Some(f as u64);
            }
        }
        None
    }

    fn path_to_string(path: &Path) -> Result<String, CefOsrError> {
        path.to_str()
            .map(str::to_string)
            .ok_or_else(|| CefOsrError::Io(format!("path is not valid UTF-8: {}", path.display())))
    }

    fn browser_subprocess_path() -> Result<Option<String>, CefOsrError> {
        if let Ok(helper) = std::env::var("NF_CEF_HELPER") {
            return Ok(Some(helper));
        }
        let Some(path) = default_macos_helper_path() else {
            return Ok(None);
        };
        Ok(Some(path_to_string(&path)?))
    }

    fn default_macos_helper_path() -> Option<PathBuf> {
        #[cfg(not(target_os = "macos"))]
        {
            return None;
        }

        #[cfg(target_os = "macos")]
        {
            let exe = std::env::current_exe().ok()?;
            let exe_name = exe.file_name()?.to_str()?;
            let contents_dir = exe.parent()?.parent()?;
            if contents_dir.file_name()?.to_str()? != "Contents" {
                return None;
            }
            let helper_name = format!("{exe_name} Helper");
            let helper = contents_dir
                .join("Frameworks")
                .join(format!("{helper_name}.app"))
                .join("Contents")
                .join("MacOS")
                .join(helper_name);
            helper.exists().then_some(helper)
        }
    }

    fn create_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"));
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        std::fs::canonicalize(&dir).map_err(|err| err.to_string())
    }

    fn file_url(path: &Path) -> Result<String, CefOsrError> {
        let abs = std::fs::canonicalize(path).map_err(|err| CefOsrError::Io(err.to_string()))?;
        let raw = abs.to_string_lossy();
        let mut out = String::from("file://");
        for b in raw.as_bytes() {
            match *b {
                b'/' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
                b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => out.push(*b as char),
                other => {
                    out.push('%');
                    out.push_str(&format!("{other:02X}"));
                }
            }
        }
        Ok(out)
    }

    impl From<CefOsrError> for RecordError {
        fn from(err: CefOsrError) -> Self {
            match err {
                CefOsrError::Unavailable(_) => RecordError::UnsupportedPlatform(err.to_string()),
                CefOsrError::Init(_) => RecordError::UnsupportedPlatform(err.to_string()),
                CefOsrError::Load(_) => RecordError::BundleLoadFailed(err.to_string()),
                CefOsrError::Js(_) => RecordError::ShellError(err.to_string()),
                CefOsrError::PaintTimeout(_) => RecordError::FrameReadyTimeout(err.to_string()),
                CefOsrError::IOSurface(_) => RecordError::CARendererInitFailed(err.to_string()),
                CefOsrError::Io(_) => RecordError::PipelineError(err.to_string()),
            }
        }
    }
}

#[cfg(feature = "cef-osr")]
pub use imp::{maybe_run_subprocess, probe_duration, run, snapshot_png};
