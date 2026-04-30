use wasm_bindgen::prelude::*;

use capy_canvas_core::state::{CanvasContentKind, FillStyle};

use super::{redraw_via_shared, shared_state};

pub(super) fn set_vector_style(
    stroke: &str,
    fill: &str,
    fill_style: &str,
) -> Result<String, JsValue> {
    let stroke = parse_hex_color(stroke)
        .ok_or_else(|| JsValue::from_str(&format!("set_vector_style(): bad stroke '{stroke}'")))?;
    let fill = parse_hex_color(fill)
        .ok_or_else(|| JsValue::from_str(&format!("set_vector_style(): bad fill '{fill}'")))?;
    let fill_style = parse_fill_style(fill_style).ok_or_else(|| {
        JsValue::from_str(&format!(
            "set_vector_style(): bad fill_style '{fill_style}'"
        ))
    })?;
    let state_arc = shared_state().ok_or_else(|| {
        JsValue::from_str("set_vector_style(): no shared state · call start() first")
    })?;
    {
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("set_vector_style(): state lock poisoned"))?;
        state.color = stroke;
        state.fill_color = fill;
        state.fill_style = fill_style;
        state.stroke_width = 2.2;
        let selected = state.selected.clone();
        for idx in selected {
            let Some(shape) = state.shapes.get_mut(idx) else {
                continue;
            };
            if shape.content_kind() != CanvasContentKind::Shape {
                continue;
            }
            shape.stroke_color = stroke;
            shape.color = fill;
            shape.fill_style = fill_style;
            shape.stroke_width = 2.2;
        }
    }
    redraw_via_shared();
    Ok(format!(
        "stroke=#{stroke:06x};fill=#{fill:06x};fill_style={}",
        fill_style_label(fill_style)
    ))
}

fn parse_hex_color(value: &str) -> Option<u32> {
    let raw = value.trim().trim_start_matches('#');
    if raw.len() == 6 {
        u32::from_str_radix(raw, 16).ok()
    } else if raw.len() == 3 {
        let mut expanded = String::with_capacity(6);
        for ch in raw.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        u32::from_str_radix(&expanded, 16).ok()
    } else {
        None
    }
}

fn parse_fill_style(value: &str) -> Option<FillStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "hachure" | "sketch" => Some(FillStyle::Hachure),
        "solid" => Some(FillStyle::Solid),
        "none" | "transparent" => Some(FillStyle::None),
        _ => None,
    }
}

fn fill_style_label(value: FillStyle) -> &'static str {
    match value {
        FillStyle::None => "none",
        FillStyle::Solid => "solid",
        FillStyle::Hachure => "hachure",
    }
}
