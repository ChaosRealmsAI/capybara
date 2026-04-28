//! Style panel hit testing and state mutation.

use crate::state::{AppState, CursorStyle, DragMode};
use crate::ui_style_panel::{
    BTN, BTN_GAP, COLOR_D, COLOR_GAP, FILL_D, FILL_GAP, FILL_STYLES, HEADER_H, LABEL_GAP,
    PALETTE_COLORS, PANEL_H, PANEL_PAD, PANEL_W, SECTION_GAP, STROKE_STYLES, STROKE_WIDTHS,
    StyleAction,
};

pub(crate) fn handle_mouse_down(state: &mut AppState, sx: f64, sy: f64) -> bool {
    if state.selected.is_empty()
        || !panel_interactive(state)
        || !point_in_rect(sx, sy, panel_rect(state))
    {
        return false;
    }
    match hit_test(state, sx, sy) {
        Some(StyleAction::Close) => {
            state.selected.clear();
            state.selected_connector = None;
        }
        Some(StyleAction::OpacityStart) => {
            state.push_undo();
            state.drag_mode = DragMode::OpacityDrag;
            set_opacity_from_cursor(state, sx);
        }
        Some(action) => apply_action(state, action),
        None if point_in_rect(sx, sy, header_rect(state)) => {
            state.drag_mode = DragMode::StylePanelDrag {
                offset_x: sx - state.style_panel_pos.0,
                offset_y: sy - state.style_panel_pos.1,
            };
        }
        None => {}
    }
    true
}

pub(crate) fn handle_drag(state: &mut AppState, sx: f64, sy: f64) -> bool {
    match state.drag_mode {
        DragMode::StylePanelDrag { offset_x, offset_y } => {
            state.style_panel_pos = (sx - offset_x, sy - offset_y);
            true
        }
        DragMode::OpacityDrag => {
            set_opacity_from_cursor(state, sx);
            true
        }
        _ => false,
    }
}

pub(crate) fn cursor_for_point(state: &AppState, sx: f64, sy: f64) -> Option<CursorStyle> {
    if state.selected.is_empty()
        || !panel_interactive(state)
        || !point_in_rect(sx, sy, panel_rect(state))
    {
        return None;
    }
    if hit_test(state, sx, sy).is_some() {
        return Some(CursorStyle::Pointer);
    }
    if point_in_rect(sx, sy, header_rect(state)) {
        return Some(match state.drag_mode {
            DragMode::StylePanelDrag { .. } => CursorStyle::Grabbing,
            _ => CursorStyle::Grab,
        });
    }
    Some(CursorStyle::Default)
}

pub(crate) fn hit_test(state: &AppState, sx: f64, sy: f64) -> Option<StyleAction> {
    if state.selected.is_empty() || !panel_interactive(state) {
        return None;
    }
    let (sx, sy) = to_local(state, sx, sy)?;
    if point_in_rect(sx, sy, close_rect_local()) {
        return Some(StyleAction::Close);
    }

    let (x, y) = (0.0, 0.0);
    let left = x + PANEL_PAD;
    let mut top = y + HEADER_H + 12.0 + LABEL_GAP;

    for (i, &color) in PALETTE_COLORS.iter().enumerate() {
        let cx = left + COLOR_D / 2.0 + i as f64 * (COLOR_D + COLOR_GAP);
        if hit_circle(sx, sy, cx, top + COLOR_D / 2.0, COLOR_D / 2.0 + 4.0) {
            return Some(StyleAction::StrokeColor(color));
        }
    }

    top += COLOR_D + SECTION_GAP + LABEL_GAP;
    if hit_circle(
        sx,
        sy,
        left + FILL_D / 2.0,
        top + FILL_D / 2.0,
        FILL_D / 2.0 + 4.0,
    ) {
        return Some(StyleAction::FillColor(None));
    }
    for (i, &color) in PALETTE_COLORS.iter().enumerate() {
        let cx = left + FILL_D + FILL_GAP + FILL_D / 2.0 + i as f64 * (FILL_D + FILL_GAP);
        if hit_circle(sx, sy, cx, top + FILL_D / 2.0, FILL_D / 2.0 + 4.0) {
            return Some(StyleAction::FillColor(Some(color)));
        }
    }

    top += FILL_D + SECTION_GAP + LABEL_GAP;
    if let Some(action) = hit_buttons(sx, sy, left, top, FillButtons) {
        return Some(action);
    }

    top += BTN + SECTION_GAP + LABEL_GAP;
    if let Some(action) = hit_buttons(sx, sy, left, top, WidthButtons) {
        return Some(action);
    }

    top += BTN + SECTION_GAP + LABEL_GAP;
    if let Some(action) = hit_buttons(sx, sy, left, top, StrokeButtons) {
        return Some(action);
    }

    top += BTN + SECTION_GAP + LABEL_GAP;
    if let Some(action) = hit_buttons(sx, sy, left, top, EdgesButtons) {
        return Some(action);
    }

    point_in_rect(sx, sy, opacity_rect(state)).then_some(StyleAction::OpacityStart)
}

trait ButtonSet {
    fn count() -> usize {
        3
    }
    fn action(index: usize) -> StyleAction;
}

struct FillButtons;
struct WidthButtons;
struct StrokeButtons;
struct EdgesButtons;

impl ButtonSet for FillButtons {
    fn action(index: usize) -> StyleAction {
        StyleAction::FillStyle(FILL_STYLES[index])
    }
}

impl ButtonSet for WidthButtons {
    fn action(index: usize) -> StyleAction {
        StyleAction::StrokeWidth(STROKE_WIDTHS[index])
    }
}

impl ButtonSet for StrokeButtons {
    fn action(index: usize) -> StyleAction {
        StyleAction::StrokeStyle(STROKE_STYLES[index])
    }
}

impl ButtonSet for EdgesButtons {
    fn count() -> usize {
        2
    }
    fn action(index: usize) -> StyleAction {
        StyleAction::SetRounded(index == 1)
    }
}

fn hit_buttons<T: ButtonSet>(sx: f64, sy: f64, x: f64, y: f64, _: T) -> Option<StyleAction> {
    for index in 0..T::count() {
        if point_in_rect(sx, sy, (x + index as f64 * (BTN + BTN_GAP), y, BTN, BTN)) {
            return Some(T::action(index));
        }
    }
    None
}

fn apply_action(state: &mut AppState, action: StyleAction) {
    state.push_undo();
    match action {
        StyleAction::StrokeColor(color) => {
            state.color = color;
            for &idx in &state.selected.clone() {
                if let Some(shape) = state.shapes.get_mut(idx) {
                    shape.stroke_color = color;
                }
            }
        }
        StyleAction::FillColor(color) => {
            if let Some(color) = color {
                state.fill_color = color;
                if state.fill_style == crate::state::FillStyle::None {
                    state.fill_style = crate::state::FillStyle::Solid;
                }
            } else {
                state.fill_style = crate::state::FillStyle::None;
            }
            for &idx in &state.selected.clone() {
                if let Some(shape) = state.shapes.get_mut(idx) {
                    if let Some(color) = color {
                        shape.color = color;
                        if shape.fill_style == crate::state::FillStyle::None {
                            shape.fill_style = crate::state::FillStyle::Solid;
                        }
                    } else {
                        shape.fill_style = crate::state::FillStyle::None;
                    }
                }
            }
        }
        StyleAction::FillStyle(fill_style) => {
            state.fill_style = fill_style;
            for &idx in &state.selected.clone() {
                if let Some(shape) = state.shapes.get_mut(idx) {
                    shape.fill_style = fill_style;
                }
            }
        }
        StyleAction::StrokeWidth(width) => {
            state.stroke_width = width;
            for &idx in &state.selected.clone() {
                if let Some(shape) = state.shapes.get_mut(idx) {
                    shape.stroke_width = width;
                }
            }
        }
        StyleAction::StrokeStyle(stroke_style) => {
            state.stroke_style = stroke_style;
            for &idx in &state.selected.clone() {
                if let Some(shape) = state.shapes.get_mut(idx) {
                    shape.stroke_style = stroke_style;
                }
            }
        }
        StyleAction::SetRounded(rounded) => {
            state.rounded = rounded;
            for &idx in &state.selected.clone() {
                if let Some(shape) = state.shapes.get_mut(idx) {
                    shape.rounded = rounded;
                }
            }
        }
        StyleAction::OpacityStart | StyleAction::Close => {}
    }
}

fn set_opacity_from_cursor(state: &mut AppState, sx: f64) {
    let (x, _, w, _) = opacity_rect(state);
    let opacity = ((sx - x) / w).clamp(0.0, 1.0) as f32;
    state.opacity = opacity;
    for &idx in &state.selected.clone() {
        if let Some(shape) = state.shapes.get_mut(idx) {
            shape.opacity = opacity;
        }
    }
}

fn panel_rect(state: &AppState) -> (f64, f64, f64, f64) {
    let scale = crate::ui::overlay_scale(state.viewport_w, state.viewport_h);
    (
        state.style_panel_pos.0,
        state.style_panel_pos.1,
        PANEL_W * scale,
        PANEL_H * scale,
    )
}

fn header_rect(state: &AppState) -> (f64, f64, f64, f64) {
    let scale = crate::ui::overlay_scale(state.viewport_w, state.viewport_h);
    (
        state.style_panel_pos.0,
        state.style_panel_pos.1,
        PANEL_W * scale,
        HEADER_H * scale,
    )
}

fn close_rect_local() -> (f64, f64, f64, f64) {
    (PANEL_W - PANEL_PAD - 22.0, 4.0, 20.0, 20.0)
}

fn opacity_rect(state: &AppState) -> (f64, f64, f64, f64) {
    let scale = crate::ui::overlay_scale(state.viewport_w, state.viewport_h);
    (
        state.style_panel_pos.0 + PANEL_PAD * scale,
        state.style_panel_pos.1 + (PANEL_H - 24.0) * scale,
        (PANEL_W - PANEL_PAD * 2.0) * scale,
        16.0 * scale,
    )
}

fn to_local(state: &AppState, sx: f64, sy: f64) -> Option<(f64, f64)> {
    let scale = crate::ui::overlay_scale(state.viewport_w, state.viewport_h);
    let x = (sx - state.style_panel_pos.0) / scale;
    let y = (sy - state.style_panel_pos.1) / scale;
    point_in_rect(x, y, (0.0, 0.0, PANEL_W, PANEL_H)).then_some((x, y))
}

fn point_in_rect(sx: f64, sy: f64, rect: (f64, f64, f64, f64)) -> bool {
    sx >= rect.0 && sx <= rect.0 + rect.2 && sy >= rect.1 && sy <= rect.1 + rect.3
}

fn hit_circle(sx: f64, sy: f64, cx: f64, cy: f64, radius: f64) -> bool {
    let dx = sx - cx;
    let dy = sy - cy;
    dx * dx + dy * dy <= radius * radius
}

fn panel_interactive(state: &AppState) -> bool {
    state.selection_time.elapsed().as_millis() >= 120
}
