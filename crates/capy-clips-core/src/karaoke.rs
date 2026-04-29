//! Self-contained bilingual karaoke HTML generation for clip episodes.
//!
//! The episode layout follows Capybara's sentence-id-driven clips flow:
//!
//! ```text
//! <episode>/
//!   plan.json                         # optional source slug: {"source":"demo"}
//!   sources/<slug>/words.json          # source-level word timestamps, seconds
//!   clips/cut_report.json              # clip cuts, seconds
//!   clips/clip_NN.translations.zh.json # bilingual segment timing, seconds
//! ```
//!
//! The generated `clips/index.html` inlines all data so it can run from
//! `file://` as well as a local HTTP server.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ClipResult, CutReport, Word, WordsFile};

const TEMPLATE: &str = include_str!("karaoke_template.html");

/// Summary returned after writing the karaoke HTML.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KaraokeSummary {
    pub out_path: PathBuf,
    pub bytes: usize,
    pub clips: usize,
    pub segments: usize,
}

#[derive(Debug, Serialize)]
struct KaraokeData {
    clips: Vec<KaraokeClip>,
}

#[derive(Debug, Serialize)]
struct KaraokeClip {
    id: u32,
    title: String,
    file: String,
    sub: String,
    duration_s: f64,
    segments: Vec<KaraokeSegment>,
}

#[derive(Debug, Serialize)]
struct KaraokeSegment {
    start_ms: u32,
    end_ms: u32,
    en: String,
    en_words: Vec<TimedText>,
    cn: Vec<TimedText>,
    zh_chars: Vec<TimedText>,
}

#[derive(Debug, Serialize)]
struct TimedText {
    text: String,
    start_ms: u32,
    end_ms: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    sp: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct TranslationFile {
    segments: Vec<TranslationSegment>,
}

#[derive(Debug, Deserialize)]
struct TranslationSegment {
    en: String,
    start: f64,
    end: f64,
    cn: Vec<ChineseCue>,
}

#[derive(Debug, Deserialize)]
struct ChineseCue {
    text: String,
    start: f64,
    end: f64,
}

/// Generate `<episode_dir>/clips/index.html`.
pub fn generate_karaoke_html(episode_dir: &Path) -> Result<KaraokeSummary> {
    if !episode_dir.is_dir() {
        bail!("episode directory not found: {}", episode_dir.display());
    }

    let clips_dir = episode_dir.join("clips");
    let sources_dir = episode_dir.join("sources");
    let source_slug = detect_source_slug(episode_dir, &sources_dir)?;
    let words_path = sources_dir.join(&source_slug).join("words.json");
    let words = load_words(&words_path)?;
    let cut_report_path = clips_dir.join("cut_report.json");
    let cut_report = CutReport::from_path(&cut_report_path)
        .with_context(|| format!("load {}", cut_report_path.display()))?;

    if cut_report.success.is_empty() {
        bail!(
            "{} has no successful clips; run `capy clips cut` first",
            cut_report_path.display()
        );
    }

    let mut clips = Vec::with_capacity(cut_report.success.len());
    for clip in &cut_report.success {
        let translation_path =
            clips_dir.join(format!("clip_{:02}.translations.zh.json", clip.clip_num));
        let translation = load_translation(&translation_path)?;
        let segments = build_segments(&translation, clip, &words.words);

        clips.push(KaraokeClip {
            id: clip.clip_num,
            title: clip.title.clone(),
            file: clip_file_name(&clip.file),
            sub: derive_sub(&clip.text_preview),
            duration_s: clip.duration,
            segments,
        });
    }

    let data = KaraokeData { clips };
    let data_json = inline_json(&data)?;
    let html = TEMPLATE.replace("{{DATA_JSON}}", &data_json);
    let out_path = clips_dir.join("index.html");
    fs::write(&out_path, &html).with_context(|| format!("write {}", out_path.display()))?;

    Ok(KaraokeSummary {
        out_path,
        bytes: html.len(),
        clips: data.clips.len(),
        segments: data.clips.iter().map(|clip| clip.segments.len()).sum(),
    })
}

fn detect_source_slug(episode_dir: &Path, sources_dir: &Path) -> Result<String> {
    let plan_path = episode_dir.join("plan.json");
    if plan_path.is_file() {
        let raw = fs::read_to_string(&plan_path)
            .with_context(|| format!("read {}", plan_path.display()))?;
        if let Ok(value) = serde_json::from_str::<Value>(&raw) {
            if let Some(source) = value.get("source").and_then(Value::as_str) {
                if !source.trim().is_empty() {
                    return Ok(source.to_string());
                }
            }
        }
    }

    let entries =
        fs::read_dir(sources_dir).with_context(|| format!("read {}", sources_dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() && path.join("words.json").is_file() {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                return Ok(name.to_string());
            }
        }
    }

    bail!(
        "cannot detect clips source under {}; expected plan.json `source` or sources/<slug>/words.json",
        episode_dir.display()
    )
}

fn load_words(path: &Path) -> Result<WordsFile> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn load_translation(path: &Path) -> Result<TranslationFile> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn build_segments(
    translation: &TranslationFile,
    clip: &ClipResult,
    words: &[Word],
) -> Vec<KaraokeSegment> {
    translation
        .segments
        .iter()
        .map(|segment| build_segment(segment, clip, words))
        .collect()
}

fn build_segment(
    segment: &TranslationSegment,
    clip: &ClipResult,
    words: &[Word],
) -> KaraokeSegment {
    let segment_start = segment.start - 0.1;
    let segment_end = segment.end + 0.1;
    let en_words = words
        .iter()
        .filter(|word| word.start >= segment_start && word.end <= segment_end)
        .map(|word| TimedText {
            text: word.text.clone(),
            start_ms: source_to_clip_ms(word.start, clip.start),
            end_ms: source_to_clip_ms(word.end, clip.start),
            sp: None,
        })
        .collect();
    let cn = segment
        .cn
        .iter()
        .map(|cue| TimedText {
            text: cue.text.clone(),
            start_ms: source_to_clip_ms(cue.start, clip.start),
            end_ms: source_to_clip_ms(cue.end, clip.start),
            sp: None,
        })
        .collect();
    let zh_chars = segment
        .cn
        .iter()
        .flat_map(|cue| interpolate_chars(cue, clip.start))
        .collect();

    KaraokeSegment {
        start_ms: source_to_clip_ms(segment.start, clip.start),
        end_ms: source_to_clip_ms(segment.end, clip.start),
        en: segment.en.clone(),
        en_words,
        cn,
        zh_chars,
    }
}

fn interpolate_chars(cue: &ChineseCue, clip_start: f64) -> Vec<TimedText> {
    let chars = cue.text.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return Vec::new();
    }

    let start = source_to_clip_ms(cue.start, clip_start) as f64;
    let end = source_to_clip_ms(cue.end, clip_start) as f64;
    let step = (end - start).max(0.0) / chars.len() as f64;
    chars
        .into_iter()
        .enumerate()
        .map(|(index, ch)| {
            let char_start = start + index as f64 * step;
            let char_end = start + (index + 1) as f64 * step;
            TimedText {
                text: ch.to_string(),
                start_ms: char_start.round() as u32,
                end_ms: char_end.round() as u32,
                sp: ch.is_whitespace().then_some(true),
            }
        })
        .collect()
}

fn source_to_clip_ms(source_sec: f64, clip_start_sec: f64) -> u32 {
    ((source_sec - clip_start_sec).max(0.0) * 1000.0).round() as u32
}

fn clip_file_name(file: &str) -> String {
    Path::new(file)
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| file.to_string())
}

fn derive_sub(preview: &str) -> String {
    let clean = preview.replace(['\n', '\r'], " ");
    let trimmed = clean.trim();
    if trimmed.chars().count() <= 36 {
        return trimmed.to_string();
    }
    format!("{}...", trimmed.chars().take(36).collect::<String>())
}

fn inline_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)
        .context("serialize karaoke data")?
        .replace("</", "<\\/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClipFailure;

    fn write_sample_episode(root: &Path) -> Result<()> {
        let source_dir = root.join("sources/demo");
        let clips_dir = root.join("clips");
        fs::create_dir_all(&source_dir)?;
        fs::create_dir_all(&clips_dir)?;
        fs::write(root.join("plan.json"), r#"{"source":"demo","clips":[]}"#)?;
        WordsFile {
            total_words: 2,
            words: vec![
                Word {
                    text: "Hello".to_string(),
                    start: 10.0,
                    end: 10.5,
                },
                Word {
                    text: "world.".to_string(),
                    start: 10.6,
                    end: 11.0,
                },
            ],
        }
        .write_to_path(&source_dir.join("words.json"))?;
        CutReport {
            success: vec![ClipResult {
                clip_num: 1,
                title: "Opening".to_string(),
                from_id: 1,
                to_id: 1,
                start: 9.5,
                end: 11.5,
                duration: 2.0,
                file: "/tmp/clip_01.mp4".to_string(),
                text_preview: "Hello world.".to_string(),
            }],
            failed: Vec::<ClipFailure>::new(),
        }
        .write_to_path(&clips_dir.join("cut_report.json"))?;
        fs::write(
            clips_dir.join("clip_01.translations.zh.json"),
            r#"{"clip_num":1,"lang":"zh","segments":[{"id":1,"en":"Hello world.","start":10.0,"end":11.0,"cn":[{"text":"你好世界","start":10.0,"end":11.0}]}]}"#,
        )?;

        Ok(())
    }

    #[test]
    fn segment_builder_filters_words_and_interpolates_zh_chars() {
        let clip = ClipResult {
            clip_num: 1,
            title: "Clip".to_string(),
            from_id: 1,
            to_id: 1,
            start: 9.5,
            end: 11.5,
            duration: 2.0,
            file: "clip_01.mp4".to_string(),
            text_preview: "Hello world.".to_string(),
        };
        let translation = TranslationSegment {
            en: "Hello world.".to_string(),
            start: 10.0,
            end: 11.0,
            cn: vec![ChineseCue {
                text: "你好世界".to_string(),
                start: 10.0,
                end: 11.0,
            }],
        };
        let words = vec![
            Word {
                text: "Hello".to_string(),
                start: 10.0,
                end: 10.5,
            },
            Word {
                text: "world.".to_string(),
                start: 10.6,
                end: 11.0,
            },
            Word {
                text: "Outside".to_string(),
                start: 20.0,
                end: 20.5,
            },
        ];

        let segment = build_segment(&translation, &clip, &words);

        assert_eq!(segment.en_words.len(), 2);
        assert_eq!(segment.en_words[0].start_ms, 500);
        assert_eq!(segment.en_words[1].start_ms, 1100);
        assert_eq!(segment.zh_chars.len(), 4);
        assert_eq!(segment.zh_chars[0].text, "你");
        assert_eq!(segment.zh_chars[0].start_ms, 500);
        assert_eq!(segment.zh_chars[3].text, "界");
        assert!(segment.zh_chars[3].end_ms >= 1490);
    }

    #[test]
    fn generate_writes_file_url_safe_html() -> Result<()> {
        let temp = tempfile::tempdir()?;
        write_sample_episode(temp.path())?;

        let summary = generate_karaoke_html(temp.path())?;
        let html = fs::read_to_string(&summary.out_path)?;

        assert_eq!(summary.clips, 1);
        assert_eq!(summary.segments, 1);
        assert!(html.contains("const __DATA = {"));
        assert!(!html.contains("{{DATA_JSON}}"));
        assert!(html.contains(r#""file":"clip_01.mp4""#));
        assert!(html.contains("Hello world."));
        assert!(html.contains("你好世界"));
        assert!(html.contains(r#"class="video-frame""#));
        assert!(html.contains("object-fit:contain"));
        assert!(html.contains(r#"<video id="vid""#));
        Ok(())
    }
}
