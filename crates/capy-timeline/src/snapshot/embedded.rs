use std::fs;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, RgbaImage};
use serde_json::Value;

use super::report::SnapshotError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotMetrics {
    pub width: u32,
    pub height: u32,
    pub byte_size: u64,
}

pub fn snapshot_embedded(
    render_source_path: &Path,
    out: &Path,
) -> Result<SnapshotMetrics, SnapshotError> {
    let image = render_frame(render_source_path)?;
    write_png(&image, out)?;
    read_png_metrics(out)
}

pub fn render_frame(render_source_path: &Path) -> Result<RgbaImage, SnapshotError> {
    let source = read_source(render_source_path)?;
    let viewport = viewport(&source);
    let poster = match poster_from_source(&source) {
        Ok(poster) => poster,
        Err(_) if has_scroll_chapter_component(&source) => {
            let mut image =
                ImageBuffer::from_pixel(viewport.0, viewport.1, Rgba([17, 24, 39, 255]));
            render_scroll_placeholder(&mut image);
            return Ok(image);
        }
        Err(err) => return Err(err),
    };
    let background = poster
        .get("canvas")
        .and_then(|canvas| canvas.get("background"))
        .and_then(Value::as_str)
        .and_then(parse_color)
        .unwrap_or(Rgba([255, 255, 255, 255]));
    let mut image = ImageBuffer::from_pixel(viewport.0, viewport.1, background);
    render_layers(&mut image, &poster, render_source_path);
    Ok(image)
}

fn has_scroll_chapter_component(source: &Value) -> bool {
    source
        .get("components")
        .and_then(|components| components.get("html.capy-scroll-chapter"))
        .is_some()
}

fn render_scroll_placeholder(image: &mut RgbaImage) {
    let width = image.width();
    let height = image.height();
    let panel = Rect {
        x: width / 8,
        y: height / 3,
        w: width.saturating_mul(3) / 4,
        h: height / 3,
    };
    fill_rect(image, panel, Rgba([31, 41, 55, 255]));
    let line_h = (panel.h / 10).max(4);
    fill_rect(
        image,
        Rect {
            x: panel.x + panel.w / 12,
            y: panel.y + panel.h / 4,
            w: panel.w / 3,
            h: line_h,
        },
        Rgba([156, 163, 175, 255]),
    );
    fill_rect(
        image,
        Rect {
            x: panel.x + panel.w / 12,
            y: panel.y + panel.h / 2,
            w: panel.w.saturating_mul(2) / 3,
            h: line_h * 2,
        },
        Rgba([249, 250, 251, 255]),
    );
}

pub fn read_png_metrics(path: &Path) -> Result<SnapshotMetrics, SnapshotError> {
    let image = image::open(path).map_err(|err| {
        SnapshotError::new(
            "SNAPSHOT_FAILED",
            "$.snapshot_path",
            format!("read PNG metrics failed: {err}"),
            "next step · rerun capy timeline snapshot",
        )
    })?;
    let metadata = fs::metadata(path).map_err(|err| {
        SnapshotError::new(
            "SNAPSHOT_FAILED",
            "$.snapshot_path",
            format!("read PNG metadata failed: {err}"),
            "next step · check snapshot output permissions",
        )
    })?;
    Ok(SnapshotMetrics {
        width: image.width(),
        height: image.height(),
        byte_size: metadata.len(),
    })
}

fn read_source(path: &Path) -> Result<Value, SnapshotError> {
    let text = fs::read_to_string(path).map_err(|err| {
        SnapshotError::new(
            "RENDER_SOURCE_MISSING",
            "$.render_source_path",
            format!("read render_source failed: {err}"),
            "next step · run capy timeline compile --composition <path>",
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        SnapshotError::new(
            "SNAPSHOT_FAILED",
            "$.render_source",
            format!("render_source JSON is invalid: {err}"),
            "next step · rerun capy timeline compile",
        )
    })
}

fn viewport(source: &Value) -> (u32, u32) {
    let width = source
        .get("viewport")
        .and_then(|viewport| viewport.get("w").or_else(|| viewport.get("width")))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(1080);
    let height = source
        .get("viewport")
        .and_then(|viewport| viewport.get("h").or_else(|| viewport.get("height")))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(1080);
    (width, height)
}

fn poster_from_source(source: &Value) -> Result<Value, SnapshotError> {
    let tracks = source
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_source("$.tracks", "render_source has no tracks array"))?;
    for track in tracks {
        let Some(clips) = track.get("clips").and_then(Value::as_array) else {
            continue;
        };
        for clip in clips {
            let poster = clip
                .get("params")
                .and_then(|params| params.get("params"))
                .and_then(|params| params.get("poster"));
            if let Some(poster) = poster {
                return Ok(poster.clone());
            }
        }
    }
    Err(invalid_source(
        "$.tracks[].clips[].params.params.poster",
        "render_source has no poster payload",
    ))
}

fn render_layers(image: &mut RgbaImage, poster: &Value, render_source_path: &Path) {
    let mut layers = poster
        .get("layers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    layers.sort_by_key(|layer| layer.get("z").and_then(Value::as_i64).unwrap_or(0));
    for layer in layers {
        render_layer(image, poster, &layer, render_source_path);
    }
}

fn render_layer(image: &mut RgbaImage, poster: &Value, layer: &Value, render_source_path: &Path) {
    match layer.get("type").and_then(Value::as_str).unwrap_or("") {
        "image" => render_image_layer(image, poster, layer, render_source_path),
        "shape" => render_shape_layer(image, layer),
        "text" => render_text_layer(image, layer),
        _ => {}
    }
}

fn render_shape_layer(image: &mut RgbaImage, layer: &Value) {
    let rect = rect(layer);
    let color = style_color(layer, "fill").unwrap_or(Rgba([220, 220, 220, 255]));
    if layer.get("shape").and_then(Value::as_str) == Some("ellipse") {
        fill_ellipse(image, rect, color);
    } else {
        fill_rect(image, rect, color);
    }
}

fn render_text_layer(image: &mut RgbaImage, layer: &Value) {
    let rect = rect(layer);
    let fill = style_color(layer, "fill").unwrap_or(Rgba([0, 0, 0, 0]));
    if fill.0[3] > 0 {
        fill_rect(image, rect, fill);
    }
    let color = style_color(layer, "color").unwrap_or(Rgba([28, 28, 28, 255]));
    let bar_height = (rect.h / 7).max(2);
    let mut y = rect.y.saturating_add(bar_height);
    for line in text_lines(layer).take(4) {
        let chars = u32::try_from(line.chars().count()).unwrap_or(u32::MAX);
        let line_width = rect
            .w
            .min(chars.saturating_mul(bar_height).max(bar_height * 4));
        fill_rect(
            image,
            Rect {
                x: rect.x.saturating_add(bar_height),
                y,
                w: line_width.saturating_sub(bar_height * 2),
                h: bar_height,
            },
            color,
        );
        y = y.saturating_add(bar_height * 2);
        if y >= rect.y.saturating_add(rect.h) {
            break;
        }
    }
}

fn render_image_layer(image: &mut RgbaImage, poster: &Value, layer: &Value, source_path: &Path) {
    let Some(asset_id) = layer.get("assetId").and_then(Value::as_str) else {
        return;
    };
    let Some(src) = poster
        .get("assets")
        .and_then(|assets| assets.get(asset_id))
        .and_then(|asset| asset.get("src"))
        .and_then(Value::as_str)
    else {
        return;
    };
    let asset_path = resolve_asset_path(src, source_path);
    let Ok(asset) = image::open(asset_path) else {
        return;
    };
    let rect = rect(layer);
    if rect.w == 0 || rect.h == 0 {
        return;
    }
    let resized = asset.resize_exact(rect.w, rect.h, FilterType::Lanczos3);
    overlay(image, &resized.to_rgba8(), rect.x, rect.y);
}

pub fn write_png(image: &RgbaImage, out: &Path) -> Result<(), SnapshotError> {
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|err| {
            SnapshotError::new(
                "SNAPSHOT_FAILED",
                "$.snapshot_path",
                format!("create snapshot parent failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    image.save(out).map_err(|err| {
        SnapshotError::new(
            "SNAPSHOT_FAILED",
            "$.snapshot_path",
            format!("write PNG failed: {err}"),
            "next step · check output path and permissions",
        )
    })
}

fn overlay(base: &mut RgbaImage, top: &RgbaImage, x: u32, y: u32) {
    for top_y in 0..top.height() {
        for top_x in 0..top.width() {
            let base_x = x.saturating_add(top_x);
            let base_y = y.saturating_add(top_y);
            if base_x < base.width() && base_y < base.height() {
                let pixel = top.get_pixel(top_x, top_y);
                base.put_pixel(base_x, base_y, *pixel);
            }
        }
    }
}

fn fill_rect(image: &mut RgbaImage, rect: Rect, color: Rgba<u8>) {
    let end_x = rect.x.saturating_add(rect.w).min(image.width());
    let end_y = rect.y.saturating_add(rect.h).min(image.height());
    for y in rect.y.min(image.height())..end_y {
        for x in rect.x.min(image.width())..end_x {
            image.put_pixel(x, y, color);
        }
    }
}

fn fill_ellipse(image: &mut RgbaImage, rect: Rect, color: Rgba<u8>) {
    if rect.w == 0 || rect.h == 0 {
        return;
    }
    let rx = f64::from(rect.w) / 2.0;
    let ry = f64::from(rect.h) / 2.0;
    let cx = f64::from(rect.x) + rx;
    let cy = f64::from(rect.y) + ry;
    let end_x = rect.x.saturating_add(rect.w).min(image.width());
    let end_y = rect.y.saturating_add(rect.h).min(image.height());
    for y in rect.y.min(image.height())..end_y {
        for x in rect.x.min(image.width())..end_x {
            let dx = (f64::from(x) + 0.5 - cx) / rx;
            let dy = (f64::from(y) + 0.5 - cy) / ry;
            if dx * dx + dy * dy <= 1.0 {
                image.put_pixel(x, y, color);
            }
        }
    }
}

fn style_color(layer: &Value, key: &str) -> Option<Rgba<u8>> {
    layer
        .get("style")
        .and_then(|style| style.get(key))
        .and_then(Value::as_str)
        .and_then(parse_color)
}

fn parse_color(raw: &str) -> Option<Rgba<u8>> {
    let hex = raw.trim().strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(Rgba([r, g, b, 255]))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Rgba([r, g, b, 255]))
        }
        _ => None,
    }
}

fn text_lines(layer: &Value) -> impl Iterator<Item = &str> {
    layer
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .lines()
}

fn resolve_asset_path(src: &str, source_path: &Path) -> PathBuf {
    let path = PathBuf::from(src);
    if path.is_absolute() {
        path
    } else {
        source_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

fn rect(layer: &Value) -> Rect {
    Rect {
        x: number(layer, "x").max(0.0).round() as u32,
        y: number(layer, "y").max(0.0).round() as u32,
        w: number(layer, "width").max(0.0).round() as u32,
        h: number(layer, "height").max(0.0).round() as u32,
    }
}

fn number(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn invalid_source(path: &str, message: &str) -> SnapshotError {
    SnapshotError::new(
        "SNAPSHOT_FAILED",
        path,
        message,
        "next step · rerun capy timeline compile",
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::{read_png_metrics, snapshot_embedded};

    #[test]
    fn embedded_snapshot_writes_png_with_viewport_dimensions()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("happy")?;
        let source = dir.join("render_source.json");
        let out = dir.join("frame.png");
        fs::write(&source, serde_json::to_vec_pretty(&render_source())?)?;

        let metrics = snapshot_embedded(&source, &out)?;

        assert!(out.is_file());
        assert!(metrics.byte_size > 0);
        assert_eq!(metrics.width, 320);
        assert_eq!(metrics.height, 180);
        let decoded = read_png_metrics(&out)?;
        assert_eq!(decoded, metrics);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn embedded_snapshot_defaults_viewport() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("default-viewport")?;
        let source = dir.join("render_source.json");
        let out = dir.join("frame.png");
        let mut value = render_source();
        value["viewport"] = json!({});
        fs::write(&source, serde_json::to_vec_pretty(&value)?)?;

        let metrics = snapshot_embedded(&source, &out)?;

        assert_eq!(metrics.width, 1080);
        assert_eq!(metrics.height, 1080);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn embedded_snapshot_reports_missing_poster() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("missing-poster")?;
        let source = dir.join("render_source.json");
        let out = dir.join("frame.png");
        fs::write(&source, serde_json::to_vec_pretty(&json!({"tracks": []}))?)?;

        let err = snapshot_embedded(&source, &out)
            .err()
            .ok_or("missing poster should fail")?;

        assert_eq!(err.code, "SNAPSHOT_FAILED");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    fn render_source() -> serde_json::Value {
        json!({
            "schema_version": "capy.timeline.render_source.v1",
            "viewport": {"w": 320, "h": 180},
            "tracks": [{
                "id": "poster.document",
                "clips": [{
                    "params": {
                        "params": {
                            "poster": {
                                "canvas": {"background": "#ffffff"},
                                "assets": {},
                                "layers": [
                                    {
                                        "id": "shape",
                                        "type": "shape",
                                        "shape": "rect",
                                        "x": 0,
                                        "y": 0,
                                        "width": 320,
                                        "height": 180,
                                        "style": {"fill": "#f0f0f0"}
                                    },
                                    {
                                        "id": "headline",
                                        "type": "text",
                                        "text": "Launch",
                                        "x": 20,
                                        "y": 20,
                                        "width": 120,
                                        "height": 40,
                                        "style": {"color": "#111111"}
                                    }
                                ]
                            }
                        }
                    }
                }]
            }]
        })
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-timeline-snapshot-embedded-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(Path::new(&dir))?;
        Ok(dir)
    }
}
