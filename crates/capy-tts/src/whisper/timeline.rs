//! whisper timeline models
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::backend::WordBoundary;

use super::aligner::{FfaOutput, FfaUnit};

/// Alignment timeline for a synthesized audio file.
///
/// Schema (v1.12.1+): both a flat top-level `words` array (primary contract
/// per `spec/interfaces.json`) AND the historical nested `segments` structure
/// are emitted, so downstream consumers can pick whichever shape they prefer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    /// Total duration of the audio in milliseconds (from the aligner, or
    /// fallback to `max(words[].end_ms)` if the aligner did not report one).
    pub duration_ms: u64,
    /// Voice identifier used during synthesis (e.g. "zh-CN-XiaoxiaoNeural").
    /// May be empty when the caller does not know / does not care.
    pub voice: String,
    /// Flat, top-level per-unit timing — `text` + start/end. Primary contract.
    pub words: Vec<TimelineWord>,
    /// Segmented view (sentence / punctuation-bounded). Kept for backward
    /// compatibility with pre-v1.12 consumers.
    pub segments: Vec<TimelineSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineSegment {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub words: Vec<TimelineWord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineWord {
    /// Verbatim text for this unit — a CJK character or a Latin word.
    /// (Field was renamed from `word` to `text` in v1.12.1 to match
    /// `spec/interfaces.json`.)
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl Timeline {
    pub fn to_boundaries(&self) -> Vec<WordBoundary> {
        self.segments
            .iter()
            .map(|segment| WordBoundary {
                text: segment.text.clone(),
                offset_ms: segment.start_ms,
                duration_ms: segment.end_ms.saturating_sub(segment.start_ms),
            })
            .collect()
    }

    pub fn write_json(&self, audio_path: &Path) -> Result<String> {
        let json_path = audio_path.with_extension("timeline.json");
        let content =
            serde_json::to_string_pretty(self).context("failed to serialize timeline JSON")?;
        std::fs::write(&json_path, &content)
            .with_context(|| format!("failed to write {}", json_path.display()))?;
        Ok(json_path.to_string_lossy().to_string())
    }
}

pub(super) fn detect_language(text: &str) -> Option<&'static str> {
    let mut cjk = 0u32;
    let mut jp = 0u32;
    let mut kr = 0u32;
    let mut total = 0u32;

    for ch in text.chars() {
        if !ch.is_alphabetic() {
            continue;
        }
        total += 1;
        match ch as u32 {
            0x4E00..=0x9FFF => cjk += 1,
            0x3040..=0x30FF => jp += 1,
            0xAC00..=0xD7AF | 0x1100..=0x11FF => kr += 1,
            _ => {}
        }
    }

    if total == 0 {
        return None;
    }
    if jp > 0 {
        return Some("ja");
    }
    if kr > 0 {
        return Some("ko");
    }
    if cjk * 100 / total > 30 {
        return Some("zh");
    }
    Some("en")
}

fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            c,
            '，' | '。'
                | '！'
                | '？'
                | '；'
                | '：'
                | '、'
                | '\u{201C}'
                | '\u{201D}'
                | '\u{2018}'
                | '\u{2019}'
                | '（'
                | '）'
                | '【'
                | '】'
                | '《'
                | '》'
                | '…'
                | '—'
                | '～'
                | '·'
        )
}

fn is_segment_terminator(c: char) -> bool {
    matches!(
        c,
        '。' | '！' | '？' | '；' | '，' | '.' | '!' | '?' | ';' | '\n'
    )
}

fn is_char_language(lang: &str) -> bool {
    matches!(lang, "zh" | "ja" | "ko")
}

fn content_count(text: &str, is_char_lang: bool) -> usize {
    if is_char_lang {
        text.chars()
            .filter(|c| !is_punct(*c) && !c.is_whitespace())
            .count()
    } else {
        text.split_whitespace().count()
    }
}

fn split_segments(original: &str, is_char_lang: bool) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut buf = String::new();

    let flush = |buf: &mut String, out: &mut Vec<(String, usize)>| {
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            buf.clear();
            return;
        }

        let count = content_count(trimmed, is_char_lang);
        if count > 0 {
            out.push((trimmed.to_string(), count));
        }
        buf.clear();
    };

    for c in original.chars() {
        buf.push(c);
        if is_segment_terminator(c) {
            flush(&mut buf, &mut out);
        }
    }
    flush(&mut buf, &mut out);
    out
}

pub(super) fn build_timeline(ffa: FfaOutput, original_text: &str, voice: &str) -> Timeline {
    let is_char_lang = is_char_language(&ffa.language);
    let segments_raw = split_segments(original_text, is_char_lang);

    let ffa_duration_ms = ffa.duration_ms;
    let mut unit_iter = ffa.units.into_iter();
    let mut segments = Vec::with_capacity(segments_raw.len());
    let mut last_end_ms = 0u64;

    for (seg_text, expected_count) in segments_raw {
        let mut taken = Vec::with_capacity(expected_count);
        for _ in 0..expected_count {
            match unit_iter.next() {
                Some(unit) => taken.push(unit),
                None => break,
            }
        }

        let (start_ms, end_ms) = if taken.is_empty() {
            (last_end_ms, last_end_ms)
        } else {
            let start_ms = taken
                .first()
                .map(|unit| unit.start_ms)
                .unwrap_or(last_end_ms);
            let end_ms = taken
                .last()
                .map(|unit| unit.end_ms.max(unit.start_ms))
                .unwrap_or(last_end_ms);
            (start_ms, end_ms)
        };

        let words = taken
            .into_iter()
            .map(|unit| TimelineWord {
                text: unit.text,
                start_ms: unit.start_ms,
                end_ms: unit.end_ms.max(unit.start_ms),
            })
            .collect();

        last_end_ms = end_ms.max(last_end_ms);
        segments.push(TimelineSegment {
            text: seg_text,
            start_ms,
            end_ms,
            words,
        });
    }

    let leftover: Vec<FfaUnit> = unit_iter.collect();
    if !leftover.is_empty() {
        if let Some(last_segment) = segments.last_mut() {
            for unit in leftover {
                last_segment.end_ms = last_segment.end_ms.max(unit.end_ms);
                last_segment.words.push(TimelineWord {
                    text: unit.text,
                    start_ms: unit.start_ms,
                    end_ms: unit.end_ms.max(unit.start_ms),
                });
            }
        }
    }

    // Flatten: segments[*].words → flat top-level words[].
    let flat_words: Vec<TimelineWord> = segments
        .iter()
        .flat_map(|seg| seg.words.iter().cloned())
        .collect();

    // duration_ms: prefer the aligner's value; fall back to max(end_ms) over
    // all flat words so the field is always meaningful.
    let duration_ms = if ffa_duration_ms > 0 {
        ffa_duration_ms
    } else {
        flat_words.iter().map(|w| w.end_ms).max().unwrap_or(0)
    };

    Timeline {
        duration_ms,
        voice: voice.to_string(),
        words: flat_words,
        segments,
    }
}

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;
