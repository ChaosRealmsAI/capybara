//! UI overlay rendering: toolbar, color picker, minimap.
//!
//! Extended by `ui_panels` for: style panel, context menu, tooltip,
//! and by `ui_status` for: status bar, toasts, help overlay.

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Line, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

use crate::state::{AppState, PALETTE, Tool};

// Re-export everything from ui_panels so callers can use `ui::style_panel_hit` etc.
pub use crate::ui_panels::*;
pub use crate::ui_status::*;

// ── Colors (pub(crate) for ui_panels to share) ──
pub const ACCENT: Color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xff);
pub(crate) const ACCENT_BG: Color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x33);
pub(crate) const TOOLBAR_BG: Color = Color::from_rgba8(0x14, 0x14, 0x1c, 0xd9);
pub(crate) const TOOLBAR_SHADOW: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0x40);
pub(crate) const TOOLBAR_SHADOW_OUTER: Color = Color::from_rgba8(0x00, 0x00, 0x00, 0x30);
pub(crate) const HOVER_BG: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x1a);
pub(crate) const SEPARATOR_COLOR: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x22);
pub(crate) const STATUS_BG: Color = Color::from_rgba8(0x14, 0x14, 0x1c, 0xe0);
pub(crate) const STATUS_TEXT: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0xff);
pub(crate) const STATUS_DIM: Color = Color::from_rgba8(0xff, 0xff, 0xff, 0x99);
pub(crate) const STATUS_H: f64 = 28.0;
/// Light/cream color for tool icons on the dark toolbar.
pub(crate) const ICON_LIGHT: Color = Color::from_rgba8(0xf5, 0xf0, 0xe8, 0xff);

// ── Toolbar layout constants ──
const BTN_SIZE: f64 = 36.0;
const BTN_GAP: f64 = 2.0;
const BTN_RADIUS: f64 = 8.0;
const PILL_RADIUS: f64 = 12.0;
const PILL_PAD_X: f64 = 6.0;
const PILL_PAD_Y: f64 = 6.0;
const SEPARATOR_W: f64 = 8.0;
const BASE_UI_W: f64 = 1920.0;
const BASE_UI_H: f64 = 1080.0;
const MAX_UI_SCALE: f64 = 1.65;

/// Screen-space UI should stay touchable/readable on very large browser
/// viewports, including Chrome zoomed-out windows that report 3000+ CSS px.
pub fn overlay_scale(viewport_w: f64, viewport_h: f64) -> f64 {
    let scale = (viewport_w / BASE_UI_W).min(viewport_h / BASE_UI_H);
    scale.clamp(1.0, MAX_UI_SCALE)
}

fn local_viewport(viewport_w: f64, viewport_h: f64) -> (f64, f64, f64) {
    let scale = overlay_scale(viewport_w, viewport_h);
    (viewport_w / scale, viewport_h / scale, scale)
}

/// Tool groups: Select Lasso | Rect Ellipse Triangle Diamond | Line Arrow Freehand Highlighter | StickyNote Text Eraser
const GROUPS: &[&[Tool]] = &[
    &[Tool::Select, Tool::Lasso],
    &[Tool::Rect, Tool::Ellipse, Tool::Triangle, Tool::Diamond],
    &[Tool::Line, Tool::Arrow, Tool::Freehand, Tool::Highlighter],
    &[Tool::StickyNote, Tool::Text, Tool::Eraser],
];

fn toolbar_total_width() -> f64 {
    let num_btns: usize = GROUPS.iter().map(|g| g.len()).sum();
    let num_seps = GROUPS.len() - 1;
    num_btns as f64 * (BTN_SIZE + BTN_GAP) - BTN_GAP + num_seps as f64 * SEPARATOR_W
}

fn toolbar_origin_local(viewport_w: f64, viewport_h: f64) -> (f64, f64) {
    let tw = toolbar_total_width();
    let pill_w = tw + PILL_PAD_X * 2.0;
    let pill_h = BTN_SIZE + PILL_PAD_Y * 2.0;
    let bar_x = (viewport_w - pill_w) / 2.0;
    let bar_y = viewport_h - STATUS_H - pill_h - 10.0;
    (bar_x, bar_y)
}

pub fn toolbar_origin(viewport_w: f64, viewport_h: f64) -> (f64, f64) {
    let (local_w, local_h, scale) = local_viewport(viewport_w, viewport_h);
    let (bar_x, bar_y) = toolbar_origin_local(local_w, local_h);
    (bar_x * scale, bar_y * scale)
}

pub fn toolbar_rect(viewport_w: f64, viewport_h: f64) -> (f64, f64, f64, f64) {
    let (local_w, local_h, scale) = local_viewport(viewport_w, viewport_h);
    let (bar_x, bar_y) = toolbar_origin_local(local_w, local_h);
    let tw = toolbar_total_width();
    let pill_w = tw + PILL_PAD_X * 2.0;
    let pill_h = BTN_SIZE + PILL_PAD_Y * 2.0;
    (bar_x * scale, bar_y * scale, pill_w * scale, pill_h * scale)
}

pub fn draw_toolbar(scene: &mut Scene, state: &AppState) {
    let (viewport_w, viewport_h, scale) = local_viewport(state.viewport_w, state.viewport_h);
    let mut local = Scene::new();
    let tw = toolbar_total_width();
    let pill_w = tw + PILL_PAD_X * 2.0;
    let pill_h = BTN_SIZE + PILL_PAD_Y * 2.0;
    let (bar_x, bar_y) = toolbar_origin_local(viewport_w, viewport_h);
    let cursor_x_local = state.cursor_x / scale;
    let cursor_y_local = state.cursor_y / scale;

    // Outer shadow
    let shadow_outer = RoundedRect::from_rect(
        Rect::new(
            bar_x - 1.0,
            bar_y + 2.0,
            bar_x + pill_w + 1.0,
            bar_y + pill_h + 3.0,
        ),
        PILL_RADIUS + 1.0,
    );
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        TOOLBAR_SHADOW_OUTER,
        None,
        &shadow_outer,
    );

    // Inner shadow
    let shadow = RoundedRect::from_rect(
        Rect::new(bar_x, bar_y + 1.0, bar_x + pill_w, bar_y + pill_h + 1.0),
        PILL_RADIUS,
    );
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        TOOLBAR_SHADOW,
        None,
        &shadow,
    );

    // Pill background
    let pill = RoundedRect::from_rect(
        Rect::new(bar_x, bar_y, bar_x + pill_w, bar_y + pill_h),
        PILL_RADIUS,
    );
    local.fill(Fill::NonZero, Affine::IDENTITY, TOOLBAR_BG, None, &pill);
    local.stroke(
        &Stroke::new(0.5),
        Affine::IDENTITY,
        Color::from_rgba8(0xff, 0xff, 0xff, 0x11),
        None,
        &pill,
    );

    let mut cursor_x = bar_x + PILL_PAD_X;
    let btn_y = bar_y + PILL_PAD_Y;

    for (gi, group) in GROUPS.iter().enumerate() {
        for &tool in *group {
            let is_active = state.tool == tool;
            let is_hovered = is_cursor_in_rect(
                cursor_x_local,
                cursor_y_local,
                cursor_x,
                btn_y,
                BTN_SIZE,
                BTN_SIZE,
            );

            if is_active {
                let bg = RoundedRect::from_rect(
                    Rect::new(cursor_x, btn_y, cursor_x + BTN_SIZE, btn_y + BTN_SIZE),
                    BTN_RADIUS,
                );
                local.fill(Fill::NonZero, Affine::IDENTITY, ACCENT_BG, None, &bg);
            } else if is_hovered {
                let bg = RoundedRect::from_rect(
                    Rect::new(cursor_x, btn_y, cursor_x + BTN_SIZE, btn_y + BTN_SIZE),
                    BTN_RADIUS,
                );
                local.fill(Fill::NonZero, Affine::IDENTITY, HOVER_BG, None, &bg);
            }

            let icon_color = if is_active { ACCENT } else { ICON_LIGHT };
            draw_tool_icon(
                &mut local, tool, cursor_x, btn_y, BTN_SIZE, BTN_SIZE, icon_color,
            );

            cursor_x += BTN_SIZE + BTN_GAP;
        }

        if gi < GROUPS.len() - 1 {
            cursor_x -= BTN_GAP;
            let sep_x = cursor_x + SEPARATOR_W / 2.0;
            let sep_line = Line::new((sep_x, btn_y + 6.0), (sep_x, btn_y + BTN_SIZE - 6.0));
            local.stroke(
                &Stroke::new(1.0),
                Affine::IDENTITY,
                SEPARATOR_COLOR,
                None,
                &sep_line,
            );
            cursor_x += SEPARATOR_W + BTN_GAP;
        }
    }

    scene.append(&local, Some(Affine::scale(scale)));
}

fn is_cursor_in_rect(cx: f64, cy: f64, rx: f64, ry: f64, rw: f64, rh: f64) -> bool {
    cx >= rx && cx <= rx + rw && cy >= ry && cy <= ry + rh
}

fn draw_tool_icon(scene: &mut Scene, tool: Tool, x: f64, y: f64, w: f64, h: f64, color: Color) {
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let stroke = Stroke::new(1.5);
    let stroke_thin = Stroke::new(1.2);

    match tool {
        Tool::Select => {
            let mut path = BezPath::new();
            path.move_to((cx - 4.0, cy - 7.0));
            path.line_to((cx - 4.0, cy + 6.0));
            path.line_to((cx - 0.5, cy + 3.0));
            path.line_to((cx + 3.5, cy + 7.0));
            path.line_to((cx + 5.0, cy + 5.5));
            path.line_to((cx + 1.5, cy + 1.5));
            path.line_to((cx + 5.0, cy - 0.5));
            path.close_path();
            scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &path);
        }
        Tool::Rect => {
            let s = 7.0;
            let r =
                RoundedRect::from_rect(Rect::new(cx - s, cy - s + 1.0, cx + s, cy + s - 1.0), 2.0);
            scene.stroke(&stroke, Affine::IDENTITY, color, None, &r);
        }
        Tool::Ellipse => {
            let circle = Circle::new((cx, cy), 7.5);
            scene.stroke(&stroke, Affine::IDENTITY, color, None, &circle);
        }
        Tool::Line => {
            let line = Line::new((cx - 7.0, cy + 6.0), (cx + 7.0, cy - 6.0));
            scene.stroke(&stroke, Affine::IDENTITY, color, None, &line);
        }
        Tool::Arrow => {
            let line = Line::new((cx - 7.0, cy + 5.0), (cx + 7.0, cy - 5.0));
            scene.stroke(&stroke, Affine::IDENTITY, color, None, &line);
            let mut ah = BezPath::new();
            ah.move_to((cx + 7.0, cy - 5.0));
            ah.line_to((cx + 2.0, cy - 5.5));
            ah.line_to((cx + 4.5, cy - 0.5));
            ah.close_path();
            scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &ah);
        }
        Tool::Freehand => {
            let mut path = BezPath::new();
            path.move_to((cx - 8.0, cy + 2.0));
            path.quad_to((cx - 4.0, cy - 6.0), (cx, cy));
            path.quad_to((cx + 4.0, cy + 6.0), (cx + 8.0, cy - 2.0));
            scene.stroke(&stroke, Affine::IDENTITY, color, None, &path);
        }
        Tool::Text => {
            let thick = Stroke::new(2.0);
            let top = Line::new((cx - 6.0, cy - 7.0), (cx + 6.0, cy - 7.0));
            scene.stroke(&thick, Affine::IDENTITY, color, None, &top);
            let stem = Line::new((cx, cy - 7.0), (cx, cy + 7.0));
            scene.stroke(&thick, Affine::IDENTITY, color, None, &stem);
            let serif = Line::new((cx - 3.0, cy + 7.0), (cx + 3.0, cy + 7.0));
            scene.stroke(&stroke_thin, Affine::IDENTITY, color, None, &serif);
        }
        Tool::Eraser => {
            let mut path = BezPath::new();
            path.move_to((cx - 3.0, cy - 7.0));
            path.line_to((cx + 7.0, cy - 7.0));
            path.line_to((cx + 7.0, cy + 2.0));
            path.line_to((cx + 3.0, cy + 7.0));
            path.line_to((cx - 7.0, cy + 7.0));
            path.line_to((cx - 7.0, cy - 2.0));
            path.close_path();
            scene.stroke(&stroke, Affine::IDENTITY, color, None, &path);
            let div = Line::new((cx - 3.0, cy - 7.0), (cx - 7.0, cy - 2.0));
            scene.stroke(&stroke_thin, Affine::IDENTITY, color, None, &div);
            let mut btm = BezPath::new();
            btm.move_to((cx - 7.0, cy - 2.0));
            btm.line_to((cx - 3.0, cy - 7.0));
            btm.line_to((cx - 3.0, cy + 2.0));
            btm.line_to((cx - 7.0, cy + 7.0));
            btm.close_path();
            let rgba = color.to_rgba8();
            let fill_alpha = Color::from_rgba8(rgba.r, rgba.g, rgba.b, 0x33);
            scene.fill(Fill::NonZero, Affine::IDENTITY, fill_alpha, None, &btm);
        }
        Tool::Triangle => {
            let mut path = BezPath::new();
            path.move_to((cx, cy - 7.0));
            path.line_to((cx - 7.0, cy + 6.0));
            path.line_to((cx + 7.0, cy + 6.0));
            path.close_path();
            scene.stroke(&stroke, Affine::IDENTITY, color, None, &path);
        }
        Tool::Diamond => {
            let mut path = BezPath::new();
            path.move_to((cx, cy - 7.0));
            path.line_to((cx + 7.0, cy));
            path.line_to((cx, cy + 7.0));
            path.line_to((cx - 7.0, cy));
            path.close_path();
            scene.stroke(&stroke, Affine::IDENTITY, color, None, &path);
        }
        Tool::Highlighter => {
            let r = Rect::new(cx - 3.0, cy - 6.0, cx + 3.0, cy + 6.0);
            let rgba = color.to_rgba8();
            let fill_color = Color::from_rgba8(rgba.r, rgba.g, rgba.b, 0x66);
            scene.fill(Fill::NonZero, Affine::IDENTITY, fill_color, None, &r);
        }
        Tool::StickyNote => {
            let r = RoundedRect::from_rect(Rect::new(cx - 7.0, cy - 7.0, cx + 7.0, cy + 7.0), 2.0);
            let rgba = color.to_rgba8();
            let fill_color = Color::from_rgba8(rgba.r, rgba.g, rgba.b, 0x44);
            scene.fill(Fill::NonZero, Affine::IDENTITY, fill_color, None, &r);
            scene.stroke(&stroke_thin, Affine::IDENTITY, color, None, &r);
        }
        Tool::Lasso => {
            let circle = Circle::new((cx, cy), 6.0);
            let dashed = Stroke::new(1.2).with_dashes(0.0, [3.0, 3.0]);
            scene.stroke(&dashed, Affine::IDENTITY, color, None, &circle);
        }
    }
}

pub fn draw_color_picker(scene: &mut Scene, state: &AppState) {
    let swatch_d = 24.0;
    let gap = 8.0;
    let margin = 16.0;
    let swatch_r = swatch_d / 2.0;

    let num = PALETTE.len() as f64;
    let total_w = num * swatch_d + (num - 1.0) * gap;
    let pill_pad = 10.0;
    let pill_w = total_w + pill_pad * 2.0;
    let pill_h = swatch_d + pill_pad * 2.0;

    let pill_x = margin;
    let pill_y = state.viewport_h - margin - STATUS_H - pill_h;

    // Shadow
    let shadow = RoundedRect::from_rect(
        Rect::new(pill_x, pill_y + 1.0, pill_x + pill_w, pill_y + pill_h + 1.0),
        pill_h / 2.0,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        TOOLBAR_SHADOW,
        None,
        &shadow,
    );

    // Pill background
    let pill = RoundedRect::from_rect(
        Rect::new(pill_x, pill_y, pill_x + pill_w, pill_y + pill_h),
        pill_h / 2.0,
    );
    scene.fill(Fill::NonZero, Affine::IDENTITY, TOOLBAR_BG, None, &pill);
    scene.stroke(
        &Stroke::new(0.5),
        Affine::IDENTITY,
        Color::from_rgba8(0xff, 0xff, 0xff, 0x11),
        None,
        &pill,
    );

    let start_x = pill_x + pill_pad + swatch_r;
    let center_y = pill_y + pill_pad + swatch_r;

    for (i, &color) in PALETTE.iter().enumerate() {
        let scx = start_x + i as f64 * (swatch_d + gap);
        let is_selected = state.color == color;

        if is_selected {
            let ring = Circle::new((scx, center_y), swatch_r + 4.0);
            scene.stroke(
                &Stroke::new(2.0),
                Affine::IDENTITY,
                Color::from_rgba8(0xff, 0xff, 0xff, 0xcc),
                None,
                &ring,
            );
        }

        let fill = crate::render::color_from_hex(color, 1.0);
        let circle = Circle::new((scx, center_y), swatch_r);
        scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &circle);
    }
}

// ── Minimap ──

pub fn draw_minimap(scene: &mut Scene, state: &AppState) {
    use crate::minimap;
    let (local_w, local_h, ui_scale) = local_viewport(state.viewport_w, state.viewport_h);
    let mut local = Scene::new();
    let mx = local_w - minimap::MINIMAP_MARGIN - minimap::MINIMAP_W;
    let my = local_h - minimap::MINIMAP_MARGIN - minimap::STATUS_H - minimap::MINIMAP_H;
    let mw = minimap::MINIMAP_W;
    let mh = minimap::MINIMAP_H;
    let pad = minimap::MINIMAP_PAD;
    let radius = 4.0;

    let bg = RoundedRect::from_rect(Rect::new(mx, my, mx + mw, my + mh), radius);
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x00, 0x00, 0x00, 0x80),
        None,
        &bg,
    );
    local.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        Color::from_rgba8(0x55, 0x55, 0x55, 0xff),
        None,
        &bg,
    );

    let (world_bounds, _has_shapes) = minimap::compute_bounds(state);
    let (bx, by, bw, bh) = world_bounds;
    if bw < 1.0 || bh < 1.0 {
        scene.append(&local, Some(Affine::scale(ui_scale)));
        return;
    }

    let draw_w = mw - pad * 2.0;
    let draw_h = mh - pad * 2.0;
    let scale = (draw_w / bw).min(draw_h / bh);
    let off_x = mx + pad + (draw_w - bw * scale) / 2.0;
    let off_y = my + pad + (draw_h - bh * scale) / 2.0;

    for s in &state.shapes {
        let sx = off_x + (s.x - bx) * scale;
        let sy = off_y + (s.y - by) * scale;
        let sw = (s.w * scale).max(2.0);
        let sh = (s.h * scale).max(2.0);
        let c = crate::render::color_from_hex(s.color, 0.8);
        let r = Rect::new(sx, sy, sx + sw, sy + sh);
        local.fill(Fill::NonZero, Affine::IDENTITY, c, None, &r);
    }

    let cam = &state.camera;
    let vw_world = state.viewport_w / cam.zoom;
    let vh_world = state.viewport_h / cam.zoom;
    let cam_x = -cam.offset_x / cam.zoom;
    let cam_y = -cam.offset_y / cam.zoom;

    let vp_sx = off_x + (cam_x - bx) * scale;
    let vp_sy = off_y + (cam_y - by) * scale;
    let vp_sw = vw_world * scale;
    let vp_sh = vh_world * scale;

    let vp_rect = Rect::new(vp_sx, vp_sy, vp_sx + vp_sw, vp_sy + vp_sh);
    local.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        Color::from_rgba8(0xff, 0xff, 0xff, 0xcc),
        None,
        &vp_rect,
    );
    scene.append(&local, Some(Affine::scale(ui_scale)));
}

/// Returns the color picker pill rect for hit testing.
pub fn color_picker_rect(viewport_h: f64) -> (f64, f64, f64, f64) {
    let swatch_d = 24.0;
    let gap = 8.0;
    let margin = 16.0;
    let num = PALETTE.len() as f64;
    let total_w = num * swatch_d + (num - 1.0) * gap;
    let pill_pad = 10.0;
    let pill_w = total_w + pill_pad * 2.0;
    let pill_h = swatch_d + pill_pad * 2.0;

    let pill_x = margin;
    let pill_y = viewport_h - margin - STATUS_H - pill_h;
    (pill_x, pill_y, pill_w, pill_h)
}

#[cfg(test)]
mod tests {
    use super::{overlay_scale, toolbar_rect};

    #[test]
    fn overlay_scale_stays_base_for_normal_canvas() {
        assert!((overlay_scale(1400.0, 900.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn overlay_scale_grows_for_zoomed_out_browser_viewport() {
        let scale = overlay_scale(3420.0, 1902.0);
        assert!(scale > 1.4);
        assert!(scale <= 1.65);
    }

    #[test]
    fn toolbar_rect_scales_hit_area_with_overlay() {
        let (_, _, _, normal_h) = toolbar_rect(1400.0, 900.0);
        let (_, _, _, large_h) = toolbar_rect(3420.0, 1902.0);
        assert!(large_h > normal_h * 1.4);
    }
}
