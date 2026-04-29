use std::ffi::c_void;
use std::ptr::NonNull;

use capy_shell_mac::{DesktopShell, IOSurfaceHandle, MacHeadlessShell};

use super::SnapshotError;

pub(super) const MAX_COMMIT_RETRIES: u32 = 60;
pub(super) const MAX_SAFE_SNAPSHOT_PIXELS: u64 = 16_777_216;
pub(super) const FALLBACK_WIDTH: u32 = 1920;
pub(super) const FALLBACK_HEIGHT: u32 = 1080;

pub(super) async fn sample_until_committed(
    shell: &MacHeadlessShell,
) -> Result<IOSurfaceHandle, SnapshotError> {
    const PAINT_WAIT: &str =
        "return await new Promise(resolve => setTimeout(() => resolve(true), 50));";

    let mut last: Option<IOSurfaceHandle> = None;
    let mut prev_center: Option<(u8, u8, u8, u8)> = None;
    let mut stable_count: u32 = 0;

    for _ in 0..MAX_COMMIT_RETRIES {
        shell
            .call_async(PAINT_WAIT)
            .await
            .map_err(|e| SnapshotError::JsCall(format!("pump paint: {e}")))?;

        let handle = shell
            .snapshot()
            .map_err(|e| SnapshotError::Shell(format!("{e}")))?;

        let center = read_center_pixel(&handle)?;
        let non_zero = center.0 != 0 || center.1 != 0 || center.2 != 0 || center.3 != 0;

        if non_zero && prev_center == Some(center) {
            stable_count += 1;
            if stable_count >= 1 {
                return Ok(handle);
            }
        } else {
            stable_count = 0;
        }

        prev_center = Some(center);
        last = Some(handle);
    }

    last.ok_or_else(|| SnapshotError::Shell("no snapshot attempts".into()))
}

fn read_center_pixel(handle: &IOSurfaceHandle) -> Result<(u8, u8, u8, u8), SnapshotError> {
    let surface = handle.as_iosurface();
    let width = handle.width as usize;
    let height = handle.height as usize;
    if width == 0 || height == 0 {
        return Ok((0, 0, 0, 0));
    }

    let mut seed: u32 = 0;
    let lock_status = unsafe {
        surface.lock(
            objc2_io_surface::IOSurfaceLockOptions::ReadOnly,
            &mut seed as *mut u32,
        )
    };
    if lock_status != 0 {
        return Err(SnapshotError::IoSurfaceLock(lock_status));
    }

    let base = surface.base_address();
    let bpr = surface.bytes_per_row();
    let cx = width / 2;
    let cy = height / 2;
    let px = unsafe {
        let ptr = (base.as_ptr() as *const u8).add(cy * bpr + cx * 4);
        (*ptr, *ptr.add(1), *ptr.add(2), *ptr.add(3))
    };

    let unlock_status = unsafe {
        surface.unlock(
            objc2_io_surface::IOSurfaceLockOptions::ReadOnly,
            &mut seed as *mut u32,
        )
    };
    if unlock_status != 0 {
        return Err(SnapshotError::IoSurfaceLock(unlock_status));
    }
    Ok(px)
}

pub(crate) fn iosurface_to_png(handle: &IOSurfaceHandle) -> Result<Vec<u8>, SnapshotError> {
    let surface = handle.as_iosurface();
    let width = handle.width as usize;
    let height = handle.height as usize;
    if width == 0 || height == 0 {
        return Err(SnapshotError::FrameReadyContract(format!(
            "IOSurface zero extent: {width}x{height}"
        )));
    }

    let mut seed: u32 = 0;
    let lock_status = unsafe {
        surface.lock(
            objc2_io_surface::IOSurfaceLockOptions::ReadOnly,
            &mut seed as *mut u32,
        )
    };
    if lock_status != 0 {
        return Err(SnapshotError::IoSurfaceLock(lock_status));
    }

    let base: NonNull<c_void> = surface.base_address();
    let bytes_per_row = surface.bytes_per_row();
    let row_bytes = width
        .checked_mul(4)
        .ok_or_else(|| SnapshotError::FrameReadyContract(format!("width overflow: {width}")))?;
    if bytes_per_row < row_bytes {
        let _ = unsafe {
            surface.unlock(
                objc2_io_surface::IOSurfaceLockOptions::ReadOnly,
                &mut seed as *mut u32,
            )
        };
        return Err(SnapshotError::FrameReadyContract(format!(
            "bytes_per_row ({bytes_per_row}) < width*4 ({row_bytes})"
        )));
    }

    let total = row_bytes.checked_mul(height).ok_or_else(|| {
        SnapshotError::FrameReadyContract(format!("raster overflow: {width}x{height}"))
    })?;
    let mut rgba: Vec<u8> = vec![0u8; total];
    let base_ptr = base.as_ptr() as *const u8;
    for y in 0..height {
        let src_row = unsafe { base_ptr.add(y * bytes_per_row) };
        let dst_row = &mut rgba[y * row_bytes..(y + 1) * row_bytes];
        for x in 0..width {
            let px = unsafe { src_row.add(x * 4) };
            let b = unsafe { *px };
            let g = unsafe { *px.add(1) };
            let r = unsafe { *px.add(2) };
            let a = unsafe { *px.add(3) };
            let dst_off = x * 4;
            dst_row[dst_off] = r;
            dst_row[dst_off + 1] = g;
            dst_row[dst_off + 2] = b;
            dst_row[dst_off + 3] = a;
        }
    }

    let unlock_status = unsafe {
        surface.unlock(
            objc2_io_surface::IOSurfaceLockOptions::ReadOnly,
            &mut seed as *mut u32,
        )
    };
    if unlock_status != 0 {
        return Err(SnapshotError::IoSurfaceLock(unlock_status));
    }

    let mut buf: Vec<u8> = Vec::with_capacity(total / 4);
    {
        let mut encoder = ::png::Encoder::new(&mut buf, handle.width, handle.height);
        encoder.set_color(::png::ColorType::Rgba);
        encoder.set_depth(::png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| SnapshotError::PngEncode(format!("header: {e}")))?;
        writer
            .write_image_data(&rgba)
            .map_err(|e| SnapshotError::PngEncode(format!("image data: {e}")))?;
        drop(writer);
    }

    Ok(buf)
}

pub(super) fn rgba_png_looks_black(png: &[u8]) -> bool {
    let decoder = ::png::Decoder::new(std::io::Cursor::new(png));
    let mut reader = match decoder.read_info() {
        Ok(reader) => reader,
        Err(_) => return false,
    };
    let out_size = reader.output_buffer_size();
    if out_size == 0 {
        return false;
    }
    let mut buf = vec![0u8; out_size];
    let info = match reader.next_frame(&mut buf) {
        Ok(info) => info,
        Err(_) => return false,
    };
    let bytes = &buf[..info.buffer_size()];
    if bytes.len() < 4 {
        return false;
    }

    let px_count = bytes.len() / 4;
    let stride = (px_count / 4096).max(1);
    let mut bright_samples = 0usize;
    let mut sampled = 0usize;
    let mut i = 0usize;
    while i + 3 < bytes.len() {
        let r = bytes[i] as u16;
        let g = bytes[i + 1] as u16;
        let b = bytes[i + 2] as u16;
        let a = bytes[i + 3] as u16;
        if a > 8 && (r + g + b) > 24 {
            bright_samples += 1;
        }
        sampled += 1;
        i += stride * 4;
    }

    sampled > 0 && bright_samples == 0
}
