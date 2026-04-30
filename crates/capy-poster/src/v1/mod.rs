mod export;
mod pdf;
mod pptx;
mod raster;
mod svg;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{PosterError, Result};

pub use export::{
    ExportFormat, ExportReport, ExportRequest, export_document, export_document_value,
};
pub use svg::render_page_svg;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterDocumentV1 {
    pub schema: String,
    pub id: String,
    #[serde(default)]
    pub title: String,
    pub viewport: PosterViewportV1,
    #[serde(default)]
    pub theme: BTreeMap<String, Value>,
    #[serde(default)]
    pub assets: BTreeMap<String, PosterAssetV1>,
    #[serde(default)]
    pub components: BTreeMap<String, ComponentDefinitionV1>,
    pub pages: Vec<PosterPageV1>,
    #[serde(default)]
    pub exports: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterViewportV1 {
    #[serde(default)]
    pub w: Option<u32>,
    #[serde(default)]
    pub h: Option<u32>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub ratio: String,
}

impl PosterViewportV1 {
    pub fn size(&self) -> Option<(u32, u32)> {
        let w = self.w.or(self.width)?;
        let h = self.h.or(self.height)?;
        (w > 0 && h > 0).then_some((w, h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_component_package_runtime_and_svg() -> Result<()> {
        let dir = unique_dir("poster-component-package")?;
        let package_dir = dir.join("components").join("html.capy-title");
        std::fs::create_dir_all(&package_dir).map_err(|source| PosterError::Write {
            path: package_dir.display().to_string(),
            source,
        })?;
        write(
            &package_dir.join("component.json"),
            r#"{
              "schema": "capy.component.v1",
              "id": "html.capy-title",
              "version": "0.1.0",
              "entrypoints": { "runtime": "runtime.js", "static_svg": "static.svg" },
              "trusted": true
            }"#,
        )?;
        write(
            &package_dir.join("runtime.js"),
            "export function mount() {}\nexport function update() {}\n",
        )?;
        write(
            &package_dir.join("static.svg"),
            "<svg>{{params.title}}</svg>",
        )?;
        let mut document: PosterDocumentV1 = serde_json::from_value(json!({
            "schema": "capy.poster.document.v1",
            "id": "unit",
            "viewport": { "w": 640, "h": 360 },
            "components": {
                "html.capy-title": { "package": "components/html.capy-title/component.json" }
            },
            "pages": [{
                "id": "cover",
                "layers": [{
                    "id": "cmp",
                    "kind": "component",
                    "component": "html.capy-title",
                    "bounds": { "x": 0, "y": 0, "w": 640, "h": 360 }
                }]
            }]
        }))
        .map_err(PosterError::Json)?;

        resolve_component_packages(&mut document, &dir)?;

        assert!(
            document.components["html.capy-title"]
                .runtime_source()
                .is_some_and(|source| source.contains("export function update"))
        );
        assert_eq!(
            document.components["html.capy-title"].svg_template(),
            Some("<svg>{{params.title}}</svg>")
        );
        std::fs::remove_dir_all(dir).ok();
        Ok(())
    }

    fn write(path: &Path, text: &str) -> Result<()> {
        std::fs::write(path, text).map_err(|source| PosterError::Write {
            path: path.display().to_string(),
            source,
        })
    }

    fn unique_dir(prefix: &str) -> Result<PathBuf> {
        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| PosterError::Export(err.to_string()))?
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).map_err(|source| PosterError::Write {
            path: dir.display().to_string(),
            source,
        })?;
        Ok(dir)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterAssetV1 {
    #[serde(rename = "type")]
    pub asset_type: String,
    pub src: String,
    #[serde(default)]
    pub provenance: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ComponentDefinitionV1 {
    Runtime(String),
    Detailed {
        #[serde(default)]
        runtime: Option<String>,
        #[serde(default)]
        svg: Option<String>,
        #[serde(default)]
        package: Option<String>,
    },
}

impl ComponentDefinitionV1 {
    pub fn runtime_source(&self) -> Option<&str> {
        match self {
            Self::Runtime(source) => Some(source.as_str()),
            Self::Detailed { runtime, .. } => runtime.as_deref(),
        }
    }

    pub fn svg_template(&self) -> Option<&str> {
        match self {
            Self::Runtime(_) => None,
            Self::Detailed { svg, .. } => svg.as_deref(),
        }
    }

    pub fn package_ref(&self) -> Option<&str> {
        match self {
            Self::Runtime(_) => None,
            Self::Detailed { package, .. } => package.as_deref(),
        }
    }

    fn hydrate_from_package(&mut self, runtime: String, svg: Option<String>) {
        if let Self::Detailed {
            runtime: current_runtime,
            svg: current_svg,
            ..
        } = self
        {
            if current_runtime.is_none() {
                *current_runtime = Some(runtime);
            }
            if current_svg.is_none() {
                *current_svg = svg;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterPageV1 {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub background: String,
    pub layers: Vec<PosterLayerV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterLayerV1 {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub shape: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub component: String,
    #[serde(default)]
    pub asset_ref: String,
    #[serde(default, rename = "assetId")]
    pub asset_id: String,
    pub bounds: PosterBoundsV1,
    #[serde(default)]
    pub z: i32,
    #[serde(default)]
    pub visible: Option<bool>,
    #[serde(default)]
    pub style: BTreeMap<String, Value>,
    #[serde(default)]
    pub params: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PosterBoundsV1 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl PosterBoundsV1 {
    pub fn is_positive(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.w.is_finite()
            && self.h.is_finite()
            && self.w > 0.0
            && self.h > 0.0
    }
}

pub fn read_document_v1(path: &Path) -> Result<PosterDocumentV1> {
    let text = fs::read_to_string(path).map_err(|source| PosterError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let mut document: PosterDocumentV1 = serde_json::from_str(&text).map_err(PosterError::Json)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    resolve_component_packages(&mut document, base_dir)?;
    validate_document_v1(&document)?;
    Ok(document)
}

pub fn resolve_component_packages(document: &mut PosterDocumentV1, base_dir: &Path) -> Result<()> {
    for (id, component) in &mut document.components {
        let Some(package_ref) = component.package_ref().map(str::to_string) else {
            continue;
        };
        let package_path = resolve_package_path(base_dir, &package_ref);
        let package = capy_components::load_component_package(&package_path)
            .map_err(|err| PosterError::Validation(err.to_string()))?;
        if package.manifest.id != *id {
            return Err(PosterError::Validation(format!(
                "component '{id}' package id mismatch: {}",
                package.manifest.id
            )));
        }
        component.hydrate_from_package(package.runtime, package.static_svg);
    }
    Ok(())
}

pub fn write_document_json(path: &Path, document: &PosterDocumentV1) -> Result<()> {
    validate_document_v1(document)?;
    let text = serde_json::to_string_pretty(document).map_err(PosterError::Json)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| PosterError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, text + "\n").map_err(|source| PosterError::Write {
        path: path.display().to_string(),
        source,
    })
}

fn resolve_package_path(base_dir: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        if raw.starts_with("/fixtures/") {
            return PathBuf::from(".").join(raw.trim_start_matches('/'));
        }
        return path.to_path_buf();
    }
    base_dir.join(path)
}

pub fn validate_document_v1(document: &PosterDocumentV1) -> Result<()> {
    if document.schema != "capy.poster.document.v1" {
        return Err(PosterError::Validation(
            "document schema must be capy.poster.document.v1".to_string(),
        ));
    }
    if document.id.trim().is_empty() {
        return Err(PosterError::Validation(
            "document id is required".to_string(),
        ));
    }
    if document.viewport.size().is_none() {
        return Err(PosterError::Validation(
            "document viewport must include positive w/h".to_string(),
        ));
    }
    if document.pages.is_empty() {
        return Err(PosterError::Validation(
            "document requires pages[]".to_string(),
        ));
    }
    for page in &document.pages {
        validate_page(document, page)?;
    }
    Ok(())
}

fn validate_page(document: &PosterDocumentV1, page: &PosterPageV1) -> Result<()> {
    if page.id.trim().is_empty() {
        return Err(PosterError::Validation(
            "every page requires id".to_string(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for layer in &page.layers {
        if !ids.insert(layer.id.as_str()) {
            return Err(PosterError::Validation(format!(
                "duplicate layer id '{}' on page '{}'",
                layer.id, page.id
            )));
        }
        validate_layer(document, page, layer)?;
    }
    Ok(())
}

fn validate_layer(
    document: &PosterDocumentV1,
    page: &PosterPageV1,
    layer: &PosterLayerV1,
) -> Result<()> {
    if layer.id.trim().is_empty() || layer.kind.trim().is_empty() {
        return Err(PosterError::Validation(format!(
            "every layer on page '{}' requires id and kind",
            page.id
        )));
    }
    if !layer.bounds.is_positive() {
        return Err(PosterError::Validation(format!(
            "layer '{}' requires positive finite bounds",
            layer.id
        )));
    }
    match layer.kind.as_str() {
        "text" | "shape" => Ok(()),
        "image" => {
            let asset = layer.asset_ref.trim();
            let asset = if asset.is_empty() {
                layer.asset_id.trim()
            } else {
                asset
            };
            document
                .assets
                .contains_key(asset)
                .then_some(())
                .ok_or_else(|| {
                    PosterError::Validation(format!(
                        "image layer '{}' references missing asset",
                        layer.id
                    ))
                })
        }
        "component" => document
            .components
            .contains_key(layer.component.as_str())
            .then_some(())
            .ok_or_else(|| {
                PosterError::Validation(format!(
                    "component layer '{}' references missing component",
                    layer.id
                ))
            }),
        other => Err(PosterError::Validation(format!(
            "layer '{}' uses unsupported kind '{}'",
            layer.id, other
        ))),
    }
}
