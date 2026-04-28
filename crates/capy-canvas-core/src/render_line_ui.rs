//! Selection and inline label UI for line and arrow shapes.

use vello::Scene;
use vello::kurbo::{Affine, Circle, Line, Rect, Stroke};
use vello::peniko::{Color, Fill};

use crate::line_geometry;
use crate::state::{AppState, Shape, ShapeKind, TextTarget};

pub(crate) fn draw_shape_selection(
    scene: &mut Scene,
    state: &AppState,
    shape: &Shape,
    camera_tf: Affine,
    zoom: f64,
    handle_alpha: f32,
) {
    let (start, end) = line_geometry::endpoints(state, shape);
    let sel_rgba = crate::render::SELECTION_COLOR.to_rgba8();
    let stroke_color = Color::from_rgba8(
        sel_rgba.r,
        sel_rgba.g,
        sel_rgba.b,
        (handle_alpha * 255.0) as u8,
    );
    let fill_color = Color::from_rgba8(0xff, 0xff, 0xff, (handle_alpha * 255.0) as u8);
    let handle_stroke = Stroke::new(1.5 / zoom);
    let endpoint_radius = line_geometry::LINE_HANDLE_RADIUS / zoom;

    for point in [start, end] {
        let handle = Circle::new(point, endpoint_radius);
        scene.fill(Fill::NonZero, camera_tf, fill_color, None, &handle);
        scene.stroke(&handle_stroke, camera_tf, stroke_color, None, &handle);
    }

    if shape.kind == ShapeKind::Arrow {
        let mid = line_geometry::mid_handle_position(state, shape);
        let mid_handle = Circle::new(mid, (line_geometry::LINE_HANDLE_RADIUS - 1.0) / zoom);
        let mid_fill = Color::from_rgba8(sel_rgba.r, sel_rgba.g, sel_rgba.b, 0x2a);
        scene.fill(Fill::NonZero, camera_tf, mid_fill, None, &mid_handle);
        scene.stroke(&handle_stroke, camera_tf, stroke_color, None, &mid_handle);
    }
}

pub(crate) fn draw_arrow_label(
    scene: &mut Scene,
    state: &AppState,
    shape: &Shape,
    index: usize,
    shape_tf: Affine,
    stroke_color: Color,
) {
    let editing = state
        .text_edit
        .as_ref()
        .filter(|te| te.shape_index == index && te.target == TextTarget::Label);
    let Some(label) = editing
        .map(|_| shape.label.as_deref().unwrap_or(""))
        .or(shape.label.as_deref())
    else {
        return;
    };
    if label.is_empty() && editing.is_none() {
        return;
    }

    let font_size = 12.0_f32;
    let family = shape.font_family;
    let bold = shape.bold;
    let italic = shape.italic;
    let text_w =
        crate::text::measure_text_styled(&state.fonts, label, font_size, family, bold, italic);
    let mid = line_geometry::midpoint(state, shape);
    let x = mid.0 - text_w / 2.0;
    let y = mid.1 - 10.0;

    if let Some(te) = editing {
        if let Some((sel_a, sel_b)) = te.selection_range() {
            if sel_a != sel_b {
                let sel_x = x + crate::text::measure_text_prefix_styled(
                    &state.fonts,
                    label,
                    sel_a,
                    font_size,
                    family,
                    bold,
                    italic,
                );
                let sel_end = x + crate::text::measure_text_prefix_styled(
                    &state.fonts,
                    label,
                    sel_b,
                    font_size,
                    family,
                    bold,
                    italic,
                );
                let sel_rect = Rect::new(sel_x, y, sel_end, y + font_size as f64 * 1.4);
                scene.fill(
                    Fill::NonZero,
                    shape_tf,
                    Color::from_rgba8(0x8a, 0x6f, 0xae, 0x44),
                    None,
                    &sel_rect,
                );
            }
        }
    }

    if !label.is_empty() {
        crate::text::draw_text_styled(
            scene,
            &state.fonts,
            label,
            x,
            y,
            font_size,
            stroke_color,
            shape_tf,
            family,
            bold,
            italic,
        );
    }

    if let Some(te) = editing {
        if te.blink_visible {
            let cursor_x = x + crate::text::measure_text_prefix_styled(
                &state.fonts,
                label,
                te.cursor,
                font_size,
                family,
                bold,
                italic,
            );
            let cursor = Line::new((cursor_x, y + 2.0), (cursor_x, y + font_size as f64 * 1.2));
            scene.stroke(&Stroke::new(1.5), shape_tf, stroke_color, None, &cursor);
        }
    }
}
