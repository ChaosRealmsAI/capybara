mod composition;
mod mapping;
mod poster;
mod slug;

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

pub use composition::{
    CAPY_COMPOSITION_SCHEMA_VERSION, COMPOSITION_SCHEMA, CompositionAsset, CompositionDocument,
    CompositionTheme, CompositionTime, CompositionTrack, CompositionViewport, POSTER_COMPONENT_ID,
    SCROLL_CHAPTER_COMPONENT_ID,
};
pub use slug::poster_slug;

use crate::asset::{
    AssetMaterializationError, AssetMaterializationWarning, MaterializeAssetsRequest,
    materialize_assets,
};
use crate::compose::mapping::poster_to_composition;
use crate::compose::poster::read_poster;
use crate::error::{ErrorBody, NextFrameError, NextFrameErrorCode};
use crate::ports::CompositionArtifact;

const DEFAULT_DURATION_MS: u64 = 1000;
const DEFAULT_COMPOSITION_ID: &str = "poster-snapshot";
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
  root.dataset.capyPosterComponent = "html.capy-poster";
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

export function destroy(root) {
  root.textContent = "";
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

function px(raw) {
  const number = Number(raw || 0);
  return (Number.isFinite(number) ? number : 0) + "px";
}

function value(raw, fallback) {
  return raw === undefined || raw === null ? fallback : String(raw);
}
"##;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposePosterRequest {
    pub poster_path: PathBuf,
    pub brand_tokens_path: Option<PathBuf>,
    pub project_slug: Option<String>,
    pub composition_id: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompositionArtifactReport {
    pub project_slug: String,
    pub composition_id: String,
    pub project_root: PathBuf,
    pub composition_path: PathBuf,
    pub component_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComposePosterResult {
    pub ok: bool,
    pub trace_id: String,
    pub stage: &'static str,
    pub project_root: PathBuf,
    pub composition_path: PathBuf,
    pub components: Vec<String>,
    pub layers: usize,
    pub assets: usize,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<AssetMaterializationError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<AssetMaterializationWarning>,
    #[serde(skip_serializing)]
    pub artifact: CompositionArtifactReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ComposePosterFailure {
    pub ok: bool,
    pub trace_id: String,
    pub stage: &'static str,
    pub error: ErrorBody,
}

pub fn compose_poster(req: ComposePosterRequest) -> Result<ComposePosterResult, NextFrameError> {
    let poster = read_poster(&req.poster_path)?;
    let project_slug = req
        .project_slug
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| slug::poster_slug(poster.id(), poster.title()));
    let composition_id = req
        .composition_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_COMPOSITION_ID.to_string());
    let output_dir = req.output_dir.unwrap_or_else(|| {
        PathBuf::from("target")
            .join("capy-nextframe")
            .join(&project_slug)
    });
    let duration_ms = if req.duration_ms == 0 {
        DEFAULT_DURATION_MS
    } else {
        req.duration_ms
    };
    let project_root = prepare_project_root(output_dir)?;
    let mut composition = poster_to_composition(&poster, composition_id.clone(), duration_ms);
    let materialized = materialize_assets(MaterializeAssetsRequest {
        poster: &poster.document,
        poster_raw: &poster.raw,
        poster_path: &req.poster_path,
        project_root: &project_root,
    });
    composition.assets = materialized.assets.clone();
    if let Some(track) = composition.tracks.get_mut(0) {
        track
            .params
            .insert("poster".to_string(), materialized.rewritten_poster.clone());
    }
    if let Some(path) = req.brand_tokens_path {
        composition.theme = Some(crate::brand::copy_tokens(&path, &project_root)?);
    }
    let component_path = write_component(&project_root)?;
    let composition_path = project_root.join("composition.json");
    write_json(&composition_path, &composition)?;
    let artifact = CompositionArtifact {
        project_slug: project_slug.clone(),
        composition_id: composition_id.clone(),
        project_root: project_root.clone(),
        composition_path: composition_path.clone(),
        component_paths: vec![component_path.clone()],
    };

    let ok = materialized.errors.is_empty();

    Ok(ComposePosterResult {
        ok,
        trace_id: trace_id("compose"),
        stage: "compose-poster",
        project_root,
        composition_path,
        components: vec![POSTER_COMPONENT_ID.to_string()],
        layers: poster.document.layers.len(),
        assets: composition.assets.len(),
        duration_ms,
        theme_hash: composition.theme.as_ref().map(|theme| theme.hash.clone()),
        errors: materialized.errors,
        warnings: materialized.warnings,
        artifact: CompositionArtifactReport {
            project_slug: artifact.project_slug,
            composition_id: artifact.composition_id,
            project_root: artifact.project_root,
            composition_path: artifact.composition_path,
            component_paths: artifact.component_paths,
        },
    })
}

pub fn failure(err: NextFrameError) -> ComposePosterFailure {
    ComposePosterFailure {
        ok: false,
        trace_id: trace_id("compose"),
        stage: "compose-poster",
        error: err.body,
    }
}

fn prepare_project_root(output_dir: PathBuf) -> Result<PathBuf, NextFrameError> {
    fs::create_dir_all(&output_dir).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::OutDirWriteFailed,
            format!(
                "create output directory {} failed: {err}",
                output_dir.display()
            ),
            format!(
                "next step · choose a writable --out directory: {}",
                output_dir.display()
            ),
        )
    })?;
    output_dir.canonicalize().map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::OutDirWriteFailed,
            format!(
                "resolve output directory {} failed: {err}",
                output_dir.display()
            ),
            format!(
                "next step · choose a writable --out directory: {}",
                output_dir.display()
            ),
        )
    })
}

fn write_component(project_root: &std::path::Path) -> Result<PathBuf, NextFrameError> {
    let path = project_root
        .join("components")
        .join(format!("{POSTER_COMPONENT_ID}.js"));
    write_text(&path, POSTER_COMPONENT_JS)?;
    Ok(path)
}

fn write_json<T: Serialize>(path: &std::path::Path, value: &T) -> Result<(), NextFrameError> {
    let text = serde_json::to_string_pretty(value).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::OutDirWriteFailed,
            format!("serialize composition failed: {err}"),
            "next step · rerun capy nextframe compose-poster after checking Poster JSON values",
        )
    })?;
    write_text(path, &(text + "\n"))
}

fn write_text(path: &std::path::Path, text: &str) -> Result<(), NextFrameError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            NextFrameError::new(
                NextFrameErrorCode::OutDirWriteFailed,
                format!("create directory {} failed: {err}", parent.display()),
                format!(
                    "next step · choose a writable --out directory: {}",
                    parent.display()
                ),
            )
        })?;
    }
    fs::write(path, text).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::OutDirWriteFailed,
            format!("write {} failed: {err}", path.display()),
            format!(
                "next step · choose a writable --out directory: {}",
                path.display()
            ),
        )
    })
}

fn trace_id(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{prefix}-{millis}-{}", std::process::id())
}
