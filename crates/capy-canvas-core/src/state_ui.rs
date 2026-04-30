//! UI interaction state types: drag, text edit, context menu, tooltip, toast,
//! cursor style, and the main AppState struct + constructor.

use crate::state::{
    Camera, Connector, FillStyle, FontFamily, Shape, Snapshot, StrokeStyle, TextAlign, Tool,
};
use crate::text::FontPair;

// ── Cursor style ──

/// Desired cursor icon, computed from tool + interaction context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    Default,
    Pointer,
    Grabbing,
    Crosshair,
    Text,
    Grab,
    NwseResize,
    NeswResize,
    NsResize,
    EwResize,
}

// ── Drag state ──

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragMode {
    None,
    Creating,
    Moving { start_wx: f64, start_wy: f64 },
    Resizing { handle: usize },
    StylePanelDrag { offset_x: f64, offset_y: f64 },
    LineHandleDrag { index: usize, handle: LineHandle },
    Rotating,
    Panning,
    Erasing,
    RubberBand { start_wx: f64, start_wy: f64 },
    OpacityDrag,
    Lasso,
    ConnectorDrag { from_id: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineHandle {
    Start,
    End,
    Mid,
}

// ── Text editing ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTarget {
    Body,
    Label,
}

#[derive(Debug, Clone)]
pub struct TextEditState {
    pub shape_index: usize,
    pub target: TextTarget,
    pub cursor: usize,
    /// Monotonic counter for blinking cursor (toggled by caller).
    pub blink_visible: bool,
    /// Start of selection range (None = no selection).
    pub selection_start: Option<usize>,
}

impl TextEditState {
    /// Returns the selection range as (start, end) with start <= end, or None.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection_start.map(|start| {
            let a = start.min(self.cursor);
            let b = start.max(self.cursor);
            (a, b)
        })
    }

    /// Returns true if there is an active non-empty selection.
    pub fn has_selection(&self) -> bool {
        matches!(self.selection_range(), Some((a, b)) if a != b)
    }

    /// Clear the selection.
    pub fn clear_selection(&mut self) {
        self.selection_start = None;
    }
}

// ── Context menu ──

#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub label: &'static str,
    pub action: ContextAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    BringToFront,
    SendToBack,
    Duplicate,
    Delete,
    Paste,
    SelectAll,
    ResetZoom,
    AlignLeft,
    AlignCenterH,
    AlignRight,
    AlignTop,
    AlignCenterV,
    AlignBottom,
    DistributeH,
    DistributeV,
    SendForward,
    SendBackward,
}

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub sx: f64,
    pub sy: f64,
    pub items: Vec<ContextMenuItem>,
    pub hovered: Option<usize>,
}

impl ContextMenu {
    pub const ITEM_H: f64 = 32.0;
    pub const ITEM_W: f64 = 160.0;
    pub const PAD: f64 = 4.0;
    pub const RADIUS: f64 = 8.0;

    pub fn total_h(&self) -> f64 {
        Self::PAD * 2.0 + self.items.len() as f64 * Self::ITEM_H
    }

    pub fn hit_item(&self, sx: f64, sy: f64) -> Option<usize> {
        let menu_x = self.sx;
        let menu_y = self.sy;
        if sx < menu_x || sx > menu_x + Self::ITEM_W {
            return None;
        }
        let ry = sy - menu_y - Self::PAD;
        if ry < 0.0 || ry >= self.items.len() as f64 * Self::ITEM_H {
            return None;
        }
        let idx = (ry / Self::ITEM_H) as usize;
        (idx < self.items.len()).then_some(idx)
    }
}

// ── Tooltip ──

#[derive(Debug, Clone)]
pub struct TooltipState {
    /// Screen position of the hovered button center-bottom.
    pub sx: f64,
    pub sy: f64,
    pub text: String,
    /// Accumulated hover time in fractional seconds.
    pub hover_time: f64,
}

// ── Toast ──

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub created: web_time::Instant,
    pub duration_ms: u64,
}

impl Toast {
    pub fn new(message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            message: message.into(),
            created: web_time::Instant::now(),
            duration_ms,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created.elapsed().as_millis() as u64 >= self.duration_ms
    }

    /// Returns opacity factor (1.0 = fully visible, fades out in last 300ms).
    pub fn opacity(&self) -> f32 {
        let elapsed = self.created.elapsed().as_millis() as u64;
        if elapsed >= self.duration_ms {
            return 0.0;
        }
        let remaining = self.duration_ms - elapsed;
        if remaining < 300 {
            remaining as f32 / 300.0
        } else {
            1.0
        }
    }
}

// ── AppState ──

#[derive(Debug, Clone)]
pub struct AppState {
    pub shapes: Vec<Shape>,
    pub connectors: Vec<Connector>,
    pub camera: Camera,
    pub tool: Tool,
    pub color: u32,
    pub fill_color: u32,
    pub stroke_width: f64,
    pub stroke_style: StrokeStyle,
    pub fill_style: FillStyle,
    pub rounded: bool,
    pub opacity: f32,
    pub selected: Vec<usize>,
    pub next_id: u64,
    pub drag_mode: DragMode,
    pub drag_start_sx: f64,
    pub drag_start_sy: f64,
    pub drag_shape_origins: Vec<(f64, f64)>,
    pub undo_stack: Vec<Snapshot>,
    pub redo_stack: Vec<Snapshot>,
    pub connector_from: Option<u64>,
    pub text_edit: Option<TextEditState>,
    pub context_menu: Option<ContextMenu>,
    pub clipboard: Vec<Shape>,
    pub next_group_id: u64,
    pub tooltip: Option<TooltipState>,
    pub rubber_band: Option<(f64, f64, f64, f64)>,
    pub hovered_shape: Option<usize>,
    pub selection_time: web_time::Instant,
    pub target_zoom: f64,
    pub viewport_w: f64,
    pub viewport_h: f64,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub space_held: bool,
    pub alt_held: bool,
    pub pending_svg_export: Option<String>,
    pub pending_save_request: bool,
    pub pending_load_request: bool,
    pub cursor_style: CursorStyle,
    pub lasso_points: Vec<(f64, f64)>,
    pub binding_indicator: Option<(f64, f64)>,
    pub connector_preview: Option<(f64, f64, f64, f64)>,
    pub selected_connector: Option<usize>,
    pub fonts: FontPair,
    pub current_font_family: FontFamily,
    pub current_font_size: f64,
    pub current_text_align: TextAlign,
    pub current_bold: bool,
    pub current_italic: bool,
    pub dark_mode: bool,
    pub toasts: Vec<Toast>,
    pub show_help: bool,
    pub style_panel_pos: (f64, f64),
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            shapes: Vec::new(),
            connectors: Vec::new(),
            camera: Camera::default(),
            tool: Tool::Select,
            color: 0x8a6fae,
            fill_color: 0xfef3c7,
            stroke_width: 2.2,
            stroke_style: StrokeStyle::default(),
            fill_style: FillStyle::Hachure,
            rounded: true,
            opacity: 1.0,
            selected: Vec::new(),
            next_id: 1,
            drag_mode: DragMode::None,
            drag_start_sx: 0.0,
            drag_start_sy: 0.0,
            drag_shape_origins: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            connector_from: None,
            text_edit: None,
            context_menu: None,
            clipboard: Vec::new(),
            next_group_id: 1,
            tooltip: None,
            rubber_band: None,
            hovered_shape: None,
            selection_time: web_time::Instant::now(),
            target_zoom: 1.0,
            viewport_w: 1200.0,
            viewport_h: 800.0,
            cursor_x: 0.0,
            cursor_y: 0.0,
            space_held: false,
            alt_held: false,
            pending_svg_export: None,
            pending_save_request: false,
            pending_load_request: false,
            cursor_style: CursorStyle::Default,
            lasso_points: Vec::new(),
            binding_indicator: None,
            connector_preview: None,
            selected_connector: None,
            fonts: FontPair::load_from_bytes(&[], &[], &[], &[], &[]),
            current_font_family: FontFamily::default(),
            current_font_size: 14.0,
            current_text_align: TextAlign::default(),
            current_bold: false,
            current_italic: false,
            dark_mode: false,
            toasts: Vec::new(),
            show_help: false,
            style_panel_pos: (16.0, 70.0),
        }
    }

    pub fn add_shape(&mut self, mut shape: Shape) -> usize {
        shape.id = self.next_id;
        self.next_id += 1;
        self.shapes.push(shape);
        self.shapes.len() - 1
    }
}
