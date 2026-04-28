use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Instant;

use image::{ImageReader, Rgba, RgbaImage};
use serde::Serialize;

const DEFAULT_TOLERANCE: u16 = 30;
const DEFAULT_FEATHER_RADIUS: u32 = 2;
const DEFAULT_MIN_COMPONENT_AREA: usize = 64;
const DEFAULT_HOLE_MIN_AREA: usize = 96;
const BACKGROUND_CANDIDATES: [Rgb; 3] = [
    Rgb::new(224, 224, 224),
    Rgb::new(242, 242, 242),
    Rgb::new(255, 255, 255),
];

#[derive(Clone, Debug)]
pub struct CutoutRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub background: String,
    pub tolerance: u16,
    pub feather_radius: u32,
    pub min_component_area: usize,
    pub hole_min_area: usize,
    pub qa_dir: Option<PathBuf>,
    pub report: Option<PathBuf>,
}

impl CutoutRequest {
    pub fn normalized(mut self) -> Self {
        if self.tolerance == 0 {
            self.tolerance = DEFAULT_TOLERANCE;
        }
        if self.feather_radius == 0 {
            self.feather_radius = DEFAULT_FEATHER_RADIUS;
        }
        if self.min_component_area == 0 {
            self.min_component_area = DEFAULT_MIN_COMPONENT_AREA;
        }
        if self.hole_min_area == 0 {
            self.hole_min_area = DEFAULT_HOLE_MIN_AREA;
        }
        self
    }
}

#[derive(Debug, Serialize)]
pub struct CutoutReport {
    pub input: String,
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub background_source: String,
    pub tolerance: u16,
    pub feather_radius: u32,
    pub min_component_area: usize,
    pub hole_min_area: usize,
    pub alpha: AlphaStats,
    pub performance: PerformanceStats,
    pub qa: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AlphaStats {
    pub transparent_pixels: usize,
    pub edge_pixels: usize,
    pub opaque_pixels: usize,
    pub nontransparent_ratio: f64,
    pub edge_ratio: f64,
    pub has_alpha: bool,
}

#[derive(Debug, Serialize)]
pub struct PerformanceStats {
    pub elapsed_ms: u128,
    pub megapixels_per_second: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

#[derive(Clone, Copy, Debug)]
enum BackgroundChoice {
    Auto,
    Fixed(Rgb),
}

#[derive(Debug)]
struct SelectedBackground {
    rgb: Rgb,
    source: String,
}

#[derive(Debug)]
struct CutoutResult {
    image: RgbaImage,
    background: SelectedBackground,
}

pub fn execute(request: CutoutRequest) -> Result<CutoutReport, String> {
    let request = request.normalized();
    let started = Instant::now();
    let input = ImageReader::open(&request.input)
        .map_err(|err| format!("open input failed: {}: {err}", request.input.display()))?
        .decode()
        .map_err(|err| format!("decode input failed: {}: {err}", request.input.display()))?
        .to_rgba8();
    let choice = parse_background(&request.background)?;
    let width = input.width();
    let height = input.height();
    let result = cutout_image(
        &input,
        choice,
        request.tolerance,
        request.feather_radius,
        request.min_component_area,
        request.hole_min_area,
    )?;

    if let Some(parent) = request
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create output directory failed: {}: {err}",
                parent.display()
            )
        })?;
    }
    result
        .image
        .save(&request.output)
        .map_err(|err| format!("save output failed: {}: {err}", request.output.display()))?;

    let qa = if let Some(dir) = &request.qa_dir {
        write_qa_previews(&result.image, dir)?
    } else {
        Vec::new()
    };

    let elapsed_ms = started.elapsed().as_millis();
    let alpha = alpha_stats(&result.image);
    let pixels = f64::from(width) * f64::from(height);
    let seconds = (elapsed_ms.max(1) as f64) / 1000.0;
    let report = CutoutReport {
        input: request.input.display().to_string(),
        output: request.output.display().to_string(),
        width,
        height,
        background: result.background.rgb.hex(),
        background_source: result.background.source,
        tolerance: request.tolerance,
        feather_radius: request.feather_radius,
        min_component_area: request.min_component_area,
        hole_min_area: request.hole_min_area,
        alpha,
        performance: PerformanceStats {
            elapsed_ms,
            megapixels_per_second: (pixels / 1_000_000.0) / seconds,
        },
        qa,
    };

    if let Some(path) = &request.report {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "create report directory failed: {}: {err}",
                    parent.display()
                )
            })?;
        }
        let json = serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?;
        std::fs::write(path, json)
            .map_err(|err| format!("write report failed: {}: {err}", path.display()))?;
    }

    Ok(report)
}

fn parse_background(value: &str) -> Result<BackgroundChoice, String> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("auto") {
        return Ok(BackgroundChoice::Auto);
    }
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() != 6 {
        return Err("background must be auto or a hex color like #E0E0E0".to_string());
    }
    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| "background has invalid red channel".to_string())?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| "background has invalid green channel".to_string())?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| "background has invalid blue channel".to_string())?;
    Ok(BackgroundChoice::Fixed(Rgb::new(r, g, b)))
}

fn cutout_image(
    input: &RgbaImage,
    choice: BackgroundChoice,
    tolerance: u16,
    feather_radius: u32,
    min_component_area: usize,
    hole_min_area: usize,
) -> Result<CutoutResult, String> {
    let width = input.width() as usize;
    let height = input.height() as usize;
    if width == 0 || height == 0 {
        return Err("input image is empty".to_string());
    }
    let background = select_background(input, choice, tolerance);
    let background_like = classify_background(input, background.rgb, tolerance);
    let mut background_mask = flood_background(&background_like, input);
    add_large_holes(
        &background_like,
        &mut background_mask,
        width,
        height,
        hole_min_area,
    );
    let subject_mask =
        keep_subject_components(&background_mask, input, width, height, min_component_area);
    let mut alpha = subject_mask
        .iter()
        .map(|keep| if *keep { 255 } else { 0 })
        .collect::<Vec<u8>>();
    alpha = box_blur_alpha(&alpha, width, height, feather_radius);

    let mut output = RgbaImage::new(input.width(), input.height());
    for (idx, pixel) in input.pixels().enumerate() {
        let source_alpha = pixel[3];
        let final_alpha = if source_alpha == 255 {
            alpha[idx]
        } else {
            ((u16::from(alpha[idx]) * u16::from(source_alpha)) / 255) as u8
        };
        let rgb = if final_alpha == 0 {
            [0, 0, 0]
        } else {
            decontaminate_rgb([pixel[0], pixel[1], pixel[2]], background.rgb, final_alpha)
        };
        output.put_pixel(
            (idx % width) as u32,
            (idx / width) as u32,
            Rgba([rgb[0], rgb[1], rgb[2], final_alpha]),
        );
    }

    Ok(CutoutResult {
        image: output,
        background,
    })
}

fn select_background(
    input: &RgbaImage,
    choice: BackgroundChoice,
    tolerance: u16,
) -> SelectedBackground {
    match choice {
        BackgroundChoice::Fixed(rgb) => SelectedBackground {
            rgb,
            source: "fixed".to_string(),
        },
        BackgroundChoice::Auto => {
            let samples = border_samples(input);
            if samples.is_empty() {
                return SelectedBackground {
                    rgb: BACKGROUND_CANDIDATES[0],
                    source: "auto-default-transparent-border".to_string(),
                };
            }
            let mut best = BACKGROUND_CANDIDATES[0];
            let mut best_score = f64::MAX;
            let tol_sq = u32::from(tolerance) * u32::from(tolerance) * 3;
            let mut best_matches = 0usize;
            for candidate in BACKGROUND_CANDIDATES {
                let mut score = 0u64;
                let mut matches = 0usize;
                for sample in &samples {
                    let dist = color_dist_sq(*sample, candidate);
                    score += u64::from(dist);
                    if dist <= tol_sq {
                        matches += 1;
                    }
                }
                let avg = score as f64 / samples.len() as f64;
                if avg < best_score {
                    best_score = avg;
                    best = candidate;
                    best_matches = matches;
                }
            }
            SelectedBackground {
                rgb: best,
                source: format!(
                    "auto-border-samples:{}-matches:{}",
                    samples.len(),
                    best_matches
                ),
            }
        }
    }
}

fn border_samples(input: &RgbaImage) -> Vec<Rgb> {
    let width = input.width();
    let height = input.height();
    let mut samples =
        Vec::with_capacity((width.saturating_mul(2) + height.saturating_mul(2)) as usize);
    for x in 0..width {
        push_sample(input.get_pixel(x, 0), &mut samples);
        if height > 1 {
            push_sample(input.get_pixel(x, height - 1), &mut samples);
        }
    }
    for y in 1..height.saturating_sub(1) {
        push_sample(input.get_pixel(0, y), &mut samples);
        if width > 1 {
            push_sample(input.get_pixel(width - 1, y), &mut samples);
        }
    }
    samples
}

fn push_sample(pixel: &Rgba<u8>, samples: &mut Vec<Rgb>) {
    if pixel[3] > 16 {
        samples.push(Rgb::new(pixel[0], pixel[1], pixel[2]));
    }
}

fn classify_background(input: &RgbaImage, background: Rgb, tolerance: u16) -> Vec<bool> {
    let tol_sq = u32::from(tolerance) * u32::from(tolerance) * 3;
    input
        .pixels()
        .map(|pixel| {
            pixel[3] <= 8
                || color_dist_sq(Rgb::new(pixel[0], pixel[1], pixel[2]), background) <= tol_sq
        })
        .collect()
}

fn flood_background(background_like: &[bool], input: &RgbaImage) -> Vec<bool> {
    let width = input.width() as usize;
    let height = input.height() as usize;
    let mut mask = vec![false; background_like.len()];
    let mut queue = VecDeque::new();
    for x in 0..width {
        push_background_seed(x, 0, width, background_like, &mut mask, &mut queue);
        push_background_seed(x, height - 1, width, background_like, &mut mask, &mut queue);
    }
    for y in 0..height {
        push_background_seed(0, y, width, background_like, &mut mask, &mut queue);
        push_background_seed(width - 1, y, width, background_like, &mut mask, &mut queue);
    }
    while let Some(idx) = queue.pop_front() {
        for neighbor in neighbors(idx, width, height) {
            if background_like[neighbor] && !mask[neighbor] {
                mask[neighbor] = true;
                queue.push_back(neighbor);
            }
        }
    }
    mask
}

fn push_background_seed(
    x: usize,
    y: usize,
    width: usize,
    background_like: &[bool],
    mask: &mut [bool],
    queue: &mut VecDeque<usize>,
) {
    let idx = y * width + x;
    if background_like[idx] && !mask[idx] {
        mask[idx] = true;
        queue.push_back(idx);
    }
}

fn add_large_holes(
    background_like: &[bool],
    background_mask: &mut [bool],
    width: usize,
    height: usize,
    hole_min_area: usize,
) {
    let mut visited = background_mask.to_vec();
    for idx in 0..background_like.len() {
        if !background_like[idx] || visited[idx] {
            continue;
        }
        let component = collect_component(
            idx,
            width,
            height,
            |candidate| background_like[candidate],
            &mut visited,
        );
        if component.len() >= hole_min_area {
            for pixel in component {
                background_mask[pixel] = true;
            }
        }
    }
}

fn keep_subject_components(
    background_mask: &[bool],
    input: &RgbaImage,
    width: usize,
    height: usize,
    min_component_area: usize,
) -> Vec<bool> {
    let source_alpha = input.pixels().map(|pixel| pixel[3]).collect::<Vec<u8>>();
    let mut keep = vec![false; background_mask.len()];
    let mut visited = vec![false; background_mask.len()];
    for idx in 0..background_mask.len() {
        if background_mask[idx] || source_alpha[idx] <= 8 || visited[idx] {
            continue;
        }
        let component = collect_component(
            idx,
            width,
            height,
            |candidate| !background_mask[candidate] && source_alpha[candidate] > 8,
            &mut visited,
        );
        if component.len() >= min_component_area {
            for pixel in component {
                keep[pixel] = true;
            }
        }
    }
    keep
}

fn collect_component<F>(
    seed: usize,
    width: usize,
    height: usize,
    mut accepts: F,
    visited: &mut [bool],
) -> Vec<usize>
where
    F: FnMut(usize) -> bool,
{
    let mut component = Vec::new();
    let mut queue = VecDeque::from([seed]);
    visited[seed] = true;
    while let Some(idx) = queue.pop_front() {
        component.push(idx);
        for neighbor in neighbors(idx, width, height) {
            if !visited[neighbor] && accepts(neighbor) {
                visited[neighbor] = true;
                queue.push_back(neighbor);
            }
        }
    }
    component
}

fn neighbors(idx: usize, width: usize, height: usize) -> impl Iterator<Item = usize> {
    let x = idx % width;
    let y = idx / width;
    let mut values = [usize::MAX; 4];
    let mut count = 0;
    if x > 0 {
        values[count] = idx - 1;
        count += 1;
    }
    if x + 1 < width {
        values[count] = idx + 1;
        count += 1;
    }
    if y > 0 {
        values[count] = idx - width;
        count += 1;
    }
    if y + 1 < height {
        values[count] = idx + width;
        count += 1;
    }
    values.into_iter().take(count)
}

fn box_blur_alpha(alpha: &[u8], width: usize, height: usize, radius: u32) -> Vec<u8> {
    if radius == 0 {
        return alpha.to_vec();
    }
    let radius = radius as usize;
    let mut horizontal = vec![0u8; alpha.len()];
    for y in 0..height {
        for x in 0..width {
            let start = x.saturating_sub(radius);
            let end = (x + radius).min(width - 1);
            let mut sum = 0u32;
            for sx in start..=end {
                sum += u32::from(alpha[y * width + sx]);
            }
            horizontal[y * width + x] = (sum / (end - start + 1) as u32) as u8;
        }
    }
    let mut output = vec![0u8; alpha.len()];
    for y in 0..height {
        let start_y = y.saturating_sub(radius);
        let end_y = (y + radius).min(height - 1);
        for x in 0..width {
            let mut sum = 0u32;
            for sy in start_y..=end_y {
                sum += u32::from(horizontal[sy * width + x]);
            }
            output[y * width + x] = (sum / (end_y - start_y + 1) as u32) as u8;
        }
    }
    output
}

fn decontaminate_rgb(rgb: [u8; 3], background: Rgb, alpha: u8) -> [u8; 3] {
    if alpha >= 250 {
        return rgb;
    }
    let alpha = f32::from(alpha).max(1.0) / 255.0;
    [
        decontaminate_channel(rgb[0], background.r, alpha),
        decontaminate_channel(rgb[1], background.g, alpha),
        decontaminate_channel(rgb[2], background.b, alpha),
    ]
}

fn decontaminate_channel(value: u8, background: u8, alpha: f32) -> u8 {
    let foreground = (f32::from(value) - f32::from(background) * (1.0 - alpha)) / alpha;
    foreground.round().clamp(0.0, 255.0) as u8
}

fn color_dist_sq(a: Rgb, b: Rgb) -> u32 {
    let dr = i32::from(a.r) - i32::from(b.r);
    let dg = i32::from(a.g) - i32::from(b.g);
    let db = i32::from(a.b) - i32::from(b.b);
    (dr * dr + dg * dg + db * db) as u32
}

fn alpha_stats(image: &RgbaImage) -> AlphaStats {
    let mut transparent = 0usize;
    let mut edge = 0usize;
    let mut opaque = 0usize;
    for pixel in image.pixels() {
        match pixel[3] {
            0..=8 => transparent += 1,
            247..=255 => opaque += 1,
            _ => edge += 1,
        }
    }
    let total = image.width() as usize * image.height() as usize;
    AlphaStats {
        transparent_pixels: transparent,
        edge_pixels: edge,
        opaque_pixels: opaque,
        nontransparent_ratio: (edge + opaque) as f64 / total as f64,
        edge_ratio: edge as f64 / total as f64,
        has_alpha: transparent > 0 || edge > 0,
    }
}

fn write_qa_previews(image: &RgbaImage, out_dir: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|err| format!("create QA directory failed: {}: {err}", out_dir.display()))?;
    let backgrounds = [
        ("qa-black.png", Rgb::new(0, 0, 0)),
        ("qa-white.png", Rgb::new(255, 255, 255)),
        ("qa-deep.png", Rgb::new(3, 11, 31)),
    ];
    let mut paths = Vec::new();
    for (name, background) in backgrounds {
        let mut preview = RgbaImage::new(image.width(), image.height());
        for (x, y, pixel) in image.enumerate_pixels() {
            let alpha = u16::from(pixel[3]);
            let inv = 255 - alpha;
            let r = (u16::from(pixel[0]) * alpha + u16::from(background.r) * inv + 127) / 255;
            let g = (u16::from(pixel[1]) * alpha + u16::from(background.g) * inv + 127) / 255;
            let b = (u16::from(pixel[2]) * alpha + u16::from(background.b) * inv + 127) / 255;
            preview.put_pixel(x, y, Rgba([r as u8, g as u8, b as u8, 255]));
        }
        let path = out_dir.join(name);
        preview
            .save(&path)
            .map_err(|err| format!("save QA preview failed: {}: {err}", path.display()))?;
        paths.push(path.display().to_string());
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_fixed_grey_backdrop() -> Result<(), String> {
        let img = synthetic_rect(Rgb::new(224, 224, 224), Rgb::new(180, 40, 80));
        let result = cutout_image(
            &img,
            BackgroundChoice::Auto,
            30,
            2,
            DEFAULT_MIN_COMPONENT_AREA,
            DEFAULT_HOLE_MIN_AREA,
        )?;
        assert_eq!(result.background.rgb, Rgb::new(224, 224, 224));
        assert!(result.image.get_pixel(0, 0)[3] <= 8);
        assert!(result.image.get_pixel(32, 32)[3] >= 247);
        Ok(())
    }

    #[test]
    fn selects_white_backdrop() -> Result<(), String> {
        let img = synthetic_rect(Rgb::new(255, 255, 255), Rgb::new(30, 80, 170));
        let result = cutout_image(
            &img,
            BackgroundChoice::Auto,
            24,
            2,
            DEFAULT_MIN_COMPONENT_AREA,
            DEFAULT_HOLE_MIN_AREA,
        )?;
        assert_eq!(result.background.rgb, Rgb::new(255, 255, 255));
        assert!(result.image.get_pixel(4, 4)[3] <= 8);
        assert!(result.image.get_pixel(32, 32)[3] >= 247);
        Ok(())
    }

    #[test]
    fn cuts_large_interior_holes() -> Result<(), String> {
        let bg = Rgb::new(224, 224, 224);
        let mut img = RgbaImage::from_pixel(72, 72, Rgba([bg.r, bg.g, bg.b, 255]));
        for y in 12..60 {
            for x in 12..60 {
                img.put_pixel(x, y, Rgba([50, 140, 210, 255]));
            }
        }
        for y in 28..44 {
            for x in 28..44 {
                img.put_pixel(x, y, Rgba([bg.r, bg.g, bg.b, 255]));
            }
        }
        let result = cutout_image(
            &img,
            BackgroundChoice::Fixed(bg),
            30,
            1,
            DEFAULT_MIN_COMPONENT_AREA,
            64,
        )?;
        assert!(result.image.get_pixel(36, 36)[3] <= 8);
        assert!(result.image.get_pixel(18, 18)[3] >= 247);
        Ok(())
    }

    #[test]
    fn preserves_existing_alpha() -> Result<(), String> {
        let bg = Rgb::new(224, 224, 224);
        let mut img = synthetic_rect(bg, Rgb::new(220, 100, 40));
        img.put_pixel(32, 32, Rgba([220, 100, 40, 128]));
        let result = cutout_image(
            &img,
            BackgroundChoice::Fixed(bg),
            30,
            0,
            DEFAULT_MIN_COMPONENT_AREA,
            DEFAULT_HOLE_MIN_AREA,
        )?;
        assert_eq!(result.image.get_pixel(32, 32)[3], 128);
        Ok(())
    }

    fn synthetic_rect(background: Rgb, subject: Rgb) -> RgbaImage {
        let mut img = RgbaImage::from_pixel(
            64,
            64,
            Rgba([background.r, background.g, background.b, 255]),
        );
        for y in 16..48 {
            for x in 14..50 {
                img.put_pixel(x, y, Rgba([subject.r, subject.g, subject.b, 255]));
            }
        }
        img
    }
}
