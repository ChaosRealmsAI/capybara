use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::packager::{Result, ScrollMediaError, prepare_output_dir, read_source_metadata};
use crate::story_packager::{read_story_source, validate_story_source};
use crate::types::{
    CompositionEmitReport, ScrollCompositionRequest, SourceMetadata, StoryCompositionRequest,
    StorySourceChapter, StorySourceManifest,
};

const COMPOSITION_SCHEMA: &str = "nextframe.composition.v2";
const CAPY_COMPOSITION_SCHEMA_VERSION: &str = "capy.composition.v1";
const SCROLL_CHAPTER_COMPONENT_ID: &str = "html.capy-scroll-chapter";
const COMPONENT_FILE: &str = "components/html.capy-scroll-chapter.js";
const COMPONENT_JS: &str = r##"export function mount(root) {
  root.textContent = "";
}

export function update(root, ctx) {
  const params = ctx && ctx.params ? ctx.params : {};
  root.dataset.renderState = "ready";
  root.dataset.capyScrollComponent = "html.capy-scroll-chapter";
  root.style.position = "absolute";
  root.style.inset = "0";
  root.style.display = "grid";
  root.style.placeItems = "center";
  root.style.boxSizing = "border-box";
  root.style.padding = "48px";
  root.style.background = "#111827";
  root.style.color = "#f9fafb";
  root.style.fontFamily = "system-ui, -apple-system, BlinkMacSystemFont, sans-serif";

  const frame = document.createElement("div");
  frame.className = "capy-scroll-chapter-placeholder";
  frame.style.maxWidth = "760px";
  frame.style.width = "100%";
  frame.style.border = "1px solid rgba(255,255,255,0.18)";
  frame.style.borderRadius = "8px";
  frame.style.padding = "28px";
  frame.style.background = "rgba(255,255,255,0.08)";

  const label = document.createElement("p");
  label.textContent = "Scroll chapter " + String(Number(params.chapter_index || 0) + 1);
  label.style.margin = "0 0 12px";
  label.style.fontSize = "14px";
  label.style.opacity = "0.72";

  const title = document.createElement("h1");
  title.textContent = String(params.title || "Untitled chapter");
  title.style.margin = "0 0 16px";
  title.style.fontSize = "34px";
  title.style.lineHeight = "1.1";

  const narration = document.createElement("p");
  narration.textContent = String(params.narration || "");
  narration.style.margin = "0";
  narration.style.fontSize = "18px";
  narration.style.lineHeight = "1.45";
  narration.style.opacity = "0.88";

  frame.replaceChildren(label, title, narration);
  root.replaceChildren(frame);
}

export function destroy(root) {
  root.textContent = "";
}
"##;

pub fn emit_scroll_composition(request: ScrollCompositionRequest) -> Result<CompositionEmitReport> {
    validate_scroll_request(&request)?;
    prepare_output_dir(&request.out_dir, request.overwrite)?;
    let source = metadata_for_input(&request.input)?;
    let duration_ms = duration_ms(&source).max(1);
    let chapter = ChapterInput {
        title: request.name.clone(),
        narration: String::new(),
        video: request.input.clone(),
        source,
        start_ms: 0,
        end_ms: duration_ms,
    };
    write_composition_package(
        &request.out_dir,
        &request.input,
        composition_json(&request.name, "scroll-media", vec![chapter]),
    )
}

pub fn emit_story_composition(request: StoryCompositionRequest) -> Result<CompositionEmitReport> {
    let source = read_story_source(&request.manifest)?;
    validate_story_source(&source)?;
    prepare_output_dir(&request.out_dir, request.overwrite)?;
    let base_dir = request
        .manifest
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let chapters = story_chapters(&source, base_dir)?;
    write_composition_package(
        &request.out_dir,
        &request.manifest,
        composition_json(&source.title, "scroll-story", chapters),
    )
}

fn validate_scroll_request(request: &ScrollCompositionRequest) -> Result<()> {
    if request.name.trim().is_empty() {
        return Err(ScrollMediaError::Message(
            "--name must not be empty".to_string(),
        ));
    }
    if !request.input.is_file() {
        return Err(ScrollMediaError::Message(format!(
            "input video not found: {}",
            request.input.display()
        )));
    }
    Ok(())
}

fn story_chapters(source: &StorySourceManifest, base_dir: &Path) -> Result<Vec<ChapterInput>> {
    let mut start_ms = 0;
    let mut chapters = Vec::with_capacity(source.chapters.len());
    for chapter in &source.chapters {
        let video = resolve_input(base_dir, chapter);
        if !video.is_file() {
            return Err(ScrollMediaError::Message(format!(
                "chapter {} video not found: {}",
                chapter.id,
                video.display()
            )));
        }
        let source = metadata_for_input(&video)?;
        let length_ms = duration_ms(&source).max(1);
        let end_ms = start_ms + length_ms;
        chapters.push(ChapterInput {
            title: chapter.title.clone(),
            narration: chapter.body.clone(),
            video,
            source,
            start_ms,
            end_ms,
        });
        start_ms = end_ms;
    }
    Ok(chapters)
}

fn resolve_input(base_dir: &Path, chapter: &StorySourceChapter) -> PathBuf {
    if chapter.video.is_absolute() {
        chapter.video.clone()
    } else {
        base_dir.join(&chapter.video)
    }
}

fn metadata_for_input(input: &Path) -> Result<SourceMetadata> {
    match read_source_metadata(input) {
        Ok(metadata) => Ok(metadata),
        Err(err) if is_json(input) => poster_json_metadata(input).map_err(|fallback_err| {
            ScrollMediaError::Message(format!(
                "{err}; poster JSON fallback failed: {fallback_err}"
            ))
        }),
        Err(err) => Err(err),
    }
}

fn poster_json_metadata(input: &Path) -> Result<SourceMetadata> {
    let raw = fs::read_to_string(input)
        .map_err(|err| ScrollMediaError::Message(format!("read poster JSON failed: {err}")))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|err| ScrollMediaError::Message(format!("parse poster JSON failed: {err}")))?;
    let canvas = value
        .get("canvas")
        .ok_or_else(|| ScrollMediaError::Message("poster JSON canvas missing".to_string()))?;
    Ok(SourceMetadata {
        width: json_u32(canvas, "width").unwrap_or(1080),
        height: json_u32(canvas, "height").unwrap_or(1080),
        duration: 1.0,
        fps: 30.0,
        frame_count: 30,
    })
}

fn json_u32(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|raw| u32::try_from(raw).ok())
        .filter(|raw| *raw > 0)
}

fn is_json(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

fn duration_ms(source: &SourceMetadata) -> u64 {
    (source.duration * 1000.0).round() as u64
}

fn composition_json(title: &str, id: &str, chapters: Vec<ChapterInput>) -> Value {
    let duration_ms = chapters.last().map(|chapter| chapter.end_ms).unwrap_or(1);
    let viewport =
        chapters
            .first()
            .map_or(json!({"w": 1080, "h": 1920, "ratio": "9:16"}), |chapter| {
                json!({
                    "w": chapter.source.width,
                    "h": chapter.source.height,
                    "ratio": format!("{}:{}", chapter.source.width, chapter.source.height)
                })
            });
    json!({
        "schema": COMPOSITION_SCHEMA,
        "schema_version": CAPY_COMPOSITION_SCHEMA_VERSION,
        "id": id,
        "title": title,
        "name": title,
        "duration_ms": duration_ms,
        "duration": format!("{duration_ms}ms"),
        "viewport": viewport,
        "tracks": tracks_json(chapters),
        "assets": []
    })
}

fn tracks_json(chapters: Vec<ChapterInput>) -> Vec<Value> {
    chapters
        .into_iter()
        .enumerate()
        .map(|(index, chapter)| {
            json!({
                "id": format!("track-scroll-chapter-{index}"),
                "kind": "component",
                "component": SCROLL_CHAPTER_COMPONENT_ID,
                "z": i32::try_from(index).unwrap_or(i32::MAX),
                "time": {
                    "start": format!("{}ms", chapter.start_ms),
                    "end": format!("{}ms", chapter.end_ms)
                },
                "duration_ms": chapter.end_ms - chapter.start_ms,
                "params": params_json(index, &chapter)
            })
        })
        .collect()
}

fn params_json(index: usize, chapter: &ChapterInput) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("chapter_index".to_string(), json!(index)),
        (
            "video_url".to_string(),
            json!(chapter.video.display().to_string()),
        ),
        ("start_ms".to_string(), json!(chapter.start_ms)),
        ("end_ms".to_string(), json!(chapter.end_ms)),
        ("title".to_string(), json!(chapter.title)),
        ("narration".to_string(), json!(chapter.narration)),
    ])
}

fn write_composition_package(
    out_dir: &Path,
    input: &Path,
    composition: Value,
) -> Result<CompositionEmitReport> {
    let component_path = out_dir.join(COMPONENT_FILE);
    let composition_path = out_dir.join("composition.json");
    write_text(&component_path, COMPONENT_JS)?;
    write_json(&composition_path, &composition)?;
    Ok(CompositionEmitReport {
        ok: true,
        input: input.display().to_string(),
        output_dir: out_dir.display().to_string(),
        composition_path: composition_path.display().to_string(),
        component_paths: vec![component_path.display().to_string()],
        tracks: composition
            .get("tracks")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0),
        duration_ms: composition
            .get("duration_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        components: vec![SCROLL_CHAPTER_COMPONENT_ID.to_string()],
    })
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let raw = serde_json::to_string_pretty(value)
        .map_err(|err| ScrollMediaError::Message(format!("serialize composition failed: {err}")))?;
    write_text(path, &raw)
}

fn write_text(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            ScrollMediaError::Message(format!("create component parent failed: {err}"))
        })?;
    }
    fs::write(path, contents)
        .map_err(|err| ScrollMediaError::Message(format!("write {} failed: {err}", path.display())))
}

struct ChapterInput {
    title: String,
    narration: String,
    video: PathBuf,
    source: SourceMetadata,
    start_ms: u64,
    end_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_single_chapter_composition_from_poster_json() -> Result<()> {
        let root = test_dir("single");
        let input = root.join("poster.json");
        fs::create_dir_all(&root).map_err(io_error)?;
        fs::write(
            &input,
            r#"{"canvas":{"width":1280,"height":720},"layers":[]}"#,
        )
        .map_err(io_error)?;
        let out_dir = root.join("out");

        let report = emit_scroll_composition(ScrollCompositionRequest {
            input,
            out_dir: out_dir.clone(),
            name: "Demo".to_string(),
            overwrite: false,
        })?;

        let composition = read_json(&out_dir.join("composition.json"))?;
        assert!(report.ok);
        assert_eq!(report.tracks, 1);
        assert_eq!(
            composition["tracks"][0]["component"],
            SCROLL_CHAPTER_COMPONENT_ID
        );
        assert_eq!(composition["tracks"][0]["params"]["title"], "Demo");
        assert!(out_dir.join(COMPONENT_FILE).is_file());
        Ok(())
    }

    #[test]
    fn rejects_empty_scroll_composition_name() {
        let err = emit_scroll_composition(ScrollCompositionRequest {
            input: PathBuf::from("missing.json"),
            out_dir: PathBuf::from("target/capy-scroll-media-test/empty"),
            name: " ".to_string(),
            overwrite: true,
        })
        .unwrap_err();
        assert!(err.to_string().contains("--name must not be empty"));
    }

    #[test]
    fn rejects_missing_scroll_composition_input() {
        let err = emit_scroll_composition(ScrollCompositionRequest {
            input: PathBuf::from("missing.json"),
            out_dir: PathBuf::from("target/capy-scroll-media-test/missing"),
            name: "Demo".to_string(),
            overwrite: true,
        })
        .unwrap_err();
        assert!(err.to_string().contains("input video not found"));
    }

    #[test]
    fn rejects_invalid_poster_json_fallback() -> Result<()> {
        let root = test_dir("bad-json");
        let input = root.join("poster.json");
        fs::create_dir_all(&root).map_err(io_error)?;
        fs::write(&input, "{}").map_err(io_error)?;
        let err = emit_scroll_composition(ScrollCompositionRequest {
            input,
            out_dir: root.join("out"),
            name: "Demo".to_string(),
            overwrite: false,
        })
        .unwrap_err();
        assert!(err.to_string().contains("poster JSON fallback failed"));
        Ok(())
    }

    #[test]
    fn emits_story_composition_with_one_track_per_chapter() -> Result<()> {
        let root = test_dir("story");
        let input = root.join("poster.json");
        let manifest = root.join("story.json");
        fs::create_dir_all(&root).map_err(io_error)?;
        fs::write(
            &input,
            r#"{"canvas":{"width":640,"height":360},"layers":[]}"#,
        )
        .map_err(io_error)?;
        fs::write(
            &manifest,
            r#"{"schema_version":1,"title":"Story","chapters":[{"id":"one","title":"One","body":"First","video":"poster.json"},{"id":"two","title":"Two","body":"Second","video":"poster.json"}]}"#,
        )
        .map_err(io_error)?;
        let out_dir = root.join("out");

        let report = emit_story_composition(StoryCompositionRequest {
            manifest,
            out_dir: out_dir.clone(),
            overwrite: false,
        })?;

        let composition = read_json(&out_dir.join("composition.json"))?;
        assert_eq!(report.tracks, 2);
        assert_eq!(composition["duration_ms"], 2000);
        assert_eq!(composition["tracks"][1]["params"]["chapter_index"], 1);
        assert_eq!(composition["tracks"][1]["params"]["narration"], "Second");
        Ok(())
    }

    fn read_json(path: &Path) -> Result<Value> {
        let text = fs::read_to_string(path).map_err(io_error)?;
        serde_json::from_str(&text)
            .map_err(|err| ScrollMediaError::Message(format!("parse JSON failed: {err}")))
    }

    fn test_dir(name: &str) -> PathBuf {
        PathBuf::from("target")
            .join("capy-scroll-media-test")
            .join(format!("{name}-{}", std::process::id()))
    }

    fn io_error(err: std::io::Error) -> ScrollMediaError {
        ScrollMediaError::Message(err.to_string())
    }
}
