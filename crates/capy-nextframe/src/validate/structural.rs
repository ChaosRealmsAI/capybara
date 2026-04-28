use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::compose::{CAPY_COMPOSITION_SCHEMA_VERSION, CompositionDocument, POSTER_COMPONENT_ID};
use crate::validate::report::{ValidationError, ValidationReport, ValidationWarning};

const REGISTERED_COMPONENTS: &[&str] = &[POSTER_COMPONENT_ID];

pub fn validate_structure(path: &Path) -> ValidationReport {
    let mut report = ValidationReport::new(path.to_path_buf(), trace_id());
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            report.push_error(ValidationError::new(
                "COMPOSITION_NOT_FOUND",
                "$",
                format!("composition not found: {}", path.display()),
                "next step · pass an existing composition.json path",
            ));
            return report;
        }
        Err(err) => {
            report.push_error(ValidationError::new(
                "COMPOSITION_READ_FAILED",
                "$",
                format!("read composition failed: {err}"),
                "next step · check file permissions and rerun validate",
            ));
            return report;
        }
    };

    let raw: Value = match serde_json::from_str(&text) {
        Ok(raw) => raw,
        Err(err) => {
            report.push_error(ValidationError::new(
                "COMPOSITION_INVALID",
                "$",
                format!("composition JSON is invalid: {err}"),
                "next step · rerun compose-poster or repair composition.json",
            ));
            return report;
        }
    };

    let composition: CompositionDocument = match serde_json::from_value(raw.clone()) {
        Ok(composition) => composition,
        Err(err) => {
            report.push_error(ValidationError::new(
                "COMPOSITION_INVALID",
                "$",
                format!("composition shape is invalid: {err}"),
                "next step · rerun compose-poster to regenerate composition.json",
            ));
            return report;
        }
    };

    if let Some(raw_trace_id) = raw.get("trace_id").and_then(Value::as_str) {
        if !raw_trace_id.trim().is_empty() {
            report.trace_id = raw_trace_id.to_string();
        }
    } else {
        report.warnings.push(ValidationWarning::new(
            "TRACE_ID_GENERATED",
            "$.trace_id",
            "composition had no trace_id; validate generated one for this report",
        ));
    }

    report.schema_version = composition.schema_version.clone();
    report.track_count = composition.tracks.len();
    report.asset_count = composition.assets.len();
    report.components = composition
        .tracks
        .iter()
        .map(|track| track.component.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    validate_schema_version(&composition, &mut report);
    validate_tracks(&composition, &mut report);
    validate_assets(&raw, path.parent(), &mut report);
    report.refresh_ok();
    report
}

fn validate_schema_version(composition: &CompositionDocument, report: &mut ValidationReport) {
    if composition.schema_version != CAPY_COMPOSITION_SCHEMA_VERSION {
        report.push_error(ValidationError::new(
            "SCHEMA_VERSION_MISMATCH",
            "$.schema_version",
            format!(
                "expected schema_version {}, got {}",
                CAPY_COMPOSITION_SCHEMA_VERSION, composition.schema_version
            ),
            "next step · rerun compose-poster to regenerate composition.json",
        ));
    }
}

fn validate_tracks(composition: &CompositionDocument, report: &mut ValidationReport) {
    if composition.tracks.is_empty() {
        report.push_error(ValidationError::new(
            "EMPTY_TRACKS",
            "$.tracks",
            "composition must include at least one track",
            "next step · composition must include at least 1 track",
        ));
    }

    for (index, track) in composition.tracks.iter().enumerate() {
        if !REGISTERED_COMPONENTS.contains(&track.component.as_str()) {
            report.push_error(ValidationError::new(
                "COMPONENT_NOT_REGISTERED",
                format!("$.tracks[{index}].component"),
                format!("component is not registered: {}", track.component),
                "next step · register or correct the component id",
            ));
        }
        if track.duration_ms == 0 {
            report.push_error(ValidationError::new(
                "INVALID_DURATION",
                format!("$.tracks[{index}].duration_ms"),
                "track duration_ms must be greater than 0",
                "next step · set duration_ms to a positive integer",
            ));
        }
    }
}

fn validate_assets(raw: &Value, composition_dir: Option<&Path>, report: &mut ValidationReport) {
    let Some(assets) = raw.get("assets").and_then(Value::as_array) else {
        return;
    };

    for (index, asset) in assets.iter().enumerate() {
        if asset
            .get("kind")
            .and_then(Value::as_str)
            .map(|kind| kind == "copied")
            .unwrap_or(false)
        {
            validate_materialized_asset(asset, index, composition_dir, report);
        }
        if asset
            .get("source_kind")
            .and_then(Value::as_str)
            .map(|kind| kind == "external")
            .unwrap_or(false)
        {
            continue;
        }
        let source = asset
            .get("source_path")
            .or_else(|| asset.get("src"))
            .and_then(Value::as_str);
        let Some(source) = source else {
            continue;
        };
        if should_skip_source(source) {
            continue;
        }
        let resolved = resolve_asset_path(source, composition_dir);
        if !resolved.is_file() {
            report.push_error(ValidationError::new(
                "ASSET_NOT_FOUND",
                format!("$.assets[{index}].source_path"),
                format!("asset source does not exist: {}", resolved.display()),
                "next step · check asset source_path",
            ));
        }
    }
}

fn validate_materialized_asset(
    asset: &Value,
    index: usize,
    composition_dir: Option<&Path>,
    report: &mut ValidationReport,
) {
    let source = asset.get("materialized_path").and_then(Value::as_str);
    let Some(source) = source.filter(|value| !value.trim().is_empty()) else {
        report.push_error(ValidationError::new(
            "ASSET_MATERIALIZATION_MISSING",
            format!("$.assets[{index}].materialized_path"),
            "copied asset is missing materialized_path",
            "next step · rerun compose-poster to materialize assets",
        ));
        return;
    };
    let resolved = resolve_asset_path(source, composition_dir);
    if !resolved.is_file() {
        report.push_error(ValidationError::new(
            "ASSET_MATERIALIZATION_MISSING",
            format!("$.assets[{index}].materialized_path"),
            format!("materialized asset does not exist: {}", resolved.display()),
            "next step · rerun compose-poster to materialize assets",
        ));
    }
}

fn should_skip_source(source: &str) -> bool {
    source.starts_with("data:")
        || source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("asset://")
}

fn resolve_asset_path(source: &str, composition_dir: Option<&Path>) -> PathBuf {
    let path = PathBuf::from(source);
    if path.is_absolute() {
        path
    } else {
        composition_dir
            .map(|dir| dir.join(path.clone()))
            .unwrap_or(path)
    }
}

fn trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("validate-{millis}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::{Value, json};

    use super::validate_structure;

    #[test]
    fn accepts_valid_composition() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("valid")?;
        let asset = dir.join("hero.png");
        fs::write(&asset, "png")?;
        let path = write_composition(
            &dir,
            json!({"assets": [{"id": "hero", "type": "image", "source_path": "hero.png"}]}),
        )?;

        let report = validate_structure(&path);

        assert!(report.ok);
        assert_eq!(report.schema_version, "capy.composition.v1");
        assert_eq!(report.track_count, 1);
        assert_eq!(report.asset_count, 1);
        assert_eq!(report.components, vec!["html.capy-poster"]);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_schema_version_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("schema")?;
        let path = write_composition(&dir, json!({"schema_version": "wrong"}))?;
        let report = validate_structure(&path);

        assert_error(&report, "SCHEMA_VERSION_MISMATCH");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_empty_tracks() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("tracks")?;
        let path = write_composition(&dir, json!({"tracks": []}))?;
        let report = validate_structure(&path);

        assert_error(&report, "EMPTY_TRACKS");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_unregistered_component() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("component")?;
        let path = write_composition(&dir, json!({"component": "html.unknown"}))?;
        let report = validate_structure(&path);

        assert_error(&report, "COMPONENT_NOT_REGISTERED");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_invalid_duration() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("duration")?;
        let path = write_composition(&dir, json!({"duration_ms": 0}))?;
        let report = validate_structure(&path);

        assert_error(&report, "INVALID_DURATION");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_missing_file_asset() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("asset")?;
        let path = write_composition(
            &dir,
            json!({"assets": [{"id": "hero", "type": "image", "source_path": "missing.png"}]}),
        )?;
        let report = validate_structure(&path);

        assert_error(&report, "ASSET_NOT_FOUND");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn skips_external_asset_sources() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("external")?;
        let path = write_composition(
            &dir,
            json!({"assets": [{"id": "hero", "type": "image", "source_kind": "external", "source_path": "missing.png"}]}),
        )?;
        let report = validate_structure(&path);

        assert!(report.ok);
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_missing_materialized_asset() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("materialized-missing")?;
        let path = write_composition(
            &dir,
            json!({"assets": [{
                "id": "hero",
                "type": "image",
                "kind": "copied",
                "src": "assets/hero.png",
                "materialized_path": "assets/hero.png",
                "source_path": "data:image/svg+xml,%3Csvg/%3E"
            }]}),
        )?;
        let report = validate_structure(&path);

        assert_error(&report, "ASSET_MATERIALIZATION_MISSING");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_missing_composition_file() {
        let report = validate_structure(Path::new("/definitely/not/composition.json"));

        assert_error(&report, "COMPOSITION_NOT_FOUND");
    }

    fn assert_error(report: &super::ValidationReport, code: &str) {
        assert!(!report.ok);
        assert!(report.errors.iter().any(|error| error.code == code));
    }

    fn write_composition(
        dir: &Path,
        overrides: Value,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut value = json!({
            "schema": "nextframe.composition.v2",
            "schema_version": "capy.composition.v1",
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
                "component": "html.capy-poster",
                "z": 10,
                "time": {"start": "0ms", "end": "1000ms"},
                "duration_ms": 1000,
                "params": {"poster": {"type": "poster"}}
            }],
            "assets": []
        });
        apply_overrides(&mut value, overrides);
        let path = dir.join("composition.json");
        fs::write(&path, serde_json::to_vec_pretty(&value)?)?;
        Ok(path)
    }

    fn apply_overrides(value: &mut Value, overrides: Value) {
        if let Some(schema_version) = overrides.get("schema_version") {
            value["schema_version"] = schema_version.clone();
        }
        if let Some(tracks) = overrides.get("tracks") {
            value["tracks"] = tracks.clone();
        }
        if let Some(component) = overrides.get("component") {
            value["tracks"][0]["component"] = component.clone();
        }
        if let Some(duration_ms) = overrides.get("duration_ms") {
            value["tracks"][0]["duration_ms"] = duration_ms.clone();
        }
        if let Some(assets) = overrides.get("assets") {
            value["assets"] = assets.clone();
        }
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-nextframe-validate-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}
