//! High-level export API · 从 source.json 直接产 MP4。
//!
//! Historical: v1.44 high-level export API.
//!
//! 架构(ADR-064):
//! - 输入:source.json 路径 + 输出 MP4 路径 + 持续时长(秒)
//! - 内部:读 source.json + include 的 runtime-iife.js 拼一个自包含 HTML ·
//!   写 tmp · 走 CEF OSR + VideoToolbox pipeline
//! - 输出:MP4 + OutputStats(主调方拿来 log / verify)
//!
//! 一致性保证:recorder 跑的是跟 preview 同一份 `nf-runtime/dist/runtime-iife.js` ·
//! source.json 也是同一份 · runtime 按 ADR-045 t 纯驱动 · 同 t 同输出。
//! recorder 的正式导出底座固定为 CEF OSR；桌面预览仍由 nf-shell 负责。

use std::path::{Path, PathBuf};

use crate::cef_osr;
use crate::orchestrator;
use crate::pipeline::VideoCodec;
use crate::record_loop::{RecordConfig, RecordError};
use crate::snapshot::SnapshotError;
use crate::OutputStats;

/// nf-runtime 浏览器端 IIFE 产物 · 编译时 inline · 跟 nf-shell preview 同源。
/// Historical: v1.44 runtime IIFE export source.
const RUNTIME_IIFE: &str = include_str!("../assets/runtime/runtime-iife.js");

/// 7 个官方 track 的 JS 源 · 编译时 inline · 喂给 runtime 解析
/// `__NF_SOURCE__.tracks[].kind` 定位到对应代码。跟 nf-shell preview 同源。
/// Historical: v1.44 official track source embedding.
const TRACK_BG: &str = include_str!("../assets/tracks/official/bg.js");
const TRACK_SCENE: &str = include_str!("../assets/tracks/official/scene.js");
const TRACK_VIDEO: &str = include_str!("../assets/tracks/official/video.js");
const TRACK_AUDIO: &str = include_str!("../assets/tracks/official/audio.js");
const TRACK_CHART: &str = include_str!("../assets/tracks/official/chart.js");
const TRACK_DATA: &str = include_str!("../assets/tracks/official/data.js");
const TRACK_SUBTITLE: &str = include_str!("../assets/tracks/official/subtitle.js");
const TRACK_TEXT: &str = include_str!("../assets/tracks/official/text.js");
const TRACK_OVERLAY: &str = include_str!("../assets/tracks/official/overlay.js");
const TRACK_COMPONENT: &str = include_str!("../assets/tracks/official/component.js");
const TRACK_WEBGL_PARTICLES: &str = include_str!("../assets/tracks/community/webgl-particles.js");
const RENDER_SOURCE_SCHEMA_VERSION: &str = "capy.timeline.render_source.v1";

fn track_source_for(kind: &str) -> Option<&'static str> {
    match kind {
        "bg" => Some(TRACK_BG),
        "scene" => Some(TRACK_SCENE),
        "video" => Some(TRACK_VIDEO),
        "audio" => Some(TRACK_AUDIO),
        "chart" => Some(TRACK_CHART),
        "data" => Some(TRACK_DATA),
        "subtitle" => Some(TRACK_SUBTITLE),
        "text" => Some(TRACK_TEXT),
        "overlay" => Some(TRACK_OVERLAY),
        "component" => Some(TRACK_COMPONENT),
        "webgl-particles" => Some(TRACK_WEBGL_PARTICLES),
        _ => None,
    }
}

fn build_tracks_map_json(source_json: &serde_json::Value) -> String {
    use serde_json::{Map, Value};
    let mut map: Map<String, Value> = Map::new();
    if let Some(tracks) = source_json.get("tracks").and_then(|v| v.as_array()) {
        for tr in tracks {
            let Some(kind) = tr.get("kind").and_then(|v| v.as_str()) else {
                continue;
            };
            if map.contains_key(kind) {
                continue;
            }
            if let Some(src) = track_source_for(kind) {
                map.insert(kind.to_string(), Value::String(src.to_string()));
            }
        }
    }
    serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
}

/// Export 参数 · lib 层封装 · 主调方(nf-shell)不用管内部 `RecordConfig` 细节。
#[derive(Debug, Clone)]
pub struct ExportOpts {
    /// 持续时长(秒)· `>0.0` 必需。
    pub duration_s: f64,
    /// Viewport(宽, 高)· 默认 (1920, 1080)。
    pub viewport: (u32, u32),
    /// 帧率 · ∈ {30, 60} · 默认 60。
    pub fps: u32,
    /// VideoToolbox 目标比特率(bps)· 默认 20Mbps。
    pub bitrate_bps: u32,
    /// 并行切片 N(ADR-061) · `None` = 按分辨率取默认
    /// (1080p=1 / 4k=2) · `Some(1)` 可显式强制串行。
    /// duration < 6s 时 orchestrator 自动降级单进程(segment boot 开销吃掉收益)。
    /// Historical: v1.44.1 / v1.56 parallel slicing.
    pub parallel: Option<usize>,
    /// CLI `--resolution` 覆盖 `source.json meta.export.resolution`。
    /// Historical: v1.55 resolution override.
    pub resolution_override: Option<ExportResolution>,
}

impl Default for ExportOpts {
    fn default() -> Self {
        Self {
            duration_s: 5.0,
            viewport: (1920, 1080),
            fps: 60,
            bitrate_bps: 20_000_000,
            parallel: None,
            resolution_override: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportResolution {
    P720,
    P1080,
    K4,
}

impl ExportResolution {
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "720p" => Some(Self::P720),
            "1080p" => Some(Self::P1080),
            "4k" => Some(Self::K4),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::K4 => "4k",
        }
    }

    pub fn viewport(self) -> (u32, u32) {
        match self {
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::K4 => (3840, 2160),
        }
    }

    pub fn bitrate_bps(self) -> u32 {
        match self {
            Self::P720 => 8_000_000,
            Self::P1080 => 20_000_000,
            Self::K4 => 80_000_000,
        }
    }

    pub fn codec(self) -> VideoCodec {
        match self {
            // Keep the default 1080p path on H.264 for regression parity.
            Self::P720 => VideoCodec::H264,
            Self::P1080 => VideoCodec::H264,
            Self::K4 => VideoCodec::HevcMain8,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedExportPreset {
    viewport: (u32, u32),
    bitrate_bps: u32,
    codec: VideoCodec,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RenderSourceSummary {
    pub schema_version: String,
    pub duration_ms: u64,
    pub viewport: (u32, u32),
    pub tracks: usize,
    pub clips: usize,
    pub components: usize,
    pub visual_tracks: usize,
    pub audio_tracks: usize,
    pub background: String,
    pub warnings: Vec<String>,
}

pub fn validate_render_source_file(source_path: &Path) -> Result<RenderSourceSummary, String> {
    let source_text = std::fs::read_to_string(source_path)
        .map_err(|err| format!("read source {}: {err}", source_path.display()))?;
    let source_json: serde_json::Value =
        serde_json::from_str(&source_text).map_err(|err| format!("source JSON: {err}"))?;
    validate_render_source(&source_json)
}

pub fn validate_render_source(
    source_json: &serde_json::Value,
) -> Result<RenderSourceSummary, String> {
    let root = source_json
        .as_object()
        .ok_or_else(|| "render source must be a JSON object".to_string())?;
    let schema_version = root
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "schema_version is required".to_string())?;
    if schema_version != RENDER_SOURCE_SCHEMA_VERSION {
        return Err(format!(
            "schema_version must be {RENDER_SOURCE_SCHEMA_VERSION} (got {schema_version})"
        ));
    }
    let duration_ms = root
        .get("duration_ms")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "duration_ms is required".to_string())?;
    if duration_ms == 0 {
        return Err("duration_ms must be > 0".to_string());
    }
    let viewport = root
        .get("viewport")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "viewport object is required".to_string())?;
    let vp_w = viewport
        .get("w")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "viewport.w is required".to_string())?;
    let vp_h = viewport
        .get("h")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "viewport.h is required".to_string())?;
    if vp_w == 0 || vp_h == 0 || vp_w > u64::from(u32::MAX) || vp_h > u64::from(u32::MAX) {
        return Err(format!(
            "viewport must be a positive u32 size (got {vp_w}x{vp_h})"
        ));
    }
    let tracks = root
        .get("tracks")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "tracks array is required".to_string())?;
    if tracks.is_empty() {
        return Err("tracks must not be empty".to_string());
    }
    let components = root
        .get("components")
        .and_then(serde_json::Value::as_object)
        .map(serde_json::Map::len)
        .unwrap_or(0);
    let background = resolve_stage_background(source_json);
    let mut warnings = Vec::new();
    let mut clips = 0_usize;
    let mut visual_tracks = 0_usize;
    let mut audio_tracks = 0_usize;

    for track in tracks {
        let track_id = track
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("<missing>");
        let kind = track
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("track '{track_id}' kind is required"))?;
        if kind == "audio" {
            audio_tracks += 1;
        } else {
            visual_tracks += 1;
        }
        let track_clips = track
            .get("clips")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("track '{track_id}' clips array is required"))?;
        if track_clips.is_empty() {
            warnings.push(format!("track '{track_id}' has no clips"));
        }
        for clip in track_clips {
            clips += 1;
            let clip_id = clip
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<missing>");
            let begin = clip
                .get("begin_ms")
                .or_else(|| clip.get("begin"))
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| format!("clip '{track_id}.{clip_id}' begin_ms is required"))?;
            let end = clip
                .get("end_ms")
                .or_else(|| clip.get("end"))
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| format!("clip '{track_id}.{clip_id}' end_ms is required"))?;
            if end <= begin {
                return Err(format!(
                    "clip '{track_id}.{clip_id}' end_ms must be greater than begin_ms"
                ));
            }
            if end > duration_ms {
                warnings.push(format!(
                    "clip '{track_id}.{clip_id}' ends after source duration"
                ));
            }
        }
    }

    if visual_tracks == 0 {
        return Err("at least one non-audio visual track is required".to_string());
    }

    Ok(RenderSourceSummary {
        schema_version: schema_version.to_string(),
        duration_ms,
        viewport: (vp_w as u32, vp_h as u32),
        tracks: tracks.len(),
        clips,
        components,
        visual_tracks,
        audio_tracks,
        background,
        warnings,
    })
}

/// 高层 lib API · 输入 source.json 路径 · 输出 MP4。
///
/// Historical: v1.44 high-level lib export API.
///
/// 主调方(nf-shell):
/// ```ignore
/// let stats = capy_recorder::run_export_from_source(
///     Path::new("demo/v1.8/source.json"),
///     Path::new("/tmp/out.mp4"),
///     ExportOpts { duration_s: 3.0, ..Default::default() },
/// ).await?;
/// println!("wrote {} bytes", stats.bytes_written);
/// ```
///
/// 内部:构造临时 HTML `{tmp}/nf-export-{pid}-{nanos}.html` · 写 · 喂给
/// CEF OSR 录制路径 · 退出时临时文件自动 drop(OS tmp cleanup)。
pub async fn run_export_from_source(
    source_path: &Path,
    output: &Path,
    opts: ExportOpts,
) -> Result<OutputStats, RecordError> {
    if !source_path.exists() {
        return Err(RecordError::BundleLoadFailed(format!(
            "source.json not found: {}",
            source_path.display()
        )));
    }
    if opts.duration_s <= 0.0 {
        return Err(RecordError::FrameReadyContract(format!(
            "duration_s must be > 0 (got {})",
            opts.duration_s
        )));
    }

    // 读 source.json · inline 到 HTML 的 __NF_SOURCE__ 里。
    let source_text = std::fs::read_to_string(source_path).map_err(|e| {
        RecordError::BundleLoadFailed(format!("read source.json {}: {e}", source_path.display()))
    })?;
    // Parse 一次拿到逻辑 viewport；resolution preset 只决定输出像素，不改 source 布局基准。
    let source_json: serde_json::Value = serde_json::from_str(&source_text)
        .map_err(|e| RecordError::BundleLoadFailed(format!("source.json not valid JSON: {e}")))?;
    let source_summary = validate_render_source(&source_json)
        .map_err(|err| RecordError::BundleLoadFailed(format!("invalid render source: {err}")))?;
    let preset = resolve_export_preset(&source_json, &opts)?;
    let tracks_map_json = build_tracks_map_json(&source_json);
    let stage_background = resolve_stage_background(&source_json);
    let source_text = serde_json::to_string(&source_json).map_err(|e| {
        RecordError::BundleLoadFailed(format!("serialize source.json for export HTML: {e}"))
    })?;

    let (out_w, out_h) = preset.viewport;
    let requested_duration_ms = (opts.duration_s.max(0.0) * 1000.0).round() as u64;
    let html = build_export_html(
        &source_text,
        &tracks_map_json,
        preset.viewport,
        source_summary.viewport,
        requested_duration_ms,
        &stage_background,
    );

    // 写 tmp file · macOS /tmp 没 gitignore 问题 · 独占进程 pid + nanos 防撞。
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp_html: PathBuf = std::env::temp_dir().join(format!("nf-export-{pid}-{nanos}.html"));
    std::fs::write(&tmp_html, html.as_bytes()).map_err(|e| {
        RecordError::BundleLoadFailed(format!("write tmp html {}: {e}", tmp_html.display()))
    })?;

    // max_duration_s 给 recorder 留一点 buffer · ceil + 2s。
    let max_duration_s = (opts.duration_s.ceil() as u32).saturating_add(2);

    let cfg = RecordConfig {
        bundle: tmp_html.clone(),
        output: output.to_path_buf(),
        width: out_w,
        height: out_h,
        fps: opts.fps,
        bitrate_bps: preset.bitrate_bps,
        codec: preset.codec,
        max_duration_s,
        frame_range: None,
    };

    let parallel = orchestrator::resolve_requested_parallel(
        opts.parallel,
        preset.viewport.0,
        preset.viewport.1,
    )?;

    // Historical: v1.44.1 / v1.56 parallel >= 2 走 orchestrator (spawn N 子进程 +
    // ffmpeg concat) · 4k 未显式指定时默认 parallel=2。
    // 短视频(<6s) orchestrator 内部自动降级单进程 · duration 够长走真并行。
    // 单进程路径用 CEF OSR 拿 OutputStats · 并行路径 orchestrator 返 ()
    // · 用一个 synthetic stats 满足返回类型(size 从文件 metadata 读).
    let result: Result<OutputStats, RecordError> = if parallel >= 2 {
        let total_frames = ((opts.duration_s * f64::from(opts.fps)).round()) as u64;
        match orchestrator::run_parallel(cfg, parallel).await {
            Ok(()) => {
                let size_bytes = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
                Ok(OutputStats {
                    path: output.to_path_buf(),
                    frames: total_frames,
                    duration_ms: (opts.duration_s * 1000.0) as u64,
                    size_bytes,
                    moov_front: true, // orchestrator ffmpeg concat 强制 +faststart
                })
            }
            Err(e) => Err(e),
        }
    } else {
        run_record_config(cfg).await
    };

    // 清临时文件 · 不管 result 成功与否。
    let _ = std::fs::remove_file(&tmp_html);

    if result.is_err() {
        cleanup_export_temp_outputs(output);
    }
    let mut stats = result?;
    cleanup_export_temp_outputs(output);
    if mux_audio_tracks(&source_json, source_path, output, opts.duration_s)? {
        stats.size_bytes = std::fs::metadata(output)
            .map(|m| m.len())
            .unwrap_or(stats.size_bytes);
        stats.moov_front = true;
    }
    Ok(stats)
}

pub async fn snapshot_from_source(
    source_path: &Path,
    output: &Path,
    t_ms: u64,
    viewport_override: Option<ExportResolution>,
) -> Result<(), SnapshotError> {
    let source_text = std::fs::read_to_string(source_path).map_err(|err| {
        SnapshotError::BundleLoad(format!("read source.json {}: {err}", source_path.display()))
    })?;
    let source_json: serde_json::Value = serde_json::from_str(&source_text)
        .map_err(|err| SnapshotError::BundleLoad(format!("source.json not valid JSON: {err}")))?;
    let source_summary = validate_render_source(&source_json)
        .map_err(|err| SnapshotError::BundleLoad(format!("invalid render source: {err}")))?;
    let opts = ExportOpts {
        duration_s: source_json
            .get("duration_ms")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(1000.0)
            / 1000.0,
        resolution_override: viewport_override,
        ..Default::default()
    };
    let preset = resolve_export_preset(&source_json, &opts)
        .map_err(|err| SnapshotError::BundleLoad(format!("{err}")))?;
    let tracks_map_json = build_tracks_map_json(&source_json);
    let stage_background = resolve_stage_background(&source_json);
    let source_text = serde_json::to_string(&source_json)
        .map_err(|err| SnapshotError::BundleLoad(format!("serialize source.json: {err}")))?;
    let requested_duration_ms = source_json
        .get("duration_ms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(t_ms.max(1));
    let (vp_w, vp_h) = preset.viewport;
    let html = build_export_html(
        &source_text,
        &tracks_map_json,
        preset.viewport,
        source_summary.viewport,
        requested_duration_ms,
        &stage_background,
    );
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp_html: PathBuf =
        std::env::temp_dir().join(format!("nf-source-snapshot-{pid}-{nanos}.html"));
    std::fs::write(&tmp_html, html.as_bytes())
        .map_err(|err| SnapshotError::BundleLoad(format!("write tmp html: {err}")))?;
    let result = cef_osr::snapshot_png(&tmp_html, t_ms, output, vp_w, vp_h)
        .await
        .map_err(|err| SnapshotError::Shell(format!("{err}")));
    let _ = std::fs::remove_file(&tmp_html);
    result
}

pub async fn run_record_config(cfg: RecordConfig) -> Result<OutputStats, RecordError> {
    cef_osr::run(cfg).await
}

mod audio;
mod html;

use audio::{cleanup_export_temp_outputs, mux_audio_tracks};
use html::{build_export_html, resolve_export_preset, resolve_stage_background};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validates_render_source_v1_contract() {
        let source = json!({
            "schema_version": "capy.timeline.render_source.v1",
            "duration_ms": 1000,
            "viewport": { "w": 1280, "h": 720 },
            "theme": { "background": "#05070a" },
            "components": {},
            "tracks": [{
                "id": "intro.stage",
                "kind": "component",
                "clips": [{
                    "id": "intro.stage.main",
                    "begin_ms": 0,
                    "end_ms": 1000,
                    "params": {}
                }]
            }],
            "assets": []
        });

        let result = validate_render_source(&source);
        assert!(result.is_ok(), "valid render_source.v1: {result:?}");
        if let Ok(summary) = result {
            assert_eq!(summary.duration_ms, 1000);
            assert_eq!(summary.visual_tracks, 1);
            assert_eq!(summary.background, "#05070a");
        }
    }
}
