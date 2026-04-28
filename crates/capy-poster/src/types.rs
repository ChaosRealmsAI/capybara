use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterDocument {
    pub version: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub canvas: PosterCanvas,
    #[serde(default)]
    pub assets: BTreeMap<String, PosterAsset>,
    pub layers: Vec<PosterLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterCanvas {
    pub width: u32,
    pub height: u32,
    #[serde(default, rename = "aspectRatio")]
    pub aspect_ratio: String,
    #[serde(default = "default_background")]
    pub background: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterAsset {
    #[serde(rename = "type")]
    pub asset_type: String,
    pub src: String,
    #[serde(default)]
    pub mask: Option<String>,
    #[serde(default)]
    pub provenance: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosterLayer {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: PosterLayerKind,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default, rename = "assetId")]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub z: i32,
    #[serde(default)]
    pub style: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PosterLayerKind {
    Image,
    Text,
    Shape,
}

fn default_background() -> String {
    "#ffffff".to_string()
}
