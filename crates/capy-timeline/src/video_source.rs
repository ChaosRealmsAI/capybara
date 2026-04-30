use std::path::{Path, PathBuf};

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RenderVideoSource {
    pub path: PathBuf,
    pub timeline_begin_ms: u64,
    pub timeline_end_ms: u64,
    pub start_ms: u64,
    pub duration_ms: u64,
}

pub(crate) fn video_sources(
    source: &Value,
    default_duration_ms: u64,
) -> Result<Vec<RenderVideoSource>, String> {
    let mut videos = Vec::new();
    let Some(tracks) = source.get("tracks").and_then(Value::as_array) else {
        return Ok(videos);
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
            let begin_ms = clip
                .get("begin_ms")
                .or_else(|| clip.get("begin"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
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
            let duration_ms = params
                .get("source_end_ms")
                .or_else(|| params.get("to_ms"))
                .and_then(Value::as_u64)
                .map(|end| {
                    end.saturating_sub(
                        params
                            .get("source_start_ms")
                            .or_else(|| params.get("from_ms"))
                            .and_then(Value::as_u64)
                            .unwrap_or(0),
                    )
                })
                .or(clip_duration)
                .unwrap_or(default_duration_ms)
                .max(1);
            let start_ms = params
                .get("source_start_ms")
                .or_else(|| params.get("from_ms"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            videos.push(RenderVideoSource {
                path,
                timeline_begin_ms: begin_ms,
                timeline_end_ms: begin_ms.saturating_add(duration_ms),
                start_ms,
                duration_ms,
            });
        }
    }
    videos.sort_by_key(|video| video.timeline_begin_ms);
    Ok(videos)
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

        let video = video_sources(&source, 2000)?
            .into_iter()
            .next()
            .ok_or("missing video")?;

        assert_eq!(video.path, PathBuf::from("/tmp/hello world.mp4"));
        assert_eq!(video.timeline_begin_ms, 0);
        assert_eq!(video.timeline_end_ms, 2000);
        assert_eq!(video.start_ms, 1000);
        assert_eq!(video.duration_ms, 2000);
        Ok(())
    }

    #[test]
    fn detects_ordered_video_sources() -> Result<(), Box<dyn std::error::Error>> {
        let source = json!({
            "duration_ms": 3000,
            "tracks": [
                {
                    "id": "b.video",
                    "kind": "video",
                    "clips": [{
                        "id": "b",
                        "begin_ms": 1200,
                        "end_ms": 3000,
                        "params": {
                            "src": "file:///tmp/b.mp4",
                            "source_start_ms": 500,
                            "source_end_ms": 2300
                        }
                    }]
                },
                {
                    "id": "a.video",
                    "kind": "video",
                    "clips": [{
                        "id": "a",
                        "begin_ms": 0,
                        "end_ms": 1200,
                        "params": {
                            "src": "file:///tmp/a.mp4",
                            "source_start_ms": 100,
                            "source_end_ms": 1300
                        }
                    }]
                }
            ]
        });

        let videos = video_sources(&source, 3000)?;

        assert_eq!(videos.len(), 2);
        assert_eq!(videos[0].path, PathBuf::from("/tmp/a.mp4"));
        assert_eq!(videos[0].timeline_begin_ms, 0);
        assert_eq!(videos[1].path, PathBuf::from("/tmp/b.mp4"));
        assert_eq!(videos[1].timeline_begin_ms, 1200);
        Ok(())
    }
}
