//! UI panels: context menu, hover tooltip, and style panel re-exports.

pub use crate::ui_style_panel::*;

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

use crate::state::AppState;

// ── Context menu ──

pub fn draw_context_menu(scene: &mut Scene, state: &AppState) {
    let menu = match &state.context_menu {
        Some(m) => m,
        None => return,
    };

    use crate::state::ContextMenu;
    let item_h = ContextMenu::ITEM_H;
    let item_w = ContextMenu::ITEM_W;
    let pad = ContextMenu::PAD;
    let radius = ContextMenu::RADIUS;
    let total_h = menu.total_h();

    let shadow_rect = RoundedRect::from_rect(
        Rect::new(
            menu.sx + 1.0,
            menu.sy + 2.0,
            menu.sx + item_w + 1.0,
            menu.sy + total_h + 2.0,
        ),
        radius,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x00, 0x00, 0x00, 0x40),
        None,
        &shadow_rect,
    );

    let bg = RoundedRect::from_rect(
        Rect::new(menu.sx, menu.sy, menu.sx + item_w, menu.sy + total_h),
        radius,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x1a, 0x1a, 0x2e, 0xff),
        None,
        &bg,
    );
    scene.stroke(
        &Stroke::new(0.5),
        Affine::IDENTITY,
        Color::from_rgba8(0x44, 0x44, 0x44, 0xff),
        None,
        &bg,
    );

    for (i, item) in menu.items.iter().enumerate() {
        let iy = menu.sy + pad + i as f64 * item_h;
        let is_hovered = menu.hovered == Some(i);

        if is_hovered {
            let hover_r = RoundedRect::from_rect(
                Rect::new(menu.sx + 4.0, iy, menu.sx + item_w - 4.0, iy + item_h),
                4.0,
            );
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::from_rgba8(0x33, 0x33, 0x33, 0xff),
                None,
                &hover_r,
            );
        }

        let text_x = menu.sx + 14.0;
        let text_y = iy + (item_h - 12.0) / 2.0;
        crate::text::draw_text(
            scene,
            &state.fonts,
            item.label,
            text_x,
            text_y,
            12.0,
            Color::from_rgba8(0xff, 0xff, 0xff, 0xee),
            Affine::IDENTITY,
        );
    }
}

// ── Tooltip ──

pub fn draw_tooltip(scene: &mut Scene, state: &AppState) {
    let tt = match &state.tooltip {
        Some(t) if t.hover_time >= 0.5 => t,
        _ => return,
    };

    let font_size = 11.0_f32;
    let pad_x = 8.0;
    let pad_y = 5.0;
    let text_w = crate::text::measure_text(&state.fonts, &tt.text, font_size);
    let tip_w = text_w + pad_x * 2.0;
    let tip_h = font_size as f64 + pad_y * 2.0;
    let tip_x = tt.sx - tip_w / 2.0;
    let tip_y = tt.sy + 4.0;
    let radius = 4.0;

    let bg = RoundedRect::from_rect(
        Rect::new(tip_x, tip_y, tip_x + tip_w, tip_y + tip_h),
        radius,
    );
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x1a, 0x1a, 0x1a, 0xee),
        None,
        &bg,
    );

    let mut arrow = BezPath::new();
    arrow.move_to((tt.sx - 4.0, tip_y));
    arrow.line_to((tt.sx, tip_y - 4.0));
    arrow.line_to((tt.sx + 4.0, tip_y));
    arrow.close_path();
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x1a, 0x1a, 0x1a, 0xee),
        None,
        &arrow,
    );

    let text_x = tip_x + pad_x;
    let text_y = tip_y + pad_y;
    crate::text::draw_text(
        scene,
        &state.fonts,
        &tt.text,
        text_x,
        text_y,
        font_size,
        Color::from_rgba8(0xff, 0xff, 0xff, 0xee),
        Affine::IDENTITY,
    );
}
