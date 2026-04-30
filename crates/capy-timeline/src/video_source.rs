use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RenderVideoSource {
    pub path: PathBuf,
    pub start_ms: u64,
    pub duration_ms: u64,
}

pub(crate) fn first_video_source(
    source: &Value,
    default_duration_ms: u64,
) -> Result<Option<RenderVideoSource>, String> {
    let Some(tracks) = source.get("tracks").and_then(Value::as_array) else {
        return Ok(None);
    };
    for track in tracks {
        let kind = track.get("kind").and_then(Value::as_str).unwrap_or("");
        let Some(clips) = track.get("clips").and_then(Value::as_array) else {
            continue;
        };
        for clip in clips {
            let params = clip.get("params").unwrap_or(&Value::Null);
            let src = params
                .get("src")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty());
            if kind != "video" && src.is_none() {
                continue;
            }
            let Some(src) = src else {
                continue;
            };
            let path = source_path_from_src(src)?;
            let clip_duration = clip
                .get("end_ms")
                .or_else(|| clip.get("end"))
                .and_then(Value::as_u64)
                .zip(
                    clip.get("begin_ms")
                        .or_else(|| clip.get("begin"))
                        .and_then(Value::as_u64),
                )
                .map(|(end, begin)| end.saturating_sub(begin));
            let start_ms = params
                .get("source_start_ms")
                .or_else(|| params.get("from_ms"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let duration_ms = params
                .get("source_end_ms")
                .or_else(|| params.get("to_ms"))
                .and_then(Value::as_u64)
                .map(|end| end.saturating_sub(start_ms))
                .or(clip_duration)
                .unwrap_or(default_duration_ms)
                .max(1);
            return Ok(Some(RenderVideoSource {
                path,
                start_ms,
                duration_ms,
            }));
        }
    }
    Ok(None)
}

fn source_path_from_src(src: &str) -> Result<PathBuf, String> {
    if let Some(raw) = src.strip_prefix("file://") {
        return Ok(PathBuf::from(percent_decode(raw)?));
    }
    let path = Path::new(src);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Err(format!(
            "video src must be an absolute path or file:// URL: {src}"
        ))
    }
}

fn percent_decode(raw: &str) -> Result<String, String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hi = bytes
                .get(index + 1)
                .copied()
                .ok_or("truncated percent escape")?;
            let lo = bytes
                .get(index + 2)
                .copied()
                .ok_or("truncated percent escape")?;
            let value = hex(hi)
                .and_then(|left| hex(lo).map(|right| left * 16 + right))
                .ok_or_else(|| format!("invalid percent escape in {raw}"))?;
            out.push(value);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).map_err(|err| format!("file URL path is not UTF-8: {err}"))
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn detects_file_url_video_source_range() -> Result<(), Box<dyn std::error::Error>> {
        let source = json!({
            "duration_ms": 2000,
            "tracks": [{
                "id": "source.video",
                "kind": "video",
                "clips": [{
                    "id": "clip",
                    "begin_ms": 0,
                    "end_ms": 2000,
                    "params": {
                        "src": "file:///tmp/hello%20world.mp4",
                        "source_start_ms": 1000,
                        "source_end_ms": 3000
                    }
                }]
            }]
        });

        let video = first_video_source(&source, 2000)?.ok_or("missing video")?;

        assert_eq!(video.path, PathBuf::from("/tmp/hello world.mp4"));
        assert_eq!(video.start_ms, 1000);
        assert_eq!(video.duration_ms, 2000);
        Ok(())
    }
}
