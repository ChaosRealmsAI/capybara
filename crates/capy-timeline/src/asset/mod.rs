mod copy;
mod scan;

use std::collections::BTreeMap;
use std::path::Path;

use capy_poster::PosterDocument;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::asset::copy::{CopyOutcome, copy_asset};
use crate::asset::scan::scan_asset_references;
use crate::compose::CompositionAsset;

pub const MAX_COPY_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct MaterializeAssetsRequest<'a> {
    pub poster: &'a PosterDocument,
    pub poster_raw: &'a Value,
    pub poster_path: &'a Path,
    pub project_root: &'a Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetMaterializationError {
    pub code: String,
    pub path: String,
    pub message: String,
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetMaterializationWarning {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MaterializeAssetsResult {
    pub assets: Vec<CompositionAsset>,
    pub rewritten_poster: Value,
    pub errors: Vec<AssetMaterializationError>,
    pub warnings: Vec<AssetMaterializationWarning>,
}

pub fn materialize_assets(req: MaterializeAssetsRequest<'_>) -> MaterializeAssetsResult {
    let references = scan_asset_references(req.poster, req.poster_raw);
    let mut result = MaterializeAssetsResult {
        assets: Vec::new(),
        rewritten_poster: req.poster_raw.clone(),
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    let mut rewritten_sources = BTreeMap::new();
    let base_dir = match req.poster_path.parent() {
        Some(parent) => parent,
        None => Path::new("."),
    };

    for reference in references {
        match copy_asset(&reference, base_dir, req.project_root, MAX_COPY_BYTES) {
            CopyOutcome::Copied(copied) => {
                rewritten_sources.insert(reference.asset_id.clone(), copied.relative_path.clone());
                result.assets.push(CompositionAsset {
                    id: reference.asset_id,
                    asset_type: reference.asset_type,
                    src: copied.relative_path.clone(),
                    kind: Some("copied".to_string()),
                    source_path: Some(copied.original_path.clone()),
                    source_kind: Some(reference.source_kind),
                    original_path: Some(copied.original_path),
                    materialized_path: Some(copied.relative_path),
                    byte_size: Some(copied.byte_size),
                    sha256: Some(copied.sha256),
                    mask: reference.mask,
                    provenance: reference.provenance,
                });
            }
            CopyOutcome::Referenced(reference_asset) => {
                result.warnings.push(AssetMaterializationWarning {
                    code: "ASSET_MATERIALIZATION_REF".to_string(),
                    path: format!("$.assets.{}", reference.asset_id),
                    message: format!(
                        "asset {} is {} bytes and was kept as an absolute reference",
                        reference.asset_id, reference_asset.byte_size
                    ),
                });
                rewritten_sources.insert(
                    reference.asset_id.clone(),
                    reference_asset.original_path.clone(),
                );
                result.assets.push(CompositionAsset {
                    id: reference.asset_id,
                    asset_type: reference.asset_type,
                    src: reference_asset.original_path.clone(),
                    kind: Some("ref".to_string()),
                    source_path: Some(reference_asset.original_path.clone()),
                    source_kind: Some("external".to_string()),
                    original_path: Some(reference_asset.original_path),
                    materialized_path: None,
                    byte_size: Some(reference_asset.byte_size),
                    sha256: None,
                    mask: reference.mask,
                    provenance: reference.provenance,
                });
            }
            CopyOutcome::Missing(missing) => {
                result.errors.push(AssetMaterializationError {
                    code: "ASSET_MATERIALIZATION_SOURCE_MISSING".to_string(),
                    path: format!("$.assets.{}", reference.asset_id),
                    message: format!(
                        "asset source does not exist: {}",
                        missing.original_path.display()
                    ),
                    hint: "next step · check Poster asset src path and rerun compose-poster"
                        .to_string(),
                });
                result.assets.push(CompositionAsset {
                    id: reference.asset_id,
                    asset_type: reference.asset_type,
                    src: reference.src.clone(),
                    kind: Some("missing".to_string()),
                    source_path: Some(missing.original_path.display().to_string()),
                    source_kind: Some(reference.source_kind),
                    original_path: Some(missing.original_path.display().to_string()),
                    materialized_path: None,
                    byte_size: None,
                    sha256: None,
                    mask: reference.mask,
                    provenance: reference.provenance,
                });
            }
        }
    }

    rewrite_asset_sources(&mut result.rewritten_poster, &rewritten_sources);
    result
}

fn rewrite_asset_sources(poster: &mut Value, rewritten_sources: &BTreeMap<String, String>) {
    let Some(assets) = poster.get_mut("assets").and_then(Value::as_object_mut) else {
        return;
    };
    for (id, src) in rewritten_sources {
        if let Some(asset) = assets.get_mut(id).and_then(Value::as_object_mut) {
            asset.insert("src".to_string(), Value::String(src.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use capy_poster::PosterDocument;
    use serde_json::{Value, json};

    use super::{MaterializeAssetsRequest, materialize_assets};

    #[test]
    fn asset_scan_copies_referenced_image_assets() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("scan-copy")?;
        let source = dir.join("hero.png");
        fs::write(&source, "png")?;
        let poster = poster_with_assets(json!({
            "hero": {"type": "image", "src": source.display().to_string()},
            "unused": {"type": "image", "src": source.display().to_string()}
        }))?;
        let out = dir.join("out");

        let result = materialize_assets(MaterializeAssetsRequest {
            poster: &poster.document,
            poster_raw: &poster.raw,
            poster_path: &dir.join("poster.json"),
            project_root: &out,
        });

        assert!(result.errors.is_empty());
        assert_eq!(result.assets.len(), 1);
        assert_eq!(result.assets[0].id, "hero");
        assert_eq!(result.assets[0].kind.as_deref(), Some("copied"));
        assert_eq!(result.assets[0].byte_size, Some(3));
        let sha256 = result.assets[0]
            .sha256
            .as_deref()
            .ok_or("sha256 should be present")?;
        assert!(sha256.starts_with("sha256-"));
        let materialized = out.join(
            result.assets[0]
                .materialized_path
                .as_deref()
                .ok_or("materialized_path should be present")?,
        );
        assert!(materialized.is_file());
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn asset_copy_keeps_large_files_as_refs() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("large-ref")?;
        let source = dir.join("large.bin");
        fs::write(&source, vec![0_u8; 50 * 1024 * 1024 + 1])?;
        let poster = poster_with_assets(json!({
            "hero": {"type": "image", "src": source.display().to_string()}
        }))?;

        let result = materialize_assets(MaterializeAssetsRequest {
            poster: &poster.document,
            poster_raw: &poster.raw,
            poster_path: &dir.join("poster.json"),
            project_root: &dir.join("out"),
        });

        assert!(result.errors.is_empty());
        assert_eq!(result.assets[0].kind.as_deref(), Some("ref"));
        assert_eq!(result.assets[0].byte_size, Some(50 * 1024 * 1024 + 1));
        let source_path = source.canonicalize()?;
        assert_eq!(
            result.assets[0].original_path.as_deref(),
            source_path.to_str()
        );
        assert_eq!(result.warnings.len(), 1);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn asset_copy_reports_missing_sources() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("missing")?;
        let missing = dir.join("missing.png");
        let poster = poster_with_assets(json!({
            "hero": {"type": "image", "src": missing.display().to_string()}
        }))?;

        let result = materialize_assets(MaterializeAssetsRequest {
            poster: &poster.document,
            poster_raw: &poster.raw,
            poster_path: &dir.join("poster.json"),
            project_root: &dir.join("out"),
        });

        assert_eq!(result.assets[0].kind.as_deref(), Some("missing"));
        assert_eq!(
            result.errors[0].code,
            "ASSET_MATERIALIZATION_SOURCE_MISSING"
        );
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn asset_copy_materializes_inline_svg() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("inline")?;
        let poster = poster_with_assets(json!({
            "hero": {
                "type": "image",
                "src": "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A//www.w3.org/2000/svg%22/%3E"
            }
        }))?;

        let result = materialize_assets(MaterializeAssetsRequest {
            poster: &poster.document,
            poster_raw: &poster.raw,
            poster_path: &dir.join("poster.json"),
            project_root: &dir.join("out"),
        });

        assert!(result.errors.is_empty());
        assert_eq!(result.assets[0].kind.as_deref(), Some("copied"));
        let materialized_path = result.assets[0]
            .materialized_path
            .as_deref()
            .ok_or("materialized_path should be present")?;
        assert!(materialized_path.ends_with(".svg"));
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    struct PosterFixture {
        document: PosterDocument,
        raw: Value,
    }

    fn poster_with_assets(assets: Value) -> Result<PosterFixture, serde_json::Error> {
        let raw = json!({
            "version": "capy-poster-v0.1",
            "type": "poster",
            "canvas": {
                "width": 1920,
                "height": 1080,
                "aspectRatio": "16:9",
                "background": "#fff"
            },
            "assets": assets,
            "layers": [{
                "id": "hero",
                "type": "image",
                "assetId": "hero",
                "x": 0,
                "y": 0,
                "width": 100,
                "height": 100
            }]
        });
        let document = serde_json::from_value(raw.clone())?;
        Ok(PosterFixture { document, raw })
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-timeline-asset-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}
