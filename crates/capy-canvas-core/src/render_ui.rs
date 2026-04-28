//! Render-time UI overlays in canvas space: grid, guides, and selection handles.

use vello::Scene;
use vello::kurbo::{Affine, Circle, Line, Rect, Stroke};
use vello::peniko::{Color, Fill};

use crate::render::{GRID_STEP, HANDLE_SIZE, SELECTION_COLOR};
use crate::state::{AlignGuide, Camera, Shape};

const GRID_DOT: Color = Color::from_rgba8(0xdd, 0xdd, 0xdd, 0xff);
const GRID_DOT_MAJOR: Color = Color::from_rgba8(0xcc, 0xcc, 0xcc, 0xff);
const GRID_DOT_MINOR: Color = Color::from_rgba8(0xe8, 0xe8, 0xe8, 0xff);
const GRID_DOT_DARK: Color = Color::from_rgba8(0x33, 0x33, 0x33, 0xff);
const GRID_DOT_MAJOR_DARK: Color = Color::from_rgba8(0x44, 0x44, 0x44, 0xff);
const GRID_DOT_MINOR_DARK: Color = Color::from_rgba8(0x2a, 0x2a, 0x2a, 0xff);
const GUIDE_COLOR: Color = Color::from_rgba8(0x00, 0xcc, 0xcc, 0xaa);
const DOT_RADIUS: f64 = 0.8;
const DOT_RADIUS_MAJOR: f64 = 1.2;
const DOT_RADIUS_MINOR: f64 = 0.5;

pub(crate) fn draw_grid(scene: &mut Scene, cam: &Camera, vw: f64, vh: f64, dark: bool) {
    let camera_tf = Affine::translate((cam.offset_x, cam.offset_y)) * Affine::scale(cam.zoom);
    let zoom = cam.zoom;
    let inv_zoom = 1.0 / zoom;

    let dot_color = if dark { GRID_DOT_DARK } else { GRID_DOT };
    let dot_major = if dark {
        GRID_DOT_MAJOR_DARK
    } else {
        GRID_DOT_MAJOR
    };
    let dot_minor = if dark {
        GRID_DOT_MINOR_DARK
    } else {
        GRID_DOT_MINOR
    };
    let start_x = ((-cam.offset_x * inv_zoom) / GRID_STEP).floor() as i64 - 1;
    let start_y = ((-cam.offset_y * inv_zoom) / GRID_STEP).floor() as i64 - 1;
    let end_x = start_x + (vw * inv_zoom / GRID_STEP).ceil() as i64 + 2;
    let end_y = start_y + (vh * inv_zoom / GRID_STEP).ceil() as i64 + 2;

    if zoom < 0.5 {
        for gx in start_x..end_x {
            for gy in start_y..end_y {
                if gx % 5 != 0 || gy % 5 != 0 {
                    continue;
                }
                let wx = gx as f64 * GRID_STEP;
                let wy = gy as f64 * GRID_STEP;
                scene.fill(
                    Fill::NonZero,
                    camera_tf,
                    dot_major,
                    None,
                    &Circle::new((wx, wy), DOT_RADIUS_MAJOR),
                );
            }
        }
    } else if zoom > 2.0 {
        let half_step = GRID_STEP / 2.0;
        let hs_start_x = ((-cam.offset_x * inv_zoom) / half_step).floor() as i64 - 1;
        let hs_start_y = ((-cam.offset_y * inv_zoom) / half_step).floor() as i64 - 1;
        let hs_end_x = hs_start_x + (vw * inv_zoom / half_step).ceil() as i64 + 2;
        let hs_end_y = hs_start_y + (vh * inv_zoom / half_step).ceil() as i64 + 2;

        for gx in hs_start_x..hs_end_x {
            for gy in hs_start_y..hs_end_y {
                let wx = gx as f64 * half_step;
                let wy = gy as f64 * half_step;
                let (dc, radius) = if gx % 2 == 0 && gy % 2 == 0 {
                    (dot_color, DOT_RADIUS)
                } else {
                    (dot_minor, DOT_RADIUS_MINOR)
                };
                scene.fill(
                    Fill::NonZero,
                    camera_tf,
                    dc,
                    None,
                    &Circle::new((wx, wy), radius),
                );
            }
        }
    } else {
        for gx in start_x..end_x {
            for gy in start_y..end_y {
                let wx = gx as f64 * GRID_STEP;
                let wy = gy as f64 * GRID_STEP;
                scene.fill(
                    Fill::NonZero,
                    camera_tf,
                    dot_color,
                    None,
                    &Circle::new((wx, wy), DOT_RADIUS),
                );
            }
        }
    }
}

pub(crate) fn draw_selection(
    scene: &mut Scene,
    shape: &Shape,
    camera_tf: Affine,
    zoom: f64,
    handle_alpha: f32,
) {
    let sel_tf = if shape.rotation.abs() > 1e-6 {
        let (cx, cy) = shape.center();
        camera_tf * Affine::rotate_about(shape.rotation, (cx, cy))
    } else {
        camera_tf
    };

    // Draw L-shaped corner brackets instead of a full dashed border.
    let bracket_len = 10.0 / zoom;
    let bracket_stroke = Stroke::new(1.5 / zoom);
    let bracket_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x80);
    let gap = 2.0;
    let x0 = shape.x - gap;
    let y0 = shape.y - gap;
    let x1 = shape.x + shape.w + gap;
    let y1 = shape.y + shape.h + gap;

    // Top-left corner
    scene.stroke(
        &bracket_stroke,
        sel_tf,
        bracket_color,
        None,
        &Line::new((x0, y0), (x0 + bracket_len, y0)),
    );
    scene.stroke(
        &bracket_stroke,
        sel_tf,
        bracket_color,
        None,
        &Line::new((x0, y0), (x0, y0 + bracket_len)),
    );
    // Top-right corner
    scene.stroke(
        &bracket_stroke,
        sel_tf,
        bracket_color,
        None,
        &Line::new((x1, y0), (x1 - bracket_len, y0)),
    );
    scene.stroke(
        &bracket_stroke,
        sel_tf,
        bracket_color,
        None,
        &Line::new((x1, y0), (x1, y0 + bracket_len)),
    );
    // Bottom-left corner
    scene.stroke(
        &bracket_stroke,
        sel_tf,
        bracket_color,
        None,
        &Line::new((x0, y1), (x0 + bracket_len, y1)),
    );
    scene.stroke(
        &bracket_stroke,
        sel_tf,
        bracket_color,
        None,
        &Line::new((x0, y1), (x0, y1 - bracket_len)),
    );
    // Bottom-right corner
    scene.stroke(
        &bracket_stroke,
        sel_tf,
        bracket_color,
        None,
        &Line::new((x1, y1), (x1 - bracket_len, y1)),
    );
    scene.stroke(
        &bracket_stroke,
        sel_tf,
        bracket_color,
        None,
        &Line::new((x1, y1), (x1, y1 - bracket_len)),
    );

    let hs = HANDLE_SIZE / zoom;
    let handle_stroke = Stroke::new(1.5 / zoom);
    let sel_rgba = SELECTION_COLOR.to_rgba8();
    let handle_sel = Color::from_rgba8(
        sel_rgba.r,
        sel_rgba.g,
        sel_rgba.b,
        (handle_alpha * 255.0) as u8,
    );
    let handle_fill = Color::from_rgba8(0xff, 0xff, 0xff, (handle_alpha * 255.0) as u8);

    let handles = [
        (shape.x, shape.y),
        (shape.x + shape.w, shape.y),
        (shape.x, shape.y + shape.h),
        (shape.x + shape.w, shape.y + shape.h),
        (shape.x + shape.w / 2.0, shape.y),
        (shape.x + shape.w / 2.0, shape.y + shape.h),
        (shape.x, shape.y + shape.h / 2.0),
        (shape.x + shape.w, shape.y + shape.h / 2.0),
    ];
    for (hx, hy) in handles {
        let hr = Rect::new(hx - hs / 2.0, hy - hs / 2.0, hx + hs / 2.0, hy + hs / 2.0);
        scene.fill(Fill::NonZero, sel_tf, handle_fill, None, &hr);
        scene.stroke(&handle_stroke, sel_tf, handle_sel, None, &hr);
    }

    let rot_cx = shape.x + shape.w / 2.0;
    let rot_cy = shape.y - 20.0 / zoom;
    let rot_radius = 4.0 / zoom;
    let rot_circle = Circle::new((rot_cx, rot_cy), rot_radius);
    scene.fill(Fill::NonZero, sel_tf, handle_fill, None, &rot_circle);
    scene.stroke(&handle_stroke, sel_tf, handle_sel, None, &rot_circle);
    let stem = Line::new((rot_cx, shape.y), (rot_cx, rot_cy + rot_radius));
    scene.stroke(&Stroke::new(1.0 / zoom), sel_tf, handle_sel, None, &stem);
}

pub(crate) fn draw_guides(
    scene: &mut Scene,
    guides: &[AlignGuide],
    cam: &Camera,
    vw: f64,
    vh: f64,
) {
    let camera_tf = Affine::translate((cam.offset_x, cam.offset_y)) * Affine::scale(cam.zoom);
    let inv_zoom = 1.0 / cam.zoom;
    let guide_stroke = Stroke::new(1.0 / cam.zoom).with_dashes(0.0, [6.0, 4.0]);

    for guide in guides {
        match guide {
            AlignGuide::Vertical(x) => {
                let top_y = -cam.offset_y * inv_zoom - 1000.0;
                let bot_y = top_y + vh * inv_zoom + 2000.0;
                let line = Line::new((*x, top_y), (*x, bot_y));
                scene.stroke(&guide_stroke, camera_tf, GUIDE_COLOR, None, &line);
            }
            AlignGuide::Horizontal(y) => {
                let left_x = -cam.offset_x * inv_zoom - 1000.0;
                let right_x = left_x + vw * inv_zoom + 2000.0;
                let line = Line::new((left_x, *y), (right_x, *y));
                scene.stroke(&guide_stroke, camera_tf, GUIDE_COLOR, None, &line);
            }
        }
    }
}
