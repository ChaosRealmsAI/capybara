#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use super::{Timeline, build_timeline, content_count, detect_language, split_segments};
use crate::whisper::aligner::{FfaOutput, FfaUnit};

fn unit(text: &str, start_ms: u64, end_ms: u64) -> FfaUnit {
    FfaUnit {
        text: text.into(),
        start_ms,
        end_ms,
    }
}

#[test]
fn detect_language_chinese() {
    assert_eq!(detect_language("你好世界"), Some("zh"));
}

#[test]
fn detect_language_english() {
    assert_eq!(detect_language("hello world"), Some("en"));
}

#[test]
fn detect_language_japanese() {
    assert_eq!(detect_language("こんにちは"), Some("ja"));
}

#[test]
fn content_count_chinese() {
    assert_eq!(content_count("你好，世界！", true), 4);
}

#[test]
fn content_count_english() {
    assert_eq!(content_count("hello, world!", false), 2);
    assert_eq!(content_count("  one   two three  ", false), 3);
}

#[test]
fn split_segments_chinese_splits_on_comma_and_period() {
    let segments = split_segments("今天天气真不错，我们一起去公园散步吧。", true);
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].0, "今天天气真不错，");
    assert_eq!(segments[0].1, 7);
    assert_eq!(segments[1].0, "我们一起去公园散步吧。");
    assert_eq!(segments[1].1, 10);
}

#[test]
fn split_segments_english_no_comma_split() {
    let segments = split_segments("Hello, world. How are you?", false);
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].0, "Hello, world.");
    assert_eq!(segments[0].1, 2);
    assert_eq!(segments[1].0, "How are you?");
    assert_eq!(segments[1].1, 3);
}

#[test]
fn build_timeline_preserves_punctuation_chinese() {
    let ffa = FfaOutput {
        duration_ms: 3200,
        language: "zh".into(),
        units: vec![
            unit("今", 0, 100),
            unit("天", 100, 200),
            unit("天", 200, 300),
            unit("气", 300, 400),
            unit("真", 400, 500),
            unit("不", 500, 600),
            unit("错", 600, 700),
            unit("我", 800, 900),
            unit("们", 900, 1000),
            unit("一", 1000, 1100),
            unit("起", 1100, 1200),
            unit("去", 1200, 1300),
            unit("公", 1300, 1400),
            unit("园", 1400, 1500),
            unit("散", 1500, 1600),
            unit("步", 1600, 1700),
            unit("吧", 1700, 1800),
        ],
    };
    let timeline = build_timeline(
        ffa,
        "今天天气真不错，我们一起去公园散步吧。",
        "zh-CN-XiaoxiaoNeural",
    );
    assert_eq!(timeline.segments.len(), 2);
    assert_eq!(timeline.segments[0].text, "今天天气真不错，");
    assert_eq!(timeline.segments[0].start_ms, 0);
    assert_eq!(timeline.segments[0].end_ms, 700);
    assert_eq!(timeline.segments[0].words.len(), 7);
    assert_eq!(timeline.segments[0].words[0].text, "今");
    assert_eq!(timeline.segments[1].text, "我们一起去公园散步吧。");
    assert_eq!(timeline.segments[1].start_ms, 800);
    assert_eq!(timeline.segments[1].end_ms, 1800);
    assert_eq!(timeline.segments[1].words.len(), 10);
}

#[test]
fn build_timeline_handles_missing_units_gracefully() {
    let ffa = FfaOutput {
        duration_ms: 2000,
        language: "zh".into(),
        units: vec![unit("你", 0, 200), unit("好", 200, 400)],
    };
    let timeline = build_timeline(ffa, "你好世界。", "");
    assert_eq!(timeline.segments.len(), 1);
    assert_eq!(timeline.segments[0].text, "你好世界。");
    assert_eq!(timeline.segments[0].start_ms, 0);
    assert_eq!(timeline.segments[0].end_ms, 400);
    assert_eq!(timeline.segments[0].words.len(), 2);
}

#[test]
fn build_timeline_english_word_units() {
    let ffa = FfaOutput {
        duration_ms: 2500,
        language: "en".into(),
        units: vec![
            unit("hello", 100, 500),
            unit("world", 600, 1000),
            unit("how", 1200, 1400),
            unit("are", 1400, 1600),
            unit("you", 1600, 1900),
        ],
    };
    let timeline = build_timeline(ffa, "Hello, world. How are you?", "en-US-AriaNeural");
    assert_eq!(timeline.segments.len(), 2);
    assert_eq!(timeline.segments[0].text, "Hello, world.");
    assert_eq!(timeline.segments[0].start_ms, 100);
    assert_eq!(timeline.segments[0].end_ms, 1000);
    assert_eq!(timeline.segments[0].words.len(), 2);
    assert_eq!(timeline.segments[1].text, "How are you?");
    assert_eq!(timeline.segments[1].start_ms, 1200);
    assert_eq!(timeline.segments[1].end_ms, 1900);
    assert_eq!(timeline.segments[1].words.len(), 3);
}

#[test]
fn build_timeline_to_boundaries_roundtrip() {
    let ffa = FfaOutput {
        duration_ms: 2000,
        language: "zh".into(),
        units: vec![
            unit("你", 0, 400),
            unit("好", 400, 800),
            unit("世", 1200, 1600),
            unit("界", 1600, 2000),
        ],
    };
    let timeline = build_timeline(ffa, "你好，世界。", "");
    let boundaries = timeline.to_boundaries();
    assert_eq!(boundaries.len(), 2);
    assert_eq!(boundaries[0].text, "你好，");
    assert_eq!(boundaries[0].offset_ms, 0);
    assert_eq!(boundaries[0].duration_ms, 800);
    assert_eq!(boundaries[1].text, "世界。");
    assert_eq!(boundaries[1].offset_ms, 1200);
    assert_eq!(boundaries[1].duration_ms, 800);
}

#[test]
fn build_timeline_leftover_units_attach_to_last_segment() {
    let ffa = FfaOutput {
        duration_ms: 1500,
        language: "zh".into(),
        units: vec![
            unit("你", 0, 300),
            unit("好", 300, 600),
            unit("世", 700, 1000),
            unit("界", 1000, 1300),
            unit("！", 1300, 1500),
        ],
    };
    let timeline = build_timeline(ffa, "你好世界", "");
    assert_eq!(timeline.segments.len(), 1);
    assert_eq!(timeline.segments[0].words.len(), 5);
    assert_eq!(timeline.segments[0].end_ms, 1500);
}

#[test]
fn build_timeline_empty_units_empty_timeline() {
    let ffa = FfaOutput {
        duration_ms: 0,
        language: "zh".into(),
        units: vec![],
    };
    let timeline = build_timeline(ffa, "你好世界", "");
    assert_eq!(timeline.segments.len(), 1);
    assert_eq!(timeline.segments[0].words.len(), 0);
}

#[test]
fn build_timeline_multi_segment_zh_long() {
    let ffa = FfaOutput {
        duration_ms: 15_744,
        language: "zh".into(),
        units: vec![
            unit("人", 220, 421),
            unit("工", 421, 601),
            unit("智", 601, 721),
            unit("能", 721, 841),
            unit("的", 841, 941),
            unit("发", 941, 1081),
            unit("展", 1081, 1222),
            unit("速", 1222, 1401),
            unit("度", 1401, 1521),
            unit("大", 4146, 4206),
            unit("模", 4206, 4387),
            unit("型", 4387, 4727),
            unit("让", 4727, 4868),
            unit("机", 4868, 5028),
            unit("器", 5028, 5188),
            unit("强", 5188, 5388),
            unit("大", 5388, 5588),
        ],
    };
    let timeline = build_timeline(ffa, "人工智能的发展速度，大模型让机器强大。", "");
    assert_eq!(timeline.segments.len(), 2);
    assert_eq!(timeline.segments[0].text, "人工智能的发展速度，");
    assert_eq!(timeline.segments[0].start_ms, 220);
    assert_eq!(timeline.segments[0].end_ms, 1521);
    assert_eq!(timeline.segments[0].words.len(), 9);
    assert_eq!(timeline.segments[1].start_ms, 4146);
    assert_eq!(timeline.segments[1].end_ms, 5588);
    assert_eq!(timeline.segments[1].text, "大模型让机器强大。");
    assert_eq!(timeline.segments[1].words.len(), 8);
}

#[test]
fn build_timeline_accepts_leading_silence() {
    let ffa = FfaOutput {
        duration_ms: 1500,
        language: "zh".into(),
        units: vec![unit("你", 300, 600), unit("好", 600, 900)],
    };
    let timeline = build_timeline(ffa, "你好", "");
    assert_eq!(timeline.segments[0].start_ms, 300);
    assert_eq!(timeline.segments[0].end_ms, 900);
}

#[test]
fn split_segments_handles_trailing_no_terminator() {
    let segments = split_segments("没有句号的一句话", true);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].0, "没有句号的一句话");
    assert_eq!(segments[0].1, 8);
}

#[test]
fn split_segments_empty_between_punct() {
    let segments = split_segments("嗯。。，啊。", true);
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0].0, "嗯。");
    assert_eq!(segments[1].0, "啊。");
}

// --- v1.12.1 schema contract tests ------------------------------------

#[test]
fn timeline_flat_words_match_segment_words_count_and_order() {
    // Primary flatten invariant: top-level `words[]` is the concatenation
    // of `segments[*].words[]` in order, with no gaps / dupes.
    let ffa = FfaOutput {
        duration_ms: 1900,
        language: "en".into(),
        units: vec![
            unit("hello", 100, 500),
            unit("world", 600, 1000),
            unit("how", 1200, 1400),
            unit("are", 1400, 1600),
            unit("you", 1600, 1900),
        ],
    };
    let timeline = build_timeline(ffa, "Hello, world. How are you?", "en-US-AriaNeural");

    let flat_count: usize = timeline.segments.iter().map(|s| s.words.len()).sum();
    assert_eq!(timeline.words.len(), flat_count);
    assert_eq!(timeline.words.len(), 5);

    let segment_concat: Vec<(String, u64, u64)> = timeline
        .segments
        .iter()
        .flat_map(|s| s.words.iter())
        .map(|w| (w.text.clone(), w.start_ms, w.end_ms))
        .collect();
    let flat: Vec<(String, u64, u64)> = timeline
        .words
        .iter()
        .map(|w| (w.text.clone(), w.start_ms, w.end_ms))
        .collect();
    assert_eq!(flat, segment_concat);
}

#[test]
fn timeline_empty_segments_produce_empty_flat_words() {
    // Edge case: if the aligner produced nothing and the text has no
    // recognizable content, `words` is empty (but still present).
    let ffa = FfaOutput {
        duration_ms: 0,
        language: "zh".into(),
        units: vec![],
    };
    let timeline = build_timeline(ffa, "", "zh-CN-XiaoxiaoNeural");
    assert!(timeline.words.is_empty());
    assert!(timeline.segments.is_empty());
    assert_eq!(timeline.duration_ms, 0);
    assert_eq!(timeline.voice, "zh-CN-XiaoxiaoNeural");
}

#[test]
fn timeline_json_has_both_flat_words_and_segments() {
    // The JSON wire-shape must contain BOTH top-level "words" (v1.12+
    // contract) AND nested "segments" (backward compat) at the same time,
    // plus top-level "duration_ms" + "voice".
    let ffa = FfaOutput {
        duration_ms: 1800,
        language: "zh".into(),
        units: vec![
            unit("你", 0, 300),
            unit("好", 300, 600),
            unit("世", 900, 1200),
            unit("界", 1200, 1500),
        ],
    };
    let timeline = build_timeline(ffa, "你好，世界。", "zh-CN-XiaoxiaoNeural");
    let json = serde_json::to_string(&timeline).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");

    assert!(parsed.get("words").is_some(), "missing top-level words");
    assert!(
        parsed.get("segments").is_some(),
        "missing top-level segments"
    );
    assert!(parsed.get("duration_ms").is_some(), "missing duration_ms");
    assert!(parsed.get("voice").is_some(), "missing voice");

    // words[] elements use `text` (not `word`) per v1.12 contract.
    let first_word = &parsed["words"][0];
    assert!(
        first_word.get("text").is_some(),
        "flat word missing `text` field"
    );
    assert!(
        first_word.get("start_ms").is_some(),
        "flat word missing `start_ms` field"
    );
    assert!(
        first_word.get("end_ms").is_some(),
        "flat word missing `end_ms` field"
    );

    assert_eq!(parsed["voice"], "zh-CN-XiaoxiaoNeural");
    assert_eq!(parsed["duration_ms"], 1800);
}

#[test]
fn timeline_duration_ms_falls_back_to_max_end_ms_when_ffa_reports_zero() {
    // When the aligner reports duration_ms: 0, we fall back to max end_ms
    // over flat words, so `duration_ms` is always meaningful downstream.
    let ffa = FfaOutput {
        duration_ms: 0,
        language: "zh".into(),
        units: vec![unit("你", 0, 400), unit("好", 400, 900)],
    };
    let timeline = build_timeline(ffa, "你好", "");
    assert_eq!(timeline.duration_ms, 900);
}

#[test]
fn timeline_deserializes_its_own_output() {
    // Round-trip: serialized JSON can be re-parsed into a Timeline.
    let ffa = FfaOutput {
        duration_ms: 900,
        language: "zh".into(),
        units: vec![unit("你", 0, 400), unit("好", 400, 900)],
    };
    let timeline = build_timeline(ffa, "你好", "zh-CN-XiaoxiaoNeural");
    let json = serde_json::to_string(&timeline).expect("serialize");
    let restored: Timeline = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.words.len(), 2);
    assert_eq!(restored.segments.len(), 1);
    assert_eq!(restored.duration_ms, 900);
    assert_eq!(restored.voice, "zh-CN-XiaoxiaoNeural");
    assert_eq!(restored.words[0].text, "你");
}
