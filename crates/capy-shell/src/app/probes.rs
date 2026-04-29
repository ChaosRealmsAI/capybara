use std::path::Path;

use serde_json::{Value, json};

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
        r#"(function() {{
  const selector = {selector_json};
  const el = selector ? document.querySelector(selector) : document.documentElement;
  const target = el || document.documentElement;
  const rect = target.getBoundingClientRect();
  const width = Math.max(1, Math.round((selector && el ? rect.width : window.innerWidth) || 1));
  const height = Math.max(1, Math.round((selector && el ? rect.height : window.innerHeight) || 1));
  return {{ ok: true, width, height, dpr: 1, selector, found: !!el }};
}})()"#
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
    let width = value
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1)
        .clamp(1, 4096);
    let height = value
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1)
        .clamp(1, 4096);
    let png = encode_stub_png(width, height);
    if let Err(err) = write_png(Path::new(out), &png) {
        return error_response(req_id, format!("write screenshot failed: {err}"));
    }
    IpcResponse {
        req_id: req_id.to_string(),
        ok: true,
        data: Some(json!({
            "window_id": window_id,
            "region": region,
            "out": out,
            "width": width,
            "height": height,
            "bytes": png.len(),
            "format": "png",
            "probe": value
        })),
        error: None,
    }
}

fn write_png(path: &Path, png: &[u8]) -> Result<(), std::io::Error> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, png)
}

pub fn encode_stub_png(width: u32, height: u32) -> Vec<u8> {
    let row_len = 1usize + width as usize * 4;
    let mut raw = Vec::with_capacity(row_len * height as usize);
    for y in 0..height {
        raw.push(0);
        for x in 0..width {
            let shade = 28u8.saturating_add(((x + y) % 24) as u8);
            raw.extend_from_slice(&[shade, shade, 38, 255]);
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    push_chunk(&mut png, b"IHDR", &ihdr);
    push_chunk(&mut png, b"IDAT", &zlib_store(&raw));
    push_chunk(&mut png, b"IEND", &[]);
    png
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut offset = 0usize;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let block_len = remaining.min(65_535);
        let final_block = offset + block_len == data.len();
        out.push(if final_block { 0x01 } else { 0x00 });
        let len = block_len as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + block_len]);
        offset += block_len;
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn push_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(kind.len() + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    png.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

#[cfg(test)]
mod tests {
    use super::encode_stub_png;

    #[test]
    fn screenshot_png_has_valid_signature() {
        let png = encode_stub_png(2, 2);

        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(png.windows(4).any(|chunk| chunk == b"IHDR"));
        assert!(png.windows(4).any(|chunk| chunk == b"IDAT"));
        assert!(png.windows(4).any(|chunk| chunk == b"IEND"));
    }
}
