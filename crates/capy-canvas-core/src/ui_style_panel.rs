//! Floating style panel public entry points and shared constants.

use vello::Scene;
use vello::peniko::Color;

use crate::state::{AppState, CursorStyle, FillStyle, PALETTE, StrokeStyle};

pub(crate) const PANEL_W: f64 = 220.0;
pub(crate) const PANEL_H: f64 = 336.0;
pub(crate) const PANEL_R: f64 = 14.0;
pub(crate) const PANEL_PAD: f64 = 16.0;
pub(crate) const HEADER_H: f64 = 28.0;
pub(crate) const LABEL_GAP: f64 = 14.0;
pub(crate) const SECTION_GAP: f64 = 14.0;
pub(crate) const COLOR_D: f64 = 14.0;
pub(crate) const COLOR_GAP: f64 = 4.0;
pub(crate) const FILL_D: f64 = 12.0;
pub(crate) const FILL_GAP: f64 = 4.0;
pub(crate) const BTN: f64 = 28.0;
pub(crate) const BTN_GAP: f64 = 8.0;
pub(crate) const SLIDER_H: f64 = 8.0;
pub(crate) const CREAM: Color = Color::from_rgba8(0xf5, 0xf0, 0xe8, 0xff);
pub(crate) const LABEL: Color = Color::from_rgba8(0xf5, 0xf0, 0xe8, 0x66);
pub(crate) const DOTS: Color = Color::from_rgba8(0xf5, 0xf0, 0xe8, 0x33);
pub(crate) const DARK_BORDER: Color = Color::from_rgba8(0, 0, 0, 0x4d);

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum StyleAction {
    StrokeColor(u32),
    FillColor(Option<u32>),
    FillStyle(FillStyle),
    StrokeWidth(f64),
    StrokeStyle(StrokeStyle),
    SetRounded(bool),
    OpacityStart,
    Close,
}

#[derive(Clone, Copy)]
pub(crate) struct PanelStyle {
    pub(crate) fill_color: u32,
    pub(crate) stroke_color: u32,
    pub(crate) stroke_width: f64,
    pub(crate) stroke_style: StrokeStyle,
    pub(crate) fill_style: FillStyle,
    pub(crate) opacity: f32,
    pub(crate) rounded: bool,
}

pub fn draw_style_panel(scene: &mut Scene, state: &AppState) {
    if state.selected.is_empty() {
        return;
    }
    let style = panel_style(state);
    let hover = crate::ui_style_panel_logic::hit_test(state, state.cursor_x, state.cursor_y);
    crate::ui_style_panel_draw::draw_style_panel(scene, state, style, hover);
}

pub fn handle_style_panel_mouse_down(state: &mut AppState, sx: f64, sy: f64) -> bool {
    crate::ui_style_panel_logic::handle_mouse_down(state, sx, sy)
}

pub fn handle_style_panel_drag(state: &mut AppState, sx: f64, sy: f64) -> bool {
    crate::ui_style_panel_logic::handle_drag(state, sx, sy)
}

pub fn style_panel_cursor(state: &AppState, sx: f64, sy: f64) -> Option<CursorStyle> {
    crate::ui_style_panel_logic::cursor_for_point(state, sx, sy)
}

pub(crate) fn panel_style(state: &AppState) -> PanelStyle {
    state
        .selected
        .first()
        .and_then(|&i| state.shapes.get(i))
        .map(|shape| PanelStyle {
            fill_color: shape.color,
            stroke_color: shape.stroke_color,
            stroke_width: shape.stroke_width,
            stroke_style: shape.stroke_style,
            fill_style: shape.fill_style,
            opacity: shape.opacity,
            rounded: shape.rounded,
        })
        .unwrap_or(PanelStyle {
            fill_color: state.fill_color,
            stroke_color: state.color,
            stroke_width: state.stroke_width,
            stroke_style: state.stroke_style,
            fill_style: state.fill_style,
            opacity: state.opacity,
            rounded: state.rounded,
        })
}

pub(crate) fn pastel(color: u32) -> u32 {
    let r = ((color >> 16) & 0xff) as f64;
    let g = ((color >> 8) & 0xff) as f64;
    let b = (color & 0xff) as f64;
    let mix = |channel: f64| (channel * 0.45 + 255.0 * 0.55).round() as u32;
    (mix(r) << 16) | (mix(g) << 8) | mix(b)
}

pub(crate) const STROKE_WIDTHS: [f64; 3] = [1.0, 2.0, 4.0];
pub(crate) const STROKE_STYLES: [StrokeStyle; 3] =
    [StrokeStyle::Solid, StrokeStyle::Dashed, StrokeStyle::Dotted];
pub(crate) const FILL_STYLES: [FillStyle; 3] =
    [FillStyle::None, FillStyle::Hachure, FillStyle::Solid];
pub(crate) const PALETTE_COLORS: &[u32] = PALETTE;
