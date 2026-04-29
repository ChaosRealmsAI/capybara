use std::collections::BTreeSet;

use capy_poster::{PosterDocument, PosterLayerKind};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct AssetReference {
    pub asset_id: String,
    pub asset_type: String,
    pub src: String,
    pub source_kind: String,
    pub mask: Option<String>,
    pub provenance: Option<Value>,
}

pub fn scan_asset_references(poster: &PosterDocument, poster_raw: &Value) -> Vec<AssetReference> {
    let mut ids = BTreeSet::new();
    for layer in &poster.layers {
        if matches!(layer.kind, PosterLayerKind::Image) {
            if let Some(asset_id) = layer.asset_id.as_deref().filter(|id| !id.trim().is_empty()) {
                ids.insert(asset_id.to_string());
            }
        }
        if matches!(layer.kind, PosterLayerKind::Text) {
            scan_text_font_asset_ids(layer.style.get("fontAssetId"), &mut ids);
        }
    }
    scan_raw_font_asset_ids(poster_raw, &mut ids);

    ids.into_iter()
        .filter_map(|asset_id| {
            poster.assets.get(&asset_id).map(|asset| AssetReference {
                source_kind: source_kind(&asset.src),
                asset_id,
                asset_type: asset.asset_type.clone(),
                src: asset.src.clone(),
                mask: asset.mask.clone(),
                provenance: asset.provenance.clone(),
            })
        })
        .filter(|asset| matches!(asset.asset_type.as_str(), "image" | "font" | "svg"))
        .collect()
}

fn scan_text_font_asset_ids(value: Option<&Value>, ids: &mut BTreeSet<String>) {
    if let Some(asset_id) = value.and_then(Value::as_str).map(str::trim) {
        if !asset_id.is_empty() {
            ids.insert(asset_id.to_string());
        }
    }
}

fn scan_raw_font_asset_ids(poster_raw: &Value, ids: &mut BTreeSet<String>) {
    let Some(layers) = poster_raw.get("layers").and_then(Value::as_array) else {
        return;
    };
    for layer in layers {
        let is_text = layer
            .get("type")
            .and_then(Value::as_str)
            .map(|kind| kind == "text")
            == Some(true);
        if !is_text {
            continue;
        }
        scan_text_font_asset_ids(layer.get("fontAssetId"), ids);
        scan_text_font_asset_ids(
            layer
                .get("style")
                .and_then(|style| style.get("fontAssetId")),
            ids,
        );
    }
}

fn source_kind(src: &str) -> String {
    if src.starts_with("data:") {
        "inline".to_string()
    } else if src.starts_with("fixture://") {
        "fixture".to_string()
    } else {
        "local".to_string()
    }
}
