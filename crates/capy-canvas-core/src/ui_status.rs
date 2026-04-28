//! Lower-priority overlay panels: status bar, toasts, and help.

use vello::Scene;
use vello::kurbo::{Affine, Circle, Line, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

use crate::state::AppState;
use crate::ui::{STATUS_BG, STATUS_DIM, STATUS_H, STATUS_TEXT};

pub fn draw_status_bar(scene: &mut Scene, state: &AppState) {
    let ui_scale = crate::ui::overlay_scale(state.viewport_w, state.viewport_h);
    let vw = state.viewport_w / ui_scale;
    let vh = state.viewport_h / ui_scale;
    let mut local = Scene::new();
    let y = vh - STATUS_H;
    let bar = Rect::new(0.0, y, vw, vh);
    let bg = if state.dark_mode {
        Color::from_rgba8(0x2a, 0x2a, 0x3a, 0xff)
    } else {
        STATUS_BG
    };
    local.fill(Fill::NonZero, Affine::IDENTITY, bg, None, &bar);

    let count = state.shapes.len();
    let block_w = 3.0;
    let block_h = 3.0;
    let text_y = y + (STATUS_H - block_h) / 2.0;

    let indicator_count = count.min(20);
    for i in 0..indicator_count {
        let bx = 12.0 + i as f64 * 5.0;
        let dot = Circle::new((bx + block_w / 2.0, text_y + block_h / 2.0), 1.5);
        local.fill(Fill::NonZero, Affine::IDENTITY, STATUS_TEXT, None, &dot);
    }
    if count > 0 {
        let num_x = 12.0 + indicator_count as f64 * 5.0 + 6.0;
        draw_num(
            &mut local,
            &state.fonts,
            count,
            num_x,
            text_y - 2.0,
            STATUS_DIM,
        );
    }

    let tool_label = state.tool.label();
    let tool_font_size = 11.0_f32;
    let tool_text_w = crate::text::measure_text(&state.fonts, tool_label, tool_font_size);
    let tool_label_w = tool_text_w + 12.0;
    let tool_x = (vw - tool_label_w) / 2.0;
    let tool_h = 16.0;
    let tool_y = y + (STATUS_H - tool_h) / 2.0;
    let tool_rect = RoundedRect::from_rect(
        Rect::new(tool_x, tool_y, tool_x + tool_label_w, tool_y + tool_h),
        4.0,
    );
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0xff, 0xff, 0xff, 0x1a),
        None,
        &tool_rect,
    );
    crate::text::draw_text(
        &mut local,
        &state.fonts,
        tool_label,
        tool_x + 6.0,
        tool_y + 1.0,
        tool_font_size,
        STATUS_TEXT,
        Affine::IDENTITY,
    );

    let style_font = 10.0_f32;
    let sw_label = format!("{}px", state.stroke_width as u32);
    let sw_x = tool_x - 100.0;
    crate::text::draw_text(
        &mut local,
        &state.fonts,
        &sw_label,
        sw_x,
        tool_y + 1.0,
        style_font,
        STATUS_DIM,
        Affine::IDENTITY,
    );

    let swatch_cx = sw_x + crate::text::measure_text(&state.fonts, &sw_label, style_font) + 10.0;
    let swatch_r = 5.0;
    let swatch_cy = y + STATUS_H / 2.0;
    let swatch_fill = crate::render::color_from_hex(state.color, 1.0);
    let swatch = Circle::new((swatch_cx, swatch_cy), swatch_r);
    local.fill(Fill::NonZero, Affine::IDENTITY, swatch_fill, None, &swatch);
    local.stroke(
        &Stroke::new(0.5),
        Affine::IDENTITY,
        STATUS_DIM,
        None,
        &swatch,
    );

    let font_label = state.current_font_family.label();
    let font_x = swatch_cx + swatch_r + 8.0;
    crate::text::draw_text(
        &mut local,
        &state.fonts,
        font_label,
        font_x,
        tool_y + 1.0,
        style_font,
        STATUS_DIM,
        Affine::IDENTITY,
    );

    let sfz = 10.0_f32;
    let rx = vw - 160.0;
    let zpct = (state.camera.zoom * 100.0).round() as usize;
    draw_num(
        &mut local,
        &state.fonts,
        zpct,
        rx,
        text_y - 2.0,
        STATUS_TEXT,
    );
    let pct_x = rx + num_w(&state.fonts, zpct, sfz) + 2.0;
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        STATUS_DIM,
        None,
        &Circle::new((pct_x + 2.0, text_y + 1.0), 1.0),
    );
    let sep_x = pct_x + 12.0;
    local.stroke(
        &Stroke::new(0.5),
        Affine::IDENTITY,
        Color::from_rgba8(0xff, 0xff, 0xff, 0x33),
        None,
        &Line::new((sep_x, y + 7.0), (sep_x, y + STATUS_H - 7.0)),
    );
    let (world_x, world_y) = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
    let (cx_label, cy_label) = format_cursor_coords(world_x, world_y);
    let pos_x = sep_x + 8.0;
    draw_label(
        &mut local,
        &state.fonts,
        &cx_label,
        pos_x,
        text_y - 2.0,
        STATUS_DIM,
    );
    let comma_x = pos_x + label_w(&state.fonts, &cx_label, sfz) + 3.0;
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        STATUS_DIM,
        None,
        &Circle::new((comma_x, text_y + 4.0), 1.0),
    );
    draw_label(
        &mut local,
        &state.fonts,
        &cy_label,
        comma_x + 5.0,
        text_y - 2.0,
        STATUS_DIM,
    );
    scene.append(&local, Some(Affine::scale(ui_scale)));
}

fn draw_num(scene: &mut Scene, fonts: &crate::text::FontPair, n: usize, x: f64, y: f64, c: Color) {
    draw_label(scene, fonts, &n.to_string(), x, y, c);
}

fn draw_label(
    scene: &mut Scene,
    fonts: &crate::text::FontPair,
    text: &str,
    x: f64,
    y: f64,
    c: Color,
) {
    crate::text::draw_text(scene, fonts, text, x, y, 10.0, c, Affine::IDENTITY);
}

fn num_w(fonts: &crate::text::FontPair, n: usize, sz: f32) -> f64 {
    label_w(fonts, &n.to_string(), sz)
}

fn label_w(fonts: &crate::text::FontPair, text: &str, sz: f32) -> f64 {
    crate::text::measure_text(fonts, text, sz)
}

fn format_cursor_coords(world_x: f64, world_y: f64) -> (String, String) {
    (
        format_signed_coord(world_x.round() as i64),
        format_signed_coord(world_y.round() as i64),
    )
}

fn format_signed_coord(value: i64) -> String {
    value.to_string()
}

pub fn draw_toasts(scene: &mut Scene, state: &AppState) {
    if state.toasts.is_empty() {
        return;
    }
    let font_size = 12.0_f32;
    let pad_x = 16.0;
    let pad_y = 8.0;
    let pill_h = font_size as f64 + pad_y * 2.0;
    let gap = 6.0;

    for (i, toast) in state.toasts.iter().enumerate() {
        let opacity = toast.opacity();
        if opacity < 0.01 {
            continue;
        }
        let text_w = crate::text::measure_text(&state.fonts, &toast.message, font_size);
        let pill_w = text_w + pad_x * 2.0;
        let pill_x = (state.viewport_w - pill_w) / 2.0;
        let pill_y = state.viewport_h - STATUS_H - 20.0 - (i as f64 + 1.0) * (pill_h + gap);
        let radius = pill_h / 2.0;

        let bg = RoundedRect::from_rect(
            Rect::new(pill_x, pill_y, pill_x + pill_w, pill_y + pill_h),
            radius,
        );
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(0x1a, 0x1a, 0x2e, (opacity * 230.0) as u8),
            None,
            &bg,
        );
        crate::text::draw_text(
            scene,
            &state.fonts,
            &toast.message,
            pill_x + pad_x,
            pill_y + pad_y,
            font_size,
            Color::from_rgba8(0xff, 0xff, 0xff, (opacity * 255.0) as u8),
            Affine::IDENTITY,
        );
    }
}

pub fn draw_help_overlay(scene: &mut Scene, state: &AppState) {
    if !state.show_help {
        return;
    }
    let vw = state.viewport_w;
    let vh = state.viewport_h;

    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x00, 0x00, 0x00, 0xaa),
        None,
        &Rect::new(0.0, 0.0, vw, vh),
    );

    let panel_w = 420.0;
    let panel_h = 440.0;
    let px = (vw - panel_w) / 2.0;
    let py = (vh - panel_h) / 2.0;

    let bg = RoundedRect::from_rect(Rect::new(px, py, px + panel_w, py + panel_h), 12.0);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x1a, 0x1a, 0x2e, 0xf0),
        None,
        &bg,
    );
    scene.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        Color::from_rgba8(0x44, 0x44, 0x66, 0xff),
        None,
        &bg,
    );

    let title_color = Color::from_rgba8(0xff, 0xff, 0xff, 0xff);
    let text_color = Color::from_rgba8(0xcc, 0xcc, 0xcc, 0xff);
    let key_color = Color::from_rgba8(0x90, 0x90, 0xff, 0xff);
    let font = 12.0_f32;
    let title_font = 16.0_f32;

    let mut y = py + 24.0;
    crate::text::draw_text(
        scene,
        &state.fonts,
        "Keyboard Shortcuts",
        px + 20.0,
        y,
        title_font,
        title_color,
        Affine::IDENTITY,
    );
    y += 30.0;

    let shortcuts: &[(&str, &str)] = &[
        ("V", "Select"),
        ("R / E / G / B", "Rect / Ellipse / Triangle / Diamond"),
        ("L / A / D / H", "Line / Arrow / Freehand / Highlighter"),
        ("S / T / X", "Sticky / Text / Eraser"),
        ("Q / C", "Lasso / Connector"),
        ("", ""),
        ("Cmd+Z / Cmd+Shift+Z", "Undo / Redo"),
        ("Cmd+C / Cmd+X / Cmd+V", "Copy / Cut / Paste"),
        ("Cmd+D", "Duplicate"),
        ("Cmd+A", "Select All"),
        ("Cmd+G / Cmd+Shift+G", "Group / Ungroup"),
        ("Cmd+S / Cmd+O", "Save / Load"),
        ("Cmd+E / Cmd+Shift+E", "Export PNG / SVG"),
        ("", ""),
        ("Cmd+= / Cmd+-", "Zoom In / Out"),
        ("Cmd+1 / Cmd+2", "Zoom Fit / Selection"),
        ("Cmd+Shift+D", "Toggle Dark Mode"),
        ("Cmd+/", "Toggle This Help"),
        ("Space + Drag", "Pan Canvas"),
        ("Shift+H / Shift+V", "Flip H / V"),
        ("Delete / Backspace", "Delete Selected"),
    ];

    for (key, desc) in shortcuts {
        if key.is_empty() {
            y += 8.0;
            continue;
        }
        crate::text::draw_text(
            scene,
            &state.fonts,
            key,
            px + 24.0,
            y,
            font,
            key_color,
            Affine::IDENTITY,
        );
        crate::text::draw_text(
            scene,
            &state.fonts,
            desc,
            px + 200.0,
            y,
            font,
            text_color,
            Affine::IDENTITY,
        );
        y += 18.0;
    }
}

#[cfg(test)]
mod tests {
    use super::format_cursor_coords;

    #[test]
    fn format_cursor_coords_preserves_signs() {
        let (x, y) = format_cursor_coords(-12.4, -5.6);
        assert_eq!(x, "-12");
        assert_eq!(y, "-6");
    }

    #[test]
    fn format_cursor_coords_rounds_positive_values() {
        let (x, y) = format_cursor_coords(3.6, 4.4);
        assert_eq!(x, "4");
        assert_eq!(y, "4");
    }
}
