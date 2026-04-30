use std::fs;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::{ARTIFACT_REGISTRY_SCHEMA_VERSION, ArtifactKind, ArtifactRefV1};
use crate::package::{ProjectPackage, ProjectPackageError, ProjectPackageResult, new_id, now_ms};

pub const VIDEO_IMPORT_SCHEMA_VERSION: &str = "capy.project-video-import.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoImportMetadataV1 {
    pub filename: String,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    pub byte_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoImportResultV1 {
    pub schema_version: String,
    pub project_id: String,
    pub artifact: ArtifactRefV1,
    pub metadata: VideoImportMetadataV1,
    pub poster_frame_path: String,
    pub composition_path: String,
    pub generated_at: u64,
}

impl ProjectPackage {
    pub fn import_video_artifact(
        &self,
        source_path: impl AsRef<Path>,
        title: Option<String>,
    ) -> ProjectPackageResult<VideoImportResultV1> {
        let source_rel = self.relative_existing_path(source_path.as_ref())?;
        let source_abs = self.root().join(&source_rel);
        let metadata = probe_video(&source_abs)?;
        let mut registry = self.artifacts()?;
        let now = now_ms();
        let artifact_index = registry.artifacts.iter().position(|artifact| {
            artifact.kind == ArtifactKind::Video && artifact.source_path == source_rel
        });
        let artifact_id = artifact_index
            .and_then(|index| {
                registry
                    .artifacts
                    .get(index)
                    .map(|artifact| artifact.id.clone())
            })
            .unwrap_or_else(|| new_id("art"));
        let safe_id = safe_path_id(&artifact_id);
        let poster_rel = format!(
            "{}/video-previews/{safe_id}-first-frame.png",
            crate::CAPY_DIR
        );
        let composition_rel = format!(
            "{}/video-compositions/{safe_id}/compositions/main.json",
            crate::CAPY_DIR
        );

        extract_first_frame(&source_abs, &self.root().join(&poster_rel))?;
        write_video_composition(
            &self.root().join(&composition_rel),
            &artifact_id,
            &source_rel,
            &source_abs,
            &metadata,
            title.as_deref(),
        )?;

        let artifact = ArtifactRefV1 {
            id: artifact_id.clone(),
            kind: ArtifactKind::Video,
            title: title
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| metadata.filename.clone()),
            source_path: source_rel.clone(),
            source_refs: Vec::new(),
            output_refs: vec![composition_rel.clone()],
            design_language_refs: Vec::new(),
            asset_refs: Vec::new(),
            provenance: Some(json!({
                "video_import": {
                    "schema_version": VIDEO_IMPORT_SCHEMA_VERSION,
                    "filename": metadata.filename,
                    "duration_ms": metadata.duration_ms,
                    "width": metadata.width,
                    "height": metadata.height,
                    "fps": metadata.fps,
                    "byte_size": metadata.byte_size,
                    "poster_frame_path": poster_rel,
                    "composition_path": composition_rel
                }
            })),
            evidence_refs: Vec::new(),
            updated_at: now,
        };

        if let Some(index) = artifact_index {
            registry.artifacts[index] = artifact.clone();
        } else {
            registry.artifacts.push(artifact.clone());
        }
        if registry.schema_version.trim().is_empty() {
            registry.schema_version = ARTIFACT_REGISTRY_SCHEMA_VERSION.to_string();
        }
        self.write_artifacts(&registry)?;
        self.touch_project_manifest()?;

        Ok(VideoImportResultV1 {
            schema_version: VIDEO_IMPORT_SCHEMA_VERSION.to_string(),
            project_id: self.project_manifest()?.id,
            artifact,
            metadata,
            poster_frame_path: poster_rel,
            composition_path: composition_rel,
            generated_at: now,
        })
    }
}

fn probe_video(path: &Path) -> ProjectPackageResult<VideoImportMetadataV1> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            &path.display().to_string(),
        ])
        .output()
        .map_err(|source| ProjectPackageError::Io {
            context: "spawn ffprobe for project video import".to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(ProjectPackageError::Invalid(format!(
            "ffprobe failed for {}: {}",
            path.display(),
            stderr_text(&output.stderr)
        )));
    }
    let value: Value =
        serde_json::from_slice(&output.stdout).map_err(|source| ProjectPackageError::Json {
            context: "parse ffprobe video metadata".to_string(),
            source,
        })?;
    let stream = value
        .get("streams")
        .and_then(Value::as_array)
        .and_then(|streams| {
            streams
                .iter()
                .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))
        })
        .ok_or_else(|| {
            ProjectPackageError::Invalid(format!(
                "ffprobe found no video stream: {}",
                path.display()
            ))
        })?;
    let width = stream
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| ProjectPackageError::Invalid("ffprobe video width missing".to_string()))?;
    let height = stream
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| ProjectPackageError::Invalid("ffprobe video height missing".to_string()))?;
    let duration_ms = duration_seconds(stream)
        .or_else(|| value.get("format").and_then(duration_seconds))
        .map(|seconds| (seconds * 1000.0).round() as u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            ProjectPackageError::Invalid("ffprobe video duration missing".to_string())
        })?;
    let byte_size = fs::metadata(path)
        .map_err(|source| ProjectPackageError::Io {
            context: format!("read video metadata {}", path.display()),
            source,
        })?
        .len();
    Ok(VideoImportMetadataV1 {
        filename: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("video.mp4")
            .to_string(),
        duration_ms,
        width,
        height,
        fps: stream
            .get("avg_frame_rate")
            .or_else(|| stream.get("r_frame_rate"))
            .and_then(Value::as_str)
            .and_then(parse_fraction),
        byte_size,
    })
}

fn extract_first_frame(source: &Path, out: &Path) -> ProjectPackageResult<()> {
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|source| ProjectPackageError::Io {
            context: format!("create {}", parent.display()),
            source,
        })?;
    }
    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            "0",
            "-i",
            &source.display().to_string(),
            "-frames:v",
            "1",
            &out.display().to_string(),
        ])
        .output()
        .map_err(|source| ProjectPackageError::Io {
            context: "spawn ffmpeg for project video first frame".to_string(),
            source,
        })?;
    if output.status.success() && out.is_file() {
        Ok(())
    } else {
        Err(ProjectPackageError::Invalid(format!(
            "ffmpeg first-frame extraction failed for {}: {}",
            source.display(),
            stderr_text(&output.stderr)
        )))
    }
}

fn write_video_composition(
    out: &Path,
    artifact_id: &str,
    source_rel: &str,
    source_abs: &Path,
    metadata: &VideoImportMetadataV1,
    title: Option<&str>,
) -> ProjectPackageResult<()> {
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|source| ProjectPackageError::Io {
            context: format!("create {}", parent.display()),
            source,
        })?;
    }
    let ratio = format!("{}:{}", metadata.width, metadata.height);
    let name = title
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&metadata.filename);
    let composition = json!({
        "schema": "capy.timeline.composition.v2",
        "schema_version": "capy.composition.v2",
            "id": format!("project-video-{}", safe_slug_id(artifact_id)),
        "name": name,
        "viewport": {
            "w": metadata.width,
            "h": metadata.height,
            "ratio": ratio
        },
        "theme": "default",
        "export": {
            "resolution": "1080p",
            "source": "project-video-import"
        },
        "source_artifact": {
            "artifact_id": artifact_id,
            "source_path": source_rel,
            "filename": metadata.filename
        },
        "assets": [{
            "id": "source-video",
            "type": "video",
            "source_path": source_abs.display().to_string()
        }],
        "clips": [{
            "id": "source",
            "name": metadata.filename,
            "duration": format!("{}ms", metadata.duration_ms),
            "duration_ms": metadata.duration_ms,
            "tracks": [{
                "id": "video",
                "kind": "video",
                "z": 0,
                "params": {
                    "src": file_url(source_abs),
                    "source_path": source_rel,
                    "filename": metadata.filename,
                    "duration_ms": metadata.duration_ms,
                    "width": metadata.width,
                    "height": metadata.height,
                    "fps": metadata.fps
                }
            }]
        }]
    });
    let mut payload =
        serde_json::to_string_pretty(&composition).map_err(|source| ProjectPackageError::Json {
            context: format!("serialize video composition {}", out.display()),
            source,
        })?;
    payload.push('\n');
    fs::write(out, payload).map_err(|source| ProjectPackageError::Io {
        context: format!("write {}", out.display()),
        source,
    })
}

fn duration_seconds(value: &Value) -> Option<f64> {
    value
        .get("duration")
        .and_then(Value::as_str)
        .and_then(|raw| raw.parse::<f64>().ok())
        .filter(|seconds| *seconds > 0.0)
}

fn parse_fraction(raw: &str) -> Option<f64> {
    let (num, den) = raw.split_once('/')?;
    let numerator = num.parse::<f64>().ok()?;
    let denominator = den.parse::<f64>().ok()?;
    if denominator == 0.0 {
        None
    } else {
        Some(numerator / denominator)
    }
}

fn safe_path_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "video".to_string()
    } else {
        sanitized
    }
}

fn safe_slug_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            let lower = ch.to_ascii_lowercase();
            if lower.is_ascii_alphanumeric() || lower == '-' || lower == '.' {
                lower
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let slug = if sanitized
        .chars()
        .next()
        .map(|ch| ch.is_ascii_alphabetic())
        .unwrap_or(false)
    {
        sanitized
    } else {
        format!("video-{sanitized}")
    };
    slug.chars().take(64).collect::<String>()
}

fn file_url(path: &Path) -> String {
    let mut encoded = String::new();
    for byte in path.to_string_lossy().as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(char::from(*byte))
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    format!("file://{encoded}")
}

fn stderr_text(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_video_registers_artifact_and_composition_when_ffmpeg_is_available()
    -> Result<(), Box<dyn std::error::Error>> {
        if command_missing("ffmpeg") || command_missing("ffprobe") {
            return Ok(());
        }
        let dir = std::env::temp_dir().join(format!(
            "capy-project-video-import-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let media_dir = dir.join("media");
        fs::create_dir_all(&media_dir)?;
        let video = media_dir.join("sample.mp4");
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=10",
                "-t",
                "1",
                "-pix_fmt",
                "yuv420p",
                &video.display().to_string(),
            ])
            .output()?;
        if !output.status.success() {
            return Ok(());
        }

        let package = ProjectPackage::init(&dir, Some("Video Import".to_string()))?;
        let result =
            package.import_video_artifact("media/sample.mp4", Some("Sample".to_string()))?;

        assert_eq!(result.schema_version, VIDEO_IMPORT_SCHEMA_VERSION);
        assert_eq!(result.artifact.kind, ArtifactKind::Video);
        assert!(dir.join(&result.poster_frame_path).is_file());
        assert!(dir.join(&result.composition_path).is_file());
        let composition: Value =
            serde_json::from_str(&fs::read_to_string(dir.join(&result.composition_path))?)?;
        assert_eq!(composition["clips"][0]["tracks"][0]["kind"], json!("video"));
        let _ = fs::remove_dir_all(dir);
        Ok(())
    }

    fn command_missing(binary: &str) -> bool {
        Command::new(binary).arg("-version").output().is_err()
    }
}
