//! Shape rendering: draw individual shapes and image placeholders.
//!
//! Text shape and sticky note rendering lives in `render_text`.

use std::sync::Arc;

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Rect, RoundedRect, Stroke};
use vello::peniko::{
    Blob, Color, Fill, ImageAlphaType, ImageBrush, ImageData as PenikoImageData, ImageFormat,
};

use crate::render::{SHAPE_FILL_ALPHA, SHAPE_FILL_SELECTED_ALPHA, color_from_hex};
use crate::render_lines::{
    brighten_color, build_shape_transform, build_stroke, draw_arrow, draw_freehand, draw_hachure,
    draw_highlighter, draw_line,
};
use crate::render_text;
use crate::state::{AppState, FillStyle, ShapeKind};

/// Draw a single shape onto the scene.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_shape(
    scene: &mut Scene,
    state: &AppState,
    shape: &crate::state::Shape,
    index: usize,
    camera_tf: Affine,
    is_selected: bool,
    is_hovered: bool,
) {
    let opacity = shape.opacity;
    let fill_alpha = if is_selected {
        SHAPE_FILL_SELECTED_ALPHA * opacity
    } else {
        SHAPE_FILL_ALPHA * opacity
    };
    let stroke_alpha = opacity;

    let stroke_color = if is_hovered {
        brighten_color(color_from_hex(shape.stroke_color, stroke_alpha), 0.15)
    } else {
        color_from_hex(shape.stroke_color, stroke_alpha)
    };
    let hover_width = if is_hovered && matches!(shape.kind, ShapeKind::Line | ShapeKind::Arrow) {
        shape.stroke_width + 1.5
    } else {
        shape.stroke_width
    };
    let stroke = build_stroke(hover_width, shape.stroke_style);

    // Apply rotation + flip transforms around shape center
    let shape_tf = build_shape_transform(shape, camera_tf);

    // Determine fill behavior
    let should_fill = shape.fill_style != FillStyle::None
        && !matches!(
            shape.kind,
            ShapeKind::Line | ShapeKind::Arrow | ShapeKind::Freehand | ShapeKind::Highlighter
        );

    match shape.kind {
        ShapeKind::Rect => {
            let r = Rect::new(shape.x, shape.y, shape.x + shape.w, shape.y + shape.h);
            let corner_r = if shape.rounded { 12.0 } else { 0.0 };
            let rr = RoundedRect::from_rect(r, corner_r);
            if should_fill {
                let fill_color = color_from_hex(shape.color, fill_alpha);
                scene.fill(Fill::NonZero, shape_tf, fill_color, None, &rr);
                if shape.fill_style == FillStyle::Hachure {
                    draw_hachure(
                        scene,
                        shape_tf,
                        shape.x,
                        shape.y,
                        shape.w,
                        shape.h,
                        stroke_color,
                    );
                }
            }
            scene.stroke(&stroke, shape_tf, stroke_color, None, &rr);
        }
        ShapeKind::Ellipse => {
            let cx = shape.x + shape.w / 2.0;
            let cy = shape.y + shape.h / 2.0;
            let ellipse = vello::kurbo::Ellipse::new((cx, cy), (shape.w / 2.0, shape.h / 2.0), 0.0);
            if should_fill {
                let fill_color = color_from_hex(shape.color, fill_alpha);
                scene.fill(Fill::NonZero, shape_tf, fill_color, None, &ellipse);
                if shape.fill_style == FillStyle::Hachure {
                    draw_hachure(
                        scene,
                        shape_tf,
                        shape.x,
                        shape.y,
                        shape.w,
                        shape.h,
                        stroke_color,
                    );
                }
            }
            scene.stroke(&stroke, shape_tf, stroke_color, None, &ellipse);
        }
        ShapeKind::Line => {
            draw_line(scene, state, shape, shape_tf, stroke_color, &stroke);
        }
        ShapeKind::Arrow => draw_arrow(scene, state, shape, index, shape_tf, stroke_color, &stroke),
        ShapeKind::Freehand => draw_freehand(scene, shape, shape_tf, stroke_color, &stroke),
        ShapeKind::Text => {
            render_text::draw_text_shape(scene, state, shape, index, shape_tf);
        }
        ShapeKind::Triangle => {
            let mut path = BezPath::new();
            path.move_to((shape.x + shape.w / 2.0, shape.y));
            path.line_to((shape.x, shape.y + shape.h));
            path.line_to((shape.x + shape.w, shape.y + shape.h));
            path.close_path();
            if should_fill {
                let fill_color = color_from_hex(shape.color, fill_alpha);
                scene.fill(Fill::NonZero, shape_tf, fill_color, None, &path);
                if shape.fill_style == FillStyle::Hachure {
                    draw_hachure(
                        scene,
                        shape_tf,
                        shape.x,
                        shape.y,
                        shape.w,
                        shape.h,
                        stroke_color,
                    );
                }
            }
            scene.stroke(&stroke, shape_tf, stroke_color, None, &path);
        }
        ShapeKind::Diamond => {
            let mut path = BezPath::new();
            path.move_to((shape.x + shape.w / 2.0, shape.y));
            path.line_to((shape.x + shape.w, shape.y + shape.h / 2.0));
            path.line_to((shape.x + shape.w / 2.0, shape.y + shape.h));
            path.line_to((shape.x, shape.y + shape.h / 2.0));
            path.close_path();
            if should_fill {
                let fill_color = color_from_hex(shape.color, fill_alpha);
                scene.fill(Fill::NonZero, shape_tf, fill_color, None, &path);
                if shape.fill_style == FillStyle::Hachure {
                    draw_hachure(
                        scene,
                        shape_tf,
                        shape.x,
                        shape.y,
                        shape.w,
                        shape.h,
                        stroke_color,
                    );
                }
            }
            scene.stroke(&stroke, shape_tf, stroke_color, None, &path);
        }
        ShapeKind::StickyNote => {
            render_text::draw_sticky_note(scene, state, shape, index, shape_tf);
        }
        ShapeKind::Highlighter => draw_highlighter(scene, shape, shape_tf, opacity),
        ShapeKind::Image => {
            if !draw_image_raster(scene, shape, shape_tf) {
                draw_image_placeholder(
                    scene,
                    state,
                    shape,
                    shape_tf,
                    fill_alpha,
                    stroke_color,
                    &stroke,
                );
            }
        }
    }

    // Hover feedback: subtle highlight border
    if is_hovered && !matches!(shape.kind, ShapeKind::Line | ShapeKind::Arrow) {
        let hover_stroke = Stroke::new(1.0);
        let hover_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x44);
        let gap = 1.0;
        let r = Rect::new(
            shape.x - gap,
            shape.y - gap,
            shape.x + shape.w + gap,
            shape.y + shape.h + gap,
        );
        scene.stroke(&hover_stroke, shape_tf, hover_color, None, &r);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_image_placeholder(
    scene: &mut Scene,
    state: &AppState,
    shape: &crate::state::Shape,
    shape_tf: Affine,
    fill_alpha: f32,
    stroke_color: Color,
    stroke: &Stroke,
) {
    let r = Rect::new(shape.x, shape.y, shape.x + shape.w, shape.y + shape.h);
    let rr = RoundedRect::from_rect(r, if shape.rounded { 12.0 } else { 0.0 });
    let fill_color = color_from_hex(0xdddddd, fill_alpha);
    scene.fill(Fill::NonZero, shape_tf, fill_color, None, &rr);
    scene.stroke(stroke, shape_tf, stroke_color, None, &rr);
    let tx = shape.x + shape.w / 2.0 - 12.0;
    let ty = shape.y + shape.h / 2.0;
    let label_color = Color::from_rgba8(0x88, 0x88, 0x88, 0xff);
    crate::text::draw_text(
        scene,
        &state.fonts,
        "IMG",
        tx,
        ty,
        16.0,
        label_color,
        shape_tf,
    );
}

/// Draw the shape's attached raster image via vello's GPU texture path.
///
/// Returns `true` if the image was drawn, `false` if `shape.image` had no
/// decoded RGBA bytes (caller falls back to the gray placeholder).
///
/// Why a separate fn: keeps `draw_shape` readable and lets the placeholder
/// path stay the cold fallback. Why an `Arc<Vec<u8>>` round-trip into a
/// `peniko::Blob`: peniko owns the storage as `Arc<dyn AsRef<[u8]> + Send +
/// Sync>` and `Vec<u8>` already implements `AsRef<[u8]>`, so the clone here
/// is a refcount bump rather than a byte copy. The same shape can ride
/// through 60 fps of redraws without re-uploading until vello's image cache
/// is told the bytes changed.
fn draw_image_raster(scene: &mut Scene, shape: &crate::state::Shape, shape_tf: Affine) -> bool {
    let Some(img) = shape.image.as_ref() else {
        return false;
    };
    let Some(rgba) = img.rgba.as_ref() else {
        return false;
    };
    if img.width == 0 || img.height == 0 || shape.w <= 0.0 || shape.h <= 0.0 {
        return false;
    }
    // peniko::Blob takes Arc<dyn AsRef<[T]> + Send + Sync>. Cloning the
    // Arc<Vec<u8>> is a refcount bump; the bytes are not duplicated.
    let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = rgba.clone();
    let peniko_img = PenikoImageData {
        data: Blob::new(bytes),
        format: ImageFormat::Rgba8,
        // image::DynamicImage::to_rgba8 emits straight (unpremultiplied) alpha.
        alpha_type: ImageAlphaType::Alpha,
        width: img.width,
        height: img.height,
    };
    // Place at (shape.x, shape.y), scale natural pixels to (shape.w, shape.h).
    // shape_tf already carries camera + per-shape rotation/flip; compose it
    // around our local transform so user transforms still apply on top.
    let scale_x = shape.w / img.width as f64;
    let scale_y = shape.h / img.height as f64;
    let local_tf =
        Affine::translate((shape.x, shape.y)) * Affine::scale_non_uniform(scale_x, scale_y);
    let combined_tf = shape_tf * local_tf;
    let brush = ImageBrush::new(peniko_img);
    scene.draw_image(&brush, combined_tf);
    true
}
