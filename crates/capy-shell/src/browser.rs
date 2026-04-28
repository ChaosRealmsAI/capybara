use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use raw_window_handle::HasWindowHandle;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
use tao::window::{Window, WindowBuilder};
use wef::{Browser, BrowserHandler, FuncRegistry, LogSeverity, PhysicalUnit, Size};

use crate::app::ShellEvent;

mod assets;
mod runtime;

use assets::{asset_server, frontend_url};
pub use runtime::{CefRuntime, init_cef_runtime, maybe_run_cef_subprocess};

type EvalCallback = Box<dyn Fn(String) + Send + 'static>;
type EvalCallbacks = Arc<Mutex<HashMap<String, EvalCallback>>>;

pub struct ShellBrowser {
    browser: Browser,
    callbacks: EvalCallbacks,
    next_eval: AtomicU64,
}

impl ShellBrowser {
    pub fn evaluate_script(&self, script: &str) -> Result<(), String> {
        self.execute(script)
    }

    pub fn evaluate_script_with_callback<F>(&self, script: &str, callback: F) -> Result<(), String>
    where
        F: Fn(String) + Send + 'static,
    {
        let seq = self.next_eval.fetch_add(1, Ordering::Relaxed);
        let req_id = format!("eval-{seq}");
        self.callbacks
            .lock()
            .map_err(|_| "browser eval callback lock poisoned".to_string())?
            .insert(req_id.clone(), Box::new(callback));
        let req_json = serde_json::to_string(&req_id)
            .map_err(|err| format!("eval id encode failed: {err}"))?;
        let script = format!(
            r#"(async () => {{
  try {{
    const value = await Promise.resolve({script});
    let raw = JSON.stringify(value);
    if (raw === undefined) raw = "null";
    await window.jsBridge.capyEvalResult({req_json}, raw);
  }} catch (err) {{
    const message = err && err.stack ? err.stack : String(err);
    await window.jsBridge.capyEvalResult({req_json}, JSON.stringify({{ ok: false, error: message }}));
  }}
}})();"#
        );
        if let Err(err) = self.execute(&script) {
            let _removed = self
                .callbacks
                .lock()
                .ok()
                .and_then(|mut callbacks| callbacks.remove(&req_id));
            return Err(err);
        }
        Ok(())
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.browser.resize(Size::new(
            PhysicalUnit(width.max(1) as i32),
            PhysicalUnit(height.max(1) as i32),
        ));
    }

    pub fn set_focus(&self, focus: bool) {
        self.browser.set_focus(focus);
    }

    fn execute(&self, script: &str) -> Result<(), String> {
        let frame = self
            .browser
            .main_frame()
            .ok_or_else(|| "browser main frame unavailable".to_string())?;
        frame.execute_javascript(script);
        Ok(())
    }
}

struct ShellHandler;

impl BrowserHandler for ShellHandler {
    fn on_console_message(
        &mut self,
        message: &str,
        level: LogSeverity,
        source: &str,
        line_number: i32,
    ) {
        println!("CAPYCONSOLE [{level:?}] {message} ({source}:{line_number})");
    }

    fn on_load_error(&mut self, _frame: wef::Frame, error_text: &str, failed_url: &str) {
        println!(
            "{}",
            serde_json::json!({
                "event": "browser-load-error",
                "error": error_text,
                "url": failed_url
            })
        );
    }
}

pub fn create_window(
    target: &EventLoopWindowTarget<ShellEvent>,
    proxy: EventLoopProxy<ShellEvent>,
    window_id: &str,
    project: &str,
) -> Result<(Window, ShellBrowser), String> {
    let builder = WindowBuilder::new()
        .with_title("Capybara")
        .with_inner_size(LogicalSize::new(1440.0, 900.0))
        .with_position(LogicalPosition::new(120.0, 80.0))
        .with_resizable(true)
        .with_min_inner_size(LogicalSize::new(960.0, 620.0));
    #[cfg(target_os = "macos")]
    let builder = builder
        .with_title_hidden(true)
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_has_shadow(true);

    let window = builder
        .build(target)
        .map_err(|err| format!("window build failed: {err}"))?;
    #[cfg(target_os = "macos")]
    if let Some(observer) = crate::traffic_light::install_from_tao(&window) {
        std::mem::forget(observer);
    }

    let server = asset_server()?;
    let scale = window.scale_factor().max(1.0) as f32;
    let url = frontend_url(&server.base_url, project, scale);
    let ipc_window_id = window_id.to_string();
    let ipc_proxy = proxy.clone();
    let callbacks: EvalCallbacks = Arc::new(Mutex::new(HashMap::new()));
    let result_callbacks = Arc::clone(&callbacks);
    let registry = FuncRegistry::builder()
        .register("capyShellIpc", move |body: String| -> bool {
            let _send_result = ipc_proxy.send_event(ShellEvent::IpcFromJs {
                window_id: ipc_window_id.clone(),
                body: body.clone(),
            });
            println!("CAPYIPC {body}");
            true
        })
        .register(
            "capyEvalResult",
            move |req_id: String, body: String| -> bool {
                let callback = result_callbacks
                    .lock()
                    .ok()
                    .and_then(|mut callbacks| callbacks.remove(&req_id));
                if let Some(callback) = callback {
                    callback(body);
                }
                true
            },
        )
        .build();

    let size = window.inner_size();
    let parent = window
        .window_handle()
        .map_err(|err| format!("window handle unavailable: {err}"))?
        .as_raw();
    let browser = Browser::builder()
        .parent(parent)
        .size(size.width.max(1), size.height.max(1))
        .device_scale_factor(scale)
        .frame_rate(60)
        .url(url)
        .windowed()
        .handler(ShellHandler)
        .func_registry(registry)
        .build();
    browser.set_focus(true);

    Ok((
        window,
        ShellBrowser {
            browser,
            callbacks,
            next_eval: AtomicU64::new(1),
        },
    ))
}
