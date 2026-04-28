use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone)]
pub(crate) struct AssetServer {
    pub(crate) base_url: String,
}

static ASSET_SERVER: OnceLock<Result<AssetServer, String>> = OnceLock::new();

pub(crate) fn asset_server() -> Result<AssetServer, String> {
    ASSET_SERVER.get_or_init(start_asset_server).clone()
}

pub(crate) fn frontend_url(base_url: &str, project: &str, dpr: f32) -> String {
    format!(
        "{base_url}/index.html?project={}&dpr={dpr:.3}",
        url_encode(project)
    )
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
    let cwd = default_session_cwd()
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
    let candidates = frontend_root_candidates();
    for root in candidates {
        let index = root.join("index.html");
        if index.exists() {
            return Ok(root);
        }
    }
    Err(
        "frontend missing: set CAPY_FRONTEND_ROOT or bundle Contents/Resources/capy-app"
            .to_string(),
    )
}

fn frontend_root_candidates() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("CAPY_FRONTEND_ROOT").filter(|value| !value.is_empty()) {
        roots.push(PathBuf::from(root));
    }
    if let Some(root) = bundled_frontend_root() {
        roots.push(root);
    }
    if let Ok(root) = project_root() {
        roots.push(root.join("frontend/capy-app"));
    }
    roots
}

fn bundled_frontend_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let contents_dir = exe.parent()?.parent()?;
    (contents_dir.file_name()?.to_str()? == "Contents")
        .then(|| contents_dir.join("Resources").join("capy-app"))
}

fn default_session_cwd() -> Result<PathBuf, String> {
    if let Some(cwd) = std::env::var_os("CAPY_DEFAULT_CWD").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(cwd));
    }
    project_root().or_else(|_| std::env::current_dir().map_err(|err| err.to_string()))
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
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_rejects_invalid_escape() {
        assert!(percent_decode("bad%zz").is_err());
    }

    #[test]
    fn frontend_url_escapes_project_query_value() {
        let url = frontend_url("http://127.0.0.1:1", "demo project/alpha", 2.0);
        assert_eq!(
            url,
            "http://127.0.0.1:1/index.html?project=demo%20project/alpha&dpr=2.000"
        );
    }

    #[test]
    fn wasm_assets_use_wasm_mime_type() {
        assert_eq!(
            mime_for_path(Path::new("capy_canvas_web_bg.wasm")),
            "application/wasm"
        );
    }

    #[test]
    fn source_project_root_is_a_frontend_candidate() {
        let candidates = frontend_root_candidates();
        assert!(
            candidates
                .iter()
                .any(|path| path.ends_with("frontend/capy-app"))
        );
    }
}
