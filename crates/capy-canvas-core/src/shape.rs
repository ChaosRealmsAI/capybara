use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Product-level meaning attached to a canvas shape.
///
/// `ShapeKind` stays the renderer geometry (`Rect`, `Image`, `Text`, ...).
/// `CanvasContentKind` says what product object the user and AI should treat
/// the shape as: a video block, webpage block, Brand Kit, project hub, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasContentKind {
    Project,
    Brand,
    Image,
    Poster,
    Video,
    Web,
    Text,
    Audio,
    ThreeD,
    Shape,
}

impl CanvasContentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Brand => "brand",
            Self::Image => "image",
            Self::Poster => "poster",
            Self::Video => "video",
            Self::Web => "web",
            Self::Text => "text",
            Self::Audio => "audio",
            Self::ThreeD => "3d",
            Self::Shape => "shape",
        }
    }
}

impl std::str::FromStr for CanvasContentKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "project" | "Project" => Ok(Self::Project),
            "brand" | "Brand" | "brand_kit" | "BrandKit" => Ok(Self::Brand),
            "image" | "Image" => Ok(Self::Image),
            "poster" | "Poster" | "poster_document" | "PosterDocument" => Ok(Self::Poster),
            "video" | "Video" => Ok(Self::Video),
            "web" | "Web" | "page" | "Page" => Ok(Self::Web),
            "text" | "Text" | "copy" | "Copy" => Ok(Self::Text),
            "audio" | "Audio" => Ok(Self::Audio),
            "3d" | "three_d" | "ThreeD" | "model" | "Model" => Ok(Self::ThreeD),
            "shape" | "Shape" => Ok(Self::Shape),
            other => Err(format!("invalid canvas content kind: {other}")),
        }
    }
}

/// Product metadata used by Planner chat, AI verification, and future editors.
///
/// Keep this small and serializable. Heavy bytes live in renderer-specific
/// fields such as `RasterImage`; metadata only carries product context.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanvasMetadata {
    #[serde(default)]
    pub content_kind: Option<CanvasContentKind>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub next_action: Option<String>,
    #[serde(default)]
    pub editor_route: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub mime: Option<String>,
    #[serde(default)]
    pub generation_provider: Option<String>,
    #[serde(default)]
    pub generation_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeGeometry {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasSelectionItem {
    pub index: usize,
    pub id: u64,
    pub shape_kind: ShapeKind,
    pub content_kind: CanvasContentKind,
    pub title: String,
    pub text: String,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub refs: Vec<String>,
    pub next_action: Option<String>,
    pub editor_route: Option<String>,
    pub source_path: Option<String>,
    pub mime: Option<String>,
    pub generation_provider: Option<String>,
    pub generation_prompt: Option<String>,
    pub geometry: ShapeGeometry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasSelectionContext {
    pub selected_count: usize,
    pub items: Vec<CanvasSelectionItem>,
}

/// Decoded raster image attached to a `ShapeKind::Image`.
///
/// Stores RGBA8 pixels (straight, not premultiplied) plus the dimensions, so
/// the renderer can hand them straight to `vello::Scene::draw_image` via a
/// `peniko::ImageBrush` with no per-frame decode. `rgba` is wrapped in `Arc`
/// to make `Shape::clone()` (used heavily by undo/redo + drag/resize) cheap.
///
/// `rgba` is `#[serde(skip)]` because the bytes are too large to round-trip
/// through JSON; persistence uses `data_url` instead (when present, the v0.4
/// IndexedDB save/load path can carry a base64-encoded PNG/JPEG · v0.6 just
/// drops the bytes so reload renders the placeholder · acceptable for now).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RasterImage {
    pub mime: String,
    pub width: u32,
    pub height: u32,
    #[serde(skip)]
    pub rgba: Option<Arc<Vec<u8>>>,
    #[serde(default)]
    pub data_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrokeStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FillStyle {
    None,
    #[default]
    Solid,
    Hachure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Select,
    Rect,
    Ellipse,
    Triangle,
    Diamond,
    Line,
    Arrow,
    Freehand,
    Highlighter,
    StickyNote,
    Text,
    Eraser,
    Lasso,
}

impl Tool {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Rect => "Rect",
            Self::Ellipse => "Ellipse",
            Self::Triangle => "Triangle",
            Self::Diamond => "Diamond",
            Self::Line => "Line",
            Self::Arrow => "Arrow",
            Self::Freehand => "Freehand",
            Self::Highlighter => "Highlighter",
            Self::StickyNote => "Sticky Note",
            Self::Text => "Text",
            Self::Eraser => "Eraser",
            Self::Lasso => "Lasso",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::Select => "V",
            Self::Rect => "R",
            Self::Ellipse => "E",
            Self::Triangle => "G",
            Self::Diamond => "B",
            Self::Line => "L",
            Self::Arrow => "A",
            Self::Freehand => "D",
            Self::Highlighter => "H",
            Self::StickyNote => "S",
            Self::Text => "T",
            Self::Eraser => "X",
            Self::Lasso => "Q",
        }
    }

    pub fn all_toolbar() -> &'static [Tool] {
        &[
            Tool::Select,
            Tool::Rect,
            Tool::Ellipse,
            Tool::Triangle,
            Tool::Diamond,
            Tool::Line,
            Tool::Arrow,
            Tool::Freehand,
            Tool::Highlighter,
            Tool::StickyNote,
            Tool::Text,
            Tool::Eraser,
            Tool::Lasso,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArrowHead {
    None,
    #[default]
    Triangle,
    Circle,
    Diamond,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArrowStyle {
    #[default]
    Straight,
    Curved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorStyle {
    #[default]
    Straight,
    Elbow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FontFamily {
    #[default]
    SansSerif,
    Serif,
    Mono,
    Handwritten,
}

impl FontFamily {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SansSerif => "Sans Serif",
            Self::Serif => "Serif",
            Self::Mono => "Mono",
            Self::Handwritten => "Handwritten",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Triangle,
    Diamond,
    StickyNote,
    Line,
    Arrow,
    Freehand,
    Highlighter,
    Text,
    Image,
}

impl ShapeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rect => "Rect",
            Self::Ellipse => "Ellipse",
            Self::Triangle => "Triangle",
            Self::Diamond => "Diamond",
            Self::StickyNote => "Sticky Note",
            Self::Line => "Line",
            Self::Arrow => "Arrow",
            Self::Freehand => "Freehand",
            Self::Highlighter => "Highlighter",
            Self::Text => "Text",
            Self::Image => "Image",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    pub id: u64,
    pub kind: ShapeKind,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub color: u32,
    pub stroke_color: u32,
    pub stroke_width: f64,
    #[serde(default)]
    pub stroke_style: StrokeStyle,
    #[serde(default)]
    pub fill_style: FillStyle,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub flipped_h: bool,
    #[serde(default)]
    pub flipped_v: bool,
    pub text: String,
    pub points: Vec<(f64, f64)>,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub group_id: u64,
    #[serde(default)]
    pub arrow_start: ArrowHead,
    #[serde(default)]
    pub arrow_end: ArrowHead,
    #[serde(default)]
    pub arrow_style: ArrowStyle,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub font_family: FontFamily,
    #[serde(default = "default_font_size")]
    pub font_size: f64,
    #[serde(default)]
    pub text_align: TextAlign,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub image_path: Option<String>,
    #[serde(default)]
    pub metadata: CanvasMetadata,
    /// Decoded RGBA bytes for the attached raster image.
    /// `None` falls back to the gray "IMG" placeholder render.
    #[serde(default, skip)]
    pub image: Option<RasterImage>,
    #[serde(default)]
    pub binding_start: Option<u64>,
    #[serde(default)]
    pub binding_end: Option<u64>,
    #[serde(default = "default_rounded")]
    pub rounded: bool,
}

fn default_opacity() -> f32 {
    1.0
}
fn default_rounded() -> bool {
    true
}
fn default_font_size() -> f64 {
    14.0
}

impl Shape {
    pub fn new(kind: ShapeKind, x: f64, y: f64, color: u32) -> Self {
        Self {
            id: 0,
            kind,
            x,
            y,
            w: 0.0,
            h: 0.0,
            color,
            stroke_color: color,
            stroke_width: 2.0,
            stroke_style: StrokeStyle::default(),
            fill_style: FillStyle::Solid,
            opacity: 1.0,
            flipped_h: false,
            flipped_v: false,
            text: String::new(),
            points: Vec::new(),
            rotation: 0.0,
            group_id: 0,
            arrow_start: ArrowHead::None,
            arrow_end: ArrowHead::default(),
            arrow_style: ArrowStyle::default(),
            label: None,
            font_family: FontFamily::default(),
            font_size: 14.0,
            text_align: TextAlign::default(),
            bold: false,
            italic: false,
            image_path: None,
            metadata: CanvasMetadata::default(),
            image: None,
            binding_start: None,
            binding_end: None,
            rounded: true,
        }
    }

    pub fn content_kind(&self) -> CanvasContentKind {
        if let Some(kind) = self.metadata.content_kind {
            return kind;
        }
        match self.kind {
            ShapeKind::Image => CanvasContentKind::Image,
            ShapeKind::Text | ShapeKind::StickyNote => CanvasContentKind::Text,
            _ => CanvasContentKind::Shape,
        }
    }

    pub fn display_title(&self) -> String {
        if let Some(title) = self
            .metadata
            .title
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            return title.clone();
        }
        if let Some(label) = self.label.as_ref().filter(|value| !value.is_empty()) {
            return label.clone();
        }
        let text = self.text.trim();
        if !text.is_empty() {
            return text
                .lines()
                .next()
                .unwrap_or(text)
                .chars()
                .take(48)
                .collect();
        }
        match self.content_kind() {
            CanvasContentKind::Project => "Project".to_string(),
            CanvasContentKind::Brand => "Brand Kit".to_string(),
            CanvasContentKind::Image => "Image".to_string(),
            CanvasContentKind::Poster => "Poster".to_string(),
            CanvasContentKind::Video => "Video".to_string(),
            CanvasContentKind::Web => "Web".to_string(),
            CanvasContentKind::Text => "Text".to_string(),
            CanvasContentKind::Audio => "Audio".to_string(),
            CanvasContentKind::ThreeD => "3D".to_string(),
            CanvasContentKind::Shape => self.kind.label().to_string(),
        }
    }

    pub fn selection_item(&self, index: usize) -> CanvasSelectionItem {
        CanvasSelectionItem {
            index,
            id: self.id,
            shape_kind: self.kind,
            content_kind: self.content_kind(),
            title: self.display_title(),
            text: self.text.clone(),
            status: self.metadata.status.clone(),
            owner: self.metadata.owner.clone(),
            refs: self.metadata.refs.clone(),
            next_action: self.metadata.next_action.clone(),
            editor_route: self.metadata.editor_route.clone(),
            source_path: self
                .metadata
                .source_path
                .clone()
                .or_else(|| self.image_path.clone()),
            mime: self
                .metadata
                .mime
                .clone()
                .or_else(|| self.image.as_ref().map(|image| image.mime.clone())),
            generation_provider: self.metadata.generation_provider.clone(),
            generation_prompt: self.metadata.generation_prompt.clone(),
            geometry: ShapeGeometry {
                x: self.x,
                y: self.y,
                w: self.w,
                h: self.h,
            },
        }
    }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        let (px, py) = self.untransform_point(px, py);
        match self.kind {
            ShapeKind::Rect
            | ShapeKind::Text
            | ShapeKind::Freehand
            | ShapeKind::StickyNote
            | ShapeKind::Highlighter
            | ShapeKind::Image => {
                px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
            }
            ShapeKind::Ellipse => {
                let cx = self.x + self.w / 2.0;
                let cy = self.y + self.h / 2.0;
                let rx = self.w / 2.0;
                let ry = self.h / 2.0;
                if rx <= 0.0 || ry <= 0.0 {
                    return false;
                }
                let dx = (px - cx) / rx;
                let dy = (py - cy) / ry;
                dx * dx + dy * dy <= 1.0
            }
            ShapeKind::Triangle => {
                let ax = self.x + self.w / 2.0;
                let ay = self.y;
                let bx = self.x;
                let by = self.y + self.h;
                let cx = self.x + self.w;
                let cy = self.y + self.h;
                point_in_triangle((px, py), (ax, ay), (bx, by), (cx, cy))
            }
            ShapeKind::Diamond => {
                let top = (self.x + self.w / 2.0, self.y);
                let right = (self.x + self.w, self.y + self.h / 2.0);
                let bottom = (self.x + self.w / 2.0, self.y + self.h);
                let left = (self.x, self.y + self.h / 2.0);
                point_in_triangle((px, py), top, right, left)
                    || point_in_triangle((px, py), bottom, right, left)
            }
            ShapeKind::Line | ShapeKind::Arrow => {
                let x2 = self.x + self.w;
                let y2 = self.y + self.h;
                point_to_segment_dist(px, py, self.x, self.y, x2, y2) <= 5.0
            }
        }
    }

    fn untransform_point(&self, px: f64, py: f64) -> (f64, f64) {
        if self.rotation.abs() <= 1e-6 && !self.flipped_h && !self.flipped_v {
            return (px, py);
        }

        let (cx, cy) = self.center();
        let mut dx = px - cx;
        let mut dy = py - cy;

        if self.rotation.abs() > 1e-6 {
            let (sin_r, cos_r) = self.rotation.sin_cos();
            let rotated_dx = dx * cos_r + dy * sin_r;
            let rotated_dy = -dx * sin_r + dy * cos_r;
            dx = rotated_dx;
            dy = rotated_dy;
        }
        if self.flipped_h {
            dx = -dx;
        }
        if self.flipped_v {
            dy = -dy;
        }

        (cx + dx, cy + dy)
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn edge_point(&self, tx: f64, ty: f64) -> (f64, f64) {
        let anchors = self.anchor_points();
        let mut best_anchor = anchors[0];
        let mut best_dist = f64::MAX;
        for &(ax, ay) in &anchors {
            let d = ((tx - ax).powi(2) + (ty - ay).powi(2)).sqrt();
            if d < best_dist {
                best_dist = d;
                best_anchor = (ax, ay);
            }
        }
        if best_dist < 15.0 {
            return best_anchor;
        }

        let (cx, cy) = self.center();
        let dx = tx - cx;
        let dy = ty - cy;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            return (cx, cy);
        }
        let nx = dx / len;
        let ny = dy / len;
        let hw = self.w / 2.0;
        let hh = self.h / 2.0;
        let tx_edge = if nx.abs() > 1e-6 {
            hw / nx.abs()
        } else {
            f64::MAX
        };
        let ty_edge = if ny.abs() > 1e-6 {
            hh / ny.abs()
        } else {
            f64::MAX
        };
        let t = tx_edge.min(ty_edge);
        (cx + nx * t, cy + ny * t)
    }

    pub fn anchor_points(&self) -> [(f64, f64); 4] {
        let (hw, hh) = (self.w / 2.0, self.h / 2.0);
        [
            (self.x + hw, self.y),
            (self.x + self.w, self.y + hh),
            (self.x + hw, self.y + self.h),
            (self.x, self.y + hh),
        ]
    }
    pub fn default_color_for_kind(kind: ShapeKind) -> u32 {
        match kind {
            ShapeKind::Rect => 0x5b8abf,
            ShapeKind::Ellipse => 0x3da065,
            ShapeKind::Triangle => 0xe8a348,
            ShapeKind::Diamond => 0x8a6fae,
            ShapeKind::StickyNote => 0xfef3c7,
            ShapeKind::Text => 0x1e293b,
            ShapeKind::Arrow | ShapeKind::Line | ShapeKind::Freehand => 0x64748b,
            ShapeKind::Highlighter => 0xfbbf24,
            ShapeKind::Image => 0x94a3b8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connector {
    pub from_id: u64,
    pub to_id: u64,
    pub color: u32,
    #[serde(default)]
    pub style: ConnectorStyle,
    #[serde(default)]
    pub label: Option<String>,
}

pub const PALETTE: &[u32] = &[
    0x1e1e1e, 0xd94f5c, 0x3da065, 0x5b8abf, 0xe8a348, 0x8a6fae, 0x3ea8a0, 0xd97745, 0xe88da2,
    0x64748b,
];

pub(crate) fn point_to_segment_dist(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-10 {
        return ((px - x1).powi(2) + (py - y1).powi(2)).sqrt();
    }
    let t = (((px - x1) * dx + (py - y1) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = x1 + t * dx;
    let proj_y = y1 + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}

fn point_in_triangle(point: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> bool {
    let d1 = cross_2d(point.0, point.1, a.0, a.1, b.0, b.1);
    let d2 = cross_2d(point.0, point.1, b.0, b.1, c.0, c.1);
    let d3 = cross_2d(point.0, point.1, c.0, c.1, a.0, a.1);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn cross_2d(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    (ax - px) * (by - py) - (bx - px) * (ay - py)
}

pub fn point_in_polygon(px: f64, py: f64, polygon: &[(f64, f64)]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = polygon[i];
        let (xj, yj) = polygon[j];
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}
