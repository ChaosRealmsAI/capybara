//! Shared geometry for line and arrow shapes across hit testing, rendering,
//! and direct manipulation.

use crate::shape::point_to_segment_dist;
use crate::state::{AppState, ArrowStyle, LineHandle, Shape, ShapeKind};

pub(crate) const LINE_HIT_THRESHOLD: f64 = 8.0;
pub(crate) const LINE_BIND_MARGIN: f64 = 30.0;
pub(crate) const LINE_HANDLE_RADIUS: f64 = 6.0;

pub(crate) fn endpoints(state: &AppState, shape: &Shape) -> ((f64, f64), (f64, f64)) {
    let raw_start = (shape.x, shape.y);
    let raw_end = (shape.x + shape.w, shape.y + shape.h);

    let start = if let Some(bound_id) = shape.binding_start {
        state
            .shape_by_id(bound_id)
            .map_or(raw_start, |bound| bound.edge_point(raw_end.0, raw_end.1))
    } else {
        raw_start
    };
    let end = if let Some(bound_id) = shape.binding_end {
        state
            .shape_by_id(bound_id)
            .map_or(raw_end, |bound| bound.edge_point(raw_start.0, raw_start.1))
    } else {
        raw_end
    };
    (start, end)
}

pub(crate) fn midpoint(state: &AppState, shape: &Shape) -> (f64, f64) {
    let (start, end) = endpoints(state, shape);
    if let Some(control) = control_point(shape, start, end) {
        quadratic_point(start, control, end, 0.5)
    } else {
        ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0)
    }
}

pub(crate) fn mid_handle_position(state: &AppState, shape: &Shape) -> (f64, f64) {
    let (start, end) = endpoints(state, shape);
    control_point(shape, start, end).unwrap_or_else(|| midpoint(state, shape))
}

pub(crate) fn control_point(
    shape: &Shape,
    start: (f64, f64),
    end: (f64, f64),
) -> Option<(f64, f64)> {
    if shape.kind != ShapeKind::Arrow || shape.arrow_style != ArrowStyle::Curved {
        return None;
    }
    shape
        .points
        .first()
        .copied()
        .or_else(|| Some(default_curve_control(start, end)))
}

pub(crate) fn hit_test(state: &AppState, shape: &Shape, wx: f64, wy: f64, threshold: f64) -> bool {
    let (start, end) = endpoints(state, shape);
    if let Some(control) = control_point(shape, start, end) {
        let mut prev = start;
        for step in 1..=24 {
            let point = quadratic_point(start, control, end, step as f64 / 24.0);
            if point_to_segment_dist(wx, wy, prev.0, prev.1, point.0, point.1) <= threshold {
                return true;
            }
            prev = point;
        }
        false
    } else {
        point_to_segment_dist(wx, wy, start.0, start.1, end.0, end.1) <= threshold
    }
}

pub(crate) fn hit_handle(
    state: &AppState,
    shape: &Shape,
    wx: f64,
    wy: f64,
    zoom: f64,
) -> Option<LineHandle> {
    let (start, end) = endpoints(state, shape);
    let radius = (LINE_HANDLE_RADIUS + 2.0) / zoom;
    if distance(wx, wy, start.0, start.1) <= radius {
        return Some(LineHandle::Start);
    }
    if distance(wx, wy, end.0, end.1) <= radius {
        return Some(LineHandle::End);
    }
    if shape.kind == ShapeKind::Arrow && distance_to_mid_handle(state, shape, wx, wy) <= radius {
        return Some(LineHandle::Mid);
    }
    None
}

pub(crate) fn nearest_binding(
    state: &AppState,
    dragged_shape_id: u64,
    wx: f64,
    wy: f64,
) -> Option<(u64, (f64, f64))> {
    state.shapes.iter().find_map(|shape| {
        if shape.id == dragged_shape_id {
            return None;
        }
        let near_x = wx >= shape.x - LINE_BIND_MARGIN && wx <= shape.x + shape.w + LINE_BIND_MARGIN;
        let near_y = wy >= shape.y - LINE_BIND_MARGIN && wy <= shape.y + shape.h + LINE_BIND_MARGIN;
        if !(near_x && near_y && (shape.w > 1.0 || shape.h > 1.0)) {
            return None;
        }
        let anchor = shape.anchor_points().into_iter().min_by(|a, b| {
            let da = distance_sq(wx, wy, a.0, a.1);
            let db = distance_sq(wx, wy, b.0, b.1);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })?;
        Some((shape.id, anchor))
    })
}

pub(crate) fn set_endpoint(
    shape: &mut Shape,
    handle: LineHandle,
    fixed: (f64, f64),
    point: (f64, f64),
    binding: Option<u64>,
) {
    match handle {
        LineHandle::Start => {
            shape.x = point.0;
            shape.y = point.1;
            shape.w = fixed.0 - point.0;
            shape.h = fixed.1 - point.1;
            shape.binding_start = binding;
        }
        LineHandle::End => {
            shape.x = fixed.0;
            shape.y = fixed.1;
            shape.w = point.0 - fixed.0;
            shape.h = point.1 - fixed.1;
            shape.binding_end = binding;
        }
        LineHandle::Mid => {
            shape.arrow_style = ArrowStyle::Curved;
            if shape.points.is_empty() {
                shape.points.push(point);
            } else {
                shape.points[0] = point;
            }
        }
    }
}

pub(crate) fn distance_to_mid_handle(state: &AppState, shape: &Shape, wx: f64, wy: f64) -> f64 {
    let handle = mid_handle_position(state, shape);
    distance(wx, wy, handle.0, handle.1)
}

fn default_curve_control(start: (f64, f64), end: (f64, f64)) -> (f64, f64) {
    let mx = (start.0 + end.0) / 2.0;
    let my = (start.1 + end.1) / 2.0;
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length = (dx * dx + dy * dy).sqrt();
    let normal = (-dy, dx);
    let normal_len = (normal.0 * normal.0 + normal.1 * normal.1).sqrt();
    if normal_len <= 1e-6 {
        return (mx, my - 20.0);
    }
    let offset = (length * 0.2).max(20.0);
    (
        mx + normal.0 / normal_len * offset,
        my + normal.1 / normal_len * offset,
    )
}

fn quadratic_point(start: (f64, f64), control: (f64, f64), end: (f64, f64), t: f64) -> (f64, f64) {
    let mt = 1.0 - t;
    (
        mt * mt * start.0 + 2.0 * mt * t * control.0 + t * t * end.0,
        mt * mt * start.1 + 2.0 * mt * t * control.1 + t * t * end.1,
    )
}

fn distance(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    distance_sq(ax, ay, bx, by).sqrt()
}

fn distance_sq(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    (ax - bx).powi(2) + (ay - by).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Shape, ShapeKind};

    #[test]
    fn hit_test_uses_bound_endpoints() {
        let mut state = AppState::new();

        let mut a = Shape::new(ShapeKind::Rect, 0.0, 0.0, 0);
        a.w = 100.0;
        a.h = 80.0;
        let a_idx = state.add_shape(a);

        let mut b = Shape::new(ShapeKind::Rect, 320.0, 0.0, 0);
        b.w = 120.0;
        b.h = 90.0;
        let b_idx = state.add_shape(b);

        let mut arrow = Shape::new(ShapeKind::Arrow, 50.0, 40.0, 0);
        arrow.w = 330.0;
        arrow.h = 10.0;
        arrow.binding_start = Some(state.shapes[a_idx].id);
        arrow.binding_end = Some(state.shapes[b_idx].id);
        let arrow_idx = state.add_shape(arrow);

        state.shapes[b_idx].x = 500.0;
        state.shapes[b_idx].y = 120.0;

        let (start, end) = endpoints(&state, &state.shapes[arrow_idx]);
        let probe = ((start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
        assert!(hit_test(
            &state,
            &state.shapes[arrow_idx],
            probe.0,
            probe.1,
            LINE_HIT_THRESHOLD
        ));
    }
}
