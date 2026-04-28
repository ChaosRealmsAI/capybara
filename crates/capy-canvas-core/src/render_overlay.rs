//! Render overlays beyond the core render UI: creation preview, group bounds,
//! rubber band, lasso, binding indicator, eraser cursor, rotation tooltip.

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Line, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

use crate::render::color_from_hex;
use crate::state::{AppState, ShapeKind};

/// Draw dashed preview while creating a shape.
pub(crate) fn draw_creation_preview(scene: &mut Scene, state: &AppState, camera_tf: Affine) {
    if let Some(&idx) = state.selected.first() {
        if idx >= state.shapes.len() {
            return;
        }
        let shape = &state.shapes[idx];
        if shape.w.abs() < 2.0 && shape.h.abs() < 2.0 {
            return;
        }
        let preview_fill = color_from_hex(shape.color, 0.3);
        let preview_stroke_color = color_from_hex(shape.stroke_color, 0.6);
        let dash_stroke = Stroke::new(1.5).with_dashes(0.0, [6.0, 4.0]);

        match shape.kind {
            ShapeKind::Rect => {
                let r = Rect::new(shape.x, shape.y, shape.x + shape.w, shape.y + shape.h);
                let rr = RoundedRect::from_rect(r, if shape.rounded { 12.0 } else { 0.0 });
                scene.fill(Fill::NonZero, camera_tf, preview_fill, None, &rr);
                scene.stroke(&dash_stroke, camera_tf, preview_stroke_color, None, &rr);
            }
            ShapeKind::Ellipse => {
                let cx = shape.x + shape.w / 2.0;
                let cy = shape.y + shape.h / 2.0;
                let ellipse =
                    vello::kurbo::Ellipse::new((cx, cy), (shape.w / 2.0, shape.h / 2.0), 0.0);
                scene.fill(Fill::NonZero, camera_tf, preview_fill, None, &ellipse);
                scene.stroke(
                    &dash_stroke,
                    camera_tf,
                    preview_stroke_color,
                    None,
                    &ellipse,
                );
            }
            ShapeKind::Line | ShapeKind::Arrow => {
                let line = Line::new((shape.x, shape.y), (shape.x + shape.w, shape.y + shape.h));
                scene.stroke(&dash_stroke, camera_tf, preview_stroke_color, None, &line);
            }
            ShapeKind::Triangle => {
                let mut path = BezPath::new();
                path.move_to((shape.x + shape.w / 2.0, shape.y));
                path.line_to((shape.x, shape.y + shape.h));
                path.line_to((shape.x + shape.w, shape.y + shape.h));
                path.close_path();
                scene.fill(Fill::NonZero, camera_tf, preview_fill, None, &path);
                scene.stroke(&dash_stroke, camera_tf, preview_stroke_color, None, &path);
            }
            ShapeKind::Diamond => {
                let mut path = BezPath::new();
                path.move_to((shape.x + shape.w / 2.0, shape.y));
                path.line_to((shape.x + shape.w, shape.y + shape.h / 2.0));
                path.line_to((shape.x + shape.w / 2.0, shape.y + shape.h));
                path.line_to((shape.x, shape.y + shape.h / 2.0));
                path.close_path();
                scene.fill(Fill::NonZero, camera_tf, preview_fill, None, &path);
                scene.stroke(&dash_stroke, camera_tf, preview_stroke_color, None, &path);
            }
            ShapeKind::StickyNote => {
                let r = Rect::new(shape.x, shape.y, shape.x + shape.w, shape.y + shape.h);
                let rr = RoundedRect::from_rect(r, if shape.rounded { 12.0 } else { 0.0 });
                scene.fill(Fill::NonZero, camera_tf, preview_fill, None, &rr);
                scene.stroke(&dash_stroke, camera_tf, preview_stroke_color, None, &rr);
            }
            _ => {}
        }
    }
}

/// Draw bounding boxes for groups that have selected members.
pub(crate) fn draw_group_bounds(scene: &mut Scene, state: &AppState, camera_tf: Affine) {
    let mut drawn_groups: Vec<u64> = Vec::new();
    for &i in &state.selected {
        if i >= state.shapes.len() {
            continue;
        }
        let gid = state.shapes[i].group_id;
        if gid == 0 || drawn_groups.contains(&gid) {
            continue;
        }
        drawn_groups.push(gid);
        if let Some((gx, gy, gw, gh)) = state.group_bounds(gid) {
            let gap = 4.0;
            let r = Rect::new(gx - gap, gy - gap, gx + gw + gap, gy + gh + gap);
            let group_stroke = Stroke::new(1.0).with_dashes(0.0, [8.0, 4.0]);
            let group_color = Color::from_rgba8(0x7c, 0x3a, 0xed, 0x88);
            scene.stroke(&group_stroke, camera_tf, group_color, None, &r);
        }
    }
}

/// Draw a red circle cursor when the eraser tool is active.
pub(crate) fn draw_eraser_cursor(scene: &mut Scene, state: &AppState) {
    let radius = 10.0;
    let cx = state.cursor_x;
    let cy = state.cursor_y;
    let circle = Circle::new((cx, cy), radius);
    let eraser_red = Color::from_rgba8(0xe0, 0x33, 0x33, 0x55);
    let eraser_stroke = Color::from_rgba8(0xe0, 0x33, 0x33, 0xcc);
    scene.fill(Fill::NonZero, Affine::IDENTITY, eraser_red, None, &circle);
    scene.stroke(
        &Stroke::new(1.5),
        Affine::IDENTITY,
        eraser_stroke,
        None,
        &circle,
    );
}

pub(crate) fn draw_rubber_band(scene: &mut Scene, state: &AppState, camera_tf: Affine) {
    if let Some((rx, ry, rw, rh)) = state.rubber_band {
        let rect = Rect::new(rx, ry, rx + rw, ry + rh);
        let fill = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x18);
        let stroke_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xaa);
        scene.fill(Fill::NonZero, camera_tf, fill, None, &rect);
        scene.stroke(
            &Stroke::new(1.0).with_dashes(0.0, [4.0, 3.0]),
            camera_tf,
            stroke_color,
            None,
            &rect,
        );
    }
}

pub(crate) fn draw_lasso(scene: &mut Scene, state: &AppState, camera_tf: Affine) {
    if state.lasso_points.len() < 2 {
        return;
    }
    let mut path = BezPath::new();
    path.move_to(state.lasso_points[0]);
    for &pt in &state.lasso_points[1..] {
        path.line_to(pt);
    }
    let lasso_stroke = Stroke::new(1.5).with_dashes(0.0, [6.0, 4.0]);
    let lasso_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xcc);
    scene.stroke(&lasso_stroke, camera_tf, lasso_color, None, &path);

    path.close_path();
    let lasso_fill = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x18);
    scene.fill(Fill::NonZero, camera_tf, lasso_fill, None, &path);
}

/// Draw anchor points on a shape when hovering in Arrow/Line tool for binding.
/// Shows 4 anchor points; the closest to mouse is drawn larger and fully opaque.
pub(crate) fn draw_connector_anchors(
    scene: &mut Scene,
    shape: &crate::state::Shape,
    mouse_wx: f64,
    mouse_wy: f64,
    camera_tf: Affine,
) {
    let anchors = shape.anchor_points();
    let fill_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x4d); // 30% opacity
    let stroke_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xff);

    // Find closest anchor
    let mut closest_idx = 0;
    let mut closest_dist = f64::MAX;
    for (i, &(ax, ay)) in anchors.iter().enumerate() {
        let d = ((mouse_wx - ax).powi(2) + (mouse_wy - ay).powi(2)).sqrt();
        if d < closest_dist {
            closest_dist = d;
            closest_idx = i;
        }
    }

    for (i, &(ax, ay)) in anchors.iter().enumerate() {
        if i == closest_idx {
            // Closest anchor: larger and fully opaque
            let circle = Circle::new((ax, ay), 8.0);
            let active_fill = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xaa);
            scene.fill(Fill::NonZero, camera_tf, active_fill, None, &circle);
            scene.stroke(&Stroke::new(1.5), camera_tf, stroke_color, None, &circle);
        } else {
            let circle = Circle::new((ax, ay), 6.0);
            scene.fill(Fill::NonZero, camera_tf, fill_color, None, &circle);
            scene.stroke(&Stroke::new(1.5), camera_tf, stroke_color, None, &circle);
        }
    }
}

/// Draw the drag-to-create bound arrow preview line.
pub(crate) fn draw_connector_preview(scene: &mut Scene, state: &AppState, camera_tf: Affine) {
    if let Some((x1, y1, x2, y2)) = state.connector_preview {
        let preview_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xcc);
        let dash_stroke = Stroke::new(2.0).with_dashes(0.0, [6.0, 4.0]);
        let line = Line::new((x1, y1), (x2, y2));
        scene.stroke(&dash_stroke, camera_tf, preview_color, None, &line);

        // Draw anchor circle at start point
        let start_circle = Circle::new((x1, y1), 6.0);
        let anchor_fill = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x4d);
        let anchor_stroke = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xff);
        scene.fill(Fill::NonZero, camera_tf, anchor_fill, None, &start_circle);
        scene.stroke(
            &Stroke::new(1.5),
            camera_tf,
            anchor_stroke,
            None,
            &start_circle,
        );
    }

    // Highlight the source shape with a purple glow ring when connector_from is set
    if let Some(from_id) = state.connector_from {
        if let Some(shape) = state.shape_by_id(from_id) {
            let gap = 4.0;
            let rect = Rect::new(
                shape.x - gap,
                shape.y - gap,
                shape.x + shape.w + gap,
                shape.y + shape.h + gap,
            );
            let rr = RoundedRect::from_rect(rect, 6.0);
            let glow_stroke = Stroke::new(2.0);
            let glow_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x88);
            scene.stroke(&glow_stroke, camera_tf, glow_color, None, &rr);
        }
    }
}

/// Draw a dot at the binding point when hovering in Arrow/Line tool (fallback).
pub(crate) fn draw_binding_indicator(scene: &mut Scene, wx: f64, wy: f64, camera_tf: Affine) {
    let radius = 5.0;
    let circle = Circle::new((wx, wy), radius);
    let fill_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xaa);
    let stroke_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xff);
    scene.fill(Fill::NonZero, camera_tf, fill_color, None, &circle);
    scene.stroke(&Stroke::new(1.5), camera_tf, stroke_color, None, &circle);
}

/// Draw a small tooltip showing the current rotation angle in degrees.
pub(crate) fn draw_rotation_tooltip(
    scene: &mut Scene,
    state: &AppState,
    shape: &crate::state::Shape,
) {
    let degrees = shape.rotation.to_degrees();
    let deg_norm = ((degrees % 360.0) + 360.0) % 360.0;
    let label = format!("{deg_norm:.0}\u{00B0}");

    let font_size = 11.0_f32;
    let pad_x = 6.0;
    let pad_y = 3.0;
    let text_w = crate::text::measure_text(&state.fonts, &label, font_size);
    let tip_w = text_w + pad_x * 2.0;
    let tip_h = font_size as f64 + pad_y * 2.0;

    let tip_x = state.cursor_x + 16.0;
    let tip_y = state.cursor_y - tip_h - 8.0;

    let bg = RoundedRect::from_rect(Rect::new(tip_x, tip_y, tip_x + tip_w, tip_y + tip_h), 4.0);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        Color::from_rgba8(0x1a, 0x1a, 0x1a, 0xee),
        None,
        &bg,
    );
    crate::text::draw_text(
        scene,
        &state.fonts,
        &label,
        tip_x + pad_x,
        tip_y + pad_y,
        font_size,
        Color::from_rgba8(0xff, 0xff, 0xff, 0xee),
        Affine::IDENTITY,
    );
}
