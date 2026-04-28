//! Undo/redo snapshot state and command-history helpers.

use crate::shape::{Connector, Shape};
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub shapes: Vec<Shape>,
    pub connectors: Vec<Connector>,
}

impl AppState {
    pub fn push_undo(&mut self) {
        self.undo_stack.push(Snapshot {
            shapes: self.shapes.clone(),
            connectors: self.connectors.clone(),
        });
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) {
        if let Some(snap) = self.undo_stack.pop() {
            self.redo_stack.push(Snapshot {
                shapes: self.shapes.clone(),
                connectors: self.connectors.clone(),
            });
            self.shapes = snap.shapes;
            self.connectors = snap.connectors;
            self.selected.clear();
        }
    }

    pub fn redo(&mut self) {
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(Snapshot {
                shapes: self.shapes.clone(),
                connectors: self.connectors.clone(),
            });
            self.shapes = snap.shapes;
            self.connectors = snap.connectors;
            self.selected.clear();
        }
    }
}
