use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct TimelinePreviewServer {
    base_url: String,
    registry: Arc<Mutex<BTreeMap<String, PreviewRoot>>>,
    shutdown: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
struct PreviewRoot {
    root: PathBuf,
    composition_path: PathBuf,
}

impl TimelinePreviewServer {
    pub(crate) fn start() -> Self {
        match start_server() {
            Ok(server) => server,
            Err(err) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "event": "timeline-preview-server-error",
                        "detail": err
                    })
                );
                Self {
                    base_url: String::new(),
                    registry: Arc::new(Mutex::new(BTreeMap::new())),
                    shutdown: Arc::new(AtomicBool::new(true)),
                }
            }
        }
    }

    pub(crate) fn register(
        &self,
        canvas_node_id: u64,
        composition_path: &Path,
    ) -> Result<String, String> {
        if self.base_url.is_empty() {
            return Err(preview_error(
                "DESKTOP_HOST_FAILED",
                "Timeline preview server is unavailable",
                "next step · restart capy shell",
            ));
        }
        let composition_path = composition_path.canonicalize().map_err(|err| {
            preview_error(
                "COMPOSITION_NOT_FOUND",
                format!("composition path is not readable: {err}"),
                "next step · run capy timeline attach",
            )
        })?;
        let root = materialized_root(&composition_path)?;
        let slug = preview_slug(canvas_node_id, &root);
        let mut registry = self.registry.lock().map_err(|_| {
            preview_error(
                "DESKTOP_HOST_FAILED",
                "Timeline preview registry lock failed",
                "next step · restart capy shell",
            )
        })?;
        registry.entry(slug.clone()).or_insert(PreviewRoot {
            root,
            composition_path,
        });
        Ok(format!("{}/{slug}/index.html", self.base_url))
    }
}

impl Drop for TimelinePreviewServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}

fn start_server() -> Result<TimelinePreviewServer, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("Timeline preview bind failed: {err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("Timeline preview nonblocking failed: {err}"))?;
    let addr = listener
        .local_addr()
        .map_err(|err| format!("Timeline preview addr failed: {err}"))?;
    let registry = Arc::new(Mutex::new(BTreeMap::new()));
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_registry = Arc::clone(&registry);
    let thread_shutdown = Arc::clone(&shutdown);
    std::thread::Builder::new()
        .name("capy-timeline-preview".to_string())
        .spawn(move || {
            while !thread_shutdown.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        let registry = Arc::clone(&thread_registry);
                        let _handle = std::thread::Builder::new()
                            .name("capy-timeline-preview-request".to_string())
                            .spawn(move || {
                                if let Err(err) = handle_http_stream(stream, registry) {
                                    println!(
                                        "{}",
                                        serde_json::json!({
                                            "event": "timeline-preview-request-error",
                                            "detail": err
                                        })
                                    );
                                }
                            });
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(_err) => break,
                }
            }
        })
        .map_err(|err| format!("Timeline preview thread failed: {err}"))?;
    Ok(TimelinePreviewServer {
        base_url: format!("http://{addr}"),
        registry,
        shutdown,
    })
}

fn handle_http_stream(
    mut stream: TcpStream,
    registry: Arc<Mutex<BTreeMap<String, PreviewRoot>>>,
) -> Result<(), String> {
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
    let response = preview_response(&registry, target)?;
    write_http_response(
        &mut stream,
        response.status,
        response.reason,
        response.content_type,
        response.body,
        method == "HEAD",
    )
}

struct HttpResponse {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: Vec<u8>,
}

fn preview_response(
    registry: &Arc<Mutex<BTreeMap<String, PreviewRoot>>>,
    target: &str,
) -> Result<HttpResponse, String> {
    let (path, _) = target.split_once('?').unwrap_or((target, ""));
    let path = path.trim_start_matches('/');
    let (slug, route) = path.split_once('/').unwrap_or((path, ""));
    let preview = registry
        .lock()
        .map_err(|_| "preview registry lock failed".to_string())?
        .get(slug)
        .cloned();
    let Some(preview) = preview else {
        return Ok(text_response(404, "Not Found", "preview not registered"));
    };
    match route {
        "" | "index.html" => Ok(HttpResponse {
            status: 200,
            reason: "OK",
            content_type: "text/html; charset=utf-8",
            body: preview_index_html(slug).into_bytes(),
        }),
        "composition.json" => file_response(&preview.composition_path),
        "render_source.json" => file_response(&render_source_path(&preview.composition_path)),
        route if route.starts_with("components/") => {
            file_response(&preview.root.join(percent_decode(route)?))
        }
        route if route.starts_with("assets/") => asset_response(&preview.root, route),
        _ => Ok(text_response(404, "Not Found", "preview asset not found")),
    }
}

fn file_response(path: &Path) -> Result<HttpResponse, String> {
    let body = fs::read(path).map_err(|err| format!("preview file read failed: {err}"))?;
    Ok(HttpResponse {
        status: 200,
        reason: "OK",
        content_type: mime_for_path(path),
        body,
    })
}

fn asset_response(root: &Path, route: &str) -> Result<HttpResponse, String> {
    let decoded = percent_decode(route)?;
    let candidate = root.join(&decoded);
    if candidate.is_file() {
        return file_response(&candidate);
    }
    file_response(&root.join("components").join(decoded))
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

fn preview_index_html(slug: &str) -> String {
    format!(
        r##"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Timeline Preview</title>
  <style>
    html, body, #root {{ width: 100%; height: 100%; margin: 0; overflow: hidden; background: transparent; }}
    #root {{ position: relative; }}
  </style>
</head>
<body data-capy-timeline-preview-slug="{slug}">
  <div id="root"></div>
  <script type="module">
    const root = document.getElementById("root");
    const source = await fetch("./render_source.json", {{ cache: "no-store" }}).then((response) => response.json());
    const modules = new Map();
    const mounted = new Map();
    let currentTimeMs = 0;

    root.style.position = "relative";
    root.style.overflow = "hidden";
    root.style.background = source.theme && source.theme.background ? String(source.theme.background) : "#000";

    async function componentModule(id) {{
      if (modules.has(id)) return modules.get(id);
      const sourceText = source.components && source.components[id];
      if (!sourceText) throw new Error(`missing component: ${{id}}`);
      const blob = new Blob([String(sourceText)], {{ type: "text/javascript" }});
      const url = URL.createObjectURL(blob);
      const module = await import(url);
      modules.set(id, module);
      return module;
    }}

    function clipStart(clip) {{
      return Number(clip.begin_ms ?? clip.begin ?? 0);
    }}

    function clipEnd(clip) {{
      return Number(clip.end_ms ?? clip.end ?? clipStart(clip));
    }}

    function clipContext(track, clip) {{
      const params = clip.params || {{}};
      const begin = clipStart(clip);
      const end = clipEnd(clip);
      const duration = Math.max(1, end - begin);
      const localTime = Math.max(0, Math.min(duration, currentTimeMs - begin));
      return {{
        timeMs: currentTimeMs,
        localTimeMs: localTime,
        progress: localTime / duration,
        durationMs: duration,
        params: params.params || params,
        style: params.style || {{}},
        track: params.track || {{ id: track.id, kind: track.kind }},
        theme: source.theme || {{}},
        viewport: source.viewport || {{}},
        mode: "preview"
      }};
    }}

    async function render() {{
      const active = new Set();
      const tracks = Array.isArray(source.tracks) ? source.tracks.slice() : [];
      tracks.sort((left, right) => Number(left.z || 0) - Number(right.z || 0));
      for (const track of tracks) {{
        const clips = Array.isArray(track.clips) ? track.clips : [];
        for (const clip of clips) {{
          const begin = clipStart(clip);
          const end = clipEnd(clip);
          if (currentTimeMs < begin || currentTimeMs > end) continue;
          const params = clip.params || {{}};
          const componentId = params.component;
          if (!componentId) continue;
          const key = `${{track.id}}::${{clip.id}}`;
          active.add(key);
          let entry = mounted.get(key);
          if (!entry) {{
            const el = document.createElement("div");
            el.dataset.trackId = String(track.id || "");
            el.dataset.clipId = String(clip.id || "");
            el.style.position = "absolute";
            el.style.inset = "0";
            el.style.zIndex = String(Number(track.z || 0));
            el.style.pointerEvents = "none";
            root.appendChild(el);
            const module = await componentModule(componentId);
            entry = {{ el, module }};
            mounted.set(key, entry);
            entry.module.mount && entry.module.mount(entry.el, clipContext(track, clip));
          }}
          entry.module.update && entry.module.update(entry.el, clipContext(track, clip));
        }}
      }}
      for (const [key, entry] of mounted) {{
        if (active.has(key)) continue;
        entry.module.destroy && entry.module.destroy(entry.el);
        entry.el.remove();
        mounted.delete(key);
      }}
      document.body.dataset.previewReady = "true";
      document.body.dataset.currentTimeMs = String(currentTimeMs);
    }}

    window.addEventListener("message", (event) => {{
      const data = event.data || {{}};
      if (data.type !== "capy-timeline-set-time") return;
      currentTimeMs = Math.max(0, Number(data.time_ms || 0));
      render().catch((error) => {{
        document.body.dataset.previewError = String(error && error.message || error);
      }});
    }});

    render().catch((error) => {{
      document.body.dataset.previewError = String(error && error.message || error);
      root.textContent = document.body.dataset.previewError;
    }});
  </script>
</body>
</html>
"##
    )
}

fn materialized_root(composition_path: &Path) -> Result<PathBuf, String> {
    let parent = composition_path.parent().ok_or_else(|| {
        preview_error(
            "COMPOSITION_NOT_FOUND",
            "composition path has no parent directory",
            "next step · run capy timeline attach",
        )
    })?;
    let root = if parent.file_name().and_then(|name| name.to_str()) == Some("compositions") {
        parent.parent().unwrap_or(parent)
    } else {
        parent
    };
    root.canonicalize().map_err(|err| {
        preview_error(
            "COMPOSITION_NOT_FOUND",
            format!("materialized root is not readable: {err}"),
            "next step · run capy timeline attach",
        )
    })
}

fn render_source_path(composition_path: &Path) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("render_source.json")
}

fn preview_slug(canvas_node_id: u64, root: &Path) -> String {
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("preview");
    format!("node-{canvas_node_id}-{}", sanitize_slug(name))
}

fn sanitize_slug(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            slug.push(ch);
        } else {
            slug.push('-');
        }
    }
    if slug.is_empty() {
        "preview".to_string()
    } else {
        slug
    }
}

fn percent_decode(value: &str) -> Result<PathBuf, String> {
    if value.contains("..") || value.starts_with('/') || value.starts_with('\\') {
        return Err("unsafe preview path".to_string());
    }
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3])
                .map_err(|err| format!("invalid percent escape: {err}"))?;
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|err| format!("invalid percent escape: {err}"))?;
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out)
        .map(PathBuf::from)
        .map_err(|err| format!("preview path is not UTF-8: {err}"))
}

fn mime_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn text_response(status: u16, reason: &'static str, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        reason,
        content_type: "text/plain; charset=utf-8",
        body: body.as_bytes().to_vec(),
    }
}

fn preview_error(code: &str, message: impl Into<String>, hint: &str) -> String {
    serde_json::json!({
        "code": code,
        "message": message.into(),
        "hint": hint
    })
    .to_string()
}
