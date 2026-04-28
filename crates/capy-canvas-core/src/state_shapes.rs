//! Shape operations: z-order, copy/paste, groups, alignment, distribute, flip.

use std::sync::Arc;

use crate::state::{AppState, CanvasContentKind, Shape, ShapeKind, Toast};

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

    /// Product context for the currently selected shapes.
    ///
    /// This is the AI-facing bridge: UI and CLI callers can send this compact
    /// context to Planner chat instead of forcing the model to infer from pixels.
    pub fn selected_context(&self) -> crate::shape::CanvasSelectionContext {
        let items = self
            .selected
            .iter()
            .filter_map(|&idx| self.shapes.get(idx).map(|shape| shape.selection_item(idx)))
            .collect::<Vec<_>>();
        crate::shape::CanvasSelectionContext {
            selected_count: items.len(),
            items,
        }
    }

    pub fn selected_context_text(&self) -> String {
        let context = self.selected_context();
        if context.items.is_empty() {
            return String::new();
        }
        context
            .items
            .iter()
            .map(|item| {
                let mut lines = vec![
                    format!(
                        "- {} [{} · id={}]",
                        item.title,
                        item.content_kind.as_str(),
                        item.id
                    ),
                    format!(
                        "  geometry: x={} y={} w={} h={}",
                        item.geometry.x, item.geometry.y, item.geometry.w, item.geometry.h
                    ),
                ];
                if !item.text.trim().is_empty() {
                    lines.push(format!("  text: {}", item.text.trim()));
                }
                if let Some(status) = item.status.as_ref() {
                    lines.push(format!("  status: {status}"));
                }
                if let Some(owner) = item.owner.as_ref() {
                    lines.push(format!("  owner: {owner}"));
                }
                if !item.refs.is_empty() {
                    lines.push(format!("  refs: {}", item.refs.join(", ")));
                }
                if let Some(next_action) = item.next_action.as_ref() {
                    lines.push(format!("  next: {next_action}"));
                }
                if let Some(editor_route) = item.editor_route.as_ref() {
                    lines.push(format!("  editor: {editor_route}"));
                }
                if let Some(source_path) = item.source_path.as_ref() {
                    lines.push(format!("  source: {source_path}"));
                }
                if let Some(provider) = item.generation_provider.as_ref() {
                    lines.push(format!("  generation_provider: {provider}"));
                }
                if let Some(prompt) = item.generation_prompt.as_ref() {
                    lines.push(format!("  generation_prompt: {prompt}"));
                }
                lines.join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Create a product-level content card that agents can reason about.
    ///
    /// These cards are the bridge between whiteboard drawing and design-agent
    /// work: visually they behave like canvas objects, while metadata gives AI
    /// a stable kind, status, next action, and future detail-editor route.
    pub fn create_content_card(
        &mut self,
        kind: CanvasContentKind,
        title: impl Into<String>,
        x: f64,
        y: f64,
    ) -> usize {
        self.push_undo();
        let title = title.into();
        let title = if title.trim().is_empty() {
            default_content_title(kind).to_string()
        } else {
            title.trim().to_string()
        };
        let mut shape = Shape::new(ShapeKind::StickyNote, x, y, content_card_color(kind));
        shape.w = 320.0;
        shape.h = 170.0;
        shape.stroke_color = content_card_stroke_color(kind);
        shape.stroke_width = 1.6;
        shape.font_size = 18.0;
        shape.text = format!("{}\n{}", title, content_card_subtitle(kind));
        shape.metadata.content_kind = Some(kind);
        shape.metadata.title = Some(title);
        shape.metadata.status = Some("briefing".to_string());
        shape.metadata.next_action = Some(default_content_next_action(kind).to_string());
        let idx = self.add_shape(shape);
        let id = self.shapes[idx].id;
        self.shapes[idx].metadata.editor_route =
            Some(format!("capy://canvas/{}/{id}", kind.as_str()));
        self.selected = vec![idx];
        idx
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

    // ── Alignment ──

    /// Align all selected shapes to the leftmost x.
    pub fn align_left(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let min_x = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i))
            .map(|s| s.x)
            .fold(f64::MAX, f64::min);
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                self.shapes[i].x = min_x;
            }
        }
    }

    /// Align all selected shapes so their horizontal centers match.
    pub fn align_center_h(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let min_x = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i))
            .map(|s| s.x)
            .fold(f64::MAX, f64::min);
        let max_x = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i))
            .map(|s| s.x + s.w)
            .fold(f64::MIN, f64::max);
        let center_x = (min_x + max_x) / 2.0;
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                self.shapes[i].x = center_x - self.shapes[i].w / 2.0;
            }
        }
    }

    /// Align all selected shapes to the rightmost x + width.
    pub fn align_right(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let max_x = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i))
            .map(|s| s.x + s.w)
            .fold(f64::MIN, f64::max);
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                self.shapes[i].x = max_x - self.shapes[i].w;
            }
        }
    }

    /// Align all selected shapes to the topmost y.
    pub fn align_top(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let min_y = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i))
            .map(|s| s.y)
            .fold(f64::MAX, f64::min);
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                self.shapes[i].y = min_y;
            }
        }
    }

    /// Align all selected shapes so their vertical centers match.
    pub fn align_center_v(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let min_y = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i))
            .map(|s| s.y)
            .fold(f64::MAX, f64::min);
        let max_y = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i))
            .map(|s| s.y + s.h)
            .fold(f64::MIN, f64::max);
        let center_y = (min_y + max_y) / 2.0;
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                self.shapes[i].y = center_y - self.shapes[i].h / 2.0;
            }
        }
    }

    /// Align all selected shapes to the bottommost y + height.
    pub fn align_bottom(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.push_undo();
        let max_y = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i))
            .map(|s| s.y + s.h)
            .fold(f64::MIN, f64::max);
        for &i in &self.selected.clone() {
            if i < self.shapes.len() {
                self.shapes[i].y = max_y - self.shapes[i].h;
            }
        }
    }

    /// Distribute selected shapes with even horizontal spacing (needs 3+).
    pub fn distribute_h(&mut self) {
        if self.selected.len() < 3 {
            return;
        }
        self.push_undo();
        // Sort selected by x position
        let mut indexed: Vec<(usize, f64, f64)> = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i).map(|s| (i, s.x, s.w)))
            .collect();
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let total_width: f64 = indexed.iter().map(|&(_, _, w)| w).sum();
        let first_x = indexed.first().map(|&(_, x, _)| x).unwrap_or(0.0);
        let last = indexed.last().map(|&(_, x, w)| x + w).unwrap_or(0.0);
        let available = last - first_x - total_width;
        let gap = available / (indexed.len() - 1) as f64;
        let mut cursor = first_x;
        for &(i, _, w) in &indexed {
            if i < self.shapes.len() {
                self.shapes[i].x = cursor;
            }
            cursor += w + gap;
        }
    }

    /// Distribute selected shapes with even vertical spacing (needs 3+).
    pub fn distribute_v(&mut self) {
        if self.selected.len() < 3 {
            return;
        }
        self.push_undo();
        // Sort selected by y position
        let mut indexed: Vec<(usize, f64, f64)> = self
            .selected
            .iter()
            .filter_map(|&i| self.shapes.get(i).map(|s| (i, s.y, s.h)))
            .collect();
        indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let total_height: f64 = indexed.iter().map(|&(_, _, h)| h).sum();
        let first_y = indexed.first().map(|&(_, y, _)| y).unwrap_or(0.0);
        let last = indexed.last().map(|&(_, y, h)| y + h).unwrap_or(0.0);
        let available = last - first_y - total_height;
        let gap = available / (indexed.len() - 1) as f64;
        let mut cursor = first_y;
        for &(i, _, h) in &indexed {
            if i < self.shapes.len() {
                self.shapes[i].y = cursor;
            }
            cursor += h + gap;
        }
    }

    /// Import an image as a placeholder shape (path-only · native fs uses this).
    ///
    /// The returned shape carries `image_path` but no decoded RGBA. Native callers
    /// can later populate `shape.image` via a fs decode step; in v0.6 that's not
    /// wired yet, so this still renders the gray "IMG" placeholder.
    pub fn import_image(&mut self, path: &str, x: f64, y: f64) -> usize {
        self.push_undo();
        let mut shape = Shape::new(crate::state::ShapeKind::Image, x, y, 0xdddddd);
        shape.w = 200.0;
        shape.h = 150.0;
        shape.text = "IMG".to_string();
        shape.image_path = Some(path.to_string());
        shape.metadata.content_kind = Some(crate::shape::CanvasContentKind::Image);
        shape.metadata.title = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
            .or_else(|| Some("Image".to_string()));
        shape.metadata.source_path = Some(path.to_string());
        self.add_shape(shape)
    }

    /// Import an image with already-decoded RGBA bytes. The shape is sized to
    /// the image's natural dimensions (clamped to a reasonable on-screen max so
    /// a 4K drop doesn't fly off-canvas) and renders via vello GPU texture.
    ///
    /// Used by the web crate's `add_image_at` / drag-drop path, where the JS
    /// side has already decoded the bytes through the `image` crate.
    pub fn import_image_bytes(
        &mut self,
        x: f64,
        y: f64,
        rgba: Arc<Vec<u8>>,
        width: u32,
        height: u32,
        mime: String,
    ) -> usize {
        self.import_image_asset_bytes(ImageAssetImport {
            x,
            y,
            rgba,
            width,
            height,
            mime,
            title: None,
            source_path: None,
            generation_provider: None,
            generation_prompt: None,
        })
    }

    pub fn import_image_asset_bytes(&mut self, import: ImageAssetImport) -> usize {
        self.push_undo();
        // Clamp the on-screen size: don't insert a 4000px tall thing if the
        // viewport is 800px. Preserve aspect ratio by scaling down uniformly.
        const MAX_SHAPE_DIM: f64 = 600.0;
        let nat_w = import.width as f64;
        let nat_h = import.height as f64;
        let scale = if nat_w > MAX_SHAPE_DIM || nat_h > MAX_SHAPE_DIM {
            (MAX_SHAPE_DIM / nat_w).min(MAX_SHAPE_DIM / nat_h)
        } else {
            1.0
        };
        let mime_for_metadata = import.mime.clone();
        let mut shape = Shape::new(crate::state::ShapeKind::Image, import.x, import.y, 0xdddddd);
        shape.w = nat_w * scale;
        shape.h = nat_h * scale;
        shape.text = String::new();
        shape.metadata.content_kind = Some(crate::shape::CanvasContentKind::Image);
        shape.metadata.title = import
            .title
            .filter(|value| !value.trim().is_empty())
            .or_else(|| Some("Image".to_string()));
        shape.metadata.status = Some("ready".to_string());
        shape.metadata.mime = Some(mime_for_metadata);
        shape.metadata.source_path = import.source_path.filter(|value| !value.trim().is_empty());
        shape.metadata.generation_provider = import
            .generation_provider
            .filter(|value| !value.trim().is_empty());
        shape.metadata.generation_prompt = import
            .generation_prompt
            .filter(|value| !value.trim().is_empty());
        shape.image = Some(crate::shape::RasterImage {
            mime: import.mime,
            width: import.width,
            height: import.height,
            rgba: Some(import.rgba),
            data_url: None,
        });
        self.add_shape(shape)
    }
}

fn default_content_title(kind: CanvasContentKind) -> &'static str {
    match kind {
        CanvasContentKind::Project => "Project hub",
        CanvasContentKind::Brand => "Brand system",
        CanvasContentKind::Image => "Image direction",
        CanvasContentKind::Video => "Storyboard",
        CanvasContentKind::Web => "Web page",
        CanvasContentKind::Text => "Copy block",
        CanvasContentKind::Audio => "Audio cue",
        CanvasContentKind::ThreeD => "3D object",
        CanvasContentKind::Shape => "Canvas object",
    }
}

fn content_card_subtitle(kind: CanvasContentKind) -> &'static str {
    match kind {
        CanvasContentKind::Project => "brief · scope · assets",
        CanvasContentKind::Brand => "logo · palette · mascot",
        CanvasContentKind::Image => "prompt · references · variants",
        CanvasContentKind::Video => "shots · motion · export",
        CanvasContentKind::Web => "sections · states · responsive",
        CanvasContentKind::Text => "headline · tone · variants",
        CanvasContentKind::Audio => "voice · music · timing",
        CanvasContentKind::ThreeD => "model · material · view",
        CanvasContentKind::Shape => "layout · relation · note",
    }
}

fn default_content_next_action(kind: CanvasContentKind) -> &'static str {
    match kind {
        CanvasContentKind::Project => "open project detail and plan next assets",
        CanvasContentKind::Brand => "generate brand directions and lock tokens",
        CanvasContentKind::Image => "generate image variants from references",
        CanvasContentKind::Video => "expand into storyboard shots",
        CanvasContentKind::Web => "open page editor and draft sections",
        CanvasContentKind::Text => "write copy variants in selected tone",
        CanvasContentKind::Audio => "draft voice or music direction",
        CanvasContentKind::ThreeD => "open 3D detail and define model views",
        CanvasContentKind::Shape => "describe object role in the layout",
    }
}

fn content_card_color(kind: CanvasContentKind) -> u32 {
    match kind {
        CanvasContentKind::Project => 0xfff3bf,
        CanvasContentKind::Brand => 0xffedd5,
        CanvasContentKind::Image => 0xfce7f3,
        CanvasContentKind::Video => 0xdbeafe,
        CanvasContentKind::Web => 0xd1fae5,
        CanvasContentKind::Text => 0xede9fe,
        CanvasContentKind::Audio => 0xfbcfe8,
        CanvasContentKind::ThreeD => 0xc7d2fe,
        CanvasContentKind::Shape => 0xe5e7eb,
    }
}

fn content_card_stroke_color(kind: CanvasContentKind) -> u32 {
    match kind {
        CanvasContentKind::Project => 0xd97706,
        CanvasContentKind::Brand => 0xf97316,
        CanvasContentKind::Image => 0xdb2777,
        CanvasContentKind::Video => 0x2563eb,
        CanvasContentKind::Web => 0x059669,
        CanvasContentKind::Text => 0x7c3aed,
        CanvasContentKind::Audio => 0xbe185d,
        CanvasContentKind::ThreeD => 0x4f46e5,
        CanvasContentKind::Shape => 0x64748b,
    }
}
