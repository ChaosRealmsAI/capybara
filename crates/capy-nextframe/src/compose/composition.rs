use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const COMPOSITION_SCHEMA: &str = "nextframe.composition.v2";
pub const CAPY_COMPOSITION_SCHEMA_VERSION: &str = "capy.composition.v1";
pub const POSTER_COMPONENT_ID: &str = "html.capy-poster";
pub const POSTER_TRACK_ID: &str = "track-poster";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionDocument {
    pub schema: String,
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub name: String,
    pub duration_ms: u64,
    pub duration: String,
    pub viewport: CompositionViewport,
    pub theme: String,
    pub tracks: Vec<CompositionTrack>,
    #[serde(default)]
    pub assets: Vec<CompositionAsset>,
}

impl CompositionDocument {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|err| format!("read composition failed: {err}"))?;
        serde_json::from_str(&text).map_err(|err| format!("parse composition failed: {err}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionViewport {
    pub w: u32,
    pub h: u32,
    pub ratio: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionTrack {
    pub id: String,
    pub kind: String,
    pub component: String,
    pub z: i32,
    pub time: CompositionTime,
    pub duration_ms: u64,
    pub params: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionTime {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionAsset {
    pub id: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    #[serde(default)]
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::{
        CAPY_COMPOSITION_SCHEMA_VERSION, COMPOSITION_SCHEMA, CompositionDocument, CompositionTrack,
        CompositionViewport, POSTER_COMPONENT_ID,
    };
    use crate::compose::composition::{CompositionAsset, CompositionTime};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn composition_round_trips_through_json() -> Result<(), serde_json::Error> {
        let mut params = BTreeMap::new();
        params.insert("poster".to_string(), json!({"type": "poster"}));
        let document = CompositionDocument {
            schema: COMPOSITION_SCHEMA.to_string(),
            schema_version: CAPY_COMPOSITION_SCHEMA_VERSION.to_string(),
            id: "poster-snapshot".to_string(),
            title: "Poster Snapshot".to_string(),
            name: "Poster Snapshot".to_string(),
            duration_ms: 1000,
            duration: "1000ms".to_string(),
            viewport: CompositionViewport {
                w: 1920,
                h: 1080,
                ratio: "16:9".to_string(),
            },
            theme: "default".to_string(),
            tracks: vec![CompositionTrack {
                id: "track-poster".to_string(),
                kind: "component".to_string(),
                component: POSTER_COMPONENT_ID.to_string(),
                z: 10,
                time: CompositionTime {
                    start: "0ms".to_string(),
                    end: "1000ms".to_string(),
                },
                duration_ms: 1000,
                params,
            }],
            assets: vec![CompositionAsset {
                id: "hero".to_string(),
                asset_type: "image".to_string(),
                src: "assets/hero.png".to_string(),
                source_path: None,
                source_kind: None,
                mask: None,
                provenance: None,
            }],
        };

        let text = serde_json::to_string_pretty(&document)?;
        let decoded: CompositionDocument = serde_json::from_str(&text)?;

        assert_eq!(decoded, document);
        assert_eq!(decoded.schema, COMPOSITION_SCHEMA);
        assert_eq!(decoded.schema_version, CAPY_COMPOSITION_SCHEMA_VERSION);
        Ok(())
    }
}
