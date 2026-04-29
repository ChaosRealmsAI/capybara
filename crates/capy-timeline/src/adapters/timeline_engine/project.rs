use std::fs;
use std::path::{Path, PathBuf};

use capy_timeline_project::{JsonStorage, ProjectError};
use serde_json::Value;

use crate::config::TimelineConfig;
use crate::error::{TimelineError, TimelineErrorCode};
use crate::ports::{CompileReport, CompositionArtifact, TimelineProjectPort, ValidationReport};

#[derive(Debug, Clone)]
pub struct ProjectAdapter {
    config: TimelineConfig,
}

impl ProjectAdapter {
    pub fn new(config: TimelineConfig) -> Self {
        Self { config }
    }

    fn context(&self, artifact: &CompositionArtifact) -> CrateContext {
        let project_root = if artifact.project_root.as_os_str().is_empty() {
            artifact
                .composition_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        } else {
            artifact.project_root.clone()
        };
        let root = self
            .config
            .home
            .clone()
            .or_else(|| project_root.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        let project_slug = project_root
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| artifact.project_slug.clone());

        CrateContext { root, project_slug }
    }
}

impl Default for ProjectAdapter {
    fn default() -> Self {
        Self::new(TimelineConfig::default())
    }
}

impl TimelineProjectPort for ProjectAdapter {
    fn validate(&self, artifact: &CompositionArtifact) -> Result<ValidationReport, TimelineError> {
        let context = self.context(artifact);
        let storage = JsonStorage::new(context.root);
        let composition = read_json(
            &artifact.composition_path,
            TimelineErrorCode::CompositionInvalid,
        )?;
        let report = capy_timeline_project::validate_composition_components(
            &storage,
            &context.project_slug,
            &composition,
        )
        .map_err(map_project_error)?;
        let stdout = serde_json::to_string(&report).map_err(|err| {
            TimelineError::new(
                TimelineErrorCode::CompositionInvalid,
                format!("serialize capy-timeline-project validation report failed: {err}"),
                "next step · rerun capy timeline validate",
            )
        })?;

        Ok(ValidationReport {
            ok: report.ok,
            command: crate_command("validate", &context.project_slug, &artifact.composition_id),
            stdout,
            stderr: String::new(),
        })
    }

    fn compile(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
    ) -> Result<CompileReport, TimelineError> {
        let context = self.context(artifact);
        let storage = JsonStorage::new(context.root);
        let composition = read_json(&artifact.composition_path, TimelineErrorCode::CompileFailed)?;
        let compiled = capy_timeline_project::compile_composition_source(
            &storage,
            &context.project_slug,
            &composition,
        )
        .map_err(map_project_error)?;
        write_json(out, &compiled.source)?;
        let stdout = serde_json::to_string(&serde_json::json!({
            "project": context.project_slug,
            "composition": artifact.composition_id,
            "out": out.display().to_string(),
            "schema_version": compiled.source.get("schema_version").and_then(Value::as_str),
            "duration_ms": compiled.source.get("duration_ms").and_then(Value::as_u64),
            "tracks": compiled.source.get("tracks").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "warnings": compiled.warnings
        }))
        .map_err(|err| {
            TimelineError::new(
                TimelineErrorCode::CompileFailed,
                format!("serialize capy-timeline-project compile report failed: {err}"),
                "next step · rerun capy timeline compile",
            )
        })?;

        Ok(CompileReport {
            ok: true,
            output: out.to_path_buf(),
            command: crate_command("compile", &context.project_slug, &artifact.composition_id),
            stdout,
            stderr: String::new(),
        })
    }
}

#[derive(Debug, Clone)]
struct CrateContext {
    root: PathBuf,
    project_slug: String,
}

fn crate_command(action: &str, project: &str, composition: &str) -> Vec<String> {
    vec![
        "capy-timeline-project".to_string(),
        "composition".to_string(),
        action.to_string(),
        "--project".to_string(),
        project.to_string(),
        "--composition".to_string(),
        composition.to_string(),
    ]
}

fn read_json(path: &Path, code: TimelineErrorCode) -> Result<Value, TimelineError> {
    let raw = fs::read_to_string(path).map_err(|err| {
        TimelineError::new(
            code,
            format!("read composition failed: {err}"),
            "next step · check composition path and permissions",
        )
    })?;
    serde_json::from_str(&raw).map_err(|err| {
        TimelineError::new(
            code,
            format!("composition JSON is invalid: {err}"),
            "next step · rerun capy timeline validate",
        )
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), TimelineError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            TimelineError::new(
                TimelineErrorCode::CompileFailed,
                format!("create render source directory failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    let mut text = serde_json::to_string_pretty(value).map_err(|err| {
        TimelineError::new(
            TimelineErrorCode::CompileFailed,
            format!("serialize render source failed: {err}"),
            "next step · rerun capy timeline compile",
        )
    })?;
    text.push('\n');
    fs::write(path, text).map_err(|err| {
        TimelineError::new(
            TimelineErrorCode::CompileFailed,
            format!("write render source failed: {err}"),
            "next step · check output directory permissions",
        )
    })
}

fn map_project_error(err: ProjectError) -> TimelineError {
    let code = match &err {
        ProjectError::ValidationFailed(_) | ProjectError::SlugInvalid { .. } => {
            TimelineErrorCode::CompositionInvalid
        }
        ProjectError::StorageFailed(_) => TimelineErrorCode::CompileFailed,
    };
    TimelineError::new(
        code,
        err.to_string(),
        "next step · rerun capy timeline validate",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::ProjectAdapter;
    use crate::ports::{CompositionArtifact, TimelineProjectPort};

    #[test]
    fn validates_composition_with_components() -> Result<(), Box<dyn std::error::Error>> {
        let dir = fixture_project("validate")?;
        let artifact = artifact(&dir);

        let report = ProjectAdapter::default().validate(&artifact)?;

        assert!(report.ok);
        assert_eq!(report.command[0], "capy-timeline-project");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&report.stdout)?["ok"],
            true
        );
        fs::remove_dir_all(project_parent(&dir)?)?;
        Ok(())
    }

    #[test]
    fn compiles_render_source_with_timeline_project() -> Result<(), Box<dyn std::error::Error>> {
        let dir = fixture_project("compile")?;
        let artifact = artifact(&dir);
        let out = dir.join("render_source.json");

        let report = ProjectAdapter::default().compile(&artifact, &out)?;

        assert!(report.ok);
        assert_eq!(report.output, out);
        let source: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out)?)?;
        assert_eq!(source["schema_version"], "capy.timeline.render_source.v1");
        assert_eq!(source["tracks"].as_array().map(Vec::len), Some(1));
        fs::remove_dir_all(project_parent(&dir)?)?;
        Ok(())
    }

    #[test]
    fn rejects_missing_component() -> Result<(), Box<dyn std::error::Error>> {
        let dir = fixture_project("missing-component")?;
        fs::remove_file(dir.join("components/html.capy-poster.js"))?;
        let artifact = artifact(&dir);

        let report = ProjectAdapter::default().validate(&artifact)?;

        assert!(!report.ok);
        let value: serde_json::Value = serde_json::from_str(&report.stdout)?;
        assert_eq!(value["errors"].as_array().map(Vec::len), Some(1));
        fs::remove_dir_all(project_parent(&dir)?)?;
        Ok(())
    }

    #[test]
    fn reports_invalid_json() -> Result<(), Box<dyn std::error::Error>> {
        let dir = fixture_project("invalid-json")?;
        fs::write(dir.join("composition.json"), "{")?;
        let artifact = artifact(&dir);

        let err = match ProjectAdapter::default().validate(&artifact) {
            Ok(_) => return Err("invalid JSON should fail".into()),
            Err(err) => err,
        };

        assert_eq!(err.body.code, "COMPOSITION_INVALID");
        fs::remove_dir_all(project_parent(&dir)?)?;
        Ok(())
    }

    fn fixture_project(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "capy-timeline-crate-adapter-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        let project = root.join("demo");
        fs::create_dir_all(project.join("components"))?;
        fs::write(
            project.join("components/html.capy-poster.js"),
            "export function mount() {}\nexport function update() {}\nexport function destroy() {}\n",
        )?;
        fs::write(
            project.join("composition.json"),
            serde_json::to_vec_pretty(&json!({
                "schema": "capy.timeline.composition.v1",
                "id": "poster-snapshot",
                "name": "Poster Snapshot",
                "duration": "1000ms",
                "viewport": {"w": 1920, "h": 1080, "ratio": "16:9"},
                "theme": "default",
                "tracks": [{
                    "id": "track-poster",
                    "kind": "component",
                    "component": "html.capy-poster",
                    "time": {"start": "0ms", "end": "1000ms"},
                    "params": {"poster": {"type": "poster"}}
                }],
                "assets": []
            }))?,
        )?;
        Ok(project)
    }

    fn artifact(project_root: &Path) -> CompositionArtifact {
        CompositionArtifact {
            project_slug: "demo".to_string(),
            composition_id: "poster-snapshot".to_string(),
            project_root: project_root.to_path_buf(),
            composition_path: project_root.join("composition.json"),
            component_paths: Vec::new(),
        }
    }

    fn project_parent(project_root: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
        project_root
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "project parent should exist".into())
    }
}
