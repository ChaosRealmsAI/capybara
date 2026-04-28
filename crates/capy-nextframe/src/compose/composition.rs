use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

pub const COMPOSITION_SCHEMA: &str = "nextframe.composition.v2";
pub const CAPY_COMPOSITION_SCHEMA_VERSION: &str = "capy.composition.v1";
pub const POSTER_COMPONENT_ID: &str = "html.capy-poster";
pub const SCROLL_CHAPTER_COMPONENT_ID: &str = "html.capy-scroll-chapter";
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_theme"
    )]
    pub theme: Option<CompositionTheme>,
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
pub struct CompositionTheme {
    pub tokens_ref: String,
    pub source_path: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionViewport {
    pub w: u32,
    pub h: u32,
    pub ratio: String,
}

fn deserialize_theme<'de, D>(deserializer: D) -> Result<Option<CompositionTheme>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Value::Null) | Some(Value::String(_)) => Ok(None),
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
    }
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
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub materialized_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::{
        CAPY_COMPOSITION_SCHEMA_VERSION, COMPOSITION_SCHEMA, CompositionDocument, CompositionTheme,
        CompositionTrack, CompositionViewport, POSTER_COMPONENT_ID,
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
            theme: Some(CompositionTheme {
                tokens_ref: "tokens/tokens.json".to_string(),
                source_path: "/tmp/tokens.css".to_string(),
                hash: "brand-token-fnv1a64-0000000000000000".to_string(),
            }),
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
                kind: Some("copied".to_string()),
                source_path: None,
                source_kind: None,
                original_path: Some("/tmp/hero.png".to_string()),
                materialized_path: Some("assets/hero.png".to_string()),
                byte_size: Some(3),
                sha256: Some(
                    "sha256-2d4566582844690f8634a8b2534ea5221560038c6c0650c99140759bad603ae2"
                        .to_string(),
                ),
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

    #[test]
    fn legacy_string_theme_decodes_as_no_brand() -> Result<(), serde_json::Error> {
        let decoded: CompositionDocument = serde_json::from_value(json!({
            "schema": COMPOSITION_SCHEMA,
            "schema_version": CAPY_COMPOSITION_SCHEMA_VERSION,
            "id": "poster-snapshot",
            "title": "Poster Snapshot",
            "name": "Poster Snapshot",
            "duration_ms": 1000,
            "duration": "1000ms",
            "viewport": {"w": 1920, "h": 1080, "ratio": "16:9"},
            "theme": "default",
            "tracks": [{
                "id": "track-poster",
                "kind": "component",
                "component": POSTER_COMPONENT_ID,
                "z": 10,
                "time": {"start": "0ms", "end": "1000ms"},
                "duration_ms": 1000,
                "params": {"poster": {"type": "poster"}}
            }],
            "assets": []
        }))?;

        assert!(decoded.theme.is_none());
        Ok(())
    }
}
