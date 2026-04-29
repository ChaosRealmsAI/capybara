use std::collections::BTreeMap;

use crate::compose::composition::{
    CAPY_COMPOSITION_SCHEMA_VERSION, COMPOSITION_SCHEMA, CompositionAsset, CompositionDocument,
    CompositionTime, CompositionTrack, CompositionViewport, POSTER_COMPONENT_ID, POSTER_TRACK_ID,
};
use crate::compose::poster::PosterInput;

pub fn poster_to_composition(
    poster: &PosterInput,
    composition_id: String,
    duration_ms: u64,
) -> CompositionDocument {
    let duration_ms = duration_ms.max(1);
    let title = poster
        .title()
        .map(ToString::to_string)
        .unwrap_or_else(|| composition_id.clone());
    let mut params = BTreeMap::new();
    params.insert("poster".to_string(), poster.raw.clone());
    CompositionDocument {
        schema: COMPOSITION_SCHEMA.to_string(),
        schema_version: CAPY_COMPOSITION_SCHEMA_VERSION.to_string(),
        id: composition_id,
        title: title.clone(),
        name: title,
        duration_ms,
        duration: format!("{duration_ms}ms"),
        viewport: CompositionViewport {
            w: poster.document.canvas.width,
            h: poster.document.canvas.height,
            ratio: ratio(poster),
        },
        theme: None,
        tracks: vec![CompositionTrack {
            id: POSTER_TRACK_ID.to_string(),
            kind: "component".to_string(),
            component: POSTER_COMPONENT_ID.to_string(),
            z: 10,
            time: CompositionTime {
                start: "0ms".to_string(),
                end: format!("{duration_ms}ms"),
            },
            duration_ms,
            params,
        }],
        assets: poster
            .document
            .assets
            .iter()
            .map(|(id, asset)| CompositionAsset {
                id: id.clone(),
                asset_type: asset.asset_type.clone(),
                src: asset.src.clone(),
                kind: None,
                source_path: None,
                source_kind: None,
                original_path: None,
                materialized_path: None,
                byte_size: None,
                sha256: None,
                mask: asset.mask.clone(),
                provenance: asset.provenance.clone(),
            })
            .collect(),
    }
}

fn ratio(poster: &PosterInput) -> String {
    if poster.document.canvas.aspect_ratio.trim().is_empty() {
        format!(
            "{}:{}",
            poster.document.canvas.width, poster.document.canvas.height
        )
    } else {
        poster.document.canvas.aspect_ratio.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::poster_to_composition;
    use crate::compose::composition::{CAPY_COMPOSITION_SCHEMA_VERSION, POSTER_COMPONENT_ID};
    use crate::compose::poster::PosterInput;
    use capy_poster::PosterDocument;
    use serde_json::json;

    #[test]
    fn maps_poster_to_single_component_track() -> Result<(), serde_json::Error> {
        let poster = sample_poster(json!({
            "logo": {"type": "image", "src": "assets/logo.png"}
        }))?;

        let composition = poster_to_composition(&poster, "poster-main".to_string(), 1000);

        assert_eq!(composition.schema_version, CAPY_COMPOSITION_SCHEMA_VERSION);
        assert_eq!(composition.title, "Launch Poster");
        assert_eq!(composition.tracks.len(), 1);
        assert_eq!(composition.tracks[0].component, POSTER_COMPONENT_ID);
        assert_eq!(composition.tracks[0].duration_ms, 1000);
        assert_eq!(
            composition.tracks[0].params["poster"]["title"],
            "Launch Poster"
        );
        assert_eq!(composition.assets.len(), 1);
        Ok(())
    }

    #[test]
    fn maps_empty_assets_and_layers_without_extra_tracks() -> Result<(), serde_json::Error> {
        let poster = sample_poster(json!({}))?;

        let composition = poster_to_composition(&poster, "empty".to_string(), 0);

        assert_eq!(composition.duration_ms, 1);
        assert_eq!(composition.tracks.len(), 1);
        assert!(composition.assets.is_empty());
        assert!(composition.tracks[0].params["poster"]["layers"].is_array());
        Ok(())
    }

    fn sample_poster(assets: serde_json::Value) -> Result<PosterInput, serde_json::Error> {
        let raw = json!({
            "version": "capy-poster-v0.1",
            "type": "poster",
            "title": "Launch Poster",
            "canvas": {
                "width": 1920,
                "height": 1080,
                "aspectRatio": "16:9",
                "background": "#fff"
            },
            "assets": assets,
            "layers": []
        });
        let document: PosterDocument = serde_json::from_value(raw.clone())?;
        Ok(PosterInput { document, raw })
    }
}
