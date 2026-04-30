//! Shape-aware hachure fill rendering.
//!
//! Hachure segments are clipped before rendering. This avoids the common
//! sketch-fill bug where ellipses and polygons show a rectangular hatch block
//! from their bounding box.

use vello::Scene;
use vello::kurbo::{Affine, Line, Stroke};
use vello::peniko::Color;

const EPS: f64 = 1e-6;
const MIN_SEGMENT_LEN: f64 = 2.0;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RectHachure {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EllipseHachure {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
}

#[derive(Debug, Clone, Copy)]
struct HachureSegment {
    start: (f64, f64),
    end: (f64, f64),
}

pub(crate) fn draw_rect_hachure(
    scene: &mut Scene,
    tf: Affine,
    rect: RectHachure,
    color: Color,
    stroke_width: f64,
) {
    let stroke = hachure_stroke(stroke_width);
    for segment in rect_hachure_segments(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        hachure_spacing(stroke_width),
    ) {
        scene.stroke(
            &stroke,
            tf,
            color,
            None,
            &Line::new(segment.start, segment.end),
        );
    }
}

pub(crate) fn draw_ellipse_hachure(
    scene: &mut Scene,
    tf: Affine,
    ellipse: EllipseHachure,
    color: Color,
    stroke_width: f64,
) {
    let stroke = hachure_stroke(stroke_width);
    for segment in ellipse_hachure_segments(
        ellipse.cx,
        ellipse.cy,
        ellipse.rx,
        ellipse.ry,
        hachure_spacing(stroke_width),
    ) {
        scene.stroke(
            &stroke,
            tf,
            color,
            None,
            &Line::new(segment.start, segment.end),
        );
    }
}

pub(crate) fn draw_polygon_hachure(
    scene: &mut Scene,
    tf: Affine,
    points: &[(f64, f64)],
    color: Color,
    stroke_width: f64,
) {
    let stroke = hachure_stroke(stroke_width);
    for segment in polygon_hachure_segments(points, hachure_spacing(stroke_width)) {
        scene.stroke(
            &stroke,
            tf,
            color,
            None,
            &Line::new(segment.start, segment.end),
        );
    }
}

fn hachure_spacing(stroke_width: f64) -> f64 {
    (stroke_width * 4.0).clamp(6.0, 14.0)
}

fn hachure_stroke(stroke_width: f64) -> Stroke {
    Stroke::new((stroke_width * 0.5).clamp(0.9, 2.0))
}

fn rect_hachure_segments(x: f64, y: f64, w: f64, h: f64, spacing: f64) -> Vec<HachureSegment> {
    let x0 = x.min(x + w);
    let x1 = x.max(x + w);
    let y0 = y.min(y + h);
    let y1 = y.max(y + h);
    let points = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    polygon_hachure_segments(&points, spacing)
}

fn ellipse_hachure_segments(
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    spacing: f64,
) -> Vec<HachureSegment> {
    let rx = rx.abs();
    let ry = ry.abs();
    if rx <= EPS || ry <= EPS || spacing <= EPS {
        return Vec::new();
    }

    let min_sum = (cx - rx) + (cy - ry);
    let max_sum = (cx + rx) + (cy + ry);
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let a = 1.0 / rx2 + 1.0 / ry2;

    hatch_sums(min_sum, max_sum, spacing)
        .into_iter()
        .filter_map(|sum| {
            let k = sum - cy;
            let b = -2.0 * cx / rx2 - 2.0 * k / ry2;
            let c = cx * cx / rx2 + k * k / ry2 - 1.0;
            let disc = b * b - 4.0 * a * c;
            if disc < EPS {
                return None;
            }
            let root = disc.sqrt();
            let x_a = (-b - root) / (2.0 * a);
            let x_b = (-b + root) / (2.0 * a);
            let start = (x_a, sum - x_a);
            let end = (x_b, sum - x_b);
            segment_if_visible(start, end)
        })
        .collect()
}

fn polygon_hachure_segments(points: &[(f64, f64)], spacing: f64) -> Vec<HachureSegment> {
    if points.len() < 3 || spacing <= EPS {
        return Vec::new();
    }
    let (min_sum, max_sum) = points.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(min_sum, max_sum), point| {
            let sum = point.0 + point.1;
            (min_sum.min(sum), max_sum.max(sum))
        },
    );

    let mut segments = Vec::new();
    for sum in hatch_sums(min_sum, max_sum, spacing) {
        let mut intersections = line_polygon_intersections(points, sum);
        intersections.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        dedup_points(&mut intersections);

        for pair in intersections.chunks(2) {
            if pair.len() == 2 {
                if let Some(segment) = segment_if_visible(pair[0], pair[1]) {
                    segments.push(segment);
                }
            }
        }
    }
    segments
}

fn hatch_sums(min_sum: f64, max_sum: f64, spacing: f64) -> Vec<f64> {
    let mut sums = Vec::new();
    let mut sum = (min_sum / spacing).floor() * spacing + spacing;
    if sum <= min_sum + EPS {
        sum += spacing;
    }
    while sum < max_sum - EPS {
        sums.push(sum);
        sum += spacing;
    }
    sums
}

fn line_polygon_intersections(points: &[(f64, f64)], sum: f64) -> Vec<(f64, f64)> {
    let mut intersections = Vec::new();
    for idx in 0..points.len() {
        let start = points[idx];
        let end = points[(idx + 1) % points.len()];
        let start_sum = start.0 + start.1;
        let end_sum = end.0 + end.1;
        let start_side = start_sum - sum;
        let end_side = end_sum - sum;

        if start_side.abs() <= EPS && end_side.abs() <= EPS {
            intersections.push(start);
            intersections.push(end);
            continue;
        }

        if (start_side > EPS && end_side > EPS) || (start_side < -EPS && end_side < -EPS) {
            continue;
        }

        let denom = end_sum - start_sum;
        if denom.abs() <= EPS {
            continue;
        }
        let t = (sum - start_sum) / denom;
        if (-EPS..=1.0 + EPS).contains(&t) {
            intersections.push((
                start.0 + (end.0 - start.0) * t,
                start.1 + (end.1 - start.1) * t,
            ));
        }
    }
    intersections
}

fn dedup_points(points: &mut Vec<(f64, f64)>) {
    points.dedup_by(|a, b| distance_sq(*a, *b) <= EPS);
}

fn segment_if_visible(start: (f64, f64), end: (f64, f64)) -> Option<HachureSegment> {
    if distance_sq(start, end).sqrt() <= MIN_SEGMENT_LEN {
        return None;
    }
    Some(HachureSegment { start, end })
}

fn distance_sq(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hachure_ellipse_segments_stay_inside_ellipse() {
        let segments = ellipse_hachure_segments(120.0, 80.0, 60.0, 35.0, 8.0);
        assert!(segments.len() > 4);
        for segment in segments {
            assert!(point_on_or_inside_ellipse(
                segment.start,
                120.0,
                80.0,
                60.0,
                35.0
            ));
            assert!(point_on_or_inside_ellipse(
                segment.end,
                120.0,
                80.0,
                60.0,
                35.0
            ));
            let mid = (
                (segment.start.0 + segment.end.0) / 2.0,
                (segment.start.1 + segment.end.1) / 2.0,
            );
            assert!(point_on_or_inside_ellipse(mid, 120.0, 80.0, 60.0, 35.0));
        }
    }

    #[test]
    fn hachure_polygon_segments_stay_inside_diamond() {
        let points = [(50.0, 0.0), (100.0, 40.0), (50.0, 80.0), (0.0, 40.0)];
        let segments = polygon_hachure_segments(&points, 8.0);
        assert!(segments.len() > 4);
        for segment in segments {
            assert!(point_in_convex_polygon(segment.start, &points));
            assert!(point_in_convex_polygon(segment.end, &points));
        }
    }

    #[test]
    fn hachure_polygon_segments_stay_inside_triangle() {
        let points = [(60.0, 0.0), (0.0, 90.0), (120.0, 90.0)];
        let segments = polygon_hachure_segments(&points, 8.0);
        assert!(segments.len() > 4);
        for segment in segments {
            assert!(point_in_convex_polygon(segment.start, &points));
            assert!(point_in_convex_polygon(segment.end, &points));
        }
    }

    fn point_on_or_inside_ellipse(point: (f64, f64), cx: f64, cy: f64, rx: f64, ry: f64) -> bool {
        let dx = (point.0 - cx) / rx;
        let dy = (point.1 - cy) / ry;
        dx * dx + dy * dy <= 1.0 + 1e-4
    }

    fn point_in_convex_polygon(point: (f64, f64), points: &[(f64, f64)]) -> bool {
        let mut sign = 0.0;
        for idx in 0..points.len() {
            let a = points[idx];
            let b = points[(idx + 1) % points.len()];
            let cross = (b.0 - a.0) * (point.1 - a.1) - (b.1 - a.1) * (point.0 - a.0);
            if cross.abs() <= 1e-4 {
                continue;
            }
            if sign == 0.0 {
                sign = cross.signum();
            } else if sign * cross < -1e-4 {
                return false;
            }
        }
        true
    }
}
