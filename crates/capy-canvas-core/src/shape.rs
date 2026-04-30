use std::sync::Arc;

use serde::{Deserialize, Serialize};

mod geometry;

pub use geometry::{point_in_polygon, point_to_segment_dist};

/// Product-level meaning attached to a canvas shape.
///
/// `ShapeKind` stays the renderer geometry (`Rect`, `Image`, `Text`, ...).
/// `CanvasContentKind` says what product object the user and AI should treat
/// the shape as: a video block, webpage block, Brand Kit, project hub, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasContentKind {
    Project,
    ProjectArtifact,
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
            Self::ProjectArtifact => "project_artifact",
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
            "project_artifact" | "project-artifact" | "ProjectArtifact" => {
                Ok(Self::ProjectArtifact)
            }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_ref: Option<CanvasArtifactRef>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanvasArtifactRef {
    pub project_id: String,
    pub surface_node_id: String,
    pub artifact_id: String,
    pub artifact_kind: String,
    pub source_path: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_ref: Option<CanvasArtifactRef>,
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
