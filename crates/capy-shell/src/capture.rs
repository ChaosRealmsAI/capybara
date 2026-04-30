use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};

const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureResult {
    pub out: PathBuf,
    pub bytes: u64,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub viewport_width: f64,
    pub viewport_height: f64,
    pub dpr: f64,
}

pub fn capture_rgba_image(
    out: &Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<CaptureResult, String> {
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create capture directory failed: {err}"))?;
    }
    let expected_len = width as usize * height as usize * 4;
    if rgba.len() != expected_len {
        return Err(format!(
            "invalid RGBA capture buffer: got {} bytes, expected {expected_len}",
            rgba.len()
        ));
    }
    let image =
        image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba.to_vec())
            .ok_or_else(|| "create RGBA capture image failed".to_string())?;
    image
        .save(out)
        .map_err(|err| format!("write capture PNG failed for {}: {err}", out.display()))?;
    capture_result_from_file(out, width as usize, height as usize)
}

pub fn capture_rgba_region(
    out: &Path,
    source_width: u32,
    source_height: u32,
    rgba: &[u8],
    region: CaptureRegion,
) -> Result<CaptureResult, String> {
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create screenshot directory failed: {err}"))?;
    }
    let expected_len = source_width as usize * source_height as usize * 4;
    if rgba.len() != expected_len {
        return Err(format!(
            "invalid RGBA screenshot buffer: got {} bytes, expected {expected_len}",
            rgba.len()
        ));
    }
    let image = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(
        source_width,
        source_height,
        rgba.to_vec(),
    )
    .ok_or_else(|| "create RGBA screenshot image failed".to_string())?;
    let (x, y, width, height) =
        scaled_region_bounds(region, source_width as usize, source_height as usize)?;
    let cropped = image::DynamicImage::ImageRgba8(image).crop_imm(x, y, width, height);
    cropped.save(out).map_err(|err| {
        format!(
            "write cropped screenshot failed for {}: {err}",
            out.display()
        )
    })?;
    capture_result_from_file(out, width as usize, height as usize)
}

pub fn capture_png_data_url(
    out: &Path,
    data_url: &str,
    width: usize,
    height: usize,
) -> Result<CaptureResult, String> {
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create capture directory failed: {err}"))?;
    }
    let Some(encoded) = data_url.strip_prefix("data:image/png;base64,") else {
        return Err("app-view capture did not return a PNG data URL".to_string());
    };
    let png = STANDARD
        .decode(encoded)
        .map_err(|err| format!("decode app-view PNG data URL failed: {err}"))?;
    if png.len() < PNG_MAGIC.len() || &png[..PNG_MAGIC.len()] != PNG_MAGIC {
        return Err("app-view capture data URL is not PNG data".to_string());
    }
    std::fs::write(out, png)
        .map_err(|err| format!("write app-view capture failed for {}: {err}", out.display()))?;
    capture_result_from_file(out, width, height)
}

fn scaled_region_bounds(
    region: CaptureRegion,
    capture_width: usize,
    capture_height: usize,
) -> Result<(u32, u32, u32, u32), String> {
    let viewport_width = finite_positive(region.viewport_width, "viewport_width")?;
    let viewport_height = finite_positive(region.viewport_height, "viewport_height")?;
    let region_width = finite_positive(region.width, "width")?;
    let region_height = finite_positive(region.height, "height")?;
    let capture_width = capture_width.max(1) as f64;
    let capture_height = capture_height.max(1) as f64;
    let dpr = (region.dpr.is_finite() && region.dpr > 0.0).then_some(region.dpr);
    let scale_x = dpr.unwrap_or(capture_width / viewport_width);
    let scale_y = dpr.unwrap_or(capture_height / viewport_height);

    let x = (region.x.max(0.0) * scale_x)
        .floor()
        .clamp(0.0, capture_width - 1.0);
    let y = (region.y.max(0.0) * scale_y)
        .floor()
        .clamp(0.0, capture_height - 1.0);
    let right = ((region.x + region_width) * scale_x)
        .ceil()
        .clamp(x + 1.0, capture_width);
    let bottom = ((region.y + region_height) * scale_y)
        .ceil()
        .clamp(y + 1.0, capture_height);

    Ok((
        x as u32,
        y as u32,
        (right - x).max(1.0) as u32,
        (bottom - y).max(1.0) as u32,
    ))
}

fn finite_positive(value: f64, name: &str) -> Result<f64, String> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(format!("invalid screenshot region {name}: {value}"))
    }
}

fn validate_png_magic(out: &Path) -> Result<(), String> {
    let header = read_png_magic(out)?;
    if &header == PNG_MAGIC {
        Ok(())
    } else {
        Err(format!("capture did not produce a PNG: {}", out.display()))
    }
}

fn capture_result_from_file(
    out: &Path,
    width: usize,
    height: usize,
) -> Result<CaptureResult, String> {
    validate_png_magic(out)?;
    let bytes = std::fs::metadata(out)
        .map_err(|err| format!("read capture metadata failed for {}: {err}", out.display()))?
        .len();
    Ok(CaptureResult {
        out: out.to_path_buf(),
        bytes,
        width,
        height,
    })
}

fn read_png_magic(out: &Path) -> Result<[u8; 8], String> {
    use std::io::Read;

    let mut file = std::fs::File::open(out)
        .map_err(|err| format!("read capture header failed for {}: {err}", out.display()))?;
    let mut header = [0u8; 8];
    file.read_exact(&mut header)
        .map_err(|err| format!("read capture header failed for {}: {err}", out.display()))?;
    Ok(header)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{CaptureRegion, PNG_MAGIC, read_png_magic, scaled_region_bounds};

    #[test]
    fn capture_png_magic_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!(
            "capybara-capture-magic-{}-{nanos}.png",
            std::process::id()
        ));
        std::fs::write(&path, [PNG_MAGIC.as_slice(), b"capture"].concat())?;

        let header = read_png_magic(&path)?;

        std::fs::remove_file(path)?;
        assert_eq!(&header, PNG_MAGIC);
        Ok(())
    }

    #[test]
    fn capture_png_data_url_writes_png() -> Result<(), Box<dyn std::error::Error>> {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path = std::env::temp_dir().join(format!("capybara-data-url-{nanos}.png"));
        let data_url = "data:image/png;base64,iVBORw0KGgo=";

        let result = super::capture_png_data_url(&path, data_url, 1, 1)?;

        assert_eq!(result.width, 1);
        assert_eq!(result.height, 1);
        assert_eq!(std::fs::read(&path)?[..8], *b"\x89PNG\r\n\x1a\n");
        let _remove_result = std::fs::remove_file(path);
        Ok(())
    }

    #[test]
    fn scaled_region_bounds_maps_css_rect_to_capture_pixels() -> Result<(), String> {
        let region = CaptureRegion {
            x: 100.0,
            y: 50.0,
            width: 320.0,
            height: 180.0,
            viewport_width: 1440.0,
            viewport_height: 900.0,
            dpr: 2.0,
        };

        let bounds = scaled_region_bounds(region, 2880, 1800)?;

        assert_eq!(bounds, (200, 100, 640, 360));
        Ok(())
    }
}
