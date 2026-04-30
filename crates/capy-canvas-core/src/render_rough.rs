//! Deterministic sketch-style outlines for vector canvas shapes.
//!
//! This keeps the hand-drawn feel local to rendering: geometry, hit testing,
//! export data, and AI snapshots stay precise and stable.

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Stroke};
use vello::peniko::Color;

const ROUGH_PASSES: usize = 2;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RoughRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RoughEllipse {
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
}

pub(crate) fn draw_rough_line(
    scene: &mut Scene,
    tf: Affine,
    start: (f64, f64),
    end: (f64, f64),
    color: Color,
    stroke: &Stroke,
    seed: u64,
) {
    for pass in 0..ROUGH_PASSES {
        let mut path = BezPath::new();
        path.move_to(jitter_point(seed, pass as u64, start, 1.15));
        let mid = ((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
        path.quad_to(
            jitter_point(seed, pass as u64 + 11, mid, 1.9),
            jitter_point(seed, pass as u64 + 23, end, 1.15),
        );
        scene.stroke(stroke, tf, color, None, &path);
    }
}

pub(crate) fn draw_rough_rect(
    scene: &mut Scene,
    tf: Affine,
    rect: RoughRect,
    color: Color,
    stroke: &Stroke,
    seed: u64,
) {
    let points = [
        (rect.x, rect.y),
        (rect.x + rect.w, rect.y),
        (rect.x + rect.w, rect.y + rect.h),
        (rect.x, rect.y + rect.h),
    ];
    draw_rough_polygon(scene, tf, &points, color, stroke, seed, true);
}

pub(crate) fn draw_rough_polygon(
    scene: &mut Scene,
    tf: Affine,
    points: &[(f64, f64)],
    color: Color,
    stroke: &Stroke,
    seed: u64,
    closed: bool,
) {
    if points.len() < 2 {
        return;
    }
    for pass in 0..ROUGH_PASSES {
        let mut path = BezPath::new();
        path.move_to(jitter_point(seed, pass as u64, points[0], 1.2));
        for (idx, pair) in points.windows(2).enumerate() {
            rough_segment(&mut path, seed, pass as u64, idx as u64, pair[0], pair[1]);
        }
        if closed {
            rough_segment(
                &mut path,
                seed,
                pass as u64,
                points.len() as u64,
                *points.last().unwrap_or(&points[0]),
                points[0],
            );
            path.close_path();
        }
        scene.stroke(stroke, tf, color, None, &path);
    }
}

pub(crate) fn draw_rough_ellipse(
    scene: &mut Scene,
    tf: Affine,
    ellipse: RoughEllipse,
    color: Color,
    stroke: &Stroke,
    seed: u64,
) {
    let RoughEllipse { cx, cy, rx, ry } = ellipse;
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let k = 0.552_284_749_830_793_6;
    for pass in 0..ROUGH_PASSES {
        let salt = pass as u64 * 97;
        let mut path = BezPath::new();
        path.move_to(jitter_point(seed, salt, (cx + rx, cy), 1.25));
        path.curve_to(
            jitter_point(seed, salt + 1, (cx + rx, cy + ry * k), 1.6),
            jitter_point(seed, salt + 2, (cx + rx * k, cy + ry), 1.6),
            jitter_point(seed, salt + 3, (cx, cy + ry), 1.25),
        );
        path.curve_to(
            jitter_point(seed, salt + 4, (cx - rx * k, cy + ry), 1.6),
            jitter_point(seed, salt + 5, (cx - rx, cy + ry * k), 1.6),
            jitter_point(seed, salt + 6, (cx - rx, cy), 1.25),
        );
        path.curve_to(
            jitter_point(seed, salt + 7, (cx - rx, cy - ry * k), 1.6),
            jitter_point(seed, salt + 8, (cx - rx * k, cy - ry), 1.6),
            jitter_point(seed, salt + 9, (cx, cy - ry), 1.25),
        );
        path.curve_to(
            jitter_point(seed, salt + 10, (cx + rx * k, cy - ry), 1.6),
            jitter_point(seed, salt + 11, (cx + rx, cy - ry * k), 1.6),
            jitter_point(seed, salt + 12, (cx + rx, cy), 1.25),
        );
        path.close_path();
        scene.stroke(stroke, tf, color, None, &path);
    }
}

fn rough_segment(
    path: &mut BezPath,
    seed: u64,
    pass: u64,
    idx: u64,
    start: (f64, f64),
    end: (f64, f64),
) {
    let mid = ((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
    let control = jitter_point(seed, pass * 101 + idx * 13 + 5, mid, 1.8);
    let end = jitter_point(seed, pass * 101 + idx * 13 + 9, end, 1.2);
    path.quad_to(control, end);
}

fn jitter_point(seed: u64, salt: u64, point: (f64, f64), amount: f64) -> (f64, f64) {
    (
        point.0 + jitter(seed, salt, amount),
        point.1 + jitter(seed, salt + 41, amount),
    )
}

fn jitter(seed: u64, salt: u64, amount: f64) -> f64 {
    let v = hash(seed ^ salt.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let unit = (v as f64 / u64::MAX as f64) * 2.0 - 1.0;
    unit * amount
}

fn hash(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}
