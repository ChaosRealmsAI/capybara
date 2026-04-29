//! Shape operations: z-order, copy/paste, groups, alignment, distribute, flip.

use std::sync::Arc;

use crate::state::{AppState, Shape, Toast};

mod arrange;
mod context_cards;
mod image_import;

pub struct ImageAssetImport {
    pub x: f64,
    pub y: f64,
    pub rgba: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub mime: String,
    pub title: Option<String>,
    pub source_path: Option<String>,
    pub generation_provider: Option<String>,
    pub generation_prompt: Option<String>,
}

impl AppState {
    pub fn shape_index_by_id(&self, id: u64) -> Option<usize> {
        self.shapes.iter().position(|shape| shape.id == id)
    }

    pub fn shape_by_id_mut(&mut self, id: u64) -> Option<&mut Shape> {
        self.shapes.iter_mut().find(|shape| shape.id == id)
    }

    pub fn select_shape_ids(&mut self, ids: &[u64]) -> Result<usize, String> {
        let mut indices = Vec::with_capacity(ids.len());
        for &id in ids {
            let idx = self
                .shape_index_by_id(id)
                .ok_or_else(|| format!("shape id {id} not found"))?;
            if !indices.contains(&idx) {
                indices.push(idx);
            }
        }
        self.selected = indices;
        Ok(self.selected.len())
    }

    pub fn move_shape_by_id(&mut self, id: u64, x: f64, y: f64) -> Result<usize, String> {
        let idx = self
            .shape_index_by_id(id)
            .ok_or_else(|| format!("shape id {id} not found"))?;
        self.push_undo();
        self.shapes[idx].x = x;
        self.shapes[idx].y = y;
        self.selected = vec![idx];
        Ok(idx)
    }

    pub fn delete_shape_by_id(&mut self, id: u64) -> Result<usize, String> {
        let idx = self
            .shape_index_by_id(id)
            .ok_or_else(|| format!("shape id {id} not found"))?;
        self.push_undo();
        self.shapes.remove(idx);
        crate::connector::remove_connectors_for_shape(self, id);
        crate::connector::clear_bindings_for_shape(self, id);
        self.selected.clear();
        Ok(idx)
    }

    pub fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let mut indices = self.selected.clone();
        indices.sort_unstable();
        indices.reverse();
        let removed_ids: Vec<u64> = indices.iter().map(|&i| self.shapes[i].id).collect();
        for i in indices {
            self.shapes.remove(i);
        }
        self.connectors
            .retain(|c| !removed_ids.contains(&c.from_id) && !removed_ids.contains(&c.to_id));
        // Clear bindings on arrows that reference deleted shapes
        for rid in &removed_ids {
            crate::connector::clear_bindings_for_shape(self, *rid);
        }
        self.selected.clear();
    }

    pub fn bring_to_front(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let mut indices = self.selected.clone();
        indices.sort_unstable();
        let picked: Vec<Shape> = indices.iter().map(|&i| self.shapes[i].clone()).collect();
        // Remove from back to front to keep indices stable
        for &i in indices.iter().rev() {
            self.shapes.remove(i);
        }
        let new_start = self.shapes.len();
        self.shapes.extend(picked);
        self.selected = (new_start..self.shapes.len()).collect();
    }

    pub fn send_to_back(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let mut indices = self.selected.clone();
        indices.sort_unstable();
        let picked: Vec<Shape> = indices.iter().map(|&i| self.shapes[i].clone()).collect();
        for &i in indices.iter().rev() {
            self.shapes.remove(i);
        }
        let count = picked.len();
        // Insert at front
        for (j, s) in picked.into_iter().enumerate() {
            self.shapes.insert(j, s);
        }
        self.selected = (0..count).collect();
    }

    /// Send selected shape one step forward in z-order (swap with shape above).
    pub fn send_forward(&mut self) {
        if self.selected.len() != 1 {
            return;
        }
        let idx = self.selected[0];
        if idx >= self.shapes.len() - 1 {
            return; // already at top
        }
        self.push_undo();
        self.shapes.swap(idx, idx + 1);
        self.selected = vec![idx + 1];
    }

    /// Send selected shape one step backward in z-order (swap with shape below).
    pub fn send_backward(&mut self) {
        if self.selected.len() != 1 {
            return;
        }
        let idx = self.selected[0];
        if idx == 0 {
            return; // already at bottom
        }
        self.push_undo();
        self.shapes.swap(idx, idx - 1);
        self.selected = vec![idx - 1];
    }

    /// Cut: copy selected shapes, then delete them.
    pub fn cut_selected(&mut self) {
        self.copy_selected();
        self.delete_selected();
        self.show_toast("Cut to clipboard", 2000);
    }

    /// Zoom in by 1.25x centered on viewport center.
    pub fn zoom_in(&mut self) {
        let cx = self.viewport_w / 2.0;
        let cy = self.viewport_h / 2.0;
        let new_zoom = (self.camera.zoom * 1.25).clamp(0.1, 10.0);
        let factor = new_zoom / self.camera.zoom;
        self.camera.zoom_at(cx, cy, factor);
        self.target_zoom = self.camera.zoom;
    }

    /// Zoom out by 0.8x centered on viewport center.
    pub fn zoom_out(&mut self) {
        let cx = self.viewport_w / 2.0;
        let cy = self.viewport_h / 2.0;
        let new_zoom = (self.camera.zoom * 0.8).clamp(0.1, 10.0);
        let factor = new_zoom / self.camera.zoom;
        self.camera.zoom_at(cx, cy, factor);
        self.target_zoom = self.camera.zoom;
    }

    /// Show a toast notification.
    pub fn show_toast(&mut self, message: impl Into<String>, duration_ms: u64) {
        self.toasts.push(Toast::new(message, duration_ms));
    }

    /// Remove expired toasts.
    pub fn gc_toasts(&mut self) {
        self.toasts.retain(|t| !t.is_expired());
    }

    pub fn duplicate_selected(&mut self, offset_x: f64, offset_y: f64) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        let mut new_indices = Vec::new();
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                let mut clone = self.shapes[i].clone();
                clone.x += offset_x;
                clone.y += offset_y;
                clone.group_id = 0; // duplicates are ungrouped
                let idx = self.add_shape(clone);
                new_indices.push(idx);
            }
        }
        self.selected = new_indices;
    }

    pub fn copy_selected(&mut self) {
        self.clipboard = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i).cloned())
            .collect();
    }

    pub fn paste_at(&mut self, wx: f64, wy: f64) {
        if self.clipboard.is_empty() {
            return;
        }
        self.push_undo();
        // Compute center of clipboard shapes for offset
        let cx = self.clipboard.iter().map(|s| s.x + s.w / 2.0).sum::<f64>()
            / self.clipboard.len() as f64;
        let cy = self.clipboard.iter().map(|s| s.y + s.h / 2.0).sum::<f64>()
            / self.clipboard.len() as f64;
        let mut new_sel = Vec::new();
        for s in self.clipboard.clone() {
            let mut ns = s;
            ns.x += wx - cx;
            ns.y += wy - cy;
            ns.group_id = 0;
            let idx = self.add_shape(ns);
            new_sel.push(idx);
        }
        self.selected = new_sel;
    }

    pub fn group_selected(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let gid = self.next_group_id;
        self.next_group_id += 1;
        for &i in &self.selected {
            if i < self.shapes.len() {
                self.shapes[i].group_id = gid;
            }
        }
    }

    pub fn ungroup_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        for &i in &self.selected {
            if i < self.shapes.len() {
                self.shapes[i].group_id = 0;
            }
        }
    }

    /// Expand selection to include all shapes in the same group.
    pub fn expand_selection_to_groups(&mut self) {
        let group_ids: Vec<u64> = self
            .selected
            .iter()
            .filter_map(|&i| {
                let gid = self.shapes.get(i)?.group_id;
                if gid > 0 { Some(gid) } else { None }
            })
            .collect();
        if group_ids.is_empty() {
            return;
        }
        for (i, s) in self.shapes.iter().enumerate() {
            if s.group_id > 0 && group_ids.contains(&s.group_id) && !self.selected.contains(&i) {
                self.selected.push(i);
            }
        }
    }

    /// Bounding box of a group (returns (x, y, w, h) or None).
    pub fn group_bounds(&self, group_id: u64) -> Option<(f64, f64, f64, f64)> {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut found = false;
        for s in &self.shapes {
            if s.group_id == group_id {
                found = true;
                min_x = min_x.min(s.x);
                min_y = min_y.min(s.y);
                max_x = max_x.max(s.x + s.w);
                max_y = max_y.max(s.y + s.h);
            }
        }
        if found {
            Some((min_x, min_y, max_x - min_x, max_y - min_y))
        } else {
            None
        }
    }

    /// Flip selected shapes horizontally (mirror x around bounding box center).
    pub fn flip_selected_h(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                self.shapes[i].flipped_h = !self.shapes[i].flipped_h;
            }
        }
    }

    /// Flip selected shapes vertically (mirror y around bounding box center).
    pub fn flip_selected_v(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.push_undo();
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                self.shapes[i].flipped_v = !self.shapes[i].flipped_v;
            }
        }
    }
}
