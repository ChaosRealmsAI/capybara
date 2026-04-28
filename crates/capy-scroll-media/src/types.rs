use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::packager::ScrollMediaError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClipRole {
    Default,
    Fallback,
    Hq,
}

impl ClipRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Fallback => "fallback",
            Self::Hq => "hq",
        }
    }
}

impl fmt::Display for ClipRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipPreset {
    pub role: ClipRole,
    pub height: u32,
    pub crf: u8,
}

impl ClipPreset {
    pub fn new(role: ClipRole, height: u32, crf: u8) -> Self {
        Self { role, height, crf }
    }

    pub fn parse(role: ClipRole, value: &str) -> Result<Self, ScrollMediaError> {
        let parts = value.split(':').collect::<Vec<_>>();
        if parts.len() != 2 {
            return Err(ScrollMediaError::Message(format!(
                "invalid {role} preset: {value}; expected HEIGHT:CRF, for example 720:23"
            )));
        }
        let height = parse_u32(parts[0], "height")?;
        let crf = parse_u8(parts[1], "crf")?;
        if height == 0 {
            return Err(ScrollMediaError::Message(
                "preset height must be greater than 0".to_string(),
            ));
        }
        if crf > 51 {
            return Err(ScrollMediaError::Message(
                "preset crf must be between 0 and 51".to_string(),
            ));
        }
        Ok(Self::new(role, height, crf))
    }

    pub fn file_name(&self) -> String {
        format!(
            "scrub-{}-crf{}-allkey.mp4",
            self.height,
            u16::from(self.crf)
        )
    }
}

fn parse_u32(value: &str, label: &str) -> Result<u32, ScrollMediaError> {
    u32::from_str(value)
        .map_err(|err| ScrollMediaError::Message(format!("invalid preset {label}: {value}: {err}")))
}

fn parse_u8(value: &str, label: &str) -> Result<u8, ScrollMediaError> {
    u8::from_str(value)
        .map_err(|err| ScrollMediaError::Message(format!("invalid preset {label}: {value}: {err}")))
}

#[derive(Debug, Clone)]
pub struct ScrollPackRequest {
    pub input: PathBuf,
    pub out_dir: PathBuf,
    pub name: String,
    pub poster_width: u32,
    pub default_preset: ClipPreset,
    pub fallback_preset: ClipPreset,
    pub hq_preset: ClipPreset,
    pub verify: bool,
    pub overwrite: bool,
    pub dry_run: bool,
}

impl ScrollPackRequest {
    pub fn presets(&self) -> [ClipPreset; 3] {
        [
            self.default_preset.clone(),
            self.fallback_preset.clone(),
            self.hq_preset.clone(),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollPackManifest {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub duration: f64,
    pub fps: f64,
    pub frame_count: u64,
    pub width: u32,
    pub height: u32,
    pub poster: String,
    pub default_clip: String,
    pub fallback_clip: String,
    pub hq_clip: String,
    pub runtime: RuntimeFiles,
    pub requires: ManifestRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeFiles {
    pub js: String,
    pub css: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestRequirements {
    pub http_range: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMetadata {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub fps: f64,
    pub frame_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFile {
    pub role: String,
    pub path: String,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSummary {
    pub checked: bool,
    pub all_keyframe_clips: bool,
    pub clips: Vec<ClipVerification>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipVerification {
    pub path: String,
    pub keyframe_count: u64,
    pub non_keyframe_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollPackReport {
    pub ok: bool,
    pub dry_run: bool,
    pub input: String,
    pub output_dir: String,
    pub manifest_path: String,
    pub source: Option<SourceMetadata>,
    pub manifest: Option<ScrollPackManifest>,
    pub files: Vec<PackFile>,
    pub verification: Option<VerificationSummary>,
}

impl ScrollPackManifest {
    pub fn from_source(
        name: String,
        source: SourceMetadata,
        poster: String,
        default_clip: String,
        fallback_clip: String,
        hq_clip: String,
    ) -> Self {
        Self {
            schema_version: 1,
            kind: "capy-scroll-media-pack".to_string(),
            name,
            duration: source.duration,
            fps: source.fps,
            frame_count: source.frame_count,
            width: source.width,
            height: source.height,
            poster,
            default_clip,
            fallback_clip,
            hq_clip,
            runtime: RuntimeFiles {
                js: "runtime/scroll-video.js".to_string(),
                css: "runtime/scroll-video.css".to_string(),
            },
            requires: ManifestRequirements { http_range: true },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clip_preset() -> Result<(), ScrollMediaError> {
        let preset = ClipPreset::parse(ClipRole::Default, "720:23")?;
        assert_eq!(preset.height, 720);
        assert_eq!(preset.crf, 23);
        assert_eq!(preset.file_name(), "scrub-720-crf23-allkey.mp4");
        Ok(())
    }

    #[test]
    fn rejects_bad_preset_format() {
        assert!(ClipPreset::parse(ClipRole::Default, "720").is_err());
        assert!(ClipPreset::parse(ClipRole::Default, "720:99").is_err());
    }

    #[test]
    fn manifest_shape_is_stable() -> Result<(), ScrollMediaError> {
        let manifest = ScrollPackManifest::from_source(
            "watch".to_string(),
            SourceMetadata {
                width: 1920,
                height: 1080,
                duration: 8.042,
                fps: 24.0,
                frame_count: 193,
            },
            "poster-1280.jpg".to_string(),
            "scrub-720-crf23-allkey.mp4".to_string(),
            "scrub-720-crf27-allkey.mp4".to_string(),
            "scrub-1080-crf24-allkey.mp4".to_string(),
        );
        let value = serde_json::to_value(manifest)
            .map_err(|err| ScrollMediaError::Message(err.to_string()))?;
        assert_eq!(value["kind"], "capy-scroll-media-pack");
        assert_eq!(value["requires"]["http_range"], true);
        assert_eq!(value["runtime"]["js"], "runtime/scroll-video.js");
        Ok(())
    }
}
