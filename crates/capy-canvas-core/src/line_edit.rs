//! Direct manipulation behavior for selected line and arrow shapes.

use crate::input_tools::snap_to_grid;
use crate::line_geometry;
use crate::state::{AppState, CursorStyle, DragMode, LineHandle, ShapeKind};
use crate::text_edit::{begin_label_edit, begin_text_edit};

pub(crate) fn begin_handle_drag(state: &mut AppState, wx: f64, wy: f64) -> bool {
    if state.selected.len() != 1 {
        return false;
    }
    let idx = state.selected[0];
    let Some(shape) = state.shapes.get(idx) else {
        return false;
    };
    if !matches!(shape.kind, ShapeKind::Line | ShapeKind::Arrow) {
        return false;
    }
    let Some(handle) = line_geometry::hit_handle(state, shape, wx, wy, state.camera.zoom) else {
        return false;
    };
    state.drag_mode = DragMode::LineHandleDrag { index: idx, handle };
    state.push_undo();
    true
}

pub(crate) fn handle_drag_move(
    state: &mut AppState,
    index: usize,
    handle: LineHandle,
    wx: f64,
    wy: f64,
) -> bool {
    if index >= state.shapes.len() {
        return false;
    }

    if handle == LineHandle::Mid {
        let point = if state.alt_held {
            (wx, wy)
        } else {
            (snap_to_grid(wx), snap_to_grid(wy))
        };
        line_geometry::set_endpoint(&mut state.shapes[index], handle, point, point, None);
        return true;
    }

    let (start, end) = {
        let shape = &state.shapes[index];
        line_geometry::endpoints(state, shape)
    };
    let fixed = match handle {
        LineHandle::Start => (end, start),
        LineHandle::End => (start, end),
        LineHandle::Mid => unreachable!(),
    }
    .0;

    let snapped = if state.alt_held {
        (wx, wy)
    } else {
        (snap_to_grid(wx), snap_to_grid(wy))
    };
    let (binding, point) = line_geometry::nearest_binding(state, state.shapes[index].id, wx, wy)
        .map_or((None, snapped), |(shape_id, anchor)| {
            (Some(shape_id), anchor)
        });

    let shape = &mut state.shapes[index];
    line_geometry::set_endpoint(shape, handle, fixed, point, binding);

    if handle == LineHandle::Start {
        if binding.is_none() {
            shape.binding_start = None;
        }
    } else if binding.is_none() {
        shape.binding_end = None;
    }
    true
}

pub(crate) fn handle_drag_move_screen(
    state: &mut AppState,
    index: usize,
    handle: LineHandle,
    sx: f64,
    sy: f64,
) -> bool {
    let (wx, wy) = state.camera.screen_to_world(sx, sy);
    handle_drag_move(state, index, handle, wx, wy)
}

pub(crate) fn cursor_for_selected_handle(
    state: &AppState,
    idx: usize,
    wx: f64,
    wy: f64,
) -> Option<CursorStyle> {
    let shape = state.shapes.get(idx)?;
    let handle = line_geometry::hit_handle(state, shape, wx, wy, state.camera.zoom)?;
    Some(match handle {
        LineHandle::Mid => CursorStyle::Pointer,
        LineHandle::Start | LineHandle::End => CursorStyle::Pointer,
    })
}

pub(crate) fn begin_arrow_label_edit_if_hit(state: &mut AppState, wx: f64, wy: f64) -> bool {
    let Some(idx) = state.hit_test(wx, wy) else {
        return false;
    };
    if state.shapes[idx].kind != ShapeKind::Arrow {
        return false;
    }
    begin_label_edit(state, idx);
    true
}

pub(crate) fn handle_double_click(state: &mut AppState) -> bool {
    let (wx, wy) = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
    if begin_arrow_label_edit_if_hit(state, wx, wy) {
        return true;
    }
    if let Some(idx) = state.hit_test(wx, wy) {
        if matches!(
            state.shapes[idx].kind,
            ShapeKind::Text | ShapeKind::StickyNote
        ) {
            begin_text_edit(state, idx);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Shape, Tool};

    #[test]
    fn begin_handle_drag_detects_endpoint() {
        let mut state = AppState::new();
        state.tool = Tool::Select;

        let mut line = Shape::new(ShapeKind::Line, 10.0, 20.0, 0);
        line.w = 100.0;
        line.h = 40.0;
        let idx = state.add_shape(line);
        state.selected = vec![idx];

        assert!(begin_handle_drag(&mut state, 10.0, 20.0));
        assert_eq!(
            state.drag_mode,
            DragMode::LineHandleDrag {
                index: idx,
                handle: LineHandle::Start,
            }
        );
    }
}
