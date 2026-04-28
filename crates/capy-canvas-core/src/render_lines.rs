//! Shared line/connector rendering helpers used by the shape renderer.

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Line, Stroke};
use vello::peniko::{Color, Fill};

use crate::line_geometry;
use crate::render::color_from_hex;
use crate::state::{AppState, ArrowHead, Connector, ConnectorStyle, Shape, StrokeStyle};

pub(crate) fn draw_line(
    scene: &mut Scene,
    state: &AppState,
    shape: &Shape,
    shape_tf: Affine,
    stroke_color: Color,
    stroke: &Stroke,
) {
    let (start, end) = line_geometry::endpoints(state, shape);
    let line = Line::new(start, end);
    scene.stroke(stroke, shape_tf, stroke_color, None, &line);
}

pub(crate) fn draw_arrow(
    scene: &mut Scene,
    state: &AppState,
    shape: &Shape,
    index: usize,
    shape_tf: Affine,
    stroke_color: Color,
    stroke: &Stroke,
) {
    let (start, end) = line_geometry::endpoints(state, shape);

    match line_geometry::control_point(shape, start, end) {
        Some(control) => {
            let mut path = BezPath::new();
            path.move_to(start);
            path.quad_to(control, end);
            scene.stroke(stroke, shape_tf, stroke_color, None, &path);
        }
        None => draw_line(scene, state, shape, shape_tf, stroke_color, stroke),
    }
    draw_arrowhead_typed(
        scene,
        shape_tf,
        start,
        end,
        stroke_color,
        shape.arrow_start,
        true,
    );
    draw_arrowhead_typed(
        scene,
        shape_tf,
        start,
        end,
        stroke_color,
        shape.arrow_end,
        false,
    );
    crate::render_line_ui::draw_arrow_label(scene, state, shape, index, shape_tf, stroke_color);
}

pub(crate) fn draw_freehand(
    scene: &mut Scene,
    shape: &Shape,
    shape_tf: Affine,
    stroke_color: Color,
    stroke: &Stroke,
) {
    if shape.points.len() >= 2 {
        let mut path = BezPath::new();
        path.move_to(shape.points[0]);
        for pi in 1..shape.points.len() {
            if pi + 1 < shape.points.len() {
                let mid_x = (shape.points[pi].0 + shape.points[pi + 1].0) / 2.0;
                let mid_y = (shape.points[pi].1 + shape.points[pi + 1].1) / 2.0;
                path.quad_to(shape.points[pi], (mid_x, mid_y));
            } else {
                path.line_to(shape.points[pi]);
            }
        }
        scene.stroke(stroke, shape_tf, stroke_color, None, &path);
    }
}

pub(crate) fn draw_highlighter(scene: &mut Scene, shape: &Shape, shape_tf: Affine, opacity: f32) {
    if shape.points.len() >= 2 {
        let mut path = BezPath::new();
        path.move_to(shape.points[0]);
        for pi in 1..shape.points.len() {
            if pi + 1 < shape.points.len() {
                let mid_x = (shape.points[pi].0 + shape.points[pi + 1].0) / 2.0;
                let mid_y = (shape.points[pi].1 + shape.points[pi + 1].1) / 2.0;
                path.quad_to(shape.points[pi], (mid_x, mid_y));
            } else {
                path.line_to(shape.points[pi]);
            }
        }
        let highlight_color = color_from_hex(shape.color, 0.4 * opacity);
        let highlight_stroke = Stroke::new(20.0);
        scene.stroke(&highlight_stroke, shape_tf, highlight_color, None, &path);
    }
}

pub(crate) fn build_stroke(width: f64, style: StrokeStyle) -> Stroke {
    match style {
        StrokeStyle::Solid => Stroke::new(width),
        StrokeStyle::Dashed => Stroke::new(width).with_dashes(0.0, [8.0, 5.0]),
        StrokeStyle::Dotted => Stroke::new(width).with_dashes(0.0, [2.0, 3.0]),
    }
}

pub(crate) fn build_shape_transform(shape: &Shape, camera_tf: Affine) -> Affine {
    let (cx, cy) = shape.center();
    let has_rotation = shape.rotation.abs() > 1e-6;
    let has_flip = shape.flipped_h || shape.flipped_v;

    if !has_rotation && !has_flip {
        return camera_tf;
    }

    let mut local_tf = Affine::IDENTITY;
    if has_rotation {
        local_tf = Affine::rotate_about(shape.rotation, (cx, cy));
    }
    if has_flip {
        let sx = if shape.flipped_h { -1.0 } else { 1.0 };
        let sy = if shape.flipped_v { -1.0 } else { 1.0 };
        let flip_tf = Affine::translate((cx, cy))
            * Affine::scale_non_uniform(sx, sy)
            * Affine::translate((-cx, -cy));
        local_tf *= flip_tf;
    }

    camera_tf * local_tf
}

pub(crate) fn draw_hachure(
    scene: &mut Scene,
    tf: Affine,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: Color,
) {
    let spacing = 6.0;
    let stroke = Stroke::new(1.0);
    let max_dist = w + h;
    let mut d = spacing;
    while d < max_dist {
        let (mut x1, mut y1, mut x2, mut y2);
        if d <= h {
            x1 = x;
            y1 = y + d;
            x2 = x + d.min(w);
            y2 = y;
        } else if d <= w {
            x1 = x + d - h;
            y1 = y + h;
            x2 = x + d;
            y2 = y;
        } else {
            x1 = x + d - h;
            y1 = y + h;
            x2 = x + w;
            y2 = y + d - w;
        }
        x1 = x1.clamp(x, x + w);
        y1 = y1.clamp(y, y + h);
        x2 = x2.clamp(x, x + w);
        y2 = y2.clamp(y, y + h);
        let line = Line::new((x1, y1), (x2, y2));
        scene.stroke(&stroke, tf, color, None, &line);
        d += spacing;
    }
}

pub(crate) fn draw_arrowhead_typed(
    scene: &mut Scene,
    tf: Affine,
    start: (f64, f64),
    end: (f64, f64),
    color: Color,
    head: ArrowHead,
    is_start: bool,
) {
    if head == ArrowHead::None {
        return;
    }
    let (tip_x, tip_y, base_x, base_y) = if is_start {
        (start.0, start.1, end.0, end.1)
    } else {
        (end.0, end.1, start.0, start.1)
    };
    let dx = tip_x - base_x;
    let dy = tip_y - base_y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return;
    }
    let nx = dx / len;
    let ny = dy / len;
    let arrow_len = 10.0;
    let arrow_w = 5.0;
    let ax = tip_x - nx * arrow_len;
    let ay = tip_y - ny * arrow_len;

    match head {
        ArrowHead::None => {}
        ArrowHead::Triangle => {
            let mut path = BezPath::new();
            path.move_to((tip_x, tip_y));
            path.line_to((ax - ny * arrow_w, ay + nx * arrow_w));
            path.line_to((ax + ny * arrow_w, ay - nx * arrow_w));
            path.close_path();
            scene.fill(Fill::NonZero, tf, color, None, &path);
        }
        ArrowHead::Circle => {
            let r = arrow_w;
            let cx = tip_x - nx * r;
            let cy = tip_y - ny * r;
            let circle = Circle::new((cx, cy), r);
            scene.fill(Fill::NonZero, tf, color, None, &circle);
        }
        ArrowHead::Diamond => {
            let half = arrow_len * 0.6;
            let mid_x = tip_x - nx * half;
            let mid_y = tip_y - ny * half;
            let back_x = tip_x - nx * half * 2.0;
            let back_y = tip_y - ny * half * 2.0;
            let mut path = BezPath::new();
            path.move_to((tip_x, tip_y));
            path.line_to((mid_x - ny * arrow_w, mid_y + nx * arrow_w));
            path.line_to((back_x, back_y));
            path.line_to((mid_x + ny * arrow_w, mid_y - nx * arrow_w));
            path.close_path();
            scene.fill(Fill::NonZero, tf, color, None, &path);
        }
        ArrowHead::Bar => {
            let bar_half = arrow_w * 1.2;
            let bar = Line::new(
                (tip_x - ny * bar_half, tip_y + nx * bar_half),
                (tip_x + ny * bar_half, tip_y - nx * bar_half),
            );
            scene.stroke(&Stroke::new(2.0), tf, color, None, &bar);
        }
    }
}

/// Check if a world-space point is within `threshold` px of a connector line.
pub(crate) fn connector_hit_test(
    state: &AppState,
    conn: &Connector,
    wx: f64,
    wy: f64,
    threshold: f64,
) -> bool {
    let from = state.shape_by_id(conn.from_id);
    let to = state.shape_by_id(conn.to_id);
    if let (Some(a), Some(b)) = (from, to) {
        let (bcx, bcy) = b.center();
        let (acx, acy) = a.center();
        let p1 = a.edge_point(bcx, bcy);
        let p2 = b.edge_point(acx, acy);
        match conn.style {
            ConnectorStyle::Straight => {
                crate::shape::point_to_segment_dist(wx, wy, p1.0, p1.1, p2.0, p2.1) <= threshold
            }
            ConnectorStyle::Elbow => {
                let bend = crate::connector::elbow_route(p1, p2);
                let d1 = crate::shape::point_to_segment_dist(wx, wy, p1.0, p1.1, bend.0, bend.1);
                let d2 = crate::shape::point_to_segment_dist(wx, wy, bend.0, bend.1, p2.0, p2.1);
                d1.min(d2) <= threshold
            }
        }
    } else {
        false
    }
}

pub(crate) fn draw_connector_indexed(
    scene: &mut Scene,
    state: &AppState,
    conn: &Connector,
    connector_index: usize,
    camera_tf: Affine,
) {
    draw_connector_inner(scene, state, conn, Some(connector_index), camera_tf);
}

#[allow(dead_code)]
pub(crate) fn draw_connector(
    scene: &mut Scene,
    state: &AppState,
    conn: &Connector,
    camera_tf: Affine,
) {
    draw_connector_inner(scene, state, conn, None, camera_tf);
}

fn draw_connector_inner(
    scene: &mut Scene,
    state: &AppState,
    conn: &Connector,
    connector_index: Option<usize>,
    camera_tf: Affine,
) {
    let from = state.shape_by_id(conn.from_id);
    let to = state.shape_by_id(conn.to_id);
    if let (Some(a), Some(b)) = (from, to) {
        let (bcx, bcy) = b.center();
        let (acx, acy) = a.center();
        let p1 = a.edge_point(bcx, bcy);
        let p2 = b.edge_point(acx, acy);

        // Check hover and selection state
        let (mouse_wx, mouse_wy) = state.camera.screen_to_world(state.cursor_x, state.cursor_y);
        let is_hovered = connector_hit_test(state, conn, mouse_wx, mouse_wy, 8.0);
        let is_selected = connector_index.is_some() && state.selected_connector == connector_index;

        let (color, conn_stroke) = if is_selected || is_hovered {
            (Color::from_rgba8(0x8a, 0x6f, 0xae, 0xff), Stroke::new(3.0))
        } else {
            (color_from_hex(conn.color, 1.0), Stroke::new(2.0))
        };

        let mid = match conn.style {
            ConnectorStyle::Straight => {
                let line = Line::new(p1, p2);
                scene.stroke(&conn_stroke, camera_tf, color, None, &line);
                ((p1.0 + p2.0) / 2.0, (p1.1 + p2.1) / 2.0)
            }
            ConnectorStyle::Elbow => {
                let bend = crate::connector::elbow_route(p1, p2);
                let mut path = BezPath::new();
                path.move_to(p1);
                path.line_to(bend);
                path.line_to(p2);
                scene.stroke(&conn_stroke, camera_tf, color, None, &path);
                bend
            }
        };
        draw_arrowhead_typed(scene, camera_tf, p1, p2, color, ArrowHead::Triangle, false);

        // Draw anchor circles at both ends when hovered or selected
        if is_hovered || is_selected {
            let anchor_radius = 6.0;
            let anchor_fill = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x4d);
            let anchor_stroke = Color::from_rgba8(0x8a, 0x6f, 0xae, 0xff);
            let c1 = Circle::new(p1, anchor_radius);
            let c2 = Circle::new(p2, anchor_radius);
            scene.fill(Fill::NonZero, camera_tf, anchor_fill, None, &c1);
            scene.stroke(&Stroke::new(1.5), camera_tf, anchor_stroke, None, &c1);
            scene.fill(Fill::NonZero, camera_tf, anchor_fill, None, &c2);
            scene.stroke(&Stroke::new(1.5), camera_tf, anchor_stroke, None, &c2);
        }

        if let Some(ref lbl) = conn.label {
            if !lbl.is_empty() {
                crate::text::draw_text(
                    scene,
                    &state.fonts,
                    lbl,
                    mid.0,
                    mid.1 - 10.0,
                    12.0,
                    color,
                    camera_tf,
                );
            }
        }
    }
}

pub(crate) fn brighten_color(color: Color, amount: f32) -> Color {
    let rgba = color.to_rgba8();
    let brighten = |c: u8| -> u8 {
        let v = c as f32 + (255.0 - c as f32) * amount;
        (v.min(255.0)) as u8
    };
    Color::from_rgba8(brighten(rgba.r), brighten(rgba.g), brighten(rgba.b), rgba.a)
}
