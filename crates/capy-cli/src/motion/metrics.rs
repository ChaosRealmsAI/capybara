use std::fs;
use std::path::{Path, PathBuf};

use image::{GenericImageView, Rgba, RgbaImage, imageops};
use serde_json::{Value, json};

use super::model::{BBox, FrameStat, PackagePaths, QA_SCHEMA, rel_path};

const ALPHA_VISIBLE: u8 = 8;
const ALPHA_EDGE_MAX: u8 = 247;
const CELL_PADDING: u32 = 24;

pub(super) struct MetricsOutput {
    pub report: Value,
    pub warnings: Vec<String>,
}

pub(super) fn analyze_and_build(paths: &PackagePaths, fps: f64) -> Result<MetricsOutput, String> {
    let frames = collect_pngs(&paths.rgba_frames_dir)?;
    if frames.is_empty() {
        return Err(format!(
            "{} has no RGBA frames",
            paths.rgba_frames_dir.display()
        ));
    }
    let mut stats = Vec::with_capacity(frames.len());
    for (index, rgba_path) in frames.iter().enumerate() {
        let mask_path = paths.masks_dir.join(frame_name(index + 1));
        stats.push(frame_stat(index + 1, rgba_path, &mask_path)?);
    }
    let atlas = write_atlas(paths, &stats, fps)?;
    write_contact_sheet(paths, &stats)?;
    let (metrics, warnings, notes, verdict) = quality_metrics(&stats);
    let report = json!({
        "schema": QA_SCHEMA,
        "verdict": verdict,
        "frame_count": stats.len(),
        "metrics": metrics,
        "warnings": warnings,
        "notes": notes,
        "atlas": atlas,
        "previews": {
            "contact_deep": "qa/contact-deep.png",
            "preview_html": "qa/preview.html"
        }
    });
    write_json(&paths.qa_dir.join("report.json"), &report)?;
    Ok(MetricsOutput { report, warnings })
}

fn collect_pngs(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = fs::read_dir(dir)
        .map_err(|err| format!("read {} failed: {err}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn frame_stat(index: usize, rgba_path: &Path, mask_path: &Path) -> Result<FrameStat, String> {
    let image = image::open(rgba_path)
        .map_err(|err| format!("open {} failed: {err}", rgba_path.display()))?
        .to_rgba8();
    let (bbox, visible, edge) = alpha_bbox_and_counts(&image);
    let pixels = (image.width() * image.height()).max(1) as f64;
    Ok(FrameStat {
        index,
        rgba_path: rgba_path.to_path_buf(),
        mask_path: mask_path.to_path_buf(),
        bbox,
        nontransparent_ratio: visible as f64 / pixels,
        edge_ratio: edge as f64 / pixels,
    })
}

fn alpha_bbox_and_counts(image: &RgbaImage) -> (BBox, u64, u64) {
    let mut min_x = image.width();
    let mut min_y = image.height();
    let mut max_x = 0;
    let mut max_y = 0;
    let mut visible = 0_u64;
    let mut edge = 0_u64;
    for (x, y, pixel) in image.enumerate_pixels() {
        let alpha = pixel[3];
        if alpha > ALPHA_VISIBLE {
            visible += 1;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            if alpha < ALPHA_EDGE_MAX {
                edge += 1;
            }
        }
    }
    if visible == 0 {
        return (
            BBox {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            visible,
            edge,
        );
    }
    (
        BBox {
            x: min_x,
            y: min_y,
            width: max_x.saturating_sub(min_x).saturating_add(1),
            height: max_y.saturating_sub(min_y).saturating_add(1),
        },
        visible,
        edge,
    )
}

fn write_atlas(paths: &PackagePaths, stats: &[FrameStat], fps: f64) -> Result<Value, String> {
    let cell_w = stats
        .iter()
        .map(|frame| frame.bbox.width)
        .max()
        .unwrap_or(1)
        .saturating_add(CELL_PADDING * 2);
    let cell_h = stats
        .iter()
        .map(|frame| frame.bbox.height)
        .max()
        .unwrap_or(1)
        .saturating_add(CELL_PADDING * 2);
    let cols = (stats.len() as u32).clamp(1, 12);
    let rows = ((stats.len() as u32).saturating_add(cols - 1) / cols).max(1);
    let mut atlas = RgbaImage::from_pixel(cols * cell_w, rows * cell_h, Rgba([0, 0, 0, 0]));
    let mut frame_json = Vec::with_capacity(stats.len());
    for (offset, stat) in stats.iter().enumerate() {
        let img = image::open(&stat.rgba_path)
            .map_err(|err| format!("open {} failed: {err}", stat.rgba_path.display()))?
            .to_rgba8();
        let crop = img.view(stat.bbox.x, stat.bbox.y, stat.bbox.width, stat.bbox.height);
        let col = offset as u32 % cols;
        let row = offset as u32 / cols;
        let draw_x = col * cell_w + (cell_w.saturating_sub(stat.bbox.width) / 2);
        let draw_y = row * cell_h + cell_h.saturating_sub(CELL_PADDING + stat.bbox.height);
        imageops::overlay(
            &mut atlas,
            &crop.to_image(),
            i64::from(draw_x),
            i64::from(draw_y),
        );
        let cropped_path = paths.cropped_frames_dir.join(frame_name(stat.index));
        save_image(&cropped_path, &crop.to_image())?;
        frame_json.push(json!({
            "index": stat.index,
            "x": col * cell_w,
            "y": row * cell_h,
            "w": cell_w,
            "h": cell_h,
            "duration_ms": (1000.0 / fps.max(1.0)).round() as u32,
            "anchor": { "x": cell_w / 2, "y": cell_h - CELL_PADDING },
            "source_bbox": {
                "x": stat.bbox.x,
                "y": stat.bbox.y,
                "w": stat.bbox.width,
                "h": stat.bbox.height
            },
            "rgba": rel_path(&paths.root, &stat.rgba_path),
            "mask": rel_path(&paths.root, &stat.mask_path),
            "cropped": rel_path(&paths.root, &cropped_path)
        }));
    }
    let atlas_path = paths.atlas_dir.join("walk.png");
    save_image(&atlas_path, &atlas)?;
    let atlas_json = json!({
        "schema": "capy.motion_asset.atlas.v1",
        "image": "walk.png",
        "frame_count": stats.len(),
        "cell": { "w": cell_w, "h": cell_h },
        "layout": { "cols": cols, "rows": rows },
        "frames": frame_json
    });
    write_json(&paths.atlas_dir.join("walk.json"), &atlas_json)?;
    Ok(atlas_json)
}

fn write_contact_sheet(paths: &PackagePaths, stats: &[FrameStat]) -> Result<(), String> {
    let samples = sample_indices(stats.len(), 8);
    let thumb_w = 220;
    let thumb_h = 180;
    let mut sheet = RgbaImage::from_pixel(
        thumb_w * samples.len() as u32,
        thumb_h,
        Rgba([3, 11, 31, 255]),
    );
    for (slot, index) in samples.iter().enumerate() {
        let img = image::open(&stats[*index].rgba_path)
            .map_err(|err| format!("open {} failed: {err}", stats[*index].rgba_path.display()))?
            .to_rgba8();
        let thumb = imageops::resize(&img, thumb_w, thumb_h, imageops::FilterType::Lanczos3);
        imageops::overlay(&mut sheet, &thumb, i64::from(slot as u32 * thumb_w), 0);
    }
    save_image(&paths.qa_dir.join("contact-deep.png"), &sheet)
}

fn sample_indices(len: usize, count: usize) -> Vec<usize> {
    if len <= count {
        return (0..len).collect();
    }
    (0..count)
        .map(|i| ((i as f64) * ((len - 1) as f64) / ((count - 1) as f64)).round() as usize)
        .collect()
}

fn quality_metrics(stats: &[FrameStat]) -> (Value, Vec<String>, Vec<String>, &'static str) {
    let widths = stats
        .iter()
        .map(|frame| frame.bbox.width as f64)
        .collect::<Vec<_>>();
    let heights = stats
        .iter()
        .map(|frame| frame.bbox.height as f64)
        .collect::<Vec<_>>();
    let baselines = stats
        .iter()
        .map(|frame| frame.bbox.bottom() as f64)
        .collect::<Vec<_>>();
    let coverage = stats
        .iter()
        .map(|frame| frame.nontransparent_ratio)
        .collect::<Vec<_>>();
    let edges = stats
        .iter()
        .map(|frame| frame.edge_ratio)
        .collect::<Vec<_>>();
    let width_jitter = normalized_range(&widths);
    let height_jitter = normalized_range(&heights);
    let baseline_drift_px = range(&baselines).round() as u32;
    let alpha_coverage_delta = range(&coverage);
    let max_edge_delta = adjacent_max_delta(&edges);
    let mut warnings = Vec::new();
    let mut notes = Vec::new();
    if width_jitter > 0.45 {
        notes.push(format!(
            "travel-through crop width variation is high: {width_jitter:.3}; atlas cells keep a fixed frame box and anchor"
        ));
    }
    if height_jitter > 0.35 {
        warnings.push(format!("crop height jitter is high: {height_jitter:.3}"));
    }
    if baseline_drift_px > 90 {
        warnings.push(format!(
            "foot baseline drift is high: {baseline_drift_px}px"
        ));
    }
    if alpha_coverage_delta > 0.09 {
        warnings.push(format!(
            "alpha coverage delta is high: {alpha_coverage_delta:.3}"
        ));
    }
    if max_edge_delta > 0.025 {
        warnings.push(format!("edge shimmer proxy is high: {max_edge_delta:.3}"));
    }
    let verdict = if warnings.is_empty() {
        "app_ready"
    } else {
        "draft"
    };
    (
        json!({
            "width_jitter": round4(width_jitter),
            "height_jitter": round4(height_jitter),
            "foot_baseline_drift_px": baseline_drift_px,
            "alpha_coverage_delta": round4(alpha_coverage_delta),
            "max_edge_ratio_delta": round4(max_edge_delta),
            "mean_alpha_coverage": round4(mean(&coverage)),
            "mean_edge_ratio": round4(mean(&edges))
        }),
        warnings,
        notes,
        verdict,
    )
}

fn normalized_range(values: &[f64]) -> f64 {
    let avg = mean(values);
    if avg <= 0.0 { 0.0 } else { range(values) / avg }
}

fn range(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    max - min
}

fn adjacent_max_delta(values: &[f64]) -> f64 {
    values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f64::max)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn frame_name(index: usize) -> String {
    format!("frame_{index:06}.png")
}

fn save_image(path: &Path, image: &RgbaImage) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    image
        .save(path)
        .map_err(|err| format!("save {} failed: {err}", path.display()))
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    fs::write(path, format!("{text}\n"))
        .map_err(|err| format!("write {} failed: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_metrics_flags_large_baseline_drift() {
        let frames = vec![
            stat(
                1,
                BBox {
                    x: 0,
                    y: 0,
                    width: 10,
                    height: 20,
                },
            ),
            stat(
                2,
                BBox {
                    x: 0,
                    y: 100,
                    width: 10,
                    height: 20,
                },
            ),
        ];
        let (_metrics, warnings, _notes, verdict) = quality_metrics(&frames);
        assert_eq!(verdict, "draft");
        assert!(warnings.iter().any(|warning| warning.contains("baseline")));
    }

    #[test]
    fn quality_metrics_keeps_travel_width_variation_as_note() {
        let frames = vec![
            stat(
                1,
                BBox {
                    x: 0,
                    y: 0,
                    width: 40,
                    height: 100,
                },
            ),
            stat(
                2,
                BBox {
                    x: 20,
                    y: 0,
                    width: 120,
                    height: 100,
                },
            ),
        ];
        let (_metrics, warnings, notes, verdict) = quality_metrics(&frames);
        assert_eq!(verdict, "app_ready");
        assert!(warnings.is_empty());
        assert!(notes.iter().any(|note| note.contains("travel-through")));
    }

    fn stat(index: usize, bbox: BBox) -> FrameStat {
        FrameStat {
            index,
            rgba_path: PathBuf::from(format!("frame_{index}.png")),
            mask_path: PathBuf::from(format!("mask_{index}.png")),
            bbox,
            nontransparent_ratio: 0.1,
            edge_ratio: 0.01,
        }
    }
}
