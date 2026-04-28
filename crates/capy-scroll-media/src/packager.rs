use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;
use thiserror::Error;

use crate::templates;
use crate::types::{
    ClipVerification, PackFile, ScrollPackManifest, ScrollPackReport, ScrollPackRequest,
    SourceMetadata, VerificationSummary,
};

#[derive(Debug, Error)]
pub enum ScrollMediaError {
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, ScrollMediaError>;

pub fn scroll_pack(request: ScrollPackRequest) -> Result<ScrollPackReport> {
    validate_request(&request)?;
    let manifest_path = request.out_dir.join("manifest.json");
    let source = if request.dry_run {
        None
    } else {
        Some(read_source_metadata(&request.input)?)
    };
    let poster = format!("poster-{}.jpg", request.poster_width);
    let default_clip = request.default_preset.file_name();
    let fallback_clip = request.fallback_preset.file_name();
    let hq_clip = request.hq_preset.file_name();

    if request.dry_run {
        return Ok(ScrollPackReport {
            ok: true,
            dry_run: true,
            input: request.input.display().to_string(),
            output_dir: request.out_dir.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            source,
            manifest: None,
            files: planned_files(&poster, &default_clip, &fallback_clip, &hq_clip),
            verification: None,
        });
    }

    prepare_output_dir(&request.out_dir, request.overwrite)?;
    let source = source.ok_or_else(|| {
        ScrollMediaError::Message("source metadata missing after ffprobe".to_string())
    })?;
    let manifest = ScrollPackManifest::from_source(
        request.name.clone(),
        source.clone(),
        poster.clone(),
        default_clip.clone(),
        fallback_clip.clone(),
        hq_clip.clone(),
    );

    write_poster(
        &request.input,
        &request.out_dir.join(&poster),
        request.poster_width,
    )?;
    for preset in request.presets() {
        encode_all_keyframe_clip(
            &request.input,
            &request.out_dir.join(preset.file_name()),
            preset.height,
            preset.crf,
        )?;
    }
    write_runtime_files(&request.out_dir)?;
    write_manifest(&manifest_path, &manifest)?;

    let verification = if request.verify {
        Some(verify_clips(&request.out_dir, &manifest)?)
    } else {
        None
    };
    let mut report = ScrollPackReport {
        ok: true,
        dry_run: false,
        input: request.input.display().to_string(),
        output_dir: request.out_dir.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        source: Some(source),
        manifest: Some(manifest),
        files: collect_files(
            &request.out_dir,
            &poster,
            &default_clip,
            &fallback_clip,
            &hq_clip,
        )?,
        verification,
    };
    let metrics_path = request.out_dir.join("evidence").join("metrics.json");
    write_report(&metrics_path, &report)?;
    if let Ok(metadata) = fs::metadata(&metrics_path) {
        report.files.push(PackFile {
            role: "metrics".to_string(),
            path: "evidence/metrics.json".to_string(),
            bytes: Some(metadata.len()),
        });
        write_report(&metrics_path, &report)?;
    }
    Ok(report)
}

pub fn inspect_manifest(path: &Path) -> Result<ScrollPackManifest> {
    let raw = fs::read_to_string(path)
        .map_err(|err| ScrollMediaError::Message(format!("read manifest failed: {err}")))?;
    serde_json::from_str(&raw)
        .map_err(|err| ScrollMediaError::Message(format!("parse manifest failed: {err}")))
}

fn validate_request(request: &ScrollPackRequest) -> Result<()> {
    if request.name.trim().is_empty() {
        return Err(ScrollMediaError::Message(
            "--name must not be empty".to_string(),
        ));
    }
    if request.poster_width == 0 {
        return Err(ScrollMediaError::Message(
            "--poster-width must be greater than 0".to_string(),
        ));
    }
    if !request.dry_run && !request.input.is_file() {
        return Err(ScrollMediaError::Message(format!(
            "input video not found: {}",
            request.input.display()
        )));
    }
    Ok(())
}

pub(crate) fn prepare_output_dir(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() {
        if !overwrite {
            return Err(ScrollMediaError::Message(format!(
                "output directory already exists: {}; pass --overwrite to replace it",
                path.display()
            )));
        }
        fs::remove_dir_all(path).map_err(|err| {
            ScrollMediaError::Message(format!("remove output directory failed: {err}"))
        })?;
    }
    fs::create_dir_all(path)
        .map_err(|err| ScrollMediaError::Message(format!("create output dir failed: {err}")))
}

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

pub(crate) fn write_poster(input: &Path, output: &Path, poster_width: u32) -> Result<()> {
    create_parent_dir(output)?;
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
    create_parent_dir(output)?;
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

fn write_runtime_files(out_dir: &Path) -> Result<()> {
    let runtime_dir = out_dir.join("runtime");
    let evidence_dir = out_dir.join("evidence");
    fs::create_dir_all(&runtime_dir)
        .map_err(|err| ScrollMediaError::Message(format!("create runtime dir failed: {err}")))?;
    fs::create_dir_all(&evidence_dir)
        .map_err(|err| ScrollMediaError::Message(format!("create evidence dir failed: {err}")))?;
    write_text(
        &runtime_dir.join("scroll-video.js"),
        templates::runtime_js(),
    )?;
    write_text(
        &runtime_dir.join("scroll-video.css"),
        templates::runtime_css(),
    )?;
    write_text(&out_dir.join("demo.html"), templates::demo_html())?;
    write_text(&out_dir.join("scroll-hq.html"), templates::scroll_hq_html())?;
    write_text(
        &out_dir.join("raw-quality.html"),
        templates::raw_quality_html(),
    )?;
    Ok(())
}

fn write_manifest(path: &Path, manifest: &ScrollPackManifest) -> Result<()> {
    let raw = serde_json::to_string_pretty(manifest)
        .map_err(|err| ScrollMediaError::Message(format!("serialize manifest failed: {err}")))?;
    write_text(path, &raw)
}

fn write_report(path: &Path, report: &ScrollPackReport) -> Result<()> {
    let raw = serde_json::to_string_pretty(report)
        .map_err(|err| ScrollMediaError::Message(format!("serialize report failed: {err}")))?;
    write_text(path, &raw)
}

pub(crate) fn write_text(path: &Path, contents: &str) -> Result<()> {
    create_parent_dir(path)?;
    fs::write(path, contents)
        .map_err(|err| ScrollMediaError::Message(format!("write {} failed: {err}", path.display())))
}

fn create_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            ScrollMediaError::Message(format!("create parent directory failed: {err}"))
        })?;
    }
    Ok(())
}

fn verify_clips(out_dir: &Path, manifest: &ScrollPackManifest) -> Result<VerificationSummary> {
    let clip_paths = [
        manifest.default_clip.as_str(),
        manifest.fallback_clip.as_str(),
        manifest.hq_clip.as_str(),
    ];
    let mut clips = Vec::new();
    for clip in clip_paths {
        clips.push(verify_all_keyframes(&out_dir.join(clip))?);
    }
    let all_keyframe_clips = clips.iter().all(|clip| clip.non_keyframe_count == 0);
    if !all_keyframe_clips {
        return Err(ScrollMediaError::Message(
            "one or more scrub clips contain non-keyframes".to_string(),
        ));
    }
    Ok(VerificationSummary {
        checked: true,
        all_keyframe_clips,
        clips,
    })
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

fn collect_files(
    out_dir: &Path,
    poster: &str,
    default_clip: &str,
    fallback_clip: &str,
    hq_clip: &str,
) -> Result<Vec<PackFile>> {
    let paths = [
        ("manifest", "manifest.json"),
        ("poster", poster),
        ("default", default_clip),
        ("fallback", fallback_clip),
        ("hq", hq_clip),
        ("runtime-js", "runtime/scroll-video.js"),
        ("runtime-css", "runtime/scroll-video.css"),
        ("demo", "demo.html"),
        ("scroll-hq", "scroll-hq.html"),
        ("raw-quality", "raw-quality.html"),
    ];
    paths
        .iter()
        .map(|(role, relative)| {
            let path = out_dir.join(relative);
            let bytes = fs::metadata(&path)
                .map_err(|err| {
                    ScrollMediaError::Message(format!("read file metadata failed: {err}"))
                })?
                .len();
            Ok(PackFile {
                role: (*role).to_string(),
                path: (*relative).to_string(),
                bytes: Some(bytes),
            })
        })
        .collect()
}

fn planned_files(
    poster: &str,
    default_clip: &str,
    fallback_clip: &str,
    hq_clip: &str,
) -> Vec<PackFile> {
    [
        ("manifest", "manifest.json"),
        ("poster", poster),
        ("default", default_clip),
        ("fallback", fallback_clip),
        ("hq", hq_clip),
        ("runtime-js", "runtime/scroll-video.js"),
        ("runtime-css", "runtime/scroll-video.css"),
        ("demo", "demo.html"),
        ("scroll-hq", "scroll-hq.html"),
        ("raw-quality", "raw-quality.html"),
        ("metrics", "evidence/metrics.json"),
    ]
    .iter()
    .map(|(role, path)| PackFile {
        role: (*role).to_string(),
        path: (*path).to_string(),
        bytes: None,
    })
    .collect()
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
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn dry_run_does_not_require_input_file() -> Result<()> {
        let report = scroll_pack(ScrollPackRequest {
            input: PathBuf::from("missing.mp4"),
            out_dir: PathBuf::from("target/capy-scroll-pack-dry-run"),
            name: "watch".to_string(),
            poster_width: 1280,
            default_preset: crate::types::ClipPreset::new(crate::types::ClipRole::Default, 720, 23),
            fallback_preset: crate::types::ClipPreset::new(
                crate::types::ClipRole::Fallback,
                720,
                27,
            ),
            hq_preset: crate::types::ClipPreset::new(crate::types::ClipRole::Hq, 1080, 24),
            verify: true,
            overwrite: false,
            dry_run: true,
        })?;
        assert!(report.ok);
        assert!(report.dry_run);
        assert!(report.files.iter().any(|file| file.path == "manifest.json"));
        assert!(
            report
                .files
                .iter()
                .any(|file| file.path == "scroll-hq.html")
        );
        Ok(())
    }

    #[test]
    fn parses_fraction() {
        assert_eq!(parse_fraction("24/1"), Some(24.0));
        assert_eq!(parse_fraction("0/0"), None);
    }
}
