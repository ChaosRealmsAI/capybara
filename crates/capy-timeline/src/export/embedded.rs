use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::snapshot;
use crate::video_source::{RenderVideoSource, first_video_source};

use super::report::ExportError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedExportMetrics {
    pub duration_ms: u64,
    pub frame_count: u64,
    pub byte_size: u64,
}

pub fn export_embedded(
    render_source_path: &Path,
    out: &Path,
    fps: u32,
    job_id: &str,
) -> Result<EmbeddedExportMetrics, ExportError> {
    let source = read_source(render_source_path)?;
    let duration_ms = duration_ms(&source)?;
    let frame_count = frame_count(duration_ms, fps)?;
    if let Some(video) = first_video_source(&source, duration_ms).map_err(|message| {
        ExportError::new(
            "EXPORT_FAILED",
            "$.tracks[].clips[].params.src",
            message,
            "next step · inspect video track src",
        )
    })? {
        return export_video_source(&video, out, fps, frame_count);
    }
    let frame_dir = frame_dir(out, job_id);
    fs::create_dir_all(&frame_dir).map_err(|err| {
        ExportError::new(
            "EXPORT_FAILED",
            "$.frames",
            format!("create frame directory failed: {err}"),
            "next step · check export output permissions",
        )
    })?;
    render_frames(render_source_path, &frame_dir, frame_count)?;
    encode_mp4(&frame_dir, out, fps)?;
    let byte_size = fs::metadata(out).map_err(|err| {
        ExportError::new(
            "EXPORT_FAILED",
            "$.output_path",
            format!("read MP4 metadata failed: {err}"),
            "next step · rerun capy timeline export",
        )
    })?;
    let _cleanup = fs::remove_dir_all(&frame_dir);
    Ok(EmbeddedExportMetrics {
        duration_ms,
        frame_count,
        byte_size: byte_size.len(),
    })
}

fn export_video_source(
    video: &RenderVideoSource,
    out: &Path,
    _fps: u32,
    frame_count: u64,
) -> Result<EmbeddedExportMetrics, ExportError> {
    let ffmpeg = ffmpeg_path()?;
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| {
            ExportError::new(
                "EXPORT_FAILED",
                "$.output_path",
                format!("create export parent failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    let output = Command::new(&ffmpeg)
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            &seconds(video.start_ms),
            "-i",
            &video.path.display().to_string(),
            "-t",
            &seconds(video.duration_ms),
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            &out.display().to_string(),
        ])
        .output()
        .map_err(|err| {
            ExportError::new(
                "EXPORT_FAILED",
                "$.ffmpeg",
                format!("spawn {} failed: {err}", ffmpeg.display()),
                "next step · check CAPY_FFMPEG or install ffmpeg",
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ExportError::new(
            "EXPORT_FAILED",
            "$.ffmpeg",
            if stderr.is_empty() {
                format!("ffmpeg video trim failed: {}", output.status)
            } else {
                stderr
            },
            "next step · inspect the source video path and selected range",
        ));
    }
    let metadata = fs::metadata(out).map_err(|err| {
        ExportError::new(
            "EXPORT_FAILED",
            "$.output_path",
            format!("read MP4 metadata failed: {err}"),
            "next step · rerun capy timeline export",
        )
    })?;
    Ok(EmbeddedExportMetrics {
        duration_ms: video.duration_ms,
        frame_count,
        byte_size: metadata.len(),
    })
}

pub fn read_duration_ms(render_source_path: &Path) -> Result<u64, ExportError> {
    read_source(render_source_path).and_then(|source| duration_ms(&source))
}

pub fn frame_count(duration_ms: u64, fps: u32) -> Result<u64, ExportError> {
    if fps == 0 {
        return Err(ExportError::new(
            "EXPORT_FAILED",
            "$.fps",
            "fps must be greater than 0",
            "next step · pass --fps 30",
        ));
    }
    let frames = duration_ms.saturating_mul(u64::from(fps)).div_ceil(1000);
    Ok(frames.max(1))
}

fn render_frames(
    render_source_path: &Path,
    frame_dir: &Path,
    frame_count: u64,
) -> Result<(), ExportError> {
    for index in 1..=frame_count {
        let image = snapshot::embedded::render_frame(render_source_path).map_err(snapshot_error)?;
        let out = frame_dir.join(format!("frame-{index:04}.png"));
        snapshot::embedded::write_png(&image, &out).map_err(snapshot_error)?;
    }
    Ok(())
}

fn encode_mp4(frame_dir: &Path, out: &Path, fps: u32) -> Result<(), ExportError> {
    let ffmpeg = ffmpeg_path()?;
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| {
            ExportError::new(
                "EXPORT_FAILED",
                "$.output_path",
                format!("create export parent failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    let pattern = frame_dir.join("frame-%04d.png");
    let output = Command::new(&ffmpeg)
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-framerate",
            &fps.to_string(),
            "-i",
            &pattern.display().to_string(),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            &out.display().to_string(),
        ])
        .output()
        .map_err(|err| {
            ExportError::new(
                "EXPORT_FAILED",
                "$.ffmpeg",
                format!("spawn {} failed: {err}", ffmpeg.display()),
                "next step · check CAPY_FFMPEG or install ffmpeg",
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ExportError::new(
            "EXPORT_FAILED",
            "$.ffmpeg",
            if stderr.is_empty() {
                format!("ffmpeg failed: {}", output.status)
            } else {
                stderr
            },
            "next step · rerun with a valid ffmpeg installation",
        ));
    }
    Ok(())
}

fn ffmpeg_path() -> Result<PathBuf, ExportError> {
    if let Some(path) = std::env::var_os("CAPY_FFMPEG").map(PathBuf::from) {
        if path.is_file() {
            return Ok(path);
        }
        return Err(ExportError::new(
            "FFMPEG_NOT_FOUND",
            "$.ffmpeg",
            format!("CAPY_FFMPEG does not point to a file: {}", path.display()),
            "next step · set CAPY_FFMPEG to ffmpeg or install ffmpeg on PATH",
        ));
    }
    which::which("ffmpeg").map_err(|_| {
        ExportError::new(
            "FFMPEG_NOT_FOUND",
            "$.ffmpeg",
            "ffmpeg was not found on PATH or CAPY_FFMPEG",
            "next step · install ffmpeg or set CAPY_FFMPEG",
        )
    })
}

fn seconds(ms: u64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

fn read_source(path: &Path) -> Result<Value, ExportError> {
    let text = fs::read_to_string(path).map_err(|err| {
        ExportError::new(
            "RENDER_SOURCE_MISSING",
            "$.render_source_path",
            format!("read render_source failed: {err}"),
            "next step · run capy timeline compile --composition <path>",
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        ExportError::new(
            "EXPORT_FAILED",
            "$.render_source",
            format!("render_source JSON is invalid: {err}"),
            "next step · rerun capy timeline compile",
        )
    })
}

fn duration_ms(source: &Value) -> Result<u64, ExportError> {
    source
        .get("duration_ms")
        .or_else(|| source.get("duration"))
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            ExportError::new(
                "EXPORT_FAILED",
                "$.render_source.duration_ms",
                "render_source duration_ms must be greater than 0",
                "next step · rerun capy timeline compile",
            )
        })
}

fn frame_dir(out: &Path, job_id: &str) -> PathBuf {
    out.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{job_id}-frames"))
}

fn snapshot_error(err: snapshot::SnapshotError) -> ExportError {
    ExportError::new(err.code, err.path, err.message, err.hint)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::{export_embedded, frame_count};

    #[test]
    fn frame_count_ceilings_duration() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(frame_count(1000, 30)?, 30);
        assert_eq!(frame_count(1001, 30)?, 31);
        assert_eq!(frame_count(1, 24)?, 1);
        assert!(frame_count(1000, 0).is_err());
        Ok(())
    }

    #[test]
    fn embedded_export_writes_mp4_when_ffmpeg_is_available()
    -> Result<(), Box<dyn std::error::Error>> {
        if which::which("ffmpeg").is_err() {
            return Ok(());
        }
        let dir = unique_dir("happy")?;
        let source = dir.join("render_source.json");
        let out = dir.join("out.mp4");
        fs::write(&source, serde_json::to_vec_pretty(&render_source())?)?;

        let metrics = export_embedded(&source, &out, 10, "exp-test")?;

        assert!(out.is_file());
        assert!(metrics.byte_size > 0);
        assert_eq!(metrics.duration_ms, 200);
        assert_eq!(metrics.frame_count, 2);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    fn render_source() -> serde_json::Value {
        json!({
            "schema_version": "capy.timeline.render_source.v1",
            "duration_ms": 200,
            "viewport": {"w": 64, "h": 64},
            "tracks": [{
                "id": "poster.document",
                "clips": [{
                    "params": {
                        "params": {
                            "poster": {
                                "canvas": {"background": "#ffffff"},
                                "assets": {},
                                "layers": [{
                                    "id": "shape",
                                    "type": "shape",
                                    "shape": "rect",
                                    "x": 0,
                                    "y": 0,
                                    "width": 64,
                                    "height": 64,
                                    "style": {"fill": "#00aa77"}
                                }]
                            }
                        }
                    }
                }]
            }]
        })
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-timeline-export-embedded-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(Path::new(&dir))?;
        Ok(dir)
    }
}
