//! Finalization rules for newly-created canvas shapes.

use crate::state::{AppState, ShapeKind};

pub(crate) fn finalize_created_shape(state: &mut AppState) {
    let Some(&idx) = state.selected.first() else {
        return;
    };
    let Some(shape) = state.shapes.get_mut(idx) else {
        return;
    };
    match shape.kind {
        ShapeKind::Rect | ShapeKind::Ellipse | ShapeKind::Triangle | ShapeKind::Diamond => {
            if shape.w.abs() < 4.0 && shape.h.abs() < 4.0 {
                shape.w = 120.0;
                shape.h = 80.0;
            }
        }
        ShapeKind::Line | ShapeKind::Arrow => {
            if shape.w.hypot(shape.h) < 4.0 {
                shape.w = 140.0;
                shape.h = 0.0;
            }
        }
        ShapeKind::Freehand | ShapeKind::Highlighter => {
            if shape.points.len() < 2 {
                let start = (shape.x, shape.y);
                shape.points.push(start);
                shape.points.push((shape.x + 24.0, shape.y + 12.0));
                shape.w = 24.0;
                shape.h = 12.0;
            }
        }
        ShapeKind::StickyNote | ShapeKind::Text | ShapeKind::Image => {}
    }
}
