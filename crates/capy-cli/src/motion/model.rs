use std::path::{Path, PathBuf};

use serde_json::{Value, json};

pub(super) const MANIFEST_SCHEMA: &str = "capy.motion_asset.manifest.v1";
pub(super) const QA_SCHEMA: &str = "capy.motion_asset.qa.v1";

#[derive(Debug, Clone)]
pub(super) struct SourceMeta {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_sec: f64,
    pub frame_count: u32,
    pub video_codec: String,
    pub audio_codec: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct FrameStat {
    pub index: usize,
    pub rgba_path: PathBuf,
    pub mask_path: PathBuf,
    pub bbox: BBox,
    pub nontransparent_ratio: f64,
    pub edge_ratio: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct BBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub(super) struct PackagePaths {
    pub root: PathBuf,
    pub source_dir: PathBuf,
    pub source_frames_dir: PathBuf,
    pub rgba_frames_dir: PathBuf,
    pub cropped_frames_dir: PathBuf,
    pub masks_dir: PathBuf,
    pub atlas_dir: PathBuf,
    pub video_dir: PathBuf,
    pub qa_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub tmp_dir: PathBuf,
}

impl PackagePaths {
    pub(super) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            source_dir: root.join("source"),
            source_frames_dir: root.join("frames/source"),
            rgba_frames_dir: root.join("frames/rgba"),
            cropped_frames_dir: root.join("frames/cropped"),
            masks_dir: root.join("masks"),
            atlas_dir: root.join("atlas"),
            video_dir: root.join("video"),
            qa_dir: root.join("qa"),
            logs_dir: root.join("logs"),
            tmp_dir: root.join("tmp"),
        }
    }

    pub(super) fn all_dirs(&self) -> [&Path; 10] {
        [
            &self.source_dir,
            &self.source_frames_dir,
            &self.rgba_frames_dir,
            &self.cropped_frames_dir,
            &self.masks_dir,
            &self.atlas_dir,
            &self.video_dir,
            &self.qa_dir,
            &self.logs_dir,
            &self.tmp_dir,
        ]
    }
}

pub(super) fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn manifest_json(
    source: &Path,
    meta: &SourceMeta,
    paths: &PackagePaths,
    qa_report: &Value,
    warnings: &[String],
) -> Value {
    json!({
        "schema": MANIFEST_SCHEMA,
        "kind": "animation-grade-video-cutout",
        "source": {
            "path": source,
            "width": meta.width,
            "height": meta.height,
            "fps": meta.fps,
            "duration_sec": meta.duration_sec,
            "frame_count": meta.frame_count,
            "video_codec": meta.video_codec,
            "audio_codec": meta.audio_codec
        },
        "strategy": {
            "alpha": "source RGB + withoutbg/focus alpha mask",
            "motion_mode": "travel-through",
            "quality": "animation",
            "targets": ["png-sequence", "sprite-atlas", "webm-alpha", "rgb-alpha-dual-mp4"]
        },
        "outputs": {
            "rgba_frames": rel_path(&paths.root, &paths.rgba_frames_dir),
            "cropped_frames": rel_path(&paths.root, &paths.cropped_frames_dir),
            "masks": rel_path(&paths.root, &paths.masks_dir),
            "atlas_png": "atlas/walk.png",
            "atlas_json": "atlas/walk.json",
            "preview_webm": "video/preview.webm",
            "rgb_mp4": "video/rgb.mp4",
            "alpha_mp4": "video/alpha.mp4",
            "preview_html": "qa/preview.html",
            "qa_report": "qa/report.json"
        },
        "quality": {
            "verdict": qa_report.get("verdict").cloned().unwrap_or_else(|| json!("draft")),
            "metrics": qa_report.get("metrics").cloned().unwrap_or(Value::Null),
            "warnings": warnings,
            "notes": qa_report.get("notes").cloned().unwrap_or(Value::Null)
        }
    })
}

impl BBox {
    pub(super) fn bottom(self) -> u32 {
        self.y.saturating_add(self.height)
    }
}
