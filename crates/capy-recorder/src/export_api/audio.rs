use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use base64::Engine;

use crate::events::{emit, Event};
use crate::record_loop::RecordError;

#[derive(Debug, Clone)]
struct AudioClip {
    src: String,
    begin_ms: u64,
    volume: f64,
}

pub(super) fn mux_audio_tracks(
    source: &serde_json::Value,
    source_path: &Path,
    video_path: &Path,
    duration_s: f64,
) -> Result<bool, RecordError> {
    let has_audio_track = source_has_audio_tracks(source);
    let audio = collect_audio_clips(source);
    if audio.is_empty() {
        if has_audio_track {
            return Err(RecordError::PipelineError(
                "audio track exists, but no audio clip has params.src".to_string(),
            ));
        }
        return Ok(false);
    }
    let ffmpeg = resolve_ffmpeg().ok_or_else(|| {
        RecordError::PipelineError(
            "audio tracks found, but ffmpeg is unavailable; recorder refuses to export a silent MP4"
                .to_string(),
        )
    })?;
    let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    let mut audio_paths = Vec::new();
    let mut temp_audio_paths = Vec::new();
    for (index, clip) in audio.iter().enumerate() {
        let (path, is_temp) = audio_src_to_path(&clip.src, source_dir, video_path, index)
            .map_err(RecordError::PipelineError)?;
        if !path.exists() {
            cleanup_temp_audio(&temp_audio_paths);
            return Err(RecordError::PipelineError(format!(
                "audio track source not found: {}",
                path.display()
            )));
        }
        if is_temp {
            temp_audio_paths.push(path.clone());
        }
        audio_paths.push((clip.clone(), path));
    }

    emit(Event::RecordAudioMuxStart {
        inputs: audio_paths.len(),
    });
    let muxed_path = video_path.with_extension("with-audio.mp4");
    let mut command = Command::new(ffmpeg);
    command
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (_clip, path) in &audio_paths {
        command.arg("-i").arg(path);
    }

    command
        .arg("-filter_complex")
        .arg(audio_filter(&audio_paths, duration_s))
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("[aout]")
        .arg("-c:v")
        .arg("copy")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("192k")
        .arg("-t")
        .arg(format!("{duration_s:.3}"))
        .arg("-movflags")
        .arg("+faststart")
        .arg(&muxed_path);

    let output = command
        .output()
        .map_err(|err| RecordError::PipelineError(format!("spawn ffmpeg for audio mux: {err}")))?;
    if !output.status.success() {
        cleanup_temp_audio(&temp_audio_paths);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RecordError::PipelineError(format!(
            "ffmpeg audio mux failed: {}",
            stderr.trim()
        )));
    }
    fs::rename(&muxed_path, video_path)
        .map_err(|err| RecordError::PipelineError(format!("replace muxed MP4: {err}")))?;
    emit(Event::RecordAudioMuxDone {
        inputs: audio_paths.len(),
    });
    cleanup_temp_audio(&temp_audio_paths);
    Ok(true)
}

fn collect_audio_clips(source: &serde_json::Value) -> Vec<AudioClip> {
    source
        .get("tracks")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|track| track.get("kind").and_then(serde_json::Value::as_str) == Some("audio"))
        .filter_map(|track| track.get("clips").and_then(serde_json::Value::as_array))
        .flatten()
        .filter_map(|clip| {
            let params = clip.get("params")?;
            let src = params.get("src").and_then(serde_json::Value::as_str)?;
            let begin_ms = clip
                .get("begin")
                .or_else(|| clip.get("begin_ms"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let volume = params
                .get("volume")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1.0);
            Some(AudioClip {
                src: src.to_string(),
                begin_ms,
                volume,
            })
        })
        .collect()
}

fn source_has_audio_tracks(source: &serde_json::Value) -> bool {
    source
        .get("tracks")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .any(|track| track.get("kind").and_then(serde_json::Value::as_str) == Some("audio"))
}

fn audio_filter(audio_paths: &[(AudioClip, PathBuf)], duration_s: f64) -> String {
    let mut parts = Vec::new();
    let mut labels = Vec::new();
    for (index, (clip, _path)) in audio_paths.iter().enumerate() {
        let input = index + 1;
        let label = format!("a{index}");
        parts.push(format!(
            "[{input}:a]adelay={}:all=1,volume={:.3}[{label}]",
            clip.begin_ms, clip.volume
        ));
        labels.push(format!("[{label}]"));
    }
    if labels.len() == 1 {
        parts.push(format!(
            "{}apad,atrim=0:{duration_s:.3},alimiter=limit=0.95[aout]",
            labels.join("")
        ));
    } else {
        parts.push(format!(
            "{}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0,apad,atrim=0:{duration_s:.3},alimiter=limit=0.95[aout]",
            labels.join(""),
            labels.len()
        ));
    }
    parts.join(";")
}

pub(super) fn cleanup_export_temp_outputs(output: &Path) {
    let Some(dir) = output.parent() else {
        return;
    };
    let Some(stem) = output.file_stem().and_then(|value| value.to_str()) else {
        return;
    };
    let with_audio = output.with_extension("with-audio.mp4");
    let _ = fs::remove_file(&with_audio);
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with(&format!("{stem}.seg"))
            || name.starts_with(&format!("{stem}.audio-"))
            || name == format!("{stem}.concat.txt")
        {
            let _ = fs::remove_file(path);
        }
    }
}

fn audio_src_to_path(
    src: &str,
    source_dir: &Path,
    video_path: &Path,
    index: usize,
) -> Result<(PathBuf, bool), String> {
    if let Some(raw) = src.strip_prefix("file://") {
        return Ok((PathBuf::from(percent_decode(raw)?), false));
    }
    if src.starts_with("data:audio/") {
        return write_data_audio(src, video_path, index).map(|path| (path, true));
    }
    let path = PathBuf::from(src);
    if path.is_absolute() {
        Ok((path, false))
    } else {
        Ok((source_dir.join(path), false))
    }
}

fn write_data_audio(src: &str, video_path: &Path, index: usize) -> Result<PathBuf, String> {
    let (_metadata, encoded) = src
        .split_once(";base64,")
        .ok_or_else(|| "data audio src must contain ;base64,".to_string())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| format!("decode data audio src: {err}"))?;
    let path = temp_audio_path(video_path, index, data_audio_extension(src));
    fs::write(&path, bytes).map_err(|err| format!("write temp audio {}: {err}", path.display()))?;
    Ok(path)
}

fn temp_audio_path(video_path: &Path, index: usize, extension: &str) -> PathBuf {
    let stem = video_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("capy-timeline-export");
    video_path.with_file_name(format!("{stem}.audio-{index}.{extension}"))
}

fn data_audio_extension(src: &str) -> &'static str {
    if src.starts_with("data:audio/wav") || src.starts_with("data:audio/wave") {
        "wav"
    } else if src.starts_with("data:audio/mp4") || src.starts_with("data:audio/aac") {
        "m4a"
    } else {
        "mp3"
    }
}

fn cleanup_temp_audio(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hi = hex_value(bytes[index + 1])?;
                let lo = hex_value(bytes[index + 2])?;
                out.push((hi << 4) | lo);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|err| err.to_string())
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("invalid percent encoding".to_string()),
    }
}

fn resolve_ffmpeg() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("FFMPEG_BIN") {
        return Some(PathBuf::from(path));
    }
    for candidate in [
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "ffmpeg",
    ] {
        let path = PathBuf::from(candidate);
        if candidate.contains('/') {
            if path.exists() {
                return Some(path);
            }
        } else {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{audio_filter, AudioClip};

    #[test]
    fn audio_filter_keeps_preview_level_for_multi_input_mix() {
        let clips = vec![
            (
                AudioClip {
                    src: "a.mp3".to_string(),
                    begin_ms: 0,
                    volume: 1.0,
                },
                PathBuf::from("a.mp3"),
            ),
            (
                AudioClip {
                    src: "b.mp3".to_string(),
                    begin_ms: 1200,
                    volume: 1.0,
                },
                PathBuf::from("b.mp3"),
            ),
        ];

        let filter = audio_filter(&clips, 10.0);
        assert!(filter.contains("amix=inputs=2"));
        assert!(filter.contains("normalize=0"));
        assert!(filter.contains("alimiter=limit=0.95"));
    }
}
