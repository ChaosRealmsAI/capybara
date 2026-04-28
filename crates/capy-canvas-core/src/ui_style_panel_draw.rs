//! Style panel drawing helpers.

use vello::Scene;
use vello::kurbo::{Affine, Circle, Line, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

use crate::state::{AppState, FillStyle};
use crate::ui::{ACCENT, TOOLBAR_BG, TOOLBAR_SHADOW};
use crate::ui_style_panel::{
    BTN, BTN_GAP, COLOR_D, COLOR_GAP, CREAM, DARK_BORDER, DOTS, FILL_D, FILL_GAP, FILL_STYLES,
    HEADER_H, LABEL, LABEL_GAP, PALETTE_COLORS, PANEL_H, PANEL_PAD, PANEL_R, PANEL_W, PanelStyle,
    SECTION_GAP, SLIDER_H, STROKE_STYLES, STROKE_WIDTHS, StyleAction, pastel,
};

pub(crate) fn draw_style_panel(
    scene: &mut Scene,
    state: &AppState,
    style: PanelStyle,
    hover: Option<StyleAction>,
) {
    let ui_scale = crate::ui::overlay_scale(state.viewport_w, state.viewport_h);
    let (panel_x, panel_y) = state.style_panel_pos;
    let mut local = Scene::new();
    let (x, y) = (0.0, 0.0);
    let panel = RoundedRect::from_rect(Rect::new(x, y, x + PANEL_W, y + PANEL_H), PANEL_R);
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        TOOLBAR_SHADOW,
        None,
        &RoundedRect::from_rect(
            Rect::new(x - 2.0, y + 8.0, x + PANEL_W + 2.0, y + PANEL_H + 14.0),
            PANEL_R + 2.0,
        ),
    );
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x0e, 0x0e, 0x14, 0x50),
        None,
        &RoundedRect::from_rect(
            Rect::new(x, y + 3.0, x + PANEL_W, y + PANEL_H + 3.0),
            PANEL_R,
        ),
    );
    local.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x14, 0x14, 0x1c, 0xd9),
        None,
        &panel,
    );
    local.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        Color::from_rgba8(0xff, 0xff, 0xff, 0x14),
        None,
        &panel,
    );
    local.stroke(
        &Stroke::new(0.5),
        Affine::IDENTITY,
        Color::from_rgba8(0xff, 0xff, 0xff, 0x10),
        None,
        &Line::new((x, y + HEADER_H), (x + PANEL_W, y + HEADER_H)),
    );

    draw_handle(&mut local, x + PANEL_PAD, y + 10.0);
    text(&mut local, state, "STYLE", x + PANEL_PAD + 18.0, y + 8.0);
    draw_close(
        &mut local,
        x + PANEL_W - PANEL_PAD - 20.0,
        y + 5.0,
        hover == Some(StyleAction::Close),
    );

    let left = x + PANEL_PAD;
    let mut top = y + HEADER_H + 12.0;

    text(&mut local, state, "STROKE", left, top);
    top += LABEL_GAP;
    for (i, &color) in PALETTE_COLORS.iter().enumerate() {
        let cx = left + COLOR_D / 2.0 + i as f64 * (COLOR_D + COLOR_GAP);
        draw_dot(
            &mut local,
            cx,
            top + COLOR_D / 2.0,
            COLOR_D / 2.0,
            color,
            style.stroke_color == color,
        );
    }

    top += COLOR_D + SECTION_GAP;
    text(&mut local, state, "FILL", left, top);
    top += LABEL_GAP;
    draw_transparent(
        &mut local,
        left + FILL_D / 2.0,
        top + FILL_D / 2.0,
        FILL_D / 2.0,
        style.fill_style == FillStyle::None,
    );
    for (i, &color) in PALETTE_COLORS.iter().enumerate() {
        let cx = left + FILL_D + FILL_GAP + FILL_D / 2.0 + i as f64 * (FILL_D + FILL_GAP);
        draw_dot(
            &mut local,
            cx,
            top + FILL_D / 2.0,
            FILL_D / 2.0,
            pastel(color),
            style.fill_style != FillStyle::None && style.fill_color == color,
        );
    }

    top += FILL_D + SECTION_GAP;
    text(&mut local, state, "FILL", left, top);
    top += LABEL_GAP;
    for (i, fill_style) in FILL_STYLES.into_iter().enumerate() {
        let x = left + i as f64 * (BTN + BTN_GAP);
        let action = StyleAction::FillStyle(fill_style);
        button(
            &mut local,
            x,
            top,
            style.fill_style == fill_style,
            hover == Some(action),
        );
        fill_icon(&mut local, x, top, i);
    }

    top += BTN + SECTION_GAP;
    text(&mut local, state, "WIDTH", left, top);
    top += LABEL_GAP;
    for (i, width) in STROKE_WIDTHS.into_iter().enumerate() {
        let x = left + i as f64 * (BTN + BTN_GAP);
        let action = StyleAction::StrokeWidth(width);
        button(
            &mut local,
            x,
            top,
            (style.stroke_width - width).abs() < 0.1,
            hover == Some(action),
        );
        width_icon(&mut local, x, top, width);
    }

    top += BTN + SECTION_GAP;
    text(&mut local, state, "STYLE", left, top);
    top += LABEL_GAP;
    for (i, stroke_style) in STROKE_STYLES.into_iter().enumerate() {
        let x = left + i as f64 * (BTN + BTN_GAP);
        let action = StyleAction::StrokeStyle(stroke_style);
        button(
            &mut local,
            x,
            top,
            style.stroke_style == stroke_style,
            hover == Some(action),
        );
        stroke_icon(&mut local, x, top, i);
    }

    top += BTN + SECTION_GAP;
    text(&mut local, state, "EDGES", left, top);
    top += LABEL_GAP;
    for (i, round) in [false, true].into_iter().enumerate() {
        let x = left + i as f64 * (BTN + BTN_GAP);
        let active = style.rounded == round;
        let action = StyleAction::SetRounded(round);
        button(&mut local, x, top, active, hover == Some(action));
        let rect = RoundedRect::from_rect(
            Rect::new(x + 7.0, top + 8.0, x + 21.0, top + 20.0),
            if round { 4.0 } else { 1.0 },
        );
        local.stroke(&Stroke::new(1.1), Affine::IDENTITY, CREAM, None, &rect);
    }

    top += BTN + SECTION_GAP;
    text(&mut local, state, "OPACITY", left, top);
    top += 18.0;
    opacity_slider(&mut local, left, top, style.opacity);

    let transform = Affine::translate((panel_x, panel_y)) * Affine::scale(ui_scale);
    scene.append(&local, Some(transform));
}

fn text(scene: &mut Scene, state: &AppState, label: &str, x: f64, y: f64) {
    crate::text::draw_text(
        scene,
        &state.fonts,
        label,
        x,
        y,
        10.0,
        LABEL,
        Affine::IDENTITY,
    );
}

fn draw_handle(scene: &mut Scene, x: f64, y: f64) {
    for row in 0..3 {
        for col in 0..2 {
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                DOTS,
                None,
                &Circle::new(
                    (x + col as f64 * 6.0 + 2.0, y + row as f64 * 6.0 + 2.0),
                    1.2,
                ),
            );
        }
    }
}

fn draw_close(scene: &mut Scene, x: f64, y: f64, hovered: bool) {
    let color = Color::from_rgba8(0xf5, 0xf0, 0xe8, if hovered { 0xcc } else { 0x33 });
    scene.stroke(
        &Stroke::new(1.3),
        Affine::IDENTITY,
        color,
        None,
        &Line::new((x, y), (x + 9.0, y + 9.0)),
    );
    scene.stroke(
        &Stroke::new(1.3),
        Affine::IDENTITY,
        color,
        None,
        &Line::new((x + 9.0, y), (x, y + 9.0)),
    );
}

fn draw_dot(scene: &mut Scene, cx: f64, cy: f64, radius: f64, color: u32, active: bool) {
    let circle = Circle::new((cx, cy), radius);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        crate::render::color_from_hex(color, 1.0),
        None,
        &circle,
    );
    scene.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        DARK_BORDER,
        None,
        &circle,
    );
    if active {
        scene.stroke(
            &Stroke::new(2.0),
            Affine::IDENTITY,
            ACCENT,
            None,
            &Circle::new((cx, cy), radius + 3.0),
        );
    }
}

fn draw_transparent(scene: &mut Scene, cx: f64, cy: f64, radius: f64, active: bool) {
    let circle = Circle::new((cx, cy), radius);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0xfb, 0xfb, 0xfb, 0xff),
        None,
        &circle,
    );
    for (dx, dy, dark) in [
        (-2.0, -2.0, true),
        (2.0, -2.0, false),
        (-2.0, 2.0, false),
        (2.0, 2.0, true),
    ] {
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            if dark {
                Color::from_rgba8(0xcc, 0xcc, 0xcc, 0xff)
            } else {
                Color::from_rgba8(0xf1, 0xf1, 0xf1, 0xff)
            },
            None,
            &Rect::new(cx + dx - 2.0, cy + dy - 2.0, cx + dx + 2.0, cy + dy + 2.0),
        );
    }
    scene.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        DARK_BORDER,
        None,
        &circle,
    );
    if active {
        scene.stroke(
            &Stroke::new(2.0),
            Affine::IDENTITY,
            ACCENT,
            None,
            &Circle::new((cx, cy), radius + 3.0),
        );
    }
}

fn button(scene: &mut Scene, x: f64, y: f64, active: bool, hovered: bool) {
    let rect = RoundedRect::from_rect(Rect::new(x, y, x + BTN, y + BTN), 8.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, TOOLBAR_BG, None, &rect);
    scene.stroke(
        &Stroke::new(if active { 1.6 } else { 0.8 }),
        Affine::IDENTITY,
        if active {
            ACCENT
        } else if hovered {
            Color::from_rgba8(0xff, 0xff, 0xff, 0x44)
        } else {
            Color::from_rgba8(0xff, 0xff, 0xff, 0x18)
        },
        None,
        &rect,
    );
}

fn fill_icon(scene: &mut Scene, x: f64, y: f64, index: usize) {
    let rect = RoundedRect::from_rect(
        Rect::new(x + 7.0, y + 7.0, x + 21.0, y + 21.0),
        if index == 2 { 3.0 } else { 2.0 },
    );
    if index == 2 {
        scene.fill(Fill::NonZero, Affine::IDENTITY, CREAM, None, &rect);
        return;
    }
    scene.stroke(&Stroke::new(1.0), Affine::IDENTITY, CREAM, None, &rect);
    if index == 1 {
        for i in 0..4 {
            scene.stroke(
                &Stroke::new(0.8),
                Affine::IDENTITY,
                CREAM,
                None,
                &Line::new(
                    (x + 8.0 + i as f64 * 3.0, y + 21.0),
                    (x + 15.0 + i as f64 * 3.0, y + 14.0),
                ),
            );
        }
    }
}

fn width_icon(scene: &mut Scene, x: f64, y: f64, width: f64) {
    scene.stroke(
        &Stroke::new(width),
        Affine::IDENTITY,
        CREAM,
        None,
        &Line::new((x + 6.0, y + 14.0), (x + 22.0, y + 14.0)),
    );
}

fn stroke_icon(scene: &mut Scene, x: f64, y: f64, index: usize) {
    let stroke = match index {
        0 => Stroke::new(1.5),
        1 => Stroke::new(1.5).with_dashes(0.0, [4.0, 3.0]),
        _ => Stroke::new(1.5).with_dashes(0.0, [1.2, 3.2]),
    };
    scene.stroke(
        &stroke,
        Affine::IDENTITY,
        CREAM,
        None,
        &Line::new((x + 5.0, y + 14.0), (x + 23.0, y + 14.0)),
    );
}

fn opacity_slider(scene: &mut Scene, x: f64, y: f64, opacity: f32) {
    let width = PANEL_W - PANEL_PAD * 2.0;
    let track = RoundedRect::from_rect(Rect::new(x, y, x + width, y + SLIDER_H), SLIDER_H / 2.0);
    let fill_w = (width * opacity as f64).max(4.0);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x33, 0x33, 0x3d, 0xff),
        None,
        &track,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x8a, 0x6f, 0xae, 0xff),
        None,
        &RoundedRect::from_rect(Rect::new(x, y, x + fill_w, y + SLIDER_H), SLIDER_H / 2.0),
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        CREAM,
        None,
        &Circle::new((x + width * opacity as f64, y + SLIDER_H / 2.0), 4.0),
    );
}
