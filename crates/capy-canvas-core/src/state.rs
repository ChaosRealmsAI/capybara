//! Application state: camera, alignment guides, and flat re-exports for the
//! split state submodules.
//!
//! Extended by sibling modules:
//! - `shape`: Shape, ShapeKind, Tool, styling enums, geometry helpers
//! - `command`: Snapshot + undo/redo helpers
//! - `state_ui`: AppState struct, DragMode, TextEditState, ContextMenu, etc.
//! - `state_shapes`: shape operations (alignment, z-order, copy/paste, groups)
//! - `state_serial`: save/load, SVG export, zoom-to-fit
use serde::{Deserialize, Serialize};

pub use crate::command::Snapshot;
pub use crate::shape::*;
pub use crate::state_ui::*;

// ── Camera ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    pub offset_x: f64,
    pub offset_y: f64,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Camera {
    pub fn zoom_at(&mut self, cursor_x: f64, cursor_y: f64, factor: f64) {
        let new_zoom = (self.zoom * factor).clamp(0.1, 10.0);
        let scale_change = new_zoom / self.zoom;
        self.offset_x = cursor_x - (cursor_x - self.offset_x) * scale_change;
        self.offset_y = cursor_y - (cursor_y - self.offset_y) * scale_change;
        self.zoom = new_zoom;
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.offset_x += dx;
        self.offset_y += dy;
    }

    pub fn screen_to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        (
            (sx - self.offset_x) / self.zoom,
            (sy - self.offset_y) / self.zoom,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AlignGuide {
    Vertical(f64),
    Horizontal(f64),
}

#[cfg(test)]
#[path = "state_style_tests.rs"]
mod style_tests;
#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
