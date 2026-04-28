//! AI-facing canvas snapshot.
//!
//! This is the structured view agents should use for layout understanding and
//! operations. It is deliberately separate from the renderer's raw `Shape`
//! list so future UI/rendering fields do not leak into agent contracts.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::state::{
    AppState, CanvasContentKind, CanvasSelectionContext, ConnectorStyle, Shape, ShapeGeometry,
    ShapeKind,
};

pub const CANVAS_AI_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasAiSnapshot {
    pub schema_version: u32,
    pub viewport: CanvasAiViewport,
    pub nodes: Vec<CanvasAiNode>,
    pub connectors: Vec<CanvasAiConnector>,
    pub groups: Vec<CanvasAiGroup>,
    pub selection: CanvasSelectionContext,
    pub available_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasAiViewport {
    pub width: f64,
    pub height: f64,
    pub zoom: f64,
    pub camera_offset: CanvasPoint,
    pub visible_world: ShapeGeometry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasAiNode {
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
    pub bounds: ShapeGeometry,
    pub center: CanvasPoint,
    pub area: f64,
    pub z_index: usize,
    pub group_id: Option<u64>,
    pub selected: bool,
    pub available_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasAiConnector {
    pub index: usize,
    pub from_id: u64,
    pub to_id: u64,
    pub from_title: Option<String>,
    pub to_title: Option<String>,
    pub color: String,
    pub style: ConnectorStyle,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasAiGroup {
    pub group_id: u64,
    pub shape_ids: Vec<u64>,
    pub bounds: ShapeGeometry,
}

impl AppState {
    pub fn ai_snapshot(&self) -> CanvasAiSnapshot {
        CanvasAiSnapshot {
            schema_version: CANVAS_AI_SNAPSHOT_SCHEMA_VERSION,
            viewport: self.ai_viewport(),
            nodes: self.ai_nodes(),
            connectors: self.ai_connectors(),
            groups: self.ai_groups(),
            selection: self.selected_context(),
            available_actions: top_level_actions(),
        }
    }

    pub fn ai_snapshot_text(&self) -> String {
        let snapshot = self.ai_snapshot();
        let mut lines = vec![format!(
            "Canvas snapshot v{}: {} nodes, {} connectors, {} groups, {} selected",
            snapshot.schema_version,
            snapshot.nodes.len(),
            snapshot.connectors.len(),
            snapshot.groups.len(),
            snapshot.selection.selected_count
        )];

        for node in &snapshot.nodes {
            lines.push(format!(
                "- #{} {} [{} · id={}] bounds=({}, {}, {}, {}) z={}{}",
                node.index,
                node.title,
                node.content_kind.as_str(),
                node.id,
                node.bounds.x,
                node.bounds.y,
                node.bounds.w,
                node.bounds.h,
                node.z_index,
                if node.selected { " selected" } else { "" }
            ));
            if let Some(route) = node.editor_route.as_ref() {
                lines.push(format!("  editor: {route}"));
            }
            if let Some(next) = node.next_action.as_ref() {
                lines.push(format!("  next: {next}"));
            }
        }

        for connector in &snapshot.connectors {
            lines.push(format!(
                "- connector #{}: {} -> {}{}",
                connector.index,
                connector.from_title.as_deref().unwrap_or("<missing>"),
                connector.to_title.as_deref().unwrap_or("<missing>"),
                connector
                    .label
                    .as_ref()
                    .map(|label| format!(" label={label}"))
                    .unwrap_or_default()
            ));
        }

        lines.join("\n")
    }

    fn ai_viewport(&self) -> CanvasAiViewport {
        let (x0, y0) = self.camera.screen_to_world(0.0, 0.0);
        let (x1, y1) = self
            .camera
            .screen_to_world(self.viewport_w, self.viewport_h);
        CanvasAiViewport {
            width: self.viewport_w,
            height: self.viewport_h,
            zoom: self.camera.zoom,
            camera_offset: CanvasPoint {
                x: self.camera.offset_x,
                y: self.camera.offset_y,
            },
            visible_world: normalized_geometry(x0, y0, x1 - x0, y1 - y0),
        }
    }

    fn ai_nodes(&self) -> Vec<CanvasAiNode> {
        self.shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| {
                let bounds = shape_bounds(shape);
                let (cx, cy) = shape.center();
                CanvasAiNode {
                    index,
                    id: shape.id,
                    shape_kind: shape.kind,
                    content_kind: shape.content_kind(),
                    title: shape.display_title(),
                    text: shape.text.clone(),
                    status: shape.metadata.status.clone(),
                    owner: shape.metadata.owner.clone(),
                    refs: shape.metadata.refs.clone(),
                    next_action: shape.metadata.next_action.clone(),
                    editor_route: shape.metadata.editor_route.clone(),
                    source_path: shape
                        .metadata
                        .source_path
                        .clone()
                        .or_else(|| shape.image_path.clone()),
                    mime: shape
                        .metadata
                        .mime
                        .clone()
                        .or_else(|| shape.image.as_ref().map(|image| image.mime.clone())),
                    area: bounds.w * bounds.h,
                    bounds,
                    center: CanvasPoint { x: cx, y: cy },
                    z_index: index,
                    group_id: (shape.group_id > 0).then_some(shape.group_id),
                    selected: self.selected.contains(&index),
                    available_actions: node_actions(shape),
                }
            })
            .collect()
    }

    fn ai_connectors(&self) -> Vec<CanvasAiConnector> {
        self.connectors
            .iter()
            .enumerate()
            .map(|(index, connector)| {
                let from = self.shape_by_id(connector.from_id);
                let to = self.shape_by_id(connector.to_id);
                CanvasAiConnector {
                    index,
                    from_id: connector.from_id,
                    to_id: connector.to_id,
                    from_title: from.map(Shape::display_title),
                    to_title: to.map(Shape::display_title),
                    color: format!("#{:06x}", connector.color),
                    style: connector.style,
                    label: connector.label.clone(),
                }
            })
            .collect()
    }

    fn ai_groups(&self) -> Vec<CanvasAiGroup> {
        let mut groups: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for shape in &self.shapes {
            if shape.group_id > 0 {
                groups.entry(shape.group_id).or_default().push(shape.id);
            }
        }
        groups
            .into_iter()
            .filter_map(|(group_id, shape_ids)| {
                self.group_bounds(group_id)
                    .map(|(x, y, w, h)| CanvasAiGroup {
                        group_id,
                        shape_ids,
                        bounds: normalized_geometry(x, y, w, h),
                    })
            })
            .collect()
    }
}

fn top_level_actions() -> Vec<String> {
    [
        "select_by_id",
        "select_by_ids",
        "move_by_id",
        "delete_by_id",
        "update_metadata",
        "open_detail",
        "zoom_selection",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn node_actions(shape: &Shape) -> Vec<String> {
    let mut actions = vec![
        "select_by_id".to_string(),
        "move_by_id".to_string(),
        "delete_by_id".to_string(),
        "update_metadata".to_string(),
    ];
    if shape.metadata.editor_route.is_some() {
        actions.push("open_detail".to_string());
    }
    actions
}

fn shape_bounds(shape: &Shape) -> ShapeGeometry {
    normalized_geometry(shape.x, shape.y, shape.w, shape.h)
}

fn normalized_geometry(x: f64, y: f64, w: f64, h: f64) -> ShapeGeometry {
    ShapeGeometry {
        x: if w < 0.0 { x + w } else { x },
        y: if h < 0.0 { y + h } else { y },
        w: w.abs(),
        h: h.abs(),
    }
}
