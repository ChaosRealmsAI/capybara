use std::collections::BTreeSet;
use std::fs;

use crate::model::{
    DESIGN_LANGUAGE_INSPECTION_SCHEMA_VERSION, DESIGN_LANGUAGE_VALIDATION_SCHEMA_VERSION,
    DesignLanguageAssetStatusV1, DesignLanguageAssetV1, DesignLanguageInspectionV1,
    DesignLanguageManifestV1, DesignLanguageSummaryV1, DesignLanguageValidationV1,
};
use crate::package::{ProjectPackage, ProjectPackageResult, now_ms};

impl ProjectPackage {
    pub fn inspect_design_language(&self) -> ProjectPackageResult<DesignLanguageInspectionV1> {
        let project = self.project_manifest()?;
        let manifest = self.design_language()?;
        let summary = self.design_language_summary_for(&manifest);
        Ok(DesignLanguageInspectionV1 {
            schema_version: DESIGN_LANGUAGE_INSPECTION_SCHEMA_VERSION.to_string(),
            project_id: project.id,
            project_name: project.name,
            design_language_ref: summary.design_language_ref.clone(),
            summary,
            assets: self.design_language_asset_statuses(&manifest),
            manifest,
            generated_at: now_ms(),
        })
    }

    pub fn validate_design_language(&self) -> ProjectPackageResult<DesignLanguageValidationV1> {
        let project = self.project_manifest()?;
        let manifest = self.design_language()?;
        let summary = self.design_language_summary_for(&manifest);
        let assets = self.design_language_asset_statuses(&manifest);
        let mut errors = Vec::new();
        if manifest.name.trim().is_empty() {
            errors.push("design language name is required".to_string());
        }
        if manifest.version.trim().is_empty() {
            errors.push("design language version is required".to_string());
        }
        if manifest.assets.is_empty() {
            errors.push("at least one design language asset is required".to_string());
        }
        errors.extend(assets.iter().filter_map(|asset| asset.error.clone()));
        Ok(DesignLanguageValidationV1 {
            schema_version: DESIGN_LANGUAGE_VALIDATION_SCHEMA_VERSION.to_string(),
            ok: errors.is_empty(),
            project_id: project.id,
            project_name: project.name,
            design_language_ref: summary.design_language_ref.clone(),
            summary,
            assets,
            errors,
            generated_at: now_ms(),
        })
    }

    pub(crate) fn design_language_summary(&self) -> ProjectPackageResult<DesignLanguageSummaryV1> {
        let manifest = self.design_language()?;
        Ok(self.design_language_summary_for(&manifest))
    }

    pub(crate) fn design_language_summary_for(
        &self,
        manifest: &DesignLanguageManifestV1,
    ) -> DesignLanguageSummaryV1 {
        let assets = &manifest.assets;
        DesignLanguageSummaryV1 {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            summary: manifest.summary.clone(),
            design_language_ref: self.design_language_ref(manifest),
            asset_count: assets.len(),
            token_count: assets.iter().filter(|asset| is_token(asset)).count(),
            reference_image_count: assets
                .iter()
                .filter(|asset| is_reference_image(asset))
                .count(),
            rule_count: assets.iter().filter(|asset| is_rule(asset)).count(),
            example_count: assets.iter().filter(|asset| is_example(asset)).count(),
        }
    }

    pub(crate) fn design_language_asset_statuses(
        &self,
        manifest: &DesignLanguageManifestV1,
    ) -> Vec<DesignLanguageAssetStatusV1> {
        manifest
            .assets
            .iter()
            .map(|asset| {
                let path = self.root().join(&asset.path);
                match fs::metadata(&path) {
                    Ok(metadata) if metadata.is_file() => DesignLanguageAssetStatusV1 {
                        id: asset.id.clone(),
                        kind: asset.kind.clone(),
                        role: asset.role.clone(),
                        path: asset.path.clone(),
                        title: asset.title.clone(),
                        exists: true,
                        bytes: Some(metadata.len()),
                        error: None,
                    },
                    Ok(_) => DesignLanguageAssetStatusV1 {
                        id: asset.id.clone(),
                        kind: asset.kind.clone(),
                        role: asset.role.clone(),
                        path: asset.path.clone(),
                        title: asset.title.clone(),
                        exists: false,
                        bytes: None,
                        error: Some(format!(
                            "design language asset is not a file: {}",
                            asset.path
                        )),
                    },
                    Err(source) => DesignLanguageAssetStatusV1 {
                        id: asset.id.clone(),
                        kind: asset.kind.clone(),
                        role: asset.role.clone(),
                        path: asset.path.clone(),
                        title: asset.title.clone(),
                        exists: false,
                        bytes: None,
                        error: Some(format!(
                            "missing design language asset {}: {source}",
                            asset.path
                        )),
                    },
                }
            })
            .collect()
    }

    fn design_language_ref(&self, manifest: &DesignLanguageManifestV1) -> String {
        let mut hash = Fnv1a64::new();
        hash.write_str(&manifest.schema_version);
        hash.write_str(&manifest.id);
        hash.write_str(&manifest.name);
        hash.write_str(&manifest.version);
        hash.write_str(&manifest.summary);
        let mut assets = manifest.assets.iter().collect::<Vec<_>>();
        assets.sort_by(|left, right| left.id.cmp(&right.id));
        for asset in assets {
            hash.write_str(&asset.id);
            hash.write_str(&asset.kind);
            hash.write_str(asset.role.as_deref().unwrap_or(""));
            hash.write_str(&asset.path);
            hash.write_str(&asset.title);
            hash.write_str(asset.description.as_deref().unwrap_or(""));
            match fs::read(self.root().join(&asset.path)) {
                Ok(bytes) => hash.write(&bytes),
                Err(_) => hash.write_str("<missing>"),
            }
        }
        format!("dlpkg-fnv1a64-{:016x}", hash.finish())
    }
}

pub(crate) fn selected_design_assets(
    manifest: &DesignLanguageManifestV1,
    design_refs: &[String],
) -> Vec<DesignLanguageAssetV1> {
    if design_refs.is_empty() {
        return manifest.assets.clone();
    }
    let wanted = design_refs
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    manifest
        .assets
        .iter()
        .filter(|asset| wanted.contains(asset.id.as_str()))
        .cloned()
        .collect()
}

fn is_token(asset: &DesignLanguageAssetV1) -> bool {
    role_is(asset, "tokens")
        || asset.kind == "css"
        || asset.path.contains("token")
        || asset.title.to_ascii_lowercase().contains("token")
}

fn is_reference_image(asset: &DesignLanguageAssetV1) -> bool {
    role_is(asset, "reference-image") || matches!(asset.kind.as_str(), "image" | "svg" | "png")
}

fn is_rule(asset: &DesignLanguageAssetV1) -> bool {
    asset
        .role
        .as_deref()
        .map(|role| role.ends_with("rule") || role == "anti-slop-rule")
        .unwrap_or(false)
        || matches!(asset.kind.as_str(), "markdown" | "json")
}

fn is_example(asset: &DesignLanguageAssetV1) -> bool {
    role_is(asset, "example")
        || matches!(
            asset.kind.as_str(),
            "html" | "poster-json" | "ppt-json" | "composition-json"
        )
}

fn role_is(asset: &DesignLanguageAssetV1, expected: &str) -> bool {
    asset.role.as_deref() == Some(expected)
}

struct Fnv1a64(u64);

impl Fnv1a64 {
    fn new() -> Self {
        Self(0xcbf29ce484222325)
    }

    fn write_str(&mut self, value: &str) {
        self.write(value.as_bytes());
        self.write(&[0]);
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::ProjectPackage;

    #[test]
    fn validation_reports_stable_ref_and_asset_counts() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("Design Test".to_string()))?;
        fs::write(temp.path().join("tokens.css"), ":root { --brand: red; }")?;
        project.add_design_asset(
            "css".to_string(),
            Some("tokens".to_string()),
            "tokens.css",
            "Tokens".to_string(),
            None,
        )?;

        let first = project.validate_design_language()?;
        let second = project.validate_design_language()?;

        assert!(first.ok);
        assert_eq!(first.design_language_ref, second.design_language_ref);
        assert_eq!(first.summary.token_count, 1);
        Ok(())
    }

    #[test]
    fn validation_reports_missing_assets() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let project = ProjectPackage::init(temp.path(), Some("Design Test".to_string()))?;
        fs::write(temp.path().join("tokens.css"), ":root { --brand: red; }")?;
        let asset = project.add_design_asset(
            "css".to_string(),
            Some("tokens".to_string()),
            "tokens.css",
            "Tokens".to_string(),
            None,
        )?;
        fs::remove_file(temp.path().join("tokens.css"))?;

        let validation = project.validate_design_language()?;

        assert!(!validation.ok);
        assert!(
            validation
                .errors
                .iter()
                .any(|error| error.contains(&asset.path))
        );
        Ok(())
    }
}
