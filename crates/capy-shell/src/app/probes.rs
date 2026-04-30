use std::path::Path;

use serde_json::{Value, json};

use crate::capture::{self, CaptureRegion};
use crate::ipc::{IpcResponse, error_response};

pub(super) fn devtools_script(query: &str, get: &str) -> String {
    let query_json = json_string(query);
    let get_json = json_string(get);
    format!(
        r#"(function() {{
  function reply(value) {{ return JSON.stringify(value); }}
  const selector = {query_json};
  const get = {get_json};
  const el = document.querySelector(selector);
  if (!el) return reply({{ ok: false, error: "selector not found: " + selector }});
  let value;
  if (get === "bounding-rect") {{
    const rect = el.getBoundingClientRect();
    value = {{ x: rect.x, y: rect.y, width: rect.width, height: rect.height }};
  }} else if (get === "outerHTML") {{
    value = el.outerHTML;
  }} else {{
    value = el[get];
  }}
  return reply({{ ok: true, selector, get, value }});
}})()"#
    )
}

pub(super) fn state_script(key: &str) -> String {
    let key_json = json_string(key);
    format!(
        r#"(function() {{
  function reply(value) {{ return JSON.stringify(value); }}
  const key = {key_json};
  const state = window.CAPYBARA_STATE || {{}};
  const canvas = state.canvas || {{}};
  const planner = state.planner || {{}};
  let value = null;
  if (key === "canvas.ready") value = !!canvas.ready;
  else if (key === "canvas.nodeCount") value = Number(canvas.nodeCount || (Array.isArray(state.blocks) ? state.blocks.length : 0));
  else if (key === "canvas.selectedNode") value = canvas.selectedNode || null;
  else if (key === "canvas.selected-id") value = state.selectedId || null;
  else if (key === "canvas.block-count") value = Array.isArray(state.blocks) ? state.blocks.length : 0;
  else if (key === "canvas.currentTool") value = canvas.currentTool || null;
  else if (key === "canvas.snapshotText") value = canvas.snapshotText || "";
  else if (key === "canvas.context") value = state.canvasContext || planner.canvasContext || null;
  else if (key === "planner.context") value = planner.context || null;
  else if (key === "planner.canvasContext") value = planner.canvasContext || null;
  else if (key === "planner.status") value = planner.contextText ? "context-ready" : "idle";
  else return reply({{ ok: false, error: "unknown state key: " + key }});
  return reply({{ ok: true, key, value }});
}})()"#
    )
}

pub(super) fn screenshot_probe_script(region: &str) -> String {
    let selector = match region {
        "canvas" => "[data-section=\"canvas-host\"]",
        "planner" => "[data-section=\"planner-chat\"]",
        "topbar" => ".topbar",
        _ => "",
    };
    let selector_json = json_string(selector);
    format!(
        r##"(async function() {{
  const captureTimeoutMs = 15000;
  const captureWork = (async () => {{
  await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  const selector = {selector_json};
  const el = selector ? document.querySelector(selector) : document.documentElement;
  const target = el || document.documentElement;
  const rect = target.getBoundingClientRect();
  const full = !(selector && el);
  const viewportWidth = Math.max(1, window.innerWidth || document.documentElement.clientWidth || 1);
  const viewportHeight = Math.max(1, window.innerHeight || document.documentElement.clientHeight || 1);
  const x = full ? 0 : rect.x;
  const y = full ? 0 : rect.y;
  const width = Math.max(1, full ? viewportWidth : Math.min(rect.width, viewportWidth - Math.max(0, x)));
  const height = Math.max(1, full ? viewportHeight : Math.min(rect.height, viewportHeight - Math.max(0, y)));
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  const pixelWidth = Math.max(1, Math.ceil(width * dpr));
  const pixelHeight = Math.max(1, Math.ceil(height * dpr));
  const bodyClone = document.body.cloneNode(true);
  bodyClone.querySelectorAll("script").forEach((node) => node.remove());

  const sourceCanvases = Array.from(document.querySelectorAll("canvas"));
  const clonedCanvases = Array.from(bodyClone.querySelectorAll("canvas"));
  for (let i = 0; i < clonedCanvases.length; i += 1) {{
    const sourceCanvas = sourceCanvases[i];
    const clonedCanvas = clonedCanvases[i];
    const image = document.createElement("img");
    image.setAttribute("alt", "canvas snapshot");
    image.setAttribute("data-capy-canvas-snapshot", "true");
    try {{
      image.src = sourceCanvas && sourceCanvas.toDataURL ? sourceCanvas.toDataURL("image/png") : "";
    }} catch (_err) {{
      image.src = "data:image/png;base64,iVBORw0KGgo=";
    }}
    const sourceRect = sourceCanvas ? sourceCanvas.getBoundingClientRect() : clonedCanvas.getBoundingClientRect();
    image.style.width = `${{Math.max(1, sourceRect.width || clonedCanvas.width || 1)}}px`;
    image.style.height = `${{Math.max(1, sourceRect.height || clonedCanvas.height || 1)}}px`;
    image.style.display = getComputedStyle(clonedCanvas).display || "block";
    clonedCanvas.replaceWith(image);
  }}

  const seenStyleSheets = new Set();
  function collectCssRules(sheet) {{
    if (!sheet || seenStyleSheets.has(sheet)) return "";
    seenStyleSheets.add(sheet);
    try {{
      return Array.from(sheet.cssRules || []).map((rule) => {{
        if (rule.styleSheet) return collectCssRules(rule.styleSheet);
        return rule.cssText || "";
      }}).join("\n");
    }} catch (_err) {{
      return "";
    }}
  }}
  const cssText = Array.from(document.styleSheets).map(collectCssRules).join("\n");
  const background = getComputedStyle(document.body).backgroundColor || "#111";
  const wrapper = document.createElement("div");
  wrapper.setAttribute("xmlns", "http://www.w3.org/1999/xhtml");
  wrapper.style.width = `${{width}}px`;
  wrapper.style.height = `${{height}}px`;
  wrapper.style.overflow = "hidden";
  wrapper.style.background = background;
  const style = document.createElement("style");
  style.textContent = cssText;
  wrapper.appendChild(style);
  const translated = document.createElement("div");
  translated.style.width = `${{viewportWidth}}px`;
  translated.style.height = `${{viewportHeight}}px`;
  translated.style.transformOrigin = "top left";
  translated.style.transform = `translate(${{-Math.max(0, x)}}px, ${{-Math.max(0, y)}}px)`;
  translated.appendChild(bodyClone);
  wrapper.appendChild(translated);

  const serialized = new XMLSerializer().serializeToString(wrapper);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${{width}}" height="${{height}}" viewBox="0 0 ${{width}} ${{height}}"><foreignObject x="0" y="0" width="${{width}}" height="${{height}}">${{serialized}}</foreignObject></svg>`;
  const image = new Image();
  const data = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(svg);
  await new Promise((resolve, reject) => {{
    image.onload = resolve;
    image.onerror = () => reject(new Error("app-view self-render SVG failed to load"));
    image.src = data;
  }});
  const canvas = document.createElement("canvas");
  canvas.width = pixelWidth;
  canvas.height = pixelHeight;
  const ctx = canvas.getContext("2d");
  ctx.scale(dpr, dpr);
  ctx.fillStyle = background;
  ctx.fillRect(0, 0, width, height);
  ctx.drawImage(image, 0, 0, width, height);
  const dataUrl = canvas.toDataURL("image/png");
  return {{
    ok: true,
    dataUrl,
    x,
    y,
    width,
    height,
    pixelWidth,
    pixelHeight,
    viewportWidth,
    viewportHeight,
    dpr,
    selector,
    found: !!el,
    renderer: "cef-dom-self-render"
  }};
  }})();
  const timeout = new Promise((_, reject) => {{
    setTimeout(() => reject(new Error(`app-view self-render timed out after ${{captureTimeoutMs}}ms`)), captureTimeoutMs);
  }});
  return await Promise.race([captureWork, timeout]);
}})()"##
    )
}

pub(super) fn screenshot_response(
    req_id: &str,
    window_id: &str,
    region: &str,
    out: &str,
    raw: &str,
) -> IpcResponse {
    let value = match serde_json::from_str::<Value>(raw) {
        Ok(value) => value,
        Err(err) => return error_response(req_id, format!("invalid screenshot probe JSON: {err}")),
    };
    if value.get("ok").and_then(Value::as_bool) == Some(false) {
        let detail = value
            .get("error")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .unwrap_or("unknown screenshot probe failure");
        return error_response(req_id, format!("screenshot probe failed: {detail}"));
    }
    let capture_region = match capture_region_from_probe(&value) {
        Ok(region) => region,
        Err(err) => return error_response(req_id, err),
    };
    let data_url = match string_field(&value, "dataUrl") {
        Ok(value) => value,
        Err(err) => return error_response(req_id, err),
    };
    let width = match usize_field(&value, "pixelWidth") {
        Ok(value) => value,
        Err(err) => return error_response(req_id, err),
    };
    let height = match usize_field(&value, "pixelHeight") {
        Ok(value) => value,
        Err(err) => return error_response(req_id, err),
    };
    let capture = match capture::capture_png_data_url(Path::new(out), data_url, width, height) {
        Ok(capture) => capture,
        Err(err) => return error_response(req_id, err),
    };
    let mut probe = value.clone();
    if let Some(object) = probe.as_object_mut() {
        object.remove("dataUrl");
    }
    IpcResponse {
        req_id: req_id.to_string(),
        ok: true,
        data: Some(json!({
            "window_id": window_id,
            "source": "app-view",
            "region": region,
            "capture": {
                "kind": "cef-dom-self-render",
                "renderer": value.get("renderer").cloned().unwrap_or_else(|| json!("unknown")),
                "viewport_width": capture_region.viewport_width,
                "viewport_height": capture_region.viewport_height,
                "dpr": capture_region.dpr
            },
            "crop": {
                "x": capture_region.x,
                "y": capture_region.y,
                "width": capture_region.width,
                "height": capture_region.height,
                "viewport_width": capture_region.viewport_width,
                "viewport_height": capture_region.viewport_height,
                "dpr": capture_region.dpr
            },
            "out": capture.out.display().to_string(),
            "width": capture.width,
            "height": capture.height,
            "bytes": capture.bytes,
            "format": "png",
            "probe": probe
        })),
        error: None,
    }
}

fn capture_region_from_probe(value: &Value) -> Result<CaptureRegion, String> {
    Ok(CaptureRegion {
        x: number_field(value, "x").unwrap_or(0.0),
        y: number_field(value, "y").unwrap_or(0.0),
        width: number_field(value, "width")?,
        height: number_field(value, "height")?,
        viewport_width: number_field(value, "viewportWidth")?,
        viewport_height: number_field(value, "viewportHeight")?,
        dpr: number_field(value, "dpr").unwrap_or(0.0),
    })
}

fn number_field(value: &Value, key: &str) -> Result<f64, String> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("screenshot probe missing numeric `{key}`"))
}

fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| format!("screenshot probe missing string `{key}`"))
}

fn usize_field(value: &Value, key: &str) -> Result<usize, String> {
    let number = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("screenshot probe missing integer `{key}`"))?;
    usize::try_from(number).map_err(|_| format!("screenshot probe `{key}` is too large"))
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn capture_region_from_probe_reads_dom_rect() -> Result<(), String> {
        let value = json!({
            "x": 12.5,
            "y": 20.0,
            "width": 320.0,
            "height": 180.0,
            "viewportWidth": 1440.0,
            "viewportHeight": 900.0,
            "dpr": 2.0
        });

        let region = super::capture_region_from_probe(&value)?;

        assert_eq!(region.x, 12.5);
        assert_eq!(region.width, 320.0);
        assert_eq!(region.viewport_height, 900.0);
        assert_eq!(region.dpr, 2.0);
        Ok(())
    }

    #[test]
    fn usize_field_rejects_missing_values() -> Result<(), String> {
        let value = json!({});

        let error = match super::usize_field(&value, "pixelWidth") {
            Ok(_) => return Err("missing pixelWidth should fail".to_string()),
            Err(error) => error,
        };

        assert!(error.contains("pixelWidth"));
        Ok(())
    }

    #[test]
    fn screenshot_response_reports_probe_error_before_file_write() -> Result<(), String> {
        let response = super::screenshot_response(
            "req-1",
            "w-1",
            "full",
            "/tmp/capybara-should-not-write.png",
            r#"{"ok":false,"error":"app-view self-render timed out after 15000ms"}"#,
        );

        assert!(!response.ok);
        let error = response
            .error
            .ok_or_else(|| "expected screenshot response error".to_string())?;
        let detail = error
            .get("detail")
            .and_then(|value| value.as_str())
            .ok_or_else(|| "expected error detail".to_string())?;
        assert!(detail.contains("screenshot probe failed"));
        assert!(detail.contains("timed out"));
        Ok(())
    }
}
