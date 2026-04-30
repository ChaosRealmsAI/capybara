use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::video_source::RenderVideoSource;

use super::{EmbeddedExportMetrics, ffmpeg_path, frame_count, seconds};
use crate::export::report::ExportError;

pub(super) fn export_video_sources(
    videos: &[RenderVideoSource],
    source: &Value,
    out: &Path,
    fps: u32,
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
    let (width, height) = output_size(source);
    let mut args = vec![
        "-y".to_string(),
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
    ];
    for video in videos {
        args.extend([
            "-ss".to_string(),
            seconds(video.start_ms),
            "-t".to_string(),
            seconds(video.duration_ms),
            "-i".to_string(),
            video.path.display().to_string(),
        ]);
    }
    let filters = videos
        .iter()
        .enumerate()
        .map(|(index, _)| {
            format!(
                "[{index}:v]setpts=PTS-STARTPTS,scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1[v{index}]"
            )
        })
        .collect::<Vec<_>>();
    let concat_inputs = (0..videos.len())
        .map(|index| format!("[v{index}]"))
        .collect::<String>();
    let filter_complex = format!(
        "{};{}concat=n={}:v=1:a=0[v]",
        filters.join(";"),
        concat_inputs,
        videos.len()
    );
    args.extend([
        "-filter_complex".to_string(),
        filter_complex,
        "-map".to_string(),
        "[v]".to_string(),
        "-r".to_string(),
        fps.to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "veryfast".to_string(),
        "-pix_fmt".to_string(),
        "yuv420p".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        out.display().to_string(),
    ]);
    let output = Command::new(&ffmpeg).args(&args).output().map_err(|err| {
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
                format!("ffmpeg video concat failed: {}", output.status)
            } else {
                stderr
            },
            "next step · inspect queued source video paths and selected ranges",
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
    let duration_ms = videos.iter().map(|video| video.duration_ms).sum();
    Ok(EmbeddedExportMetrics {
        duration_ms,
        frame_count: frame_count(duration_ms, fps)?,
        byte_size: metadata.len(),
    })
}

fn output_size(source: &Value) -> (u32, u32) {
    let viewport = source.get("viewport").unwrap_or(&Value::Null);
    let width = viewport
        .get("w")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value >= 2)
        .unwrap_or(1280);
    let height = viewport
        .get("h")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value >= 2)
        .unwrap_or(720);
    (width - (width % 2), height - (height % 2))
}
