use image::{Rgba, RgbaImage};
use serde_json::Value;

use super::{Rect, fill_rect};

pub(super) fn has_scroll_chapter_component(source: &Value) -> bool {
    source
        .get("components")
        .and_then(|components| components.get("html.capy-scroll-chapter"))
        .is_some()
}

pub(super) fn has_component_tracks(source: &Value) -> bool {
    source
        .get("tracks")
        .and_then(Value::as_array)
        .map(|tracks| {
            tracks.iter().any(|track| {
                track.get("kind").and_then(Value::as_str) == Some("component")
                    || track
                        .get("clips")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .any(|clip| {
                            clip.get("params")
                                .and_then(|params| params.get("component"))
                                .and_then(Value::as_str)
                                .is_some()
                        })
            })
        })
        .unwrap_or(false)
}

pub(super) fn render_component_placeholder(image: &mut RgbaImage, source: &Value) {
    let width = image.width();
    let height = image.height();
    fill_rect(
        image,
        Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        },
        Rgba([15, 23, 42, 255]),
    );
    fill_rect(
        image,
        Rect {
            x: 0,
            y: 0,
            w: width / 3,
            h: height,
        },
        Rgba([38, 75, 84, 255]),
    );
    let params = first_component_params(source);
    let margin_x = width / 6;
    let mut y = height / 4;
    let accent_h = (height / 34).max(10);
    let accent_w = text_bar_width(width, params.get("eyebrow").and_then(Value::as_str), 8, 5);
    fill_rect(
        image,
        Rect {
            x: margin_x,
            y,
            w: accent_w,
            h: accent_h,
        },
        Rgba([94, 234, 212, 255]),
    );
    y = y.saturating_add(height / 9);
    let title_w = text_bar_width(width, params.get("title").and_then(Value::as_str), 18, 3);
    let title_h = (height / 11).max(24);
    fill_rect(
        image,
        Rect {
            x: margin_x,
            y,
            w: title_w,
            h: title_h,
        },
        Rgba([248, 250, 252, 255]),
    );
    y = y.saturating_add(title_h + height / 18);
    let subtitle_w = text_bar_width(width, params.get("subtitle").and_then(Value::as_str), 10, 4);
    fill_rect(
        image,
        Rect {
            x: margin_x,
            y,
            w: subtitle_w,
            h: (height / 22).max(14),
        },
        Rgba([203, 213, 225, 255]),
    );
}

pub(super) fn render_scroll_placeholder(image: &mut RgbaImage) {
    let width = image.width();
    let height = image.height();
    let panel = Rect {
        x: width / 8,
        y: height / 3,
        w: width.saturating_mul(3) / 4,
        h: height / 3,
    };
    fill_rect(image, panel, Rgba([31, 41, 55, 255]));
    let line_h = (panel.h / 10).max(4);
    fill_rect(
        image,
        Rect {
            x: panel.x + panel.w / 12,
            y: panel.y + panel.h / 4,
            w: panel.w / 3,
            h: line_h,
        },
        Rgba([156, 163, 175, 255]),
    );
    fill_rect(
        image,
        Rect {
            x: panel.x + panel.w / 12,
            y: panel.y + panel.h / 2,
            w: panel.w.saturating_mul(2) / 3,
            h: line_h * 2,
        },
        Rgba([249, 250, 251, 255]),
    );
}

fn first_component_params(source: &Value) -> serde_json::Map<String, Value> {
    let Some(tracks) = source.get("tracks").and_then(Value::as_array) else {
        return serde_json::Map::new();
    };
    for track in tracks {
        let Some(clips) = track.get("clips").and_then(Value::as_array) else {
            continue;
        };
        for clip in clips {
            if let Some(params) = clip
                .get("params")
                .and_then(|params| params.get("params"))
                .and_then(Value::as_object)
            {
                return params.clone();
            }
        }
    }
    serde_json::Map::new()
}

fn text_bar_width(width: u32, text: Option<&str>, per_char: u32, min_units: u32) -> u32 {
    let chars = text
        .map(|value| value.chars().count() as u32)
        .unwrap_or(min_units)
        .max(min_units);
    let max = width.saturating_mul(2) / 3;
    chars
        .saturating_mul(per_char)
        .saturating_mul(width / 100)
        .min(max)
        .max(width / 8)
}
