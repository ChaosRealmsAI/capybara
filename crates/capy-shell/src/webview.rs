use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use raw_window_handle::HasWindowHandle;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
use tao::window::{Window, WindowBuilder};
use wef::{Browser, BrowserHandler, FuncRegistry, LogSeverity, PhysicalUnit, Settings, Size};

use crate::app::ShellEvent;

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

pub struct CefRuntime {
    #[cfg(target_os = "macos")]
    _loader: wef::FrameworkLoader,
    cache_dir: PathBuf,
}

impl Drop for CefRuntime {
    fn drop(&mut self) {
        wef::shutdown();
        let _remove_result = std::fs::remove_dir_all(&self.cache_dir);
    }
}

pub fn maybe_run_cef_subprocess() -> Result<bool, String> {
    if !std::env::args().any(|arg| arg.starts_with("--type=") || arg == "--type") {
        return Ok(false);
    }
    #[cfg(target_os = "macos")]
    let _sandbox = wef::SandboxContext::new().map_err(|err| err.to_string())?;
    #[cfg(target_os = "macos")]
    let _loader = wef::FrameworkLoader::load_in_helper().map_err(|err| err.to_string())?;
    wef::exec_process().map_err(|err| err.to_string())
}

pub fn init_cef_runtime() -> Result<CefRuntime, String> {
    let cache_dir = create_temp_dir("capy-shell-cef")?;
    #[cfg(target_os = "macos")]
    let loader = wef::FrameworkLoader::load_in_main().map_err(|err| err.to_string())?;

    let mut settings = Settings::new()
        .disable_gpu(false)
        .root_cache_path(path_to_string(&cache_dir)?)
        .cache_path(path_to_string(&cache_dir.join("profile"))?);
    if let Some(helper) = browser_subprocess_path()? {
        settings = settings.browser_subprocess_path(helper);
    }
    wef::init(settings).map_err(|err| err.to_string())?;

    Ok(CefRuntime {
        #[cfg(target_os = "macos")]
        _loader: loader,
        cache_dir,
    })
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

fn frontend_url(base_url: &str, project: &str, dpr: f32) -> String {
    format!(
        "{base_url}/index.html?project={}&dpr={dpr:.3}",
        url_encode(project)
    )
}

#[derive(Clone)]
struct AssetServer {
    base_url: String,
}

static ASSET_SERVER: OnceLock<Result<AssetServer, String>> = OnceLock::new();

fn asset_server() -> Result<AssetServer, String> {
    ASSET_SERVER.get_or_init(start_asset_server).clone()
}

fn start_asset_server() -> Result<AssetServer, String> {
    let frontend_root = frontend_root()?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("asset server bind failed: {err}"))?;
    let addr = listener
        .local_addr()
        .map_err(|err| format!("asset server addr failed: {err}"))?;
    let base_url = format!("http://{addr}");
    let thread_root = frontend_root.clone();
    std::thread::Builder::new()
        .name("capy-shell-assets".to_string())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let root = thread_root.clone();
                let _handle = std::thread::Builder::new()
                    .name("capy-shell-asset-request".to_string())
                    .spawn(move || {
                        if let Err(err) = handle_http_stream(stream, &root) {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "event": "asset-server-error",
                                    "detail": err
                                })
                            );
                        }
                    });
            }
        })
        .map_err(|err| format!("asset server thread failed: {err}"))?;
    Ok(AssetServer { base_url })
}

fn handle_http_stream(mut stream: TcpStream, frontend_root: &Path) -> Result<(), String> {
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|err| format!("stream clone failed: {err}"))?,
    );
    let mut first_line = String::new();
    reader
        .read_line(&mut first_line)
        .map_err(|err| format!("request read failed: {err}"))?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    if method != "GET" && method != "HEAD" {
        return write_http_response(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"method not allowed".to_vec(),
            method == "HEAD",
        );
    }

    loop {
        let mut header = String::new();
        let read = reader
            .read_line(&mut header)
            .map_err(|err| format!("header read failed: {err}"))?;
        if read == 0 || header == "\r\n" || header == "\n" {
            break;
        }
    }

    let (path, _) = target.split_once('?').unwrap_or((target, ""));
    match frontend_response(frontend_root, path) {
        Ok(response) => write_http_response(
            &mut stream,
            response.status,
            response.reason,
            response.content_type,
            response.body,
            method == "HEAD",
        ),
        Err(err) => write_http_response(
            &mut stream,
            500,
            "Internal Server Error",
            "text/plain; charset=utf-8",
            err.into_bytes(),
            method == "HEAD",
        ),
    }
}

struct HttpResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

fn frontend_response(root: &Path, path: &str) -> Result<HttpResponse, String> {
    let root = root
        .canonicalize()
        .map_err(|err| format!("frontend root failed: {err}"))?;
    let path = path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let decoded_path = percent_decode(path)?;
    let file = root.join(decoded_path);
    let file = file
        .canonicalize()
        .map_err(|err| format!("frontend asset missing: {path}: {err}"))?;
    if !file.starts_with(&root) {
        return Ok(text_response(403, "Forbidden", "forbidden"));
    }
    let mut content =
        std::fs::read(&file).map_err(|err| format!("frontend asset failed: {err}"))?;
    if file.file_name().and_then(|name| name.to_str()) == Some("index.html") {
        content = inject_initialization_script(content)?;
    }
    Ok(HttpResponse {
        status: 200,
        reason: "OK",
        content_type: mime_for_path(&file),
        body: content,
    })
}

fn text_response(status: u16, reason: &'static str, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        reason,
        content_type: "text/plain; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: Vec<u8>,
    head_only: bool,
) -> Result<(), String> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|err| format!("response header failed: {err}"))?;
    if !head_only {
        stream
            .write_all(&body)
            .map_err(|err| format!("response body failed: {err}"))?;
    }
    Ok(())
}

fn inject_initialization_script(content: Vec<u8>) -> Result<Vec<u8>, String> {
    let html =
        String::from_utf8(content).map_err(|err| format!("index.html is not UTF-8: {err}"))?;
    let script = initialization_script();
    let marker = "<head>";
    let injected = if let Some(index) = html.find(marker) {
        let insert_at = index + marker.len();
        let mut next = String::with_capacity(html.len() + script.len() + 32);
        next.push_str(&html[..insert_at]);
        next.push_str("\n<script>");
        next.push_str(&script);
        next.push_str("</script>\n");
        next.push_str(&html[insert_at..]);
        next
    } else {
        format!("<script>{script}</script>\n{html}")
    };
    Ok(injected.into_bytes())
}

fn initialization_script() -> String {
    let cwd = std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "/".to_string());
    let cwd_json = serde_json::to_string(&cwd).unwrap_or_else(|_| "\"/\"".to_string());
    format!(
        r#"(() => {{
  const params = new URLSearchParams(window.location.search || "");
  window.CAPYBARA_SESSION = {{
    project: params.get("project") || "demo",
    cwd: {cwd_json},
    version: "{}"
  }};
  const queue = [];
  const flush = () => {{
    if (!window.jsBridge || typeof window.jsBridge.capyShellIpc !== "function") return;
    while (queue.length) {{
      try {{ window.jsBridge.capyShellIpc(String(queue.shift())); }} catch (_err) {{ break; }}
    }}
  }};
  window.ipc = {{
    postMessage(message) {{
      const raw = String(message);
      if (window.jsBridge && typeof window.jsBridge.capyShellIpc === "function") {{
        window.jsBridge.capyShellIpc(raw);
      }} else {{
        queue.push(raw);
      }}
    }}
  }};
  setInterval(flush, 50);
  const markNativeShell = () => {{
    document.documentElement.setAttribute("data-capybara-native", "true");
    document.documentElement.setAttribute("data-capy-browser", "cef");
  }};
  if (document.documentElement) {{
    markNativeShell();
  }} else {{
    document.addEventListener("DOMContentLoaded", markNativeShell, {{ once: true }});
  }}
  window.__capyConsoleEvents = window.__capyConsoleEvents || [];
  window.__capyPageErrors = window.__capyPageErrors || [];
  if (!window.__capyErrorForwarded) {{
    window.addEventListener("error", (event) => {{
      window.__capyPageErrors.push({{
        type: "error",
        message: String(event.message || ""),
        source: String(event.filename || ""),
        line: event.lineno || 0,
        column: event.colno || 0
      }});
    }});
    window.addEventListener("unhandledrejection", (event) => {{
      const reason = event.reason;
      window.__capyPageErrors.push({{
        type: "unhandledrejection",
        message: String(reason && reason.stack ? reason.stack : reason)
      }});
    }});
    Object.defineProperty(window, "__capyErrorForwarded", {{ value: true }});
  }}
  if (!console.__capyForwarded) {{
    ["log", "warn", "error", "info"].forEach((level) => {{
      const original = console[level];
      console[level] = function(...args) {{
        try {{
          const payload = {{
            type: "console",
            level,
            args: args.map((arg) => {{
              if (typeof arg === "string") return arg;
              if (arg === undefined) return "undefined";
              try {{
                const encoded = JSON.stringify(arg);
                return encoded === undefined ? String(arg) : encoded;
              }} catch (_err) {{
                  return String(arg);
              }}
            }})
          }};
          window.__capyConsoleEvents.push(payload);
          window.ipc.postMessage(JSON.stringify(payload));
        }} catch (_err) {{}}
        return original.apply(console, args);
      }};
    }});
    Object.defineProperty(console, "__capyForwarded", {{ value: true }});
  }}
}})();"#,
        env!("CARGO_PKG_VERSION")
    )
}

fn frontend_root() -> Result<PathBuf, String> {
    let root = project_root()?.join("frontend/capy-app");
    let index = root.join("index.html");
    if !index.exists() {
        return Err(format!("frontend missing: {}", index.display()));
    }
    Ok(root)
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

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hi = hex_value(bytes[index + 1])?;
                let lo = hex_value(bytes[index + 2])?;
                out.push((hi << 4) | lo);
                index += 3;
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|err| format!("path is not UTF-8: {err}"))
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("invalid percent encoding".to_string()),
    }
}

fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'/' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => out.push(byte as char),
            other => {
                out.push('%');
                out.push_str(&format!("{other:02X}"));
            }
        }
    }
    out
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

fn create_temp_dir(prefix: &str) -> Result<PathBuf, String> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("{prefix}-{pid}-{nanos}"));
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    std::fs::canonicalize(&dir).map_err(|err| err.to_string())
}

fn path_to_string(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))
}

fn browser_subprocess_path() -> Result<Option<String>, String> {
    if let Ok(helper) = std::env::var("CAPY_CEF_HELPER") {
        return Ok(Some(helper));
    }
    let Some(path) = default_macos_helper_path() else {
        return Ok(None);
    };
    Ok(Some(path_to_string(&path)?))
}

fn default_macos_helper_path() -> Option<PathBuf> {
    #[cfg(not(target_os = "macos"))]
    {
        return None;
    }

    #[cfg(target_os = "macos")]
    {
        let exe = std::env::current_exe().ok()?;
        let exe_name = exe.file_name()?.to_str()?;
        let contents_dir = exe.parent()?.parent()?;
        if contents_dir.file_name()?.to_str()? != "Contents" {
            return None;
        }
        let helper_name = format!("{exe_name} Helper");
        let helper = contents_dir
            .join("Frameworks")
            .join(format!("{helper_name}.app"))
            .join("Contents")
            .join("MacOS")
            .join(helper_name);
        helper.exists().then_some(helper)
    }
}
