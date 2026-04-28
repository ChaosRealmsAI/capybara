//! State serialization: save/load, SVG export, zoom-to-fit, alignment guides.

use serde::{Deserialize, Serialize};

use crate::line_geometry;
use crate::state::{AlignGuide, AppState, Camera, Connector, Shape, ShapeKind, TextAlign};

impl AppState {
    pub fn hit_test(&self, wx: f64, wy: f64) -> Option<usize> {
        (0..self.shapes.len()).rev().find(|&i| {
            let shape = &self.shapes[i];
            if matches!(shape.kind, ShapeKind::Line | ShapeKind::Arrow) {
                line_geometry::hit_test(self, shape, wx, wy, line_geometry::LINE_HIT_THRESHOLD)
            } else {
                shape.contains(wx, wy)
            }
        })
    }

    pub fn shape_by_id(&self, id: u64) -> Option<&Shape> {
        self.shapes.iter().find(|s| s.id == id)
    }

    /// Alignment guides: find edges aligned within threshold.
    pub fn alignment_guides(&self, dragging_indices: &[usize]) -> Vec<AlignGuide> {
        let threshold = 5.0;
        let mut guides = Vec::new();

        // Collect edges of dragged shapes
        let mut drag_edges_x = Vec::new();
        let mut drag_edges_y = Vec::new();
        for &i in dragging_indices {
            let s = &self.shapes[i];
            drag_edges_x.extend_from_slice(&[s.x, s.x + s.w / 2.0, s.x + s.w]);
            drag_edges_y.extend_from_slice(&[s.y, s.y + s.h / 2.0, s.y + s.h]);
        }

        for (i, s) in self.shapes.iter().enumerate() {
            if dragging_indices.contains(&i) {
                continue;
            }
            let edges_x = [s.x, s.x + s.w / 2.0, s.x + s.w];
            let edges_y = [s.y, s.y + s.h / 2.0, s.y + s.h];

            for &dx in &drag_edges_x {
                for &ex in &edges_x {
                    if (dx - ex).abs() < threshold {
                        guides.push(AlignGuide::Vertical(ex));
                    }
                }
            }
            for &dy in &drag_edges_y {
                for &ey in &edges_y {
                    if (dy - ey).abs() < threshold {
                        guides.push(AlignGuide::Horizontal(ey));
                    }
                }
            }
        }
        guides
    }

    /// Serialize shapes + camera + connectors to JSON.
    pub fn to_json_string(&self) -> Result<String, String> {
        let data = SaveData {
            shapes: self.shapes.clone(),
            connectors: self.connectors.clone(),
            camera: self.camera.clone(),
            next_id: self.next_id,
            next_group_id: self.next_group_id,
        };
        serde_json::to_string_pretty(&data).map_err(|e| e.to_string())
    }

    /// Load shapes + camera + connectors from JSON.
    pub fn load_from_json_str(&mut self, json: &str) -> Result<(), String> {
        let data: SaveData = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let next_group_id = data
            .next_group_id
            .max(next_group_id_from_shapes(&data.shapes));
        self.push_undo();
        self.shapes = data.shapes;
        self.connectors = data.connectors;
        self.camera = data.camera;
        self.target_zoom = self.camera.zoom;
        self.next_id = data.next_id;
        self.next_group_id = next_group_id;
        self.selected.clear();
        Ok(())
    }

    /// Compute the bounding box of the given shape indices.
    /// Returns (min_x, min_y, max_x, max_y) or None if no shapes.
    pub(crate) fn shapes_bbox(&self, indices: &[usize]) -> Option<(f64, f64, f64, f64)> {
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut found = false;
        for &i in indices {
            if let Some(s) = self.shapes.get(i) {
                found = true;
                min_x = min_x.min(s.x);
                min_y = min_y.min(s.y);
                max_x = max_x.max(s.x + s.w);
                max_y = max_y.max(s.y + s.h);
            }
        }
        if found {
            Some((min_x, min_y, max_x, max_y))
        } else {
            None
        }
    }

    /// Zoom the camera to fit all shapes in the viewport with padding.
    pub fn zoom_fit(&mut self) {
        let all: Vec<usize> = (0..self.shapes.len()).collect();
        self.zoom_to_shapes(&all);
    }

    /// Zoom the camera to fit the currently selected shapes.
    pub fn zoom_selection(&mut self) {
        let sel = self.selected.clone();
        self.zoom_to_shapes(&sel);
    }

    /// Zoom camera to fit the given shape indices in the viewport.
    fn zoom_to_shapes(&mut self, indices: &[usize]) {
        let bbox = match self.shapes_bbox(indices) {
            Some(b) => b,
            None => return,
        };
        let (min_x, min_y, max_x, max_y) = bbox;
        let content_w = minimal_span(max_x - min_x);
        let content_h = minimal_span(max_y - min_y);
        let padding = 40.0;
        let available_w = self.viewport_w - padding * 2.0;
        let available_h = self.viewport_h - padding * 2.0;
        if available_w <= 0.0 || available_h <= 0.0 {
            return;
        }
        let zoom = (available_w / content_w)
            .min(available_h / content_h)
            .clamp(0.1, 10.0);
        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;
        self.camera.zoom = zoom;
        self.camera.offset_x = self.viewport_w / 2.0 - cx * zoom;
        self.camera.offset_y = self.viewport_h / 2.0 - cy * zoom;
        self.target_zoom = zoom;
    }

    /// Generate an SVG string representing all shapes.
    pub fn export_svg(&self) -> String {
        // Compute bounding box for viewBox
        let all: Vec<usize> = (0..self.shapes.len()).collect();
        let (min_x, min_y, max_x, max_y) =
            self.shapes_bbox(&all).unwrap_or((0.0, 0.0, 800.0, 600.0));
        let pad = 20.0;
        let vx = min_x - pad;
        let vy = min_y - pad;
        let vw = (max_x - min_x) + pad * 2.0;
        let vh = (max_y - min_y) + pad * 2.0;

        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vx} {vy} {vw} {vh}" width="{vw}" height="{vh}">"#,
        );
        svg.push('\n');

        // Arrow marker definition
        svg.push_str(r#"  <defs>"#);
        svg.push('\n');
        svg.push_str(r#"    <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto">"#);
        svg.push('\n');
        svg.push_str(r#"      <polygon points="0 0, 10 3.5, 0 7" fill="context-stroke"/>"#);
        svg.push('\n');
        svg.push_str("    </marker>\n");
        svg.push_str("  </defs>\n");

        for s in &self.shapes {
            let fill = format!("#{:06x}", s.color);
            let stroke = format!("#{:06x}", s.stroke_color);
            let sw = s.stroke_width;
            let opacity = s.opacity;
            let transform = if s.rotation.abs() > 1e-6 {
                let (cx, cy) = s.center();
                format!(
                    r#" transform="rotate({},{cx},{cy})""#,
                    s.rotation.to_degrees()
                )
            } else {
                String::new()
            };

            match s.kind {
                ShapeKind::Rect | ShapeKind::Image => {
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}"{transform}/>"#,
                        s.x, s.y, s.w, s.h,
                    ));
                    svg.push('\n');
                    if s.kind == ShapeKind::Image {
                        let tx = s.x + s.w / 2.0;
                        let ty = s.y + s.h / 2.0;
                        let img_fill = "#888888";
                        svg.push_str(&format!(
                            r#"  <text x="{tx}" y="{ty}" text-anchor="middle" dominant-baseline="central" fill="{img_fill}" font-size="16"{transform}>IMG</text>"#,
                        ));
                        svg.push('\n');
                    }
                }
                ShapeKind::Ellipse => {
                    let cx = s.x + s.w / 2.0;
                    let cy = s.y + s.h / 2.0;
                    let rx = s.w / 2.0;
                    let ry = s.h / 2.0;
                    svg.push_str(&format!(
                        r#"  <ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}"{transform}/>"#,
                    ));
                    svg.push('\n');
                }
                ShapeKind::Line => {
                    let x2 = s.x + s.w;
                    let y2 = s.y + s.h;
                    svg.push_str(&format!(
                        r#"  <line x1="{}" y1="{}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}"{transform}/>"#,
                        s.x, s.y,
                    ));
                    svg.push('\n');
                }
                ShapeKind::Arrow => {
                    let x2 = s.x + s.w;
                    let y2 = s.y + s.h;
                    svg.push_str(&format!(
                        r#"  <line x1="{}" y1="{}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}" marker-end="url(#arrowhead)"{transform}/>"#,
                        s.x, s.y,
                    ));
                    svg.push('\n');
                }
                ShapeKind::Freehand | ShapeKind::Highlighter => {
                    if s.points.len() >= 2 {
                        let mut d = format!("M{},{}", s.points[0].0, s.points[0].1);
                        for &(px, py) in &s.points[1..] {
                            d.push_str(&format!(" L{px},{py}"));
                        }
                        let fill_attr = "none";
                        svg.push_str(&format!(
                            r#"  <path d="{d}" fill="{fill_attr}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}"{transform}/>"#,
                        ));
                        svg.push('\n');
                    }
                }
                ShapeKind::Text => {
                    let tx = s.x + s.w / 2.0;
                    let ty = s.y + s.h / 2.0;
                    let anchor = match s.text_align {
                        TextAlign::Left => "start",
                        TextAlign::Center => "middle",
                        TextAlign::Right => "end",
                    };
                    let weight = if s.bold { r#" font-weight="bold""# } else { "" };
                    let style = if s.italic {
                        r#" font-style="italic""#
                    } else {
                        ""
                    };
                    let escaped = xml_escape(&s.text);
                    svg.push_str(&format!(
                        r#"  <text x="{tx}" y="{ty}" text-anchor="{anchor}" dominant-baseline="central" fill="{fill}" font-size="{}"{weight}{style} opacity="{opacity}"{transform}>{escaped}</text>"#,
                        s.font_size,
                    ));
                    svg.push('\n');
                }
                ShapeKind::Triangle => {
                    let p1 = format!("{},{}", s.x + s.w / 2.0, s.y);
                    let p2 = format!("{},{}", s.x, s.y + s.h);
                    let p3 = format!("{},{}", s.x + s.w, s.y + s.h);
                    svg.push_str(&format!(
                        r#"  <polygon points="{p1} {p2} {p3}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}"{transform}/>"#,
                    ));
                    svg.push('\n');
                }
                ShapeKind::Diamond => {
                    let top = format!("{},{}", s.x + s.w / 2.0, s.y);
                    let right = format!("{},{}", s.x + s.w, s.y + s.h / 2.0);
                    let bottom = format!("{},{}", s.x + s.w / 2.0, s.y + s.h);
                    let left = format!("{},{}", s.x, s.y + s.h / 2.0);
                    svg.push_str(&format!(
                        r#"  <polygon points="{top} {right} {bottom} {left}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}"{transform}/>"#,
                    ));
                    svg.push('\n');
                }
                ShapeKind::StickyNote => {
                    svg.push_str(&format!(
                        r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}" rx="4"{transform}/>"#,
                        s.x, s.y, s.w, s.h,
                    ));
                    svg.push('\n');
                    if !s.text.is_empty() {
                        let tx = s.x + s.w / 2.0;
                        let ty = s.y + s.h / 2.0;
                        let escaped = xml_escape(&s.text);
                        let sticky_fill = "#333333";
                        svg.push_str(&format!(
                            r#"  <text x="{tx}" y="{ty}" text-anchor="middle" dominant-baseline="central" fill="{sticky_fill}" font-size="{}"{transform}>{escaped}</text>"#,
                            s.font_size,
                        ));
                        svg.push('\n');
                    }
                }
            }
        }

        svg.push_str("</svg>\n");
        svg
    }
}

/// Serializable save file format.
#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub shapes: Vec<Shape>,
    pub connectors: Vec<Connector>,
    pub camera: Camera,
    pub next_id: u64,
    #[serde(default = "default_next_group_id")]
    pub next_group_id: u64,
}

/// Escape special XML/HTML characters for SVG text content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn default_next_group_id() -> u64 {
    1
}

fn next_group_id_from_shapes(shapes: &[Shape]) -> u64 {
    shapes
        .iter()
        .map(|shape| shape.group_id)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn minimal_span(span: f64) -> f64 {
    if span.abs() < 1e-6 { 1.0 } else { span }
}
