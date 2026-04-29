use futures_channel::oneshot;
use wasm_bindgen::prelude::*;
use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

use vello::wgpu;
use vello::{AaConfig, RenderParams, Scene};

use capy_canvas_core::render;

use super::{log, shared_ready, shared_state};

/// Build a `Blob` from `bytes` (typed `mime`), create an `<a>` with
/// `href = blob URL · download = filename`, click it, revoke URL.
/// Standard browser download dance.
pub(super) fn trigger_download(bytes: &[u8], mime: &str, filename: &str) -> Result<(), String> {
    // Wrap the bytes in a JS Uint8Array; Blob takes a sequence of typed arrays.
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array.buffer());

    let opts = BlobPropertyBag::new();
    opts.set_type(mime);

    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
        .map_err(|e| format!("Blob::new: {e:?}"))?;

    let url =
        Url::create_object_url_with_blob(&blob).map_err(|e| format!("create_object_url: {e:?}"))?;

    let win = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let document = win.document().ok_or_else(|| "no document".to_string())?;
    let anchor: HtmlAnchorElement = document
        .create_element("a")
        .map_err(|e| format!("create_element a: {e:?}"))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|_| "dyn_into HtmlAnchorElement".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();

    Url::revoke_object_url(&url).map_err(|e| format!("revoke_object_url: {e:?}"))?;
    Ok(())
}

/// Render the current AppState into an offscreen RGBA texture, copy it
/// back to a mappable buffer, encode PNG, and trigger a browser download.
/// Mirrors `capy_canvas_native::export::handle_export` but `await`s the
/// `map_async` callback (since `device.poll(Wait)` is a no-op on wasm)
/// and writes a `Blob` instead of a file.
pub(super) async fn perform_png_export() -> Result<(), String> {
    let state_arc = shared_state().ok_or_else(|| "no shared state".to_string())?;
    let ready_arc = shared_ready().ok_or_else(|| "no shared ready".to_string())?;

    // Build the scene + read render dims under the lock, then drop locks
    // BEFORE the await so other spawn_local tasks (input, idb) can run.
    let (scaled_scene, base_color, width, height) = {
        let ready_guard = ready_arc
            .lock()
            .map_err(|_| "ready lock poisoned".to_string())?;
        let r = ready_guard
            .as_ref()
            .ok_or_else(|| "surface not ready".to_string())?;
        let phys_w = r.surface.config.width.max(1);
        let phys_h = r.surface.config.height.max(1);
        drop(ready_guard);

        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio().max(1.0))
            .unwrap_or(1.0);
        let (scene, base) = match state_arc.lock() {
            Ok(state) => {
                let scene = render::build_scene(&state);
                let base = if state.dark_mode {
                    vello::peniko::Color::from_rgba8(0x1e, 0x1e, 0x2e, 0xff)
                } else {
                    vello::peniko::Color::from_rgba8(0xf8, 0xf7, 0xf4, 0xff)
                };
                (scene, base)
            }
            Err(_) => return Err("state lock poisoned".to_string()),
        };
        let mut scaled = Scene::new();
        scaled.append(&scene, Some(vello::kurbo::Affine::scale(dpr)));
        (scaled, base, phys_w, phys_h)
    };

    // Render-to-texture + copy_to_buffer happens under the ready lock.
    // Buffer + receiver come out so we can await without holding it.
    let (buffer, rx, bytes_per_row) = {
        let mut ready_guard = ready_arc
            .lock()
            .map_err(|_| "ready lock poisoned".to_string())?;
        let r = ready_guard
            .as_mut()
            .ok_or_else(|| "surface not ready".to_string())?;
        let dev = &r.render_ctx.devices[r.dev_id];

        let target_tex = dev.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("export-target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target_view = target_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let params = RenderParams {
            base_color,
            width,
            height,
            antialiasing_method: AaConfig::Msaa16,
        };
        r.renderer
            .render_to_texture(
                &dev.device,
                &dev.queue,
                &scaled_scene,
                &target_view,
                &params,
            )
            .map_err(|e| format!("render_to_texture: {e}"))?;

        let bytes_per_row = (width * 4 + 255) & !255;
        let buf = dev.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("export-readback"),
            size: (bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = dev
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        dev.queue.submit(Some(encoder.finish()));

        let (tx, rx) = oneshot::channel::<Result<(), wgpu::BufferAsyncError>>();
        buf.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        // device.poll(Wait) is a no-op on web; the browser drives the
        // queue and fires the callback when the GPU work completes.
        (buf, rx, bytes_per_row)
    };

    rx.await
        .map_err(|e| format!("map_async oneshot canceled: {e}"))?
        .map_err(|e| format!("map_async failed: {e}"))?;

    // Copy out, skipping per-row stride padding.
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    {
        let slice = buffer.slice(..);
        let data = slice.get_mapped_range();
        for y in 0..height {
            let start = (y * bytes_per_row) as usize;
            let end = start + (width * 4) as usize;
            pixels.extend_from_slice(&data[start..end]);
        }
        drop(data);
    }
    buffer.unmap();

    let img = image::RgbaImage::from_raw(width, height, pixels)
        .ok_or_else(|| "RgbaImage::from_raw: dimension mismatch".to_string())?;
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        encoder
            .write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgba8)
            .map_err(|e| format!("png encode: {e}"))?;
    }

    trigger_download(&png_bytes, "image/png", "canvas.png")?;
    log(&format!(
        "[capy-canvas-web] png exported ({width}x{height}, {} bytes)",
        png_bytes.len()
    ));
    Ok(())
}
