//! Capybara-owned creative document contracts.
//!
//! This crate is the product data layer above poster, scroll media, and
//! timeline adapters. Adapter formats such as `capy.timeline.composition.v1` are
//! compiled from these contracts, not edited as product truth.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const CREATIVE_SCHEMA_VERSION: &str = "capy.creative.v1";
pub const TIMELINE_SCHEMA_VERSION: &str = "capy.timeline.v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreativeDocument {
    pub schema_version: String,
    pub kind: CreativeDocumentKind,
    pub id: String,
    pub title: String,
    pub stage: Stage,
    #[serde(default)]
    pub assets: BTreeMap<String, Asset>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl CreativeDocument {
    pub fn poster(id: impl Into<String>, title: impl Into<String>, stage: Stage) -> Self {
        Self {
            schema_version: CREATIVE_SCHEMA_VERSION.to_string(),
            kind: CreativeDocumentKind::Poster,
            id: id.into(),
            title: title.into(),
            stage,
            assets: BTreeMap::new(),
            tracks: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreativeDocumentKind {
    Poster,
    Timeline,
    ScrollStory,
    GeneratedAssetSet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: String,
    pub background: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub kind: AssetKind,
    pub source: AssetSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialized: Option<String>,
    #[serde(default)]
    pub provenance: BTreeMap<String, Value>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Image,
    Video,
    Audio,
    Font,
    Svg,
    Json,
    Mask,
    Voice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssetSource {
    File { path: String },
    DataUri { uri: String },
    Fixture { id: String },
    Generated { provider: String, task_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub kind: TrackKind,
    #[serde(default)]
    pub items: Vec<TrackItem>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Visual,
    Text,
    Audio,
    Tts,
    Caption,
    Camera,
    Interaction,
    Scroll,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackItem {
    pub id: String,
    pub kind: TrackItemKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    pub time: TimeRange,
    #[serde(default)]
    pub layout: BTreeMap<String, Value>,
    #[serde(default)]
    pub style: BTreeMap<String, Value>,
    #[serde(default)]
    pub animation: BTreeMap<String, Value>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackItemKind {
    Image,
    Text,
    Shape,
    Video,
    Audio,
    Tts,
    Caption,
    HtmlComponent,
    PosterLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_ms: u64,
    pub duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::{
        CREATIVE_SCHEMA_VERSION, CreativeDocument, Stage, TIMELINE_SCHEMA_VERSION, TimeRange,
        Track, TrackItem, TrackItemKind, TrackKind,
    };

    #[test]
    fn poster_document_has_stable_schema_name() {
        let document = CreativeDocument::poster(
            "poster-1",
            "Poster 1",
            Stage {
                width: 1920,
                height: 1080,
                aspect_ratio: "16:9".to_string(),
                background: "#fff".to_string(),
                duration_ms: 1000,
            },
        );

        assert_eq!(document.schema_version, CREATIVE_SCHEMA_VERSION);
        assert_eq!(TIMELINE_SCHEMA_VERSION, "capy.timeline.v1");
    }

    #[test]
    fn timeline_contract_can_represent_tts_and_captions() {
        let track = Track {
            id: "voice".to_string(),
            kind: TrackKind::Tts,
            items: vec![TrackItem {
                id: "line-1".to_string(),
                kind: TrackItemKind::Tts,
                asset_id: None,
                component: None,
                time: TimeRange {
                    start_ms: 0,
                    duration_ms: 2400,
                },
                layout: Default::default(),
                style: Default::default(),
                animation: Default::default(),
                metadata: Default::default(),
            }],
            metadata: Default::default(),
        };

        assert_eq!(track.kind, TrackKind::Tts);
        assert_eq!(track.items[0].kind, TrackItemKind::Tts);
    }
}
