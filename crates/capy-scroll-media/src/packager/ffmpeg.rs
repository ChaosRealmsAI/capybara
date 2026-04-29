use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

use crate::packager::{Result, ScrollMediaError};
use crate::types::{ClipVerification, SourceMetadata};

pub(crate) fn read_source_metadata(input: &Path) -> Result<SourceMetadata> {
    let output = run_command(
        "ffprobe",
        &[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,avg_frame_rate,r_frame_rate,duration,nb_frames:format=duration",
            "-of",
            "json",
            &input.display().to_string(),
        ],
    )?;
    let value: Value = serde_json::from_slice(&output)
        .map_err(|err| ScrollMediaError::Message(format!("parse ffprobe JSON failed: {err}")))?;
    let stream = value
        .get("streams")
        .and_then(Value::as_array)
        .and_then(|streams| streams.first())
        .ok_or_else(|| ScrollMediaError::Message("ffprobe found no video stream".to_string()))?;
    let width = json_u32(stream, "width")?;
    let height = json_u32(stream, "height")?;
    let duration = json_f64(stream, "duration")
        .or_else(|_| {
            value
                .get("format")
                .map_or(Err(()), |format| json_f64(format, "duration"))
        })
        .map_err(|_| ScrollMediaError::Message("ffprobe duration missing".to_string()))?;
    let fps = stream
        .get("avg_frame_rate")
        .and_then(Value::as_str)
        .and_then(parse_fraction)
        .or_else(|| {
            stream
                .get("r_frame_rate")
                .and_then(Value::as_str)
                .and_then(parse_fraction)
        })
        .ok_or_else(|| ScrollMediaError::Message("ffprobe frame rate missing".to_string()))?;
    let frame_count = stream
        .get("nb_frames")
        .and_then(Value::as_str)
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|count| *count > 0)
        .unwrap_or_else(|| (duration * fps).round() as u64);
    Ok(SourceMetadata {
        width,
        height,
        duration: round3(duration),
        fps: round3(fps),
        frame_count,
    })
}

pub(crate) fn write_poster(input: &Path, output: &Path, poster_width: u32) -> Result<()> {
    super::create_parent_dir(output)?;
    run_command(
        "ffmpeg",
        &[
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-i",
            &input.display().to_string(),
            "-vf",
            &format!("scale={poster_width}:-2"),
            "-frames:v",
            "1",
            "-q:v",
            "2",
            &output.display().to_string(),
        ],
    )?;
    Ok(())
}

pub(crate) fn encode_all_keyframe_clip(
    input: &Path,
    output: &Path,
    height: u32,
    crf: u8,
) -> Result<()> {
    super::create_parent_dir(output)?;
    run_command(
        "ffmpeg",
        &[
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-i",
            &input.display().to_string(),
            "-an",
            "-vf",
            &format!("scale=-2:{height}"),
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            &u16::from(crf).to_string(),
            "-g",
            "1",
            "-keyint_min",
            "1",
            "-sc_threshold",
            "0",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            &output.display().to_string(),
        ],
    )?;
    Ok(())
}

pub(crate) fn verify_all_keyframes(path: &Path) -> Result<ClipVerification> {
    let output = run_command(
        "ffprobe",
        &[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "frame=key_frame",
            "-of",
            "csv=p=0",
            &path.display().to_string(),
        ],
    )?;
    let text = String::from_utf8(output)
        .map_err(|err| ScrollMediaError::Message(format!("ffprobe output was not utf8: {err}")))?;
    let mut keyframe_count = 0_u64;
    let mut non_keyframe_count = 0_u64;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let first = line.split(',').next().unwrap_or_default();
        match first {
            "1" => keyframe_count += 1,
            "0" => non_keyframe_count += 1,
            _ => {}
        }
    }
    if keyframe_count == 0 {
        return Err(ScrollMediaError::Message(format!(
            "ffprobe found no frames in {}",
            path.display()
        )));
    }
    Ok(ClipVerification {
        path: path.display().to_string(),
        keyframe_count,
        non_keyframe_count,
    })
}

fn json_u32(value: &Value, key: &str) -> Result<u32> {
    let raw = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| ScrollMediaError::Message(format!("ffprobe {key} missing")))?;
    u32::try_from(raw)
        .map_err(|err| ScrollMediaError::Message(format!("ffprobe {key} out of range: {err}")))
}

fn json_f64(value: &Value, key: &str) -> std::result::Result<f64, ()> {
    value
        .get(key)
        .and_then(Value::as_str)
        .and_then(|raw| raw.parse::<f64>().ok())
        .or_else(|| value.get(key).and_then(Value::as_f64))
        .filter(|number| number.is_finite() && *number > 0.0)
        .ok_or(())
}

fn parse_fraction(value: &str) -> Option<f64> {
    let (left, right) = value.split_once('/')?;
    let numerator = left.parse::<f64>().ok()?;
    let denominator = right.parse::<f64>().ok()?;
    if denominator <= 0.0 {
        return None;
    }
    Some(numerator / denominator)
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn run_command(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| {
            ScrollMediaError::Message(format!(
                "run {program} failed: {err}; make sure {program} is installed"
            ))
        })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(ScrollMediaError::Message(format!(
        "{program} failed with status {}: {}",
        output.status,
        stderr.trim()
    )))
}

#[cfg(test)]
mod tests {
    use super::parse_fraction;

    #[test]
    fn parses_fraction() {
        assert_eq!(parse_fraction("24/1"), Some(24.0));
        assert_eq!(parse_fraction("0/0"), None);
    }
}
