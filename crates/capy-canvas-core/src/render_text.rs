//! Text shape rendering: text boxes, sticky notes, multiline text with
//! cursor, selection, and alignment.

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Circle, Ellipse, Line, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

use crate::render::color_from_hex;
use crate::state::{AppState, CanvasContentKind};

pub(crate) fn draw_text_shape(
    scene: &mut Scene,
    state: &AppState,
    shape: &crate::state::Shape,
    index: usize,
    camera_tf: Affine,
) {
    let r = Rect::new(shape.x, shape.y, shape.x + shape.w, shape.y + shape.h);
    let rr = RoundedRect::from_rect(r, if shape.rounded { 12.0 } else { 0.0 });
    scene.fill(
        Fill::NonZero,
        camera_tf,
        Color::from_rgba8(0xff, 0xff, 0xff, 0xcc),
        None,
        &rr,
    );
    scene.stroke(
        &Stroke::new(1.0),
        camera_tf,
        Color::from_rgba8(0xcc, 0xcc, 0xcc, 0xff),
        None,
        &rr,
    );
    let font_size = shape.font_size as f32;
    let text_color = color_from_hex(shape.color, 1.0);
    let pad_x = 4.0;
    let pad_y = 4.0;

    draw_multiline_text(
        scene, state, shape, index, camera_tf, font_size, text_color, pad_x, pad_y,
    );
}

/// Draw a sticky note: yellow rounded rect + text content.
pub(crate) fn draw_sticky_note(
    scene: &mut Scene,
    state: &AppState,
    shape: &crate::state::Shape,
    index: usize,
    camera_tf: Affine,
) {
    let r = Rect::new(shape.x, shape.y, shape.x + shape.w, shape.y + shape.h);
    let rr = RoundedRect::from_rect(r, if shape.rounded { 12.0 } else { 0.0 });
    let is_content_card = shape.metadata.content_kind.is_some();
    let sticky_fill = if is_content_card {
        color_from_hex(shape.color, 1.0)
    } else {
        Color::from_rgba8(0xfe, 0xf3, 0xc7, 0xff)
    };
    let sticky_border = if is_content_card {
        color_from_hex(shape.stroke_color, 1.0)
    } else {
        Color::from_rgba8(0xe8, 0xd5, 0x8a, 0xff)
    };
    scene.fill(Fill::NonZero, camera_tf, sticky_fill, None, &rr);
    scene.stroke(&Stroke::new(1.0), camera_tf, sticky_border, None, &rr);

    if let Some(kind) = shape.metadata.content_kind {
        draw_content_card_header(scene, shape, kind, camera_tf, sticky_border);
    }

    // Subtle shadow fold at bottom-right
    let fold_size = 8.0;
    let mut fold = BezPath::new();
    fold.move_to((shape.x + shape.w - fold_size, shape.y + shape.h));
    fold.line_to((shape.x + shape.w, shape.y + shape.h - fold_size));
    fold.line_to((shape.x + shape.w, shape.y + shape.h));
    fold.close_path();
    let fold_color = if is_content_card {
        color_from_hex(shape.stroke_color, 0.22)
    } else {
        Color::from_rgba8(0xd4, 0xc1, 0x7a, 0x88)
    };
    scene.fill(Fill::NonZero, camera_tf, fold_color, None, &fold);

    let font_size = shape.font_size as f32;
    let text_color = if is_content_card {
        Color::from_rgba8(0x24, 0x23, 0x2a, 0xff)
    } else {
        Color::from_rgba8(0x44, 0x3c, 0x22, 0xff)
    };
    let pad_x = if is_content_card { 52.0 } else { 8.0 };
    let pad_y = if is_content_card { 18.0 } else { 8.0 };

    draw_multiline_text(
        scene, state, shape, index, camera_tf, font_size, text_color, pad_x, pad_y,
    );
}

fn draw_content_card_header(
    scene: &mut Scene,
    shape: &crate::state::Shape,
    kind: CanvasContentKind,
    camera_tf: Affine,
    accent: Color,
) {
    let accent_fill = color_from_hex(shape.stroke_color, 0.12);
    let header = RoundedRect::from_rect(
        Rect::new(
            shape.x + 8.0,
            shape.y + 8.0,
            shape.x + shape.w - 8.0,
            shape.y + 44.0,
        ),
        10.0,
    );
    scene.fill(Fill::NonZero, camera_tf, accent_fill, None, &header);

    let icon_x = shape.x + 24.0;
    let icon_y = shape.y + 26.0;
    draw_content_kind_icon(scene, kind, icon_x, icon_y, accent, camera_tf);
}

fn draw_content_kind_icon(
    scene: &mut Scene,
    kind: CanvasContentKind,
    cx: f64,
    cy: f64,
    color: Color,
    camera_tf: Affine,
) {
    match kind {
        CanvasContentKind::Brand => {
            for (dx, dy) in [(-6.0, -6.0), (6.0, -6.0), (-6.0, 6.0), (6.0, 6.0)] {
                scene.fill(
                    Fill::NonZero,
                    camera_tf,
                    color,
                    None,
                    &Circle::new((cx + dx, cy + dy), 4.0),
                );
            }
        }
        CanvasContentKind::Image => {
            let frame =
                RoundedRect::from_rect(Rect::new(cx - 12.0, cy - 9.0, cx + 12.0, cy + 9.0), 4.0);
            scene.stroke(&Stroke::new(1.5), camera_tf, color, None, &frame);
            scene.fill(
                Fill::NonZero,
                camera_tf,
                color,
                None,
                &Circle::new((cx + 6.0, cy - 3.0), 2.5),
            );
            let mut mountain = BezPath::new();
            mountain.move_to((cx - 9.0, cy + 7.0));
            mountain.line_to((cx - 2.0, cy));
            mountain.line_to((cx + 4.0, cy + 5.0));
            mountain.line_to((cx + 10.0, cy - 1.0));
            scene.stroke(&Stroke::new(1.5), camera_tf, color, None, &mountain);
        }
        CanvasContentKind::Poster => {
            let frame =
                RoundedRect::from_rect(Rect::new(cx - 12.0, cy - 8.0, cx + 12.0, cy + 8.0), 3.0);
            scene.stroke(&Stroke::new(1.5), camera_tf, color, None, &frame);
            scene.stroke(
                &Stroke::new(1.1),
                camera_tf,
                color,
                None,
                &Line::new((cx - 9.0, cy - 2.0), (cx + 9.0, cy - 2.0)),
            );
            scene.stroke(
                &Stroke::new(1.1),
                camera_tf,
                color,
                None,
                &Line::new((cx - 9.0, cy + 3.0), (cx + 3.0, cy + 3.0)),
            );
        }
        CanvasContentKind::Video => {
            let frame =
                RoundedRect::from_rect(Rect::new(cx - 12.0, cy - 9.0, cx + 12.0, cy + 9.0), 4.0);
            scene.stroke(&Stroke::new(1.5), camera_tf, color, None, &frame);
            let mut play = BezPath::new();
            play.move_to((cx - 3.0, cy - 5.0));
            play.line_to((cx + 6.0, cy));
            play.line_to((cx - 3.0, cy + 5.0));
            play.close_path();
            scene.fill(Fill::NonZero, camera_tf, color, None, &play);
        }
        CanvasContentKind::Web => {
            let globe = Ellipse::new((cx, cy), (12.0, 9.0), 0.0);
            scene.stroke(&Stroke::new(1.5), camera_tf, color, None, &globe);
            scene.stroke(
                &Stroke::new(1.0),
                camera_tf,
                color,
                None,
                &Line::new((cx - 10.0, cy), (cx + 10.0, cy)),
            );
            scene.stroke(
                &Stroke::new(1.0),
                camera_tf,
                color,
                None,
                &Line::new((cx, cy - 9.0), (cx, cy + 9.0)),
            );
        }
        CanvasContentKind::Text => {
            for y in [-6.0, 0.0, 6.0] {
                scene.stroke(
                    &Stroke::new(2.0),
                    camera_tf,
                    color,
                    None,
                    &Line::new((cx - 10.0, cy + y), (cx + 10.0, cy + y)),
                );
            }
        }
        CanvasContentKind::Audio => {
            scene.stroke(
                &Stroke::new(2.0),
                camera_tf,
                color,
                None,
                &Line::new((cx - 9.0, cy + 4.0), (cx - 2.0, cy + 4.0)),
            );
            scene.stroke(
                &Stroke::new(2.0),
                camera_tf,
                color,
                None,
                &Line::new((cx - 2.0, cy + 4.0), (cx + 6.0, cy - 7.0)),
            );
            scene.stroke(
                &Stroke::new(2.0),
                camera_tf,
                color,
                None,
                &Line::new((cx + 6.0, cy - 7.0), (cx + 6.0, cy + 6.0)),
            );
        }
        CanvasContentKind::ThreeD => {
            let r = Rect::new(cx - 8.0, cy - 8.0, cx + 8.0, cy + 8.0);
            scene.stroke(&Stroke::new(1.5), camera_tf, color, None, &r);
            scene.stroke(
                &Stroke::new(1.5),
                camera_tf,
                color,
                None,
                &Line::new((cx - 8.0, cy - 8.0), (cx, cy - 13.0)),
            );
            scene.stroke(
                &Stroke::new(1.5),
                camera_tf,
                color,
                None,
                &Line::new((cx + 8.0, cy - 8.0), (cx, cy - 13.0)),
            );
        }
        CanvasContentKind::Project | CanvasContentKind::Shape => {
            let frame =
                RoundedRect::from_rect(Rect::new(cx - 11.0, cy - 10.0, cx + 11.0, cy + 10.0), 4.0);
            scene.stroke(&Stroke::new(1.5), camera_tf, color, None, &frame);
            scene.fill(
                Fill::NonZero,
                camera_tf,
                color,
                None,
                &Circle::new((cx, cy), 3.5),
            );
        }
    }
}

/// Render multi-line text with alignment, font styling, selection, and cursor.
#[allow(clippy::too_many_arguments)]
fn draw_multiline_text(
    scene: &mut Scene,
    state: &AppState,
    shape: &crate::state::Shape,
    index: usize,
    camera_tf: Affine,
    font_size: f32,
    text_color: Color,
    pad_x: f64,
    pad_y: f64,
) {
    let line_height = font_size as f64 * 1.4;
    let content_w = shape.w - pad_x * 2.0;
    let family = shape.font_family;
    let bold = shape.bold;
    let italic = shape.italic;

    let lines: Vec<&str> = shape.text.split('\n').collect();

    // Build char offsets for each line start
    let mut line_starts: Vec<usize> = Vec::with_capacity(lines.len());
    let mut char_offset = 0;
    for (i, line_text) in lines.iter().enumerate() {
        line_starts.push(char_offset);
        char_offset += line_text.chars().count();
        if i + 1 < lines.len() {
            char_offset += 1;
        }
    }

    let editing = state
        .text_edit
        .as_ref()
        .filter(|te| te.shape_index == index);

    // Draw selection highlight if active
    if let Some(te) = editing {
        if let Some((sel_a, sel_b)) = te.selection_range() {
            if sel_a != sel_b {
                let sel_color = Color::from_rgba8(0x8a, 0x6f, 0xae, 0x44);
                for (li, line_text) in lines.iter().enumerate() {
                    let ls = line_starts[li];
                    let le = ls + line_text.chars().count();
                    let start = sel_a.max(ls);
                    let end = sel_b.min(le);
                    if start < end {
                        let line_y = shape.y + pad_y + li as f64 * line_height;
                        let x_start = crate::text::measure_text_prefix_styled(
                            &state.fonts,
                            line_text,
                            start - ls,
                            font_size,
                            family,
                            bold,
                            italic,
                        );
                        let x_end = crate::text::measure_text_prefix_styled(
                            &state.fonts,
                            line_text,
                            end - ls,
                            font_size,
                            family,
                            bold,
                            italic,
                        );
                        let base_x = text_x_for_align(
                            shape,
                            pad_x,
                            content_w,
                            line_text,
                            font_size,
                            &state.fonts,
                            family,
                            bold,
                            italic,
                        );
                        let sel_rect = Rect::new(
                            base_x + x_start,
                            line_y,
                            base_x + x_end,
                            line_y + line_height,
                        );
                        scene.fill(Fill::NonZero, camera_tf, sel_color, None, &sel_rect);
                    }
                }
            }
        }
    }

    // Draw each line
    for (li, line_text) in lines.iter().enumerate() {
        if line_text.is_empty() {
            continue;
        }
        let line_y = shape.y + pad_y + li as f64 * line_height;
        let text_x = text_x_for_align(
            shape,
            pad_x,
            content_w,
            line_text,
            font_size,
            &state.fonts,
            family,
            bold,
            italic,
        );
        crate::text::draw_text_styled(
            scene,
            &state.fonts,
            line_text,
            text_x,
            line_y,
            font_size,
            text_color,
            camera_tf,
            family,
            bold,
            italic,
        );
    }

    // Draw cursor
    if let Some(te) = editing {
        if te.blink_visible {
            let (cursor_line, cursor_col) = cursor_to_line_col_render(&shape.text, te.cursor);
            let line_text = lines.get(cursor_line).copied().unwrap_or("");
            let line_y = shape.y + pad_y + cursor_line as f64 * line_height;
            let base_x = text_x_for_align(
                shape,
                pad_x,
                content_w,
                line_text,
                font_size,
                &state.fonts,
                family,
                bold,
                italic,
            );
            let cursor_offset = crate::text::measure_text_prefix_styled(
                &state.fonts,
                line_text,
                cursor_col,
                font_size,
                family,
                bold,
                italic,
            );
            let cx = base_x + cursor_offset;
            let cursor_line_shape = Line::new((cx, line_y + 2.0), (cx, line_y + line_height - 2.0));
            scene.stroke(
                &Stroke::new(1.5),
                camera_tf,
                Color::from_rgba8(0x00, 0x00, 0x00, 0xff),
                None,
                &cursor_line_shape,
            );
        }
    }
}

/// Compute text x position based on alignment.
#[allow(clippy::too_many_arguments)]
fn text_x_for_align(
    shape: &crate::state::Shape,
    pad_x: f64,
    content_w: f64,
    line_text: &str,
    font_size: f32,
    fonts: &crate::text::FontPair,
    family: crate::state::FontFamily,
    bold: bool,
    italic: bool,
) -> f64 {
    use crate::state::TextAlign;
    let text_w =
        crate::text::measure_text_styled(fonts, line_text, font_size, family, bold, italic);
    match shape.text_align {
        TextAlign::Left => shape.x + pad_x,
        TextAlign::Center => shape.x + pad_x + (content_w - text_w) / 2.0,
        TextAlign::Right => shape.x + pad_x + content_w - text_w,
    }
}

/// Convert flat cursor to (line, col) for rendering.
fn cursor_to_line_col_render(text: &str, cursor: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in text.chars().enumerate() {
        if i == cursor {
            return (line, col);
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}
