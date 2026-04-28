#[allow(dead_code)]
mod component;
#[deprecated(note = "use capy-nextframe::compile instead · removed v0.13.14")]
mod render_source;
mod types;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use thiserror::Error;

pub use types::{PosterAsset, PosterCanvas, PosterDocument, PosterLayer, PosterLayerKind};

#[derive(Debug, Error)]
pub enum PosterError {
    #[error("{0}")]
    Validation(String),
    #[error("read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("write {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("poster JSON: {0}")]
    Json(serde_json::Error),
}

pub type Result<T> = std::result::Result<T, PosterError>;

pub fn read_document(path: &Path) -> Result<PosterDocument> {
    let text = fs::read_to_string(path).map_err(|source| PosterError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let document = serde_json::from_str(&text).map_err(PosterError::Json)?;
    validate_document(&document)?;
    Ok(document)
}

#[deprecated(note = "use capy-nextframe::compile instead · removed v0.13.14")]
#[doc(hidden)]
#[allow(dead_code, deprecated)]
pub(crate) fn write_render_source(
    path: &Path,
    report: &render_source::RenderSourceReport,
) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| PosterError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let text = serde_json::to_string_pretty(&report.source).map_err(PosterError::Json)?;
    fs::write(path, text).map_err(|source| PosterError::Write {
        path: path.display().to_string(),
        source,
    })
}

pub fn validate_document(document: &PosterDocument) -> Result<()> {
    if document.doc_type != "poster" {
        return Err(PosterError::Validation(
            "document type must be poster".to_string(),
        ));
    }
    if document.canvas.width == 0 || document.canvas.height == 0 {
        return Err(PosterError::Validation(
            "canvas width and height must be greater than zero".to_string(),
        ));
    }
    if document.layers.is_empty() {
        return Err(PosterError::Validation(
            "poster must include at least one layer".to_string(),
        ));
    }
    validate_assets(document)?;
    let mut layer_ids = BTreeSet::new();
    for layer in &document.layers {
        if !layer_ids.insert(layer.id.as_str()) {
            return Err(PosterError::Validation(format!(
                "duplicate layer id '{}'",
                layer.id
            )));
        }
        validate_layer(layer, document)?;
    }
    Ok(())
}

fn validate_assets(document: &PosterDocument) -> Result<()> {
    for (id, asset) in &document.assets {
        if id.trim().is_empty() {
            return Err(PosterError::Validation("asset id is required".to_string()));
        }
        if asset.src.trim().is_empty() {
            return Err(PosterError::Validation(format!(
                "asset '{}' requires src",
                id
            )));
        }
    }
    Ok(())
}

fn validate_layer(layer: &PosterLayer, document: &PosterDocument) -> Result<()> {
    if layer.id.trim().is_empty() {
        return Err(PosterError::Validation("layer id is required".to_string()));
    }
    if !layer.x.is_finite()
        || !layer.y.is_finite()
        || !layer.width.is_finite()
        || !layer.height.is_finite()
    {
        return Err(PosterError::Validation(format!(
            "layer '{}' geometry must be finite numbers",
            layer.id
        )));
    }
    if layer.width <= 0.0 || layer.height <= 0.0 {
        return Err(PosterError::Validation(format!(
            "layer '{}' width and height must be greater than zero",
            layer.id
        )));
    }
    match layer.kind {
        PosterLayerKind::Image => validate_image_layer(layer, document),
        PosterLayerKind::Text => validate_text_layer(layer),
        PosterLayerKind::Shape => validate_shape_layer(layer),
    }
}

fn validate_image_layer(layer: &PosterLayer, document: &PosterDocument) -> Result<()> {
    let asset_id = layer.asset_id.as_deref().ok_or_else(|| {
        PosterError::Validation(format!("image layer '{}' requires assetId", layer.id))
    })?;
    if !document.assets.contains_key(asset_id) {
        return Err(PosterError::Validation(format!(
            "missing asset '{}' for image layer '{}'",
            asset_id, layer.id
        )));
    }
    Ok(())
}

fn validate_text_layer(layer: &PosterLayer) -> Result<()> {
    if layer.text.as_deref().unwrap_or_default().is_empty() {
        return Err(PosterError::Validation(format!(
            "text layer '{}' requires text",
            layer.id
        )));
    }
    Ok(())
}

fn validate_shape_layer(layer: &PosterLayer) -> Result<()> {
    let shape = layer.shape.as_deref().unwrap_or("rect");
    if matches!(shape, "rect" | "ellipse") {
        return Ok(());
    }
    Err(PosterError::Validation(format!(
        "shape layer '{}' uses unsupported shape '{}'",
        layer.id, shape
    )))
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_document() -> Result<PosterDocument> {
        serde_json::from_value(json!({
            "version": "capy-poster-v0.1",
            "type": "poster",
            "canvas": {
                "width": 1920,
                "height": 1080,
                "aspectRatio": "16:9",
                "background": "#f6f1e8"
            },
            "assets": {
                "logo": {
                    "type": "svg",
                    "src": "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A//www.w3.org/2000/svg%22/%3E",
                    "provenance": {
                        "model": "gpt-image-2",
                        "task_id": "task_demo"
                    }
                }
            },
            "layers": [
                {
                    "id": "bg",
                    "type": "shape",
                    "shape": "rect",
                    "x": 0,
                    "y": 0,
                    "width": 1920,
                    "height": 1080,
                    "z": 0,
                    "style": { "fill": "#f6f1e8" }
                },
                {
                    "id": "headline",
                    "type": "text",
                    "text": "LOCAL POSTER",
                    "x": 120,
                    "y": 160,
                    "width": 900,
                    "height": 180,
                    "z": 5,
                    "style": { "fontSize": 96, "color": "#1c1917" }
                },
                {
                    "id": "logo",
                    "type": "image",
                    "assetId": "logo",
                    "x": 1200,
                    "y": 240,
                    "width": 240,
                    "height": 240,
                    "z": 6
                }
            ]
        }))
        .map_err(PosterError::Json)
    }

    #[test]
    fn validates_sample_document() -> Result<()> {
        let document = sample_document()?;
        validate_document(&document)
    }

    #[test]
    fn rejects_missing_image_asset() -> Result<()> {
        let mut document = sample_document()?;
        document.assets.clear();
        let error = validate_document(&document)
            .err()
            .map(|err| err.to_string());
        assert_eq!(
            error,
            Some("missing asset 'logo' for image layer 'logo'".to_string())
        );
        Ok(())
    }

    #[test]
    fn rejects_duplicate_layer_ids() -> Result<()> {
        let mut document = sample_document()?;
        document.layers[1].id = "bg".to_string();
        let error = validate_document(&document)
            .err()
            .map(|err| err.to_string());
        assert_eq!(error, Some("duplicate layer id 'bg'".to_string()));
        Ok(())
    }

    #[test]
    fn compiles_render_source_contract() -> Result<()> {
        let document = sample_document()?;
        let report = render_source::compile_render_source(
            &document,
            render_source::CompileOptions::default(),
        )?;
        assert_eq!(
            report
                .source
                .get("schema_version")
                .and_then(serde_json::Value::as_str),
            Some("nf.render_source.v1")
        );
        assert_eq!(report.layer_count, 3);
        assert_eq!(report.generated_assets, 1);
        assert_eq!(
            report
                .source
                .get("tracks")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        Ok(())
    }
}
