use std::path::{Path, PathBuf};

use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
use tao::window::{Window, WindowBuilder};
use wry::{
    WebView, WebViewBuilder,
    http::{Request, Response, header::CONTENT_TYPE},
};

use crate::app::ShellEvent;

pub fn create_window(
    target: &EventLoopWindowTarget<ShellEvent>,
    proxy: EventLoopProxy<ShellEvent>,
    window_id: &str,
    project: &str,
) -> Result<(Window, WebView), String> {
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

    let root = project_root()?;
    let session_script = initialization_script(project);
    let ipc_window_id = window_id.to_string();
    let ipc_proxy = proxy.clone();
    let webview_builder = WebViewBuilder::new()
        .with_custom_protocol(
            "capybara".into(),
            move |_webview_id, request| match protocol_response(&root, request) {
                Ok(response) => response.map(Into::into),
                Err(err) => plain_response(500, err.into_bytes()).map(Into::into),
            },
        )
        .with_url("capybara://frontend/frontend/capy-app/index.html")
        .with_initialization_script(&session_script)
        .with_ipc_handler(move |req| {
            let body = req.body().to_string();
            let _send_result = ipc_proxy.send_event(ShellEvent::IpcFromJs {
                window_id: ipc_window_id.clone(),
                body,
            });
        });
    let webview = webview_builder
        .build(&window)
        .map_err(|err| format!("webview build failed: {err}"))?;

    Ok((window, webview))
}

fn initialization_script(project: &str) -> String {
    let cwd = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "/".to_string());
    let session = serde_json::json!({
        "project": project,
        "cwd": cwd,
        "version": env!("CARGO_PKG_VERSION")
    });
    format!(
        r#"(() => {{
  window.CAPYBARA_SESSION = {session};
  const markNativeShell = () => {{
    document.documentElement.setAttribute("data-capybara-native", "true");
  }};
  if (document.documentElement) {{
    markNativeShell();
  }} else {{
    document.addEventListener("DOMContentLoaded", markNativeShell, {{ once: true }});
  }}
}})();"#
    )
}

fn protocol_response(root: &Path, request: Request<Vec<u8>>) -> Result<Response<Vec<u8>>, String> {
    let root = root
        .canonicalize()
        .map_err(|err| format!("project root failed: {err}"))?;
    let path = request.uri().path().trim_start_matches('/');
    let path = if path.is_empty() {
        "frontend/capy-app/index.html"
    } else {
        path
    };
    let file = root.join(path);
    let file = file
        .canonicalize()
        .map_err(|err| format!("asset missing: {path}: {err}"))?;
    if !file.starts_with(&root) {
        return Ok(plain_response(403, b"forbidden".to_vec()));
    }
    let content = std::fs::read(&file).map_err(|err| format!("asset read failed: {err}"))?;
    Response::builder()
        .header(CONTENT_TYPE, mime_for_path(&file))
        .body(content)
        .map_err(|err| err.to_string())
}

fn project_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!(
                "cannot resolve project root from {}",
                manifest_dir.display()
            )
        })
}

fn plain_response(status: u16, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn mime_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
