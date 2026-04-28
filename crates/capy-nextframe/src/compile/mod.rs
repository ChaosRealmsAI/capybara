pub mod embedded;
pub mod report;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::adapter::crate_adapter::CrateAdapter;
use crate::compile::embedded::RENDER_SOURCE_SCHEMA;
pub use crate::compile::report::{
    CompileCompositionRequest, CompileError, CompileMode, CompileReport, CompileSuccess,
};
use crate::compose::CompositionDocument;
use crate::config::NextFrameConfig;
use crate::ports::{CompositionArtifact, NextFrameProjectPort};
use crate::validate;

pub fn compile_composition(req: CompileCompositionRequest) -> CompileReport {
    let started = Instant::now();
    let trace_id = trace_id();
    let composition_path = absolute_path(&req.composition_path);
    let render_source_path = render_source_path(&composition_path);

    let validation = validate::structural::validate_structure(&composition_path);
    if !validation.ok {
        return validation_failure(
            trace_id,
            composition_path,
            render_source_path,
            started,
            validation,
        );
    }

    if let report @ CompileReport { ok: true, .. } = compile_with_project_port(
        &CrateAdapter::new(NextFrameConfig::default()),
        trace_id.clone(),
        composition_path.clone(),
        render_source_path.clone(),
        started,
    ) {
        return report;
    }

    let composition = match read_composition(&composition_path) {
        Ok(composition) => composition,
        Err(error) => {
            return CompileReport::failure(
                trace_id,
                composition_path,
                render_source_path,
                started.elapsed().as_millis(),
                vec![error],
            );
        }
    };
    embedded_result(
        trace_id,
        composition_path,
        render_source_path,
        started,
        composition,
    )
}

fn validation_failure(
    trace_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    started: Instant,
    validation: validate::ValidationReport,
) -> CompileReport {
    CompileReport::failure(
        trace_id,
        composition_path,
        render_source_path,
        started.elapsed().as_millis(),
        validation
            .errors
            .into_iter()
            .map(|err| {
                let code = match err.code.as_str() {
                    "COMPOSITION_NOT_FOUND" => err.code,
                    _ => "INVALID_COMPOSITION".to_string(),
                };
                CompileError::new(code, err.path, err.message, err.hint)
            })
            .collect(),
    )
}

fn embedded_result(
    trace_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    started: Instant,
    composition: CompositionDocument,
) -> CompileReport {
    match embedded::compile_embedded(&composition, &render_source_path) {
        Ok(report) => success(
            trace_id,
            composition_path,
            render_source_path,
            started,
            report.track_count,
            CompileMode::Embedded,
        ),
        Err(error) => CompileReport::failure(
            trace_id,
            composition_path,
            render_source_path,
            started.elapsed().as_millis(),
            vec![error],
        ),
    }
}

fn success(
    trace_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    started: Instant,
    track_count: usize,
    compile_mode: CompileMode,
) -> CompileReport {
    CompileReport::success(CompileSuccess {
        trace_id,
        composition_path,
        render_source_path,
        render_source_schema: RENDER_SOURCE_SCHEMA.to_string(),
        duration_ms: started.elapsed().as_millis(),
        track_count,
        compile_mode,
        warnings: Vec::new(),
    })
}

fn compile_with_project_port(
    port: &dyn NextFrameProjectPort,
    trace_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    started: Instant,
) -> CompileReport {
    let artifact = artifact_for_path(&composition_path);
    match port.compile(&artifact, &render_source_path) {
        Ok(_) => {
            let track_count = render_source_track_count(&render_source_path).unwrap_or(0);
            success(
                trace_id,
                composition_path,
                render_source_path,
                started,
                track_count,
                CompileMode::Crate,
            )
        }
        Err(err) => CompileReport::failure(
            trace_id,
            composition_path,
            render_source_path,
            started.elapsed().as_millis(),
            vec![CompileError::new(
                err.body.code,
                "$.crate",
                err.body.message,
                format!("next step · {}", err.body.hint),
            )],
        ),
    }
}

fn artifact_for_path(composition_path: &Path) -> CompositionArtifact {
    let composition_id = composition_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("composition")
        .to_string();
    let project_root = composition_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let project_slug = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("capy-nextframe")
        .to_string();

    CompositionArtifact {
        project_slug,
        composition_id,
        project_root,
        composition_path: composition_path.to_path_buf(),
        component_paths: Vec::new(),
    }
}

fn render_source_track_count(path: &Path) -> Option<usize> {
    let raw = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("tracks")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
}

fn read_composition(path: &Path) -> Result<CompositionDocument, CompileError> {
    let text = fs::read_to_string(path).map_err(|err| {
        CompileError::new(
            "COMPILE_FAILED",
            "$",
            format!("read composition failed: {err}"),
            "next step · check composition path and permissions",
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        CompileError::new(
            "INVALID_COMPOSITION",
            "$",
            format!("composition JSON is invalid: {err}"),
            "next step · rerun capy nextframe validate",
        )
    })
}

fn render_source_path(composition_path: &Path) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("render_source.json")
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn trace_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("compile-{millis}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::{CompileCompositionRequest, compile_composition};

    #[test]
    fn embedded_compile_writes_render_source() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("happy")?;
        let composition = write_composition(&dir, valid_composition())?;
        let old_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", "/definitely/not/on/path");
            std::env::remove_var("CAPY_NF");
        }

        let report = compile_composition(CompileCompositionRequest {
            composition_path: composition,
        });

        unsafe {
            restore_env("PATH", old_path);
        }
        assert!(report.ok);
        assert_eq!(report.compile_mode, "embedded");
        assert_eq!(report.render_source_schema, "nf.render_source.v1");
        assert!(report.render_source_path.is_file());
        let source: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&report.render_source_path)?)?;
        assert_eq!(source["schema_version"], "nf.render_source.v1");
        assert_eq!(source["tracks"].as_array().map(Vec::len), Some(1));
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_missing_composition() {
        let report = compile_composition(CompileCompositionRequest {
            composition_path: PathBuf::from("/definitely/not/composition.json"),
        });

        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "COMPOSITION_NOT_FOUND");
    }

    #[test]
    fn reports_invalid_composition_for_empty_tracks() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("empty")?;
        let composition = write_composition(&dir, json!({"tracks": []}))?;

        let report = compile_composition(CompileCompositionRequest {
            composition_path: composition,
        });

        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "INVALID_COMPOSITION");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    fn valid_composition() -> serde_json::Value {
        json!({
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
                "params": {"poster": {
                    "version": "capy-poster-v0.1",
                    "type": "poster",
                    "canvas": {"width": 1920, "height": 1080, "aspectRatio": "16:9", "background": "#fff"},
                    "assets": {},
                    "layers": [{
                        "id": "headline",
                        "type": "text",
                        "text": "Launch",
                        "x": 10,
                        "y": 10,
                        "width": 100,
                        "height": 40
                    }]
                }}
            }],
            "assets": []
        })
    }

    fn write_composition(
        dir: &Path,
        value: serde_json::Value,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = dir.join("composition.json");
        fs::write(&path, serde_json::to_vec_pretty(&value)?)?;
        Ok(path)
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-nextframe-compile-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    unsafe fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => unsafe {
                std::env::set_var(key, value);
            },
            None => unsafe {
                std::env::remove_var(key);
            },
        }
    }
}
