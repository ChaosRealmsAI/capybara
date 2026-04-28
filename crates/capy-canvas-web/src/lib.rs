//! capy-canvas-web · winit-web event loop + canvas mount + IndexedDB persistence.
//!
//! v0.4 milestone: keyboard input + IndexedDB save/load. WebApp now handles
//! `KeyboardInput` and `ModifiersChanged` (mirroring native), and after each
//! redraw drains `state.pending_save_request` / `state.pending_load_request`
//! into IndexedDB. Cmd+S persists the canvas, page reload + Cmd+O restores it.
//!
//! Two paths exist for triggering save/load:
//! 1. Keyboard: Cmd+S / Cmd+O via `input::handle_key` set the pending flags;
//!    `RedrawRequested` drains them via `spawn_local`.
//! 2. JS-callable exports: `#[wasm_bindgen] save() / load()` skip the flag
//!    plumbing and do the IDB I/O directly. This exists because some browsers
//!    intercept Cmd+S as "Save Page As" before the canvas sees the keydown,
//!    and it's the path Playwright drives during verification.
//!
//! Lock discipline (the load path is the one that bites):
//! - save: lock → `to_json_string()` → drop → spawn_local IDB put
//! - load: spawn_local IDB get → re-lock → `load_from_json_str` → drop →
//!   request_redraw via the ready slot
//! - never `.await` while holding the `Mutex` (wasm32 is single-threaded but
//!   `std::sync::Mutex` will still happily deadlock if you try)
//!
//! State sharing: `thread_local!` stashes `Arc<Mutex<AppState>>` and the ready
//! slot. `start()` populates them; `save()` / `load()` exports read them.
//!
//! On non-wasm targets the entire body is `#[cfg]` away so the workspace
//! `cargo build` stays green.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::num::NonZeroUsize;
    use std::sync::{Arc, Mutex};

    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::spawn_local;

    use vello::util::{RenderContext, RenderSurface};
    use vello::wgpu;
    use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

    use winit::application::ApplicationHandler;
    use winit::event::{ElementState, Modifiers, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys};
    use winit::window::{Window, WindowAttributes, WindowId};

    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, HtmlCanvasElement, Url};

    use futures_channel::oneshot;

    use idb::{DatabaseEvent, Factory, KeyPath, ObjectStoreParams, TransactionMode};

    use capy_canvas_core::input;
    use capy_canvas_core::render;
    use capy_canvas_core::state::{AppState, CanvasContentKind, Tool};

    const DB_NAME: &str = "capy-canvas";
    const STORE_NAME: &str = "snapshots";
    const SNAPSHOT_KEY: &str = "main";
    const DB_VERSION: u32 = 1;

    /// All the things needed for rendering, populated once async surface init
    /// finishes. One slot keeps the borrow checker happy and avoids juggling
    /// half a dozen `Option<>` fields.
    struct Ready {
        render_ctx: RenderContext,
        surface: RenderSurface<'static>,
        renderer: Renderer,
        dev_id: usize,
        window: Arc<Window>,
    }

    struct WebApp {
        canvas_id: String,
        state: Arc<Mutex<AppState>>,
        ready: Arc<Mutex<Option<Ready>>>,
        modifiers: Modifiers,
    }

    // wasm32 is single-threaded, so thread_local is effectively a global.
    // start() stashes shared handles here so the JS-callable save() / load()
    // exports can find the same AppState the event loop is mutating.
    thread_local! {
        static SHARED_STATE: RefCell<Option<Arc<Mutex<AppState>>>> = const { RefCell::new(None) };
        static SHARED_READY: RefCell<Option<Arc<Mutex<Option<Ready>>>>> = const { RefCell::new(None) };
    }

    fn shared_state() -> Option<Arc<Mutex<AppState>>> {
        SHARED_STATE.with(|cell| cell.borrow().clone())
    }

    fn shared_ready() -> Option<Arc<Mutex<Option<Ready>>>> {
        SHARED_READY.with(|cell| cell.borrow().clone())
    }

    fn redraw_via_shared() {
        if let Some(ready_arc) = shared_ready() {
            if let Ok(slot) = ready_arc.lock() {
                if let Some(r) = slot.as_ref() {
                    r.window.request_redraw();
                }
            }
        }
    }

    impl WebApp {
        fn new(canvas_id: String) -> Self {
            let mut state = AppState::new();
            state.tool = Tool::Rect;
            let state_arc = Arc::new(Mutex::new(state));
            let ready_arc: Arc<Mutex<Option<Ready>>> = Arc::new(Mutex::new(None));

            // Publish to thread_local so save() / load() exports can find them.
            SHARED_STATE.with(|cell| *cell.borrow_mut() = Some(state_arc.clone()));
            SHARED_READY.with(|cell| *cell.borrow_mut() = Some(ready_arc.clone()));

            Self {
                canvas_id,
                state: state_arc,
                ready: ready_arc,
                modifiers: Modifiers::default(),
            }
        }

        fn request_redraw(&self) {
            if let Ok(slot) = self.ready.lock() {
                if let Some(r) = slot.as_ref() {
                    r.window.request_redraw();
                }
            }
        }
    }

    impl ApplicationHandler for WebApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            // Already initialized — bail out (resumed can fire more than once).
            if self.ready.lock().map(|s| s.is_some()).unwrap_or(false) {
                return;
            }

            let win = match web_sys::window() {
                Some(w) => w,
                None => {
                    log("[capy-canvas-web] no window");
                    return;
                }
            };
            let document = match win.document() {
                Some(d) => d,
                None => {
                    log("[capy-canvas-web] no document");
                    return;
                }
            };
            let canvas: HtmlCanvasElement = match document
                .get_element_by_id(&self.canvas_id)
                .and_then(|e| e.dyn_into::<HtmlCanvasElement>().ok())
            {
                Some(c) => c,
                None => {
                    log(&format!(
                        "[capy-canvas-web] canvas #{} not found",
                        self.canvas_id
                    ));
                    return;
                }
            };

            let dpr = win.device_pixel_ratio().max(1.0);
            let css_w = canvas.client_width().max(1) as f64;
            let css_h = canvas.client_height().max(1) as f64;
            let phys_w = (css_w * dpr).max(1.0) as u32;
            let phys_h = (css_h * dpr).max(1.0) as u32;
            canvas.set_width(phys_w);
            canvas.set_height(phys_h);

            // Seed AppState viewport in CSS px (mouse handlers feed CSS px too).
            if let Ok(mut state) = self.state.lock() {
                state.viewport_w = css_w;
                state.viewport_h = css_h;
            }

            // Hand the canvas to winit and create a Window backed by it.
            let attrs = WindowAttributes::default().with_canvas(Some(canvas.clone()));
            let window = match event_loop.create_window(attrs) {
                Ok(w) => Arc::new(w),
                Err(error) => {
                    log(&format!("[capy-canvas-web] create_window: {error}"));
                    return;
                }
            };

            // Create the wgpu surface synchronously from the window. Native uses
            // the same `instance.create_surface(window.clone())` path; the only
            // wasm-specific bit is the .await later.
            let mut render_ctx = RenderContext::new();
            let wgpu_surface = match render_ctx.instance.create_surface(window.clone()) {
                Ok(s) => s,
                Err(error) => {
                    log(&format!("[capy-canvas-web] create_surface: {error}"));
                    return;
                }
            };

            // Spawn the async surface init. When it resolves, drop everything
            // into `self.ready` and request a redraw to draw the first frame.
            let ready_slot = self.ready.clone();
            let window_for_async = window.clone();
            spawn_local(async move {
                let surface = match render_ctx
                    .create_render_surface(
                        wgpu_surface,
                        phys_w,
                        phys_h,
                        wgpu::PresentMode::AutoVsync,
                    )
                    .await
                {
                    Ok(s) => s,
                    Err(error) => {
                        log(&format!("[capy-canvas-web] create_render_surface: {error}"));
                        return;
                    }
                };
                let dev_id = surface.dev_id;
                let dev = &render_ctx.devices[dev_id];
                let renderer = match Renderer::new(
                    &dev.device,
                    RendererOptions {
                        use_cpu: false,
                        antialiasing_support: AaSupport::all(),
                        num_init_threads: NonZeroUsize::new(1),
                        pipeline_cache: None,
                    },
                ) {
                    Ok(r) => r,
                    Err(error) => {
                        log(&format!("[capy-canvas-web] Renderer::new: {error}"));
                        return;
                    }
                };

                if let Ok(mut slot) = ready_slot.lock() {
                    *slot = Some(Ready {
                        render_ctx,
                        surface,
                        renderer,
                        dev_id,
                        window: window_for_async.clone(),
                    });
                }
                window_for_async.request_redraw();
                log("[capy-canvas-web] surface ready");
            });
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(size) => {
                    let dpr = web_sys::window()
                        .map(|w| w.device_pixel_ratio().max(1.0))
                        .unwrap_or(1.0);
                    let phys_w = size.width.max(1);
                    let phys_h = size.height.max(1);
                    if let Ok(mut slot) = self.ready.lock() {
                        if let Some(r) = slot.as_mut() {
                            r.render_ctx.resize_surface(&mut r.surface, phys_w, phys_h);
                        }
                    }
                    if let Ok(mut state) = self.state.lock() {
                        state.viewport_w = phys_w as f64 / dpr;
                        state.viewport_h = phys_h as f64 / dpr;
                    }
                    self.request_redraw();
                }
                WindowEvent::ModifiersChanged(mods) => {
                    self.modifiers = mods;
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    let pressed = event.state == ElementState::Pressed;
                    let changed = match self.state.lock() {
                        Ok(mut state) => input::handle_key(
                            &mut state,
                            &event.logical_key,
                            pressed,
                            self.modifiers,
                        ),
                        Err(_) => false,
                    };
                    if changed {
                        self.request_redraw();
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let dpr = web_sys::window()
                        .map(|w| w.device_pixel_ratio().max(1.0))
                        .unwrap_or(1.0);
                    let lx = position.x / dpr;
                    let ly = position.y / dpr;
                    let changed = match self.state.lock() {
                        Ok(mut state) => input::handle_mouse_move(&mut state, lx, ly),
                        Err(_) => false,
                    };
                    if changed {
                        self.request_redraw();
                    }
                }
                WindowEvent::MouseInput {
                    state: btn_state,
                    button,
                    ..
                } => {
                    let pressed = btn_state == ElementState::Pressed;
                    let changed = match self.state.lock() {
                        Ok(mut state) => {
                            input::handle_mouse_button(&mut state, button, pressed, false)
                        }
                        Err(_) => false,
                    };
                    if changed {
                        self.request_redraw();
                    }
                }
                WindowEvent::RedrawRequested => {
                    self.redraw();
                    self.drain_pending_idb_requests();
                    self.drain_pending_export_requests();
                }
                _ => {}
            }
        }
    }

    impl WebApp {
        fn redraw(&mut self) {
            let mut slot = match self.ready.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            let Some(r) = slot.as_mut() else {
                return;
            };

            let phys_w = r.surface.config.width.max(1);
            let phys_h = r.surface.config.height.max(1);
            let dpr = web_sys::window()
                .map(|w| w.device_pixel_ratio().max(1.0))
                .unwrap_or(1.0);

            // Build the scene from AppState (same path as native).
            let scene = match self.state.lock() {
                Ok(state) => render::build_scene(&state),
                Err(_) => return,
            };

            // Native scales the scene by `window.scale_factor()`. On the web,
            // that's `devicePixelRatio` — same role.
            let mut scaled = Scene::new();
            scaled.append(&scene, Some(vello::kurbo::Affine::scale(dpr)));

            let dark = match self.state.lock() {
                Ok(state) => state.dark_mode,
                Err(_) => false,
            };
            let base = if dark {
                vello::peniko::Color::from_rgba8(0x1e, 0x1e, 0x2e, 0xff)
            } else {
                vello::peniko::Color::from_rgba8(0xf8, 0xf7, 0xf4, 0xff)
            };

            let params = RenderParams {
                base_color: base,
                width: phys_w,
                height: phys_h,
                antialiasing_method: AaConfig::Msaa16,
            };

            let dev = &r.render_ctx.devices[r.dev_id];
            if let Err(error) = r.renderer.render_to_texture(
                &dev.device,
                &dev.queue,
                &scaled,
                &r.surface.target_view,
                &params,
            ) {
                log(&format!("[capy-canvas-web] render_to_texture: {error}"));
                return;
            }

            let frame = match r.surface.surface.get_current_texture() {
                Ok(f) => f,
                Err(error) => {
                    log(&format!("[capy-canvas-web] get_current_texture: {error}"));
                    return;
                }
            };
            let surface_view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = dev
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            r.surface.blitter.copy(
                &dev.device,
                &mut encoder,
                &r.surface.target_view,
                &surface_view,
            );
            dev.queue.submit(Some(encoder.finish()));
            frame.present();
        }

        /// Pull `pending_save_request` / `pending_load_request` flags off the
        /// state and turn them into IndexedDB I/O. Same shape as native's
        /// `serial_fs::drain_pending_file_requests`, but the I/O is async so
        /// we `spawn_local` once the lock is released.
        fn drain_pending_idb_requests(&mut self) {
            let (save_req, load_req, json_to_save) = match self.state.lock() {
                Ok(mut state) => {
                    let save_req = state.pending_save_request;
                    let load_req = state.pending_load_request;
                    state.pending_save_request = false;
                    state.pending_load_request = false;
                    let json = if save_req {
                        match state.to_json_string() {
                            Ok(j) => Some(j),
                            Err(error) => {
                                log(&format!("[capy-canvas-web] to_json_string: {error}"));
                                None
                            }
                        }
                    } else {
                        None
                    };
                    (save_req, load_req, json)
                }
                Err(_) => return,
            };

            if save_req {
                if let Some(json) = json_to_save {
                    spawn_local(async move {
                        match idb_save(json).await {
                            Ok(()) => log("[capy-canvas-web] saved to IndexedDB"),
                            Err(error) => {
                                log(&format!("[capy-canvas-web] idb_save: {error}"));
                            }
                        }
                    });
                }
            }

            if load_req {
                spawn_local(async move {
                    perform_idb_load().await;
                });
            }
        }

        /// Drain `pending_svg_export` (sync · just blob+download) and
        /// `export_requested` (async · GPU readback + PNG encode + blob+download).
        /// Mirrors `serial_fs::drain_pending_file_requests` on native, but the
        /// PNG path is async via `spawn_local` since wgpu `map_async` is the
        /// only readback path on web.
        fn drain_pending_export_requests(&mut self) {
            let (svg_to_export, png_requested) = match self.state.lock() {
                Ok(mut state) => {
                    let svg = state.pending_svg_export.take();
                    let png = state.export_requested;
                    state.export_requested = false;
                    (svg, png)
                }
                Err(_) => return,
            };

            if let Some(svg) = svg_to_export {
                if let Err(error) = trigger_download(svg.as_bytes(), "image/svg+xml", "canvas.svg")
                {
                    log(&format!("[capy-canvas-web] svg download: {error}"));
                } else {
                    log("[capy-canvas-web] svg exported");
                }
            }

            if png_requested {
                spawn_local(async move {
                    if let Err(error) = perform_png_export().await {
                        log(&format!("[capy-canvas-web] png export: {error}"));
                    }
                });
            }
        }
    }

    /// Build a `Blob` from `bytes` (typed `mime`), create an `<a>` with
    /// `href = blob URL · download = filename`, click it, revoke URL.
    /// Standard browser download dance.
    fn trigger_download(bytes: &[u8], mime: &str, filename: &str) -> Result<(), String> {
        // Wrap the bytes in a JS Uint8Array; Blob takes a sequence of typed arrays.
        let array = js_sys::Uint8Array::from(bytes);
        let parts = js_sys::Array::new();
        parts.push(&array.buffer());

        let opts = BlobPropertyBag::new();
        opts.set_type(mime);

        let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
            .map_err(|e| format!("Blob::new: {e:?}"))?;

        let url = Url::create_object_url_with_blob(&blob)
            .map_err(|e| format!("create_object_url: {e:?}"))?;

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
    async fn perform_png_export() -> Result<(), String> {
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

    /// Open or upgrade the `capy-canvas` database. Idempotent: if the
    /// `snapshots` store already exists the upgrade callback never fires.
    async fn open_db() -> Result<idb::Database, String> {
        let factory = Factory::new().map_err(|e| format!("Factory::new: {e}"))?;
        let mut open_request = factory
            .open(DB_NAME, Some(DB_VERSION))
            .map_err(|e| format!("factory.open: {e}"))?;
        open_request.on_upgrade_needed(|event| {
            let database = match event.database() {
                Ok(d) => d,
                Err(error) => {
                    log(&format!("[capy-canvas-web] upgrade.database: {error}"));
                    return;
                }
            };
            let names = database.store_names();
            if names.iter().any(|n| n == STORE_NAME) {
                return;
            }
            let mut params = ObjectStoreParams::new();
            params.auto_increment(false);
            params.key_path(None::<KeyPath>);
            if let Err(error) = database.create_object_store(STORE_NAME, params) {
                log(&format!("[capy-canvas-web] create_object_store: {error}"));
            }
        });
        let db = open_request
            .await
            .map_err(|e| format!("open_request.await: {e}"))?;
        Ok(db)
    }

    /// Persist a single snapshot under key `"main"`.
    async fn idb_save(json: String) -> Result<(), String> {
        let db = open_db().await?;
        let tx = db
            .transaction(&[STORE_NAME], TransactionMode::ReadWrite)
            .map_err(|e| format!("transaction: {e}"))?;
        let store = tx
            .object_store(STORE_NAME)
            .map_err(|e| format!("object_store: {e}"))?;
        let value = JsValue::from_str(&json);
        let key = JsValue::from_str(SNAPSHOT_KEY);
        store
            .put(&value, Some(&key))
            .map_err(|e| format!("put: {e}"))?
            .await
            .map_err(|e| format!("put.await: {e}"))?;
        tx.commit()
            .map_err(|e| format!("commit: {e}"))?
            .await
            .map_err(|e| format!("commit.await: {e}"))?;
        db.close();
        Ok(())
    }

    /// Read back the snapshot under key `"main"`. `Ok(None)` means the store
    /// is empty (first run, or save hasn't happened yet).
    async fn idb_load() -> Result<Option<String>, String> {
        let db = open_db().await?;
        let tx = db
            .transaction(&[STORE_NAME], TransactionMode::ReadOnly)
            .map_err(|e| format!("transaction: {e}"))?;
        let store = tx
            .object_store(STORE_NAME)
            .map_err(|e| format!("object_store: {e}"))?;
        let key = JsValue::from_str(SNAPSHOT_KEY);
        let result: Option<JsValue> = store
            .get(key)
            .map_err(|e| format!("get: {e}"))?
            .await
            .map_err(|e| format!("get.await: {e}"))?;
        db.close();
        Ok(result.and_then(|v| v.as_string()))
    }

    /// Shared logic for "load JSON from IDB and stuff it into AppState".
    /// Used by both the keyboard drain path and the JS-callable `load()`.
    async fn perform_idb_load() {
        match idb_load().await {
            Ok(Some(json)) => {
                let state_arc = match shared_state() {
                    Some(s) => s,
                    None => {
                        log("[capy-canvas-web] perform_idb_load: no shared state");
                        return;
                    }
                };
                let load_result = match state_arc.lock() {
                    Ok(mut state) => state.load_from_json_str(&json),
                    Err(_) => return,
                };
                match load_result {
                    Ok(()) => {
                        log("[capy-canvas-web] loaded from IndexedDB");
                        redraw_via_shared();
                    }
                    Err(error) => {
                        log(&format!("[capy-canvas-web] load_from_json_str: {error}"));
                    }
                }
            }
            Ok(None) => log("[capy-canvas-web] idb_load: no snapshot yet"),
            Err(error) => log(&format!("[capy-canvas-web] idb_load: {error}")),
        }
    }

    fn log(msg: &str) {
        web_sys::console::log_1(&JsValue::from_str(msg));
    }

    /// JS-callable entry. Boot the winit web event loop pointed at `canvas_id`.
    #[wasm_bindgen]
    pub fn start(canvas_id: String) {
        console_error_panic_hook::set_once();
        log(&format!("[capy-canvas-web] start canvas_id={canvas_id}"));

        let event_loop = match EventLoop::new() {
            Ok(el) => el,
            Err(error) => {
                log(&format!("[capy-canvas-web] EventLoop::new: {error}"));
                return;
            }
        };
        let app = WebApp::new(canvas_id);
        // EventLoopExtWebSys: spawn_app returns immediately on web; the loop
        // runs as JS microtasks driven by requestAnimationFrame & event handlers.
        event_loop.spawn_app(app);
    }

    /// JS-callable save. Skips the keyboard pending-flag plumbing and writes
    /// straight to IndexedDB. Exists because Chrome/Firefox eat Cmd+S as
    /// "Save Page As" before winit ever sees it; Playwright drives this path.
    #[wasm_bindgen]
    pub async fn save() -> Result<(), JsValue> {
        let state_arc = shared_state()
            .ok_or_else(|| JsValue::from_str("save(): no shared state · call start() first"))?;
        let json = {
            let state = state_arc
                .lock()
                .map_err(|_| JsValue::from_str("save(): state lock poisoned"))?;
            state
                .to_json_string()
                .map_err(|e| JsValue::from_str(&format!("save(): {e}")))?
        };
        idb_save(json).await.map_err(|e| JsValue::from_str(&e))?;
        log("[capy-canvas-web] save() ok");
        Ok(())
    }

    /// JS-callable SVG export. Generates the SVG from the current AppState
    /// and triggers a browser download of `canvas.svg`. Bypasses the
    /// keyboard-flag drain path for reliability under Playwright (some browsers
    /// intercept Cmd+Shift+E for menu shortcuts before winit sees the keydown).
    #[wasm_bindgen]
    pub fn export_svg() -> Result<(), JsValue> {
        let state_arc = shared_state().ok_or_else(|| {
            JsValue::from_str("export_svg(): no shared state · call start() first")
        })?;
        let svg = {
            let state = state_arc
                .lock()
                .map_err(|_| JsValue::from_str("export_svg(): state lock poisoned"))?;
            state.export_svg()
        };
        trigger_download(svg.as_bytes(), "image/svg+xml", "canvas.svg")
            .map_err(|e| JsValue::from_str(&e))?;
        log("[capy-canvas-web] export_svg() ok");
        Ok(())
    }

    /// JS-callable PNG export. Renders current AppState to an offscreen RGBA
    /// texture, reads back via `map_async`, encodes PNG, triggers download.
    #[wasm_bindgen]
    pub async fn export_png() -> Result<(), JsValue> {
        perform_png_export()
            .await
            .map_err(|e| JsValue::from_str(&e))?;
        log("[capy-canvas-web] export_png() ok");
        Ok(())
    }

    /// JS-callable load. Mirror image of `save()`.
    #[wasm_bindgen]
    pub async fn load() -> Result<bool, JsValue> {
        let state_arc = shared_state()
            .ok_or_else(|| JsValue::from_str("load(): no shared state · call start() first"))?;
        let json = idb_load().await.map_err(|e| JsValue::from_str(&e))?;
        let Some(json) = json else {
            log("[capy-canvas-web] load(): no snapshot");
            return Ok(false);
        };
        {
            let mut state = state_arc
                .lock()
                .map_err(|_| JsValue::from_str("load(): state lock poisoned"))?;
            state
                .load_from_json_str(&json)
                .map_err(|e| JsValue::from_str(&format!("load(): {e}")))?;
        }
        redraw_via_shared();
        log("[capy-canvas-web] load() ok");
        Ok(true)
    }

    /// JS-callable image insert. Decodes the byte slice (PNG/JPEG/WebP via the
    /// `image` crate) and inserts a new `ShapeKind::Image` shape at `(x, y)`
    /// with natural dimensions clamped to a reasonable on-screen size.
    ///
    /// This is the stable contract the headless verify script drives.
    /// Drag-drop in a real browser is too flaky to script reliably; we expose
    /// the same byte-decode → state-insert path as a function call so the
    /// pixel test can pump a known-good PNG into the canvas without simulating
    /// `DragEvent`.
    #[wasm_bindgen]
    pub fn add_image_at(x: f64, y: f64, bytes: &[u8]) -> Result<u32, JsValue> {
        add_image_asset_at(x, y, bytes, "", "", "", "")
    }

    #[wasm_bindgen]
    pub fn add_image_asset_at(
        x: f64,
        y: f64,
        bytes: &[u8],
        title: &str,
        source_path: &str,
        generation_provider: &str,
        generation_prompt: &str,
    ) -> Result<u32, JsValue> {
        let state_arc = shared_state().ok_or_else(|| {
            JsValue::from_str("add_image_asset_at(): no shared state · call start() first")
        })?;
        let decoded = image::load_from_memory(bytes)
            .map_err(|e| JsValue::from_str(&format!("decode image: {e}")))?
            .to_rgba8();
        let (w, h) = decoded.dimensions();
        let rgba = Arc::new(decoded.into_raw());
        // Sniff mime from header for round-trip metadata. The renderer doesn't
        // care — `peniko::ImageFormat::Rgba8` is set unconditionally — but
        // this lets a future save/load path encode back to the original codec.
        let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png".to_string()
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            "image/jpeg".to_string()
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "image/webp".to_string()
        } else {
            "application/octet-stream".to_string()
        };
        let idx = {
            let mut state = state_arc
                .lock()
                .map_err(|_| JsValue::from_str("add_image_asset_at(): state lock poisoned"))?;
            let idx =
                state.import_image_asset_bytes(capy_canvas_core::state_shapes::ImageAssetImport {
                    x,
                    y,
                    rgba,
                    width: w,
                    height: h,
                    mime,
                    title: optional_string(title),
                    source_path: optional_string(source_path),
                    generation_provider: optional_string(generation_provider),
                    generation_prompt: optional_string(generation_prompt),
                });
            state.selected = vec![idx];
            state.tool = Tool::Select;
            idx
        };
        redraw_via_shared();
        log(&format!(
            "[capy-canvas-web] add_image_at({x}, {y}) ok · {w}x{h} idx={idx}"
        ));
        Ok(idx as u32)
    }

    /// JS-callable Lovart-style content card creation. This creates a real
    /// canvas node with semantic metadata so the AI snapshot sees a product
    /// object, not just pixels.
    #[wasm_bindgen]
    pub fn create_content_card(kind: &str, title: &str, x: f64, y: f64) -> Result<u32, JsValue> {
        let state_arc = shared_state().ok_or_else(|| {
            JsValue::from_str("create_content_card(): no shared state · call start() first")
        })?;
        let kind = kind
            .parse::<CanvasContentKind>()
            .map_err(|e| JsValue::from_str(&format!("create_content_card(): {e}")))?;
        let (idx, id) = {
            let mut state = state_arc
                .lock()
                .map_err(|_| JsValue::from_str("create_content_card(): state lock poisoned"))?;
            let idx = state.create_content_card(kind, title, x, y);
            state.tool = Tool::Select;
            (idx, state.shapes[idx].id)
        };
        redraw_via_shared();
        log(&format!(
            "[capy-canvas-web] create_content_card({kind:?}, id={id}, idx={idx}) ok"
        ));
        Ok(idx as u32)
    }

    #[wasm_bindgen]
    pub fn create_poster_document_card(
        title: &str,
        x: f64,
        y: f64,
        source_path: &str,
    ) -> Result<u32, JsValue> {
        let state_arc = shared_state().ok_or_else(|| {
            JsValue::from_str("create_poster_document_card(): no shared state · call start() first")
        })?;
        let (idx, id) = {
            let mut state = state_arc.lock().map_err(|_| {
                JsValue::from_str("create_poster_document_card(): state lock poisoned")
            })?;
            let idx = state.create_poster_document_card(title, x, y, source_path);
            state.tool = Tool::Select;
            (idx, state.shapes[idx].id)
        };
        redraw_via_shared();
        log(&format!(
            "[capy-canvas-web] create_poster_document_card(id={id}, idx={idx}) ok"
        ));
        Ok(idx as u32)
    }

    /// JS-callable selection bridge for DOM labels and desktop verification.
    #[wasm_bindgen]
    pub fn select_node(id: u32) -> Result<bool, JsValue> {
        let state_arc = shared_state().ok_or_else(|| {
            JsValue::from_str("select_node(): no shared state · call start() first")
        })?;
        let found = {
            let mut state = state_arc
                .lock()
                .map_err(|_| JsValue::from_str("select_node(): state lock poisoned"))?;
            state.select_shape_ids(&[u64::from(id)]).is_ok()
        };
        if found {
            redraw_via_shared();
            log(&format!("[capy-canvas-web] select_node(id={id}) ok"));
        }
        Ok(found)
    }

    #[wasm_bindgen]
    pub fn focus_node(id: u32) -> Result<bool, JsValue> {
        let state_arc = shared_state().ok_or_else(|| {
            JsValue::from_str("focus_node(): no shared state · call start() first")
        })?;
        let mut state = state_arc
            .lock()
            .map_err(|_| JsValue::from_str("focus_node(): state lock poisoned"))?;
        let Some(idx) = state
            .shapes
            .iter()
            .position(|shape| shape.id == u64::from(id))
        else {
            return Ok(false);
        };
        let shape = &state.shapes[idx];
        let cx = shape.x + shape.w.min(0.0) + shape.w.abs() / 2.0;
        let cy = shape.y + shape.h.min(0.0) + shape.h.abs() / 2.0;
        let zoom = state.camera.zoom;
        state.selected = vec![idx];
        state.tool = Tool::Select;
        state.camera.offset_x = state.viewport_w / 2.0 - cx * zoom;
        state.camera.offset_y = state.viewport_h / 2.0 - cy * zoom;
        state.target_zoom = zoom;
        drop(state);
        redraw_via_shared();
        Ok(true)
    }

    /// JS-callable absolute move bridge for AI actions and desktop verification.
    #[wasm_bindgen]
    pub fn move_node_by_id(id: u32, x: f64, y: f64) -> Result<bool, JsValue> {
        let state_arc = shared_state().ok_or_else(|| {
            JsValue::from_str("move_node_by_id(): no shared state · call start() first")
        })?;
        let moved = {
            let mut state = state_arc
                .lock()
                .map_err(|_| JsValue::from_str("move_node_by_id(): state lock poisoned"))?;
            state.move_shape_by_id(u64::from(id), x, y).is_ok()
        };
        if moved {
            redraw_via_shared();
            log(&format!(
                "[capy-canvas-web] move_node_by_id(id={id}, x={x:.1}, y={y:.1}) ok"
            ));
        }
        Ok(moved)
    }

    // v0.8 introspection exports used by desktop state-key scripts.

    /// Number of shapes currently on the canvas. Returns 0 before `start()`
    /// finishes wiring up `SHARED_STATE`, so the desktop state-key script can
    /// fall back to "0" without throwing.
    #[wasm_bindgen]
    pub fn shape_count() -> usize {
        shared_state()
            .and_then(|arc| arc.lock().ok().map(|s| s.shapes.len()))
            .unwrap_or(0)
    }

    /// Snake_case label of the active tool (e.g. "rect", "select"). Returns
    /// "select" as the default if state isn't ready yet — matches the initial
    /// `Tool::Select` constructor in `AppState::new()`.
    #[wasm_bindgen]
    pub fn current_tool() -> String {
        shared_state()
            .and_then(|arc| arc.lock().ok().map(|s| tool_label(s.tool).to_string()))
            .unwrap_or_else(|| "select".to_string())
    }

    /// Whether dark mode is active in AppState. Returns false if state isn't
    /// ready (matches `AppState::new()` default).
    #[wasm_bindgen]
    pub fn dark_mode() -> bool {
        shared_state()
            .and_then(|arc| arc.lock().ok().map(|s| s.dark_mode))
            .unwrap_or(false)
    }

    /// Serialize the full shape list as a JS array. The desktop shell wraps
    /// this in `JSON.stringify(...)` for transport over IPC.
    #[wasm_bindgen]
    pub fn list_shapes() -> JsValue {
        if let Some(arc) = shared_state() {
            if let Ok(state) = arc.lock() {
                return serde_wasm_bindgen::to_value(&state.shapes).unwrap_or(JsValue::NULL);
            }
        }
        JsValue::NULL
    }

    /// Product context for the current selection. Planner chat uses this to
    /// receive structured canvas context without scraping pixels.
    #[wasm_bindgen]
    pub fn selected_context() -> JsValue {
        if let Some(arc) = shared_state() {
            if let Ok(state) = arc.lock() {
                return serde_wasm_bindgen::to_value(&state.selected_context())
                    .unwrap_or(JsValue::NULL);
            }
        }
        JsValue::NULL
    }

    /// Human-readable selection summary for prompt injection.
    #[wasm_bindgen]
    pub fn selected_context_text() -> String {
        shared_state()
            .and_then(|arc| arc.lock().ok().map(|state| state.selected_context_text()))
            .unwrap_or_default()
    }

    /// Full AI-facing canvas snapshot: layout, nodes, connectors, groups,
    /// selection, and stable id-based action names.
    #[wasm_bindgen]
    pub fn ai_snapshot() -> JsValue {
        if let Some(arc) = shared_state() {
            if let Ok(state) = arc.lock() {
                return serde_wasm_bindgen::to_value(&state.ai_snapshot()).unwrap_or(JsValue::NULL);
            }
        }
        JsValue::NULL
    }

    /// Human-readable whole-canvas summary for agent prompts or CLI inspection.
    #[wasm_bindgen]
    pub fn ai_snapshot_text() -> String {
        shared_state()
            .and_then(|arc| arc.lock().ok().map(|state| state.ai_snapshot_text()))
            .unwrap_or_default()
    }

    fn tool_label(tool: Tool) -> &'static str {
        match tool {
            Tool::Select => "select",
            Tool::Rect => "rect",
            Tool::Ellipse => "ellipse",
            Tool::Triangle => "triangle",
            Tool::Diamond => "diamond",
            Tool::Line => "line",
            Tool::Arrow => "arrow",
            Tool::Freehand => "freehand",
            Tool::Highlighter => "highlighter",
            Tool::StickyNote => "sticky_note",
            Tool::Text => "text",
            Tool::Eraser => "eraser",
            Tool::Lasso => "lasso",
        }
    }

    fn optional_string(value: &str) -> Option<String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn version() -> String {
    "0.4.0".to_string()
}
