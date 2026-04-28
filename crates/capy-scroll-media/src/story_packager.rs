use std::fs;
use std::path::{Path, PathBuf};

use crate::packager::{
    Result, ScrollMediaError, encode_all_keyframe_clip, prepare_output_dir, read_source_metadata,
    verify_all_keyframes, write_poster, write_text,
};
use crate::templates;
use crate::types::{
    ClipPreset, ClipVerification, PackFile, SourceMetadata, StoryPackChapter, StoryPackManifest,
    StoryPackReport, StoryPackRequest, StorySourceChapter, StorySourceManifest,
    VerificationSummary,
};

pub fn story_pack(request: StoryPackRequest) -> Result<StoryPackReport> {
    validate_request(&request)?;
    let source_manifest = read_story_source(&request.manifest)?;
    validate_story_source(&source_manifest)?;
    let manifest_path = request.out_dir.join("manifest.json");

    if request.dry_run {
        return Ok(StoryPackReport {
            ok: true,
            dry_run: true,
            input: request.manifest.display().to_string(),
            output_dir: request.out_dir.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            manifest: None,
            files: planned_files(&source_manifest, &request),
            verification: None,
        });
    }

    prepare_output_dir(&request.out_dir, request.overwrite)?;
    let base_dir = request
        .manifest
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut chapters = Vec::with_capacity(source_manifest.chapters.len());
    for chapter in &source_manifest.chapters {
        chapters.push(package_chapter(chapter, base_dir, &request)?);
    }

    write_runtime_files(&request.out_dir)?;
    let manifest = StoryPackManifest::from_source(source_manifest, chapters);
    write_json(&manifest_path, &manifest)?;

    let verification = if request.verify {
        Some(verify_story_clips(&request.out_dir, &manifest)?)
    } else {
        None
    };

    let mut report = StoryPackReport {
        ok: true,
        dry_run: false,
        input: request.manifest.display().to_string(),
        output_dir: request.out_dir.display().to_string(),
        manifest_path: manifest_path.display().to_string(),
        manifest: Some(manifest),
        files: collect_files(&request.out_dir)?,
        verification,
    };

    let metrics_path = request.out_dir.join("evidence").join("metrics.json");
    write_json(&metrics_path, &report)?;
    if let Ok(metadata) = fs::metadata(&metrics_path) {
        report.files.push(PackFile {
            role: "metrics".to_string(),
            path: "evidence/metrics.json".to_string(),
            bytes: Some(metadata.len()),
        });
        write_json(&metrics_path, &report)?;
    }
    Ok(report)
}

fn validate_request(request: &StoryPackRequest) -> Result<()> {
    if request.poster_width == 0 {
        return Err(ScrollMediaError::Message(
            "--poster-width must be greater than 0".to_string(),
        ));
    }
    if !request.manifest.is_file() {
        return Err(ScrollMediaError::Message(format!(
            "story manifest not found: {}",
            request.manifest.display()
        )));
    }
    Ok(())
}

fn read_story_source(path: &Path) -> Result<StorySourceManifest> {
    let raw = fs::read_to_string(path)
        .map_err(|err| ScrollMediaError::Message(format!("read story manifest failed: {err}")))?;
    serde_json::from_str(&raw)
        .map_err(|err| ScrollMediaError::Message(format!("parse story manifest failed: {err}")))
}

fn validate_story_source(source: &StorySourceManifest) -> Result<()> {
    if source.schema_version != 1 {
        return Err(ScrollMediaError::Message(format!(
            "unsupported story schema_version {}; expected 1",
            source.schema_version
        )));
    }
    if source.title.trim().is_empty() {
        return Err(ScrollMediaError::Message(
            "story title must not be empty".to_string(),
        ));
    }
    if source.chapters.is_empty() {
        return Err(ScrollMediaError::Message(
            "story must contain at least one chapter".to_string(),
        ));
    }
    for chapter in &source.chapters {
        if chapter.id.trim().is_empty() {
            return Err(ScrollMediaError::Message(
                "chapter id must not be empty".to_string(),
            ));
        }
        if sanitize_id(&chapter.id) != chapter.id {
            return Err(ScrollMediaError::Message(format!(
                "chapter id must be ascii lowercase, digits, dash, or underscore: {}",
                chapter.id
            )));
        }
        if chapter.title.trim().is_empty() {
            return Err(ScrollMediaError::Message(format!(
                "chapter {} title must not be empty",
                chapter.id
            )));
        }
        if chapter.video.as_os_str().is_empty() {
            return Err(ScrollMediaError::Message(format!(
                "chapter {} video must not be empty",
                chapter.id
            )));
        }
    }
    Ok(())
}

fn package_chapter(
    chapter: &StorySourceChapter,
    base_dir: &Path,
    request: &StoryPackRequest,
) -> Result<StoryPackChapter> {
    let input = resolve_input(base_dir, &chapter.video);
    if !input.is_file() {
        return Err(ScrollMediaError::Message(format!(
            "chapter {} video not found: {}",
            chapter.id,
            input.display()
        )));
    }
    let source = read_source_metadata(&input)?;
    let poster = format!("posters/{}-{}.jpg", chapter.id, request.poster_width);
    write_poster(&input, &request.out_dir.join(&poster), request.poster_width)?;

    let default_clip = encode_chapter_clip(
        &input,
        &request.out_dir,
        &chapter.id,
        &source,
        &request.default_preset,
    )?;
    let fallback_clip = encode_chapter_clip(
        &input,
        &request.out_dir,
        &chapter.id,
        &source,
        &request.fallback_preset,
    )?;
    let hq_clip = encode_chapter_clip(
        &input,
        &request.out_dir,
        &chapter.id,
        &source,
        &request.hq_preset,
    )?;

    Ok(StoryPackChapter {
        id: chapter.id.clone(),
        title: chapter.title.clone(),
        kicker: chapter.kicker.clone(),
        body: chapter.body.clone(),
        poster,
        default_clip,
        fallback_clip,
        hq_clip,
        source,
    })
}

fn encode_chapter_clip(
    input: &Path,
    out_dir: &Path,
    id: &str,
    source: &SourceMetadata,
    preset: &ClipPreset,
) -> Result<String> {
    let height = preset.height.min(source.height);
    let relative = format!(
        "clips/{id}-{height}-crf{}-allkey.mp4",
        u16::from(preset.crf)
    );
    encode_all_keyframe_clip(input, &out_dir.join(&relative), height, preset.crf)?;
    Ok(relative)
}

fn resolve_input(base_dir: &Path, input: &Path) -> PathBuf {
    if input.is_absolute() {
        input.to_path_buf()
    } else {
        base_dir.join(input)
    }
}

fn write_runtime_files(out_dir: &Path) -> Result<()> {
    write_text(
        &out_dir.join("runtime").join("multi-video-story.css"),
        templates::multi_video_story_css(),
    )?;
    write_text(
        &out_dir.join("runtime").join("multi-video-story.js"),
        templates::multi_video_story_js(),
    )?;
    write_text(
        &out_dir.join("story.html"),
        templates::multi_video_story_html(),
    )?;
    Ok(())
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let raw = serde_json::to_string_pretty(value)
        .map_err(|err| ScrollMediaError::Message(format!("serialize JSON failed: {err}")))?;
    write_text(path, &raw)
}

fn verify_story_clips(out_dir: &Path, manifest: &StoryPackManifest) -> Result<VerificationSummary> {
    let mut clips: Vec<ClipVerification> = Vec::new();
    for chapter in &manifest.chapters {
        for clip in [
            chapter.default_clip.as_str(),
            chapter.fallback_clip.as_str(),
            chapter.hq_clip.as_str(),
        ] {
            clips.push(verify_all_keyframes(&out_dir.join(clip))?);
        }
    }
    let all_keyframe_clips = clips.iter().all(|clip| clip.non_keyframe_count == 0);
    if !all_keyframe_clips {
        return Err(ScrollMediaError::Message(
            "one or more story clips contain non-keyframes".to_string(),
        ));
    }
    Ok(VerificationSummary {
        checked: true,
        all_keyframe_clips,
        clips,
    })
}

fn collect_files(out_dir: &Path) -> Result<Vec<PackFile>> {
    let mut files = Vec::new();
    collect_files_recursive(out_dir, out_dir, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_files_recursive(root: &Path, dir: &Path, files: &mut Vec<PackFile>) -> Result<()> {
    for entry in fs::read_dir(dir)
        .map_err(|err| ScrollMediaError::Message(format!("read output dir failed: {err}")))?
    {
        let entry = entry
            .map_err(|err| ScrollMediaError::Message(format!("read dir entry failed: {err}")))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, files)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|err| ScrollMediaError::Message(format!("strip prefix failed: {err}")))?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = entry
            .metadata()
            .map_err(|err| ScrollMediaError::Message(format!("read metadata failed: {err}")))?
            .len();
        files.push(PackFile {
            role: role_for_path(&relative).to_string(),
            path: relative,
            bytes: Some(bytes),
        });
    }
    Ok(())
}

fn role_for_path(path: &str) -> &'static str {
    if path == "manifest.json" {
        "manifest"
    } else if path == "story.html" {
        "story-html"
    } else if path.starts_with("runtime/") {
        "runtime"
    } else if path.starts_with("posters/") {
        "poster"
    } else if path.starts_with("clips/") {
        "clip"
    } else if path.starts_with("evidence/") {
        "evidence"
    } else {
        "file"
    }
}

fn planned_files(source: &StorySourceManifest, request: &StoryPackRequest) -> Vec<PackFile> {
    let mut files = vec![
        planned("manifest", "manifest.json"),
        planned("story-html", "story.html"),
        planned("runtime", "runtime/multi-video-story.css"),
        planned("runtime", "runtime/multi-video-story.js"),
        planned("metrics", "evidence/metrics.json"),
    ];
    for chapter in &source.chapters {
        files.push(planned(
            "poster",
            &format!("posters/{}-{}.jpg", chapter.id, request.poster_width),
        ));
        for preset in request.presets() {
            files.push(planned(
                "clip",
                &format!(
                    "clips/{}-{}-crf{}-allkey.mp4",
                    chapter.id, preset.height, preset.crf
                ),
            ));
        }
    }
    files
}

fn planned(role: &str, path: &str) -> PackFile {
    PackFile {
        role: role.to_string(),
        path: path.to_string(),
        bytes: None,
    }
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .filter(|ch| matches!(ch, 'a'..='z' | '0'..='9' | '-' | '_'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ClipPreset, ClipRole};

    #[test]
    fn dry_run_plans_story_files() -> Result<()> {
        let source = StorySourceManifest {
            schema_version: 1,
            title: "Story".to_string(),
            eyebrow: String::new(),
            summary: String::new(),
            theme: "watch".to_string(),
            chapters: vec![StorySourceChapter {
                id: "hero".to_string(),
                title: "Hero".to_string(),
                kicker: String::new(),
                body: "Body".to_string(),
                video: PathBuf::from("hero.mp4"),
            }],
        };
        let request = StoryPackRequest {
            manifest: PathBuf::from("story.json"),
            out_dir: PathBuf::from("target/story"),
            poster_width: 1280,
            default_preset: ClipPreset::new(ClipRole::Default, 720, 23),
            fallback_preset: ClipPreset::new(ClipRole::Fallback, 720, 27),
            hq_preset: ClipPreset::new(ClipRole::Hq, 1080, 24),
            verify: true,
            overwrite: false,
            dry_run: true,
        };
        let files = planned_files(&source, &request);
        assert!(files.iter().any(|file| file.path == "story.html"));
        assert!(
            files
                .iter()
                .any(|file| file.path == "clips/hero-1080-crf24-allkey.mp4")
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_chapter_ids() {
        let source = StorySourceManifest {
            schema_version: 1,
            title: "Story".to_string(),
            eyebrow: String::new(),
            summary: String::new(),
            theme: "watch".to_string(),
            chapters: vec![StorySourceChapter {
                id: "Hero Clip".to_string(),
                title: "Hero".to_string(),
                kicker: String::new(),
                body: "Body".to_string(),
                video: PathBuf::from("hero.mp4"),
            }],
        };
        assert!(validate_story_source(&source).is_err());
    }
}
