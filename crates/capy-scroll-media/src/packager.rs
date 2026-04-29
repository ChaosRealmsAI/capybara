use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::templates;
use crate::types::{
    PackFile, ScrollPackManifest, ScrollPackReport, ScrollPackRequest, VerificationSummary,
};

mod ffmpeg;

pub(crate) use ffmpeg::{
    encode_all_keyframe_clip, read_source_metadata, verify_all_keyframes, write_poster,
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
}
