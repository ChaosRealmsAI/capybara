use std::fs;
use std::path::Path;

use capy_poster::{PosterDocument, PosterError, validate_document};
use serde::Serialize;
use serde_json::{Value, json};

use crate::compile::report::CompileError;
use crate::compose::{CompositionDocument, POSTER_COMPONENT_ID, SCROLL_CHAPTER_COMPONENT_ID};

pub const RENDER_SOURCE_SCHEMA: &str = "nf.render_source.v1";
const COMPONENT_ID: &str = "capy.poster-document";
const POSTER_COMPONENT_JS: &str = r##"export function mount(root) {
  root.textContent = "";
}

export function update(root, ctx) {
  const poster = ctx && ctx.params ? ctx.params.poster : null;
  if (!poster || !poster.canvas || !Array.isArray(poster.layers)) {
    root.textContent = "";
    root.dataset.renderState = "error";
    return;
  }
  root.dataset.renderState = "ready";
  root.dataset.capyPosterComponent = "capy.poster-document";
  root.style.position = "absolute";
  root.style.inset = "0";
  root.style.overflow = "hidden";
  root.style.background = poster.canvas.background || "#fff";

  const stage = document.createElement("div");
  stage.className = "capy-poster-stage";
  stage.dataset.posterVersion = String(poster.version || "");
  stage.style.position = "absolute";
  stage.style.inset = "0";
  stage.style.width = "100%";
  stage.style.height = "100%";
  stage.style.overflow = "hidden";
  stage.style.background = poster.canvas.background || "#fff";

  const layers = poster.layers.slice().sort((a, b) => Number(a.z || 0) - Number(b.z || 0));
  for (const layer of layers) {
    stage.appendChild(createLayer(layer, poster));
  }
  root.replaceChildren(stage);
}

function createLayer(layer, poster) {
  const element = document.createElement("div");
  element.className = "capy-poster-layer";
  element.dataset.layerId = String(layer.id || "");
  element.dataset.kind = String(layer.type || "");
  element.style.position = "absolute";
  element.style.left = px(layer.x);
  element.style.top = px(layer.y);
  element.style.width = px(layer.width);
  element.style.height = px(layer.height);
  element.style.zIndex = String(Number(layer.z || 0));
  element.style.boxSizing = "border-box";
  element.style.overflow = "hidden";
  element.style.pointerEvents = "none";
  applyStyle(element, layer.style || {}, layer);

  if (layer.type === "text") {
    element.textContent = String(layer.text || "");
    element.style.whiteSpace = "pre-line";
    element.style.display = "flex";
    element.style.alignItems = "flex-start";
    element.style.justifyContent = "flex-start";
    element.style.lineHeight = value(layer.style && layer.style.lineHeight, "1.05");
  } else if (layer.type === "image") {
    const asset = poster.assets && poster.assets[layer.assetId];
    const img = document.createElement("img");
    img.src = asset && asset.src ? String(asset.src) : "";
    img.alt = String(layer.id || "");
    img.style.width = "100%";
    img.style.height = "100%";
    img.style.objectFit = "contain";
    img.style.display = "block";
    element.appendChild(img);
  } else if (layer.type === "shape") {
    if ((layer.shape || "rect") === "ellipse") {
      element.style.borderRadius = "50%";
    }
  }
  return element;
}

function applyStyle(element, style, layer) {
  if (style.fill) element.style.background = String(style.fill);
  if (style.color) element.style.color = String(style.color);
  if (style.fontFamily) element.style.fontFamily = String(style.fontFamily);
  if (style.fontSize) element.style.fontSize = px(style.fontSize);
  if (style.fontWeight) element.style.fontWeight = String(style.fontWeight);
  if (style.opacity !== undefined) element.style.opacity = String(style.opacity);
  if (style.radius !== undefined) element.style.borderRadius = px(style.radius);
  if (style.blur !== undefined && Number(style.blur) > 0) {
    element.style.filter = "blur(" + px(style.blur) + ")";
  }
  if (layer.shape === "ellipse") element.style.borderRadius = "50%";
}

function px(value) {
  const number = Number(value || 0);
  return (Number.isFinite(number) ? number : 0) + "px";
}

function value(raw, fallback) {
  return raw === undefined || raw === null ? fallback : String(raw);
}
"##;

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddedRenderSourceReport {
    pub schema_version: String,
    pub component: String,
    pub duration_ms: u64,
    pub viewport: EmbeddedRenderViewport,
    pub layer_count: usize,
    pub asset_count: usize,
    pub generated_assets: usize,
    pub track_count: usize,
    pub source: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddedRenderViewport {
    pub w: u32,
    pub h: u32,
    pub ratio: String,
}

pub fn compile_embedded(
    composition: &CompositionDocument,
    out: &Path,
) -> Result<EmbeddedRenderSourceReport, CompileError> {
    if composition
        .tracks
        .iter()
        .any(|track| track.component == SCROLL_CHAPTER_COMPONENT_ID)
    {
        return compile_scroll_chapters(composition, out);
    }
    let poster_value = poster_param(composition)?;
    let poster: PosterDocument =
        serde_json::from_value(poster_value.clone()).map_err(invalid_poster_error)?;
    validate_document(&poster).map_err(invalid_poster_doc_error)?;
    let report = render_source(composition, &poster, poster_value);
    write_source(out, &report.source)?;
    Ok(report)
}

fn compile_scroll_chapters(
    composition: &CompositionDocument,
    out: &Path,
) -> Result<EmbeddedRenderSourceReport, CompileError> {
    let duration_ms = composition.duration_ms.max(1);
    let viewport = EmbeddedRenderViewport {
        w: composition.viewport.w,
        h: composition.viewport.h,
        ratio: composition.viewport.ratio.clone(),
    };
    let component_js = read_scroll_component(out)?;
    let source = scroll_render_source_json(composition, duration_ms, &viewport, component_js);
    let track_count = composition
        .tracks
        .iter()
        .filter(|track| track.component == SCROLL_CHAPTER_COMPONENT_ID)
        .count();
    write_source(out, &source)?;
    Ok(EmbeddedRenderSourceReport {
        schema_version: RENDER_SOURCE_SCHEMA.to_string(),
        component: SCROLL_CHAPTER_COMPONENT_ID.to_string(),
        duration_ms,
        viewport,
        layer_count: 0,
        asset_count: composition.assets.len(),
        generated_assets: 0,
        track_count,
        source,
    })
}

fn render_source(
    composition: &CompositionDocument,
    poster: &PosterDocument,
    poster_value: Value,
) -> EmbeddedRenderSourceReport {
    let duration_ms = composition.duration_ms.max(1);
    let viewport = EmbeddedRenderViewport {
        w: composition.viewport.w,
        h: composition.viewport.h,
        ratio: ratio(composition, poster),
    };
    let generated_assets = poster
        .assets
        .values()
        .filter(|asset| asset.provenance.is_some())
        .count();
    let source = render_source_json(composition, poster, poster_value, duration_ms, &viewport);
    EmbeddedRenderSourceReport {
        schema_version: RENDER_SOURCE_SCHEMA.to_string(),
        component: COMPONENT_ID.to_string(),
        duration_ms,
        viewport,
        layer_count: poster.layers.len(),
        asset_count: poster.assets.len(),
        generated_assets,
        track_count: 1,
        source,
    }
}

fn render_source_json(
    composition: &CompositionDocument,
    poster: &PosterDocument,
    poster_value: Value,
    duration_ms: u64,
    viewport: &EmbeddedRenderViewport,
) -> Value {
    json!({
        "schema_version": RENDER_SOURCE_SCHEMA,
        "duration_ms": duration_ms,
        "duration": duration_ms,
        "meta": meta_json(composition, poster, duration_ms),
        "viewport": viewport_json(viewport),
        "theme": theme_json(composition, poster),
        "assets": asset_manifest(poster),
        "components": {
            COMPONENT_ID: POSTER_COMPONENT_JS
        },
        "tracks": tracks_json(poster_value, duration_ms)
    })
}

fn scroll_render_source_json(
    composition: &CompositionDocument,
    duration_ms: u64,
    viewport: &EmbeddedRenderViewport,
    component_js: String,
) -> Value {
    json!({
        "schema_version": RENDER_SOURCE_SCHEMA,
        "duration_ms": duration_ms,
        "duration": duration_ms,
        "meta": {
            "name": composition.name,
            "project": "capybara",
            "composition": composition.id,
            "render_source_schema": RENDER_SOURCE_SCHEMA,
            "duration_ms": duration_ms,
            "source_document_type": "capy-scroll-media"
        },
        "viewport": viewport_json(viewport),
        "theme": {
            "background": "#111827",
            "css": ":root { --capy-scroll-background: #111827; } body { background: #111827; }"
        },
        "assets": composition.assets,
        "components": {
            SCROLL_CHAPTER_COMPONENT_ID: component_js
        },
        "tracks": scroll_tracks_json(composition)
    })
}

fn scroll_tracks_json(composition: &CompositionDocument) -> Value {
    Value::Array(
        composition
            .tracks
            .iter()
            .filter(|track| track.component == SCROLL_CHAPTER_COMPONENT_ID)
            .map(|track| {
                let begin_ms = parse_ms(&track.time.start).unwrap_or(0);
                let end_ms = parse_ms(&track.time.end).unwrap_or(begin_ms + track.duration_ms);
                json!({
                    "id": track.id,
                    "kind": track.kind,
                    "z": track.z,
                    "clips": [{
                        "id": format!("{}.clip", track.id),
                        "begin": begin_ms,
                        "begin_ms": begin_ms,
                        "end": end_ms,
                        "end_ms": end_ms,
                        "params": {
                            "component": SCROLL_CHAPTER_COMPONENT_ID,
                            "params": track.params,
                            "style": {},
                            "track": {
                                "id": track.id,
                                "kind": "component",
                                "source": "capy-scroll-media"
                            }
                        }
                    }]
                })
            })
            .collect(),
    )
}

fn parse_ms(value: &str) -> Option<u64> {
    value
        .strip_suffix("ms")
        .unwrap_or(value)
        .parse::<u64>()
        .ok()
}

fn read_scroll_component(out: &Path) -> Result<String, CompileError> {
    let component_path = out
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("components")
        .join("html.capy-scroll-chapter.js");
    fs::read_to_string(&component_path).map_err(|err| {
        CompileError::new(
            "COMPILE_FAILED",
            "$.components.html.capy-scroll-chapter",
            format!("read scroll chapter component failed: {err}"),
            "next step · rerun capy media scroll-pack --emit-composition",
        )
    })
}

fn meta_json(
    composition: &CompositionDocument,
    poster: &PosterDocument,
    duration_ms: u64,
) -> Value {
    json!({
        "name": composition.name,
        "project": "capybara",
        "composition": composition.id,
        "version": poster.version,
        "render_source_schema": RENDER_SOURCE_SCHEMA,
        "duration_ms": duration_ms,
        "source_document_type": poster.doc_type
    })
}

fn viewport_json(viewport: &EmbeddedRenderViewport) -> Value {
    json!({
        "w": viewport.w,
        "h": viewport.h,
        "ratio": viewport.ratio
    })
}

fn theme_json(composition: &CompositionDocument, poster: &PosterDocument) -> Value {
    let mut theme = json!({
        "background": poster.canvas.background,
        "css": poster_theme_css(&poster.canvas.background)
    });
    if let Some(brand) = &composition.theme {
        theme["tokens_ref"] = json!(brand.tokens_ref);
        theme["source_path"] = json!(brand.source_path);
        theme["hash"] = json!(brand.hash);
    }
    theme
}

fn tracks_json(poster_value: Value, duration_ms: u64) -> Value {
    json!([{
        "id": "poster.document",
        "kind": "component",
        "z": 0,
        "clips": [{
            "id": "poster.document.frame",
            "begin": 0,
            "begin_ms": 0,
            "end": duration_ms,
            "end_ms": duration_ms,
            "params": {
                "component": COMPONENT_ID,
                "params": {
                    "poster": poster_value
                },
                "style": {},
                "track": {
                    "id": "poster.document",
                    "kind": "component",
                    "source": "capy-nextframe"
                }
            }
        }]
    }])
}

fn poster_param(composition: &CompositionDocument) -> Result<Value, CompileError> {
    let track = composition
        .tracks
        .iter()
        .find(|track| track.component == POSTER_COMPONENT_ID)
        .ok_or_else(|| {
            CompileError::new(
                "INVALID_COMPOSITION",
                "$.tracks",
                "composition has no html.capy-poster track",
                "next step · rerun capy nextframe compose-poster",
            )
        })?;
    track.params.get("poster").cloned().ok_or_else(|| {
        CompileError::new(
            "INVALID_COMPOSITION",
            "$.tracks[].params.poster",
            "poster component track is missing params.poster",
            "next step · rerun capy nextframe compose-poster",
        )
    })
}

fn ratio(composition: &CompositionDocument, poster: &PosterDocument) -> String {
    if !composition.viewport.ratio.trim().is_empty() {
        composition.viewport.ratio.clone()
    } else if !poster.canvas.aspect_ratio.trim().is_empty() {
        poster.canvas.aspect_ratio.clone()
    } else {
        format!("{}:{}", poster.canvas.width, poster.canvas.height)
    }
}

fn poster_theme_css(background: &str) -> String {
    format!(
        ":root {{ --capy-poster-background: {}; }} body {{ background: {}; }}",
        background, background
    )
}

fn asset_manifest(document: &PosterDocument) -> Value {
    Value::Array(
        document
            .assets
            .iter()
            .map(|(id, asset)| {
                json!({
                    "id": id,
                    "type": asset.asset_type,
                    "src": asset.src,
                    "mask": asset.mask,
                    "provenance": asset.provenance
                })
            })
            .collect(),
    )
}

fn write_source(path: &Path, source: &Value) -> Result<(), CompileError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            CompileError::new(
                "COMPILE_FAILED",
                "$.render_source_path",
                format!("create render_source parent failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    let text = serde_json::to_string_pretty(source).map_err(|err| {
        CompileError::new(
            "COMPILE_FAILED",
            "$.render_source",
            format!("serialize render_source failed: {err}"),
            "next step · inspect composition JSON values",
        )
    })?;
    fs::write(path, text).map_err(|err| {
        CompileError::new(
            "COMPILE_FAILED",
            "$.render_source_path",
            format!("write render_source failed: {err}"),
            "next step · check output directory permissions",
        )
    })
}

fn invalid_poster_error(err: serde_json::Error) -> CompileError {
    CompileError::new(
        "INVALID_COMPOSITION",
        "$.tracks[].params.poster",
        format!("poster params are invalid: {err}"),
        "next step · rerun capy nextframe compose-poster",
    )
}

fn invalid_poster_doc_error(err: PosterError) -> CompileError {
    CompileError::new(
        "INVALID_COMPOSITION",
        "$.tracks[].params.poster",
        format!("poster params failed validation: {err}"),
        "next step · rerun capy nextframe compose-poster",
    )
}
