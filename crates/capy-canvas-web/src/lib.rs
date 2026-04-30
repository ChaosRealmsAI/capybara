//! capy-canvas-web · winit-web canvas mount, IndexedDB persistence, and
//! JS-callable canvas bridges for the desktop shell.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
mod web {
    mod downloads;
    mod exports;
    mod idb_store;
    mod io;
    mod project_artifacts;
    mod vector_style;
    mod viewport;

    pub use self::exports::*;
    pub use self::project_artifacts::*;
    pub use self::viewport::*;
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
    use winit::event_loop::ActiveEventLoop;
    use winit::platform::web::WindowAttributesExtWebSys;
    use winit::window::{Window, WindowAttributes, WindowId};

    use web_sys::HtmlCanvasElement;

    use capy_canvas_core::input;
    use capy_canvas_core::render;
    use capy_canvas_core::state::{AppState, Tool};

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
            state.tool = Tool::Select;
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
                    let shift = self.modifiers.state().shift_key();
                    let changed = match self.state.lock() {
                        Ok(mut state) => {
                            input::handle_mouse_button(&mut state, button, pressed, shift)
                        }
                        Err(_) => false,
                    };
                    if changed {
                        self.request_redraw();
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let mods = self.modifiers.state();
                    let changed = match self.state.lock() {
                        Ok(mut state) => input::handle_scroll(
                            &mut state,
                            delta,
                            mods.super_key() || mods.control_key(),
                            mods.shift_key(),
                        ),
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
    }

    fn log(msg: &str) {
        web_sys::console::log_1(&JsValue::from_str(msg));
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
