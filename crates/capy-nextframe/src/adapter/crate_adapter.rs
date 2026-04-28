use std::fs;
use std::path::{Path, PathBuf};

use nf_project::{JsonStorage, ProjectError};
use serde_json::Value;

use crate::config::NextFrameConfig;
use crate::error::{NextFrameError, NextFrameErrorCode};
use crate::ports::{
    CompileReport, CompositionArtifact, ExportOptions, ExportReport, NextFrameProjectPort,
    NextFrameRecorderPort, SnapshotOptions, SnapshotReport, ValidationReport,
};

#[derive(Debug, Clone)]
pub struct CrateAdapter {
    config: NextFrameConfig,
}

impl CrateAdapter {
    pub fn new(config: NextFrameConfig) -> Self {
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

impl Default for CrateAdapter {
    fn default() -> Self {
        Self::new(NextFrameConfig::default())
    }
}

impl NextFrameProjectPort for CrateAdapter {
    fn validate(&self, artifact: &CompositionArtifact) -> Result<ValidationReport, NextFrameError> {
        let context = self.context(artifact);
        let storage = JsonStorage::new(context.root);
        let composition = read_json(
            &artifact.composition_path,
            NextFrameErrorCode::CompositionInvalid,
        )?;
        let report = nf_project::validate_composition_components(
            &storage,
            &context.project_slug,
            &composition,
        )
        .map_err(map_project_error)?;
        let stdout = serde_json::to_string(&report).map_err(|err| {
            NextFrameError::new(
                NextFrameErrorCode::CompositionInvalid,
                format!("serialize nf-project validation report failed: {err}"),
                "next step · rerun capy nextframe validate",
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
    ) -> Result<CompileReport, NextFrameError> {
        let context = self.context(artifact);
        let storage = JsonStorage::new(context.root);
        let composition = read_json(
            &artifact.composition_path,
            NextFrameErrorCode::CompileFailed,
        )?;
        let compiled =
            nf_project::compile_composition_source(&storage, &context.project_slug, &composition)
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
            NextFrameError::new(
                NextFrameErrorCode::CompileFailed,
                format!("serialize nf-project compile report failed: {err}"),
                "next step · rerun capy nextframe compile",
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

impl NextFrameRecorderPort for CrateAdapter {
    fn snapshot(
        &self,
        source: &Path,
        out: &Path,
        options: SnapshotOptions,
    ) -> Result<SnapshotReport, NextFrameError> {
        snapshot_with_recorder_crate(source, out, options)
    }

    fn export(
        &self,
        artifact: &CompositionArtifact,
        out: &Path,
        options: ExportOptions,
    ) -> Result<ExportReport, NextFrameError> {
        export_with_recorder_crate(artifact, out, options)
    }
}

#[derive(Debug, Clone)]
struct CrateContext {
    root: PathBuf,
    project_slug: String,
}

fn crate_command(action: &str, project: &str, composition: &str) -> Vec<String> {
    vec![
        "nf-project".to_string(),
        "composition".to_string(),
        action.to_string(),
        "--project".to_string(),
        project.to_string(),
        "--composition".to_string(),
        composition.to_string(),
    ]
}

#[cfg(target_os = "macos")]
fn snapshot_with_recorder_crate(
    source: &Path,
    out: &Path,
    options: SnapshotOptions,
) -> Result<SnapshotReport, NextFrameError> {
    ensure_parent(out, NextFrameErrorCode::SnapshotFailed)?;
    let resolution = options
        .resolution
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(parse_resolution)
        .transpose()?;
    recorder_runtime()?
        .block_on(nf_recorder::snapshot_from_source(
            source,
            out,
            options.t_ms,
            resolution,
        ))
        .map_err(|err| {
            NextFrameError::new(
                NextFrameErrorCode::SnapshotFailed,
                format!("nf-recorder crate snapshot failed: {err}"),
                "next step · rerun CAPY_NEXTFRAME_MODE=crate capy nextframe snapshot",
            )
        })?;

    Ok(SnapshotReport {
        ok: true,
        output: out.to_path_buf(),
        command: recorder_snapshot_command(source, out, options.t_ms),
        stdout: String::new(),
        stderr: String::new(),
    })
}

#[cfg(not(target_os = "macos"))]
fn snapshot_with_recorder_crate(
    _source: &Path,
    _out: &Path,
    _options: SnapshotOptions,
) -> Result<SnapshotReport, NextFrameError> {
    Err(NextFrameError::new(
        NextFrameErrorCode::NextframeNotFound,
        "nf-recorder crate snapshot is only available on macOS",
        "embedded mode required",
    ))
}

#[cfg(target_os = "macos")]
fn export_with_recorder_crate(
    artifact: &CompositionArtifact,
    out: &Path,
    options: ExportOptions,
) -> Result<ExportReport, NextFrameError> {
    let source = render_source_path(&artifact.composition_path);
    ensure_parent(out, NextFrameErrorCode::ExportFailed)?;
    let summary = nf_recorder::validate_render_source_file(&source).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::ExportFailed,
            format!("nf-recorder source validation failed: {err}"),
            "next step · rerun capy nextframe compile",
        )
    })?;
    let fps = options.fps.max(1);
    let stats = recorder_runtime()?
        .block_on(nf_recorder::run_export_from_source(
            &source,
            out,
            nf_recorder::ExportOpts {
                duration_s: summary.duration_ms as f64 / 1000.0,
                viewport: summary.viewport,
                fps,
                ..Default::default()
            },
        ))
        .map_err(|err| {
            NextFrameError::new(
                NextFrameErrorCode::ExportFailed,
                format!("nf-recorder crate export failed: {err}"),
                "next step · rerun CAPY_NEXTFRAME_MODE=crate capy nextframe export",
            )
        })?;

    Ok(ExportReport {
        ok: true,
        output: out.to_path_buf(),
        command: recorder_export_command(&source, out, fps),
        stdout: serde_json::to_string(&serde_json::json!({
            "path": stats.path,
            "frames": stats.frames,
            "duration_ms": stats.duration_ms,
            "size_bytes": stats.size_bytes,
            "moov_front": stats.moov_front
        }))
        .unwrap_or_default(),
        stderr: String::new(),
    })
}

#[cfg(not(target_os = "macos"))]
fn export_with_recorder_crate(
    _artifact: &CompositionArtifact,
    _out: &Path,
    _options: ExportOptions,
) -> Result<ExportReport, NextFrameError> {
    Err(NextFrameError::new(
        NextFrameErrorCode::NextframeNotFound,
        "nf-recorder crate export is only available on macOS",
        "embedded mode required",
    ))
}

fn render_source_path(composition_path: &Path) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("render_source.json")
}

fn recorder_snapshot_command(source: &Path, out: &Path, t_ms: u64) -> Vec<String> {
    vec![
        "nf-recorder".to_string(),
        "snapshot-source".to_string(),
        "--source".to_string(),
        source.display().to_string(),
        "--t-ms".to_string(),
        t_ms.to_string(),
        "--output".to_string(),
        out.display().to_string(),
    ]
}

fn recorder_export_command(source: &Path, out: &Path, fps: u32) -> Vec<String> {
    vec![
        "nf-recorder".to_string(),
        "export".to_string(),
        "--source".to_string(),
        source.display().to_string(),
        "--profile".to_string(),
        "draft".to_string(),
        "--output".to_string(),
        out.display().to_string(),
        "--fps".to_string(),
        fps.to_string(),
    ]
}

#[cfg(target_os = "macos")]
fn parse_resolution(raw: &str) -> Result<nf_recorder::ExportResolution, NextFrameError> {
    nf_recorder::ExportResolution::parse_str(raw).ok_or_else(|| {
        NextFrameError::new(
            NextFrameErrorCode::SnapshotFailed,
            format!("unsupported snapshot resolution: {raw}"),
            "next step · pass resolution 720p, 1080p, or 4k",
        )
    })
}

#[cfg(target_os = "macos")]
fn recorder_runtime() -> Result<tokio::runtime::Runtime, NextFrameError> {
    if !current_exe_is_macos_app_bundle() {
        return Err(NextFrameError::new(
            NextFrameErrorCode::NextframeNotFound,
            "nf-recorder crate mode requires a macOS app bundle CEF runtime",
            "embedded mode required",
        ));
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| {
            NextFrameError::new(
                NextFrameErrorCode::NextframeNotFound,
                format!("create nf-recorder runtime failed: {err}"),
                "embedded mode required",
            )
        })
}

#[cfg(target_os = "macos")]
fn current_exe_is_macos_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            let contents_dir = exe.parent()?.parent()?;
            let app_dir = contents_dir.parent()?;
            let has_contents = contents_dir.file_name()?.to_str()? == "Contents";
            let has_app_extension = app_dir.extension()?.to_str()? == "app";
            Some(has_contents && has_app_extension)
        })
        .unwrap_or(false)
}

fn ensure_parent(path: &Path, code: NextFrameErrorCode) -> Result<(), NextFrameError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            NextFrameError::new(
                code,
                format!("create output parent failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    Ok(())
}

fn read_json(path: &Path, code: NextFrameErrorCode) -> Result<Value, NextFrameError> {
    let raw = fs::read_to_string(path).map_err(|err| {
        NextFrameError::new(
            code,
            format!("read composition failed: {err}"),
            "next step · check composition path and permissions",
        )
    })?;
    serde_json::from_str(&raw).map_err(|err| {
        NextFrameError::new(
            code,
            format!("composition JSON is invalid: {err}"),
            "next step · rerun capy nextframe validate",
        )
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), NextFrameError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            NextFrameError::new(
                NextFrameErrorCode::CompileFailed,
                format!("create render source directory failed: {err}"),
                "next step · check output directory permissions",
            )
        })?;
    }
    let mut text = serde_json::to_string_pretty(value).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::CompileFailed,
            format!("serialize render source failed: {err}"),
            "next step · rerun capy nextframe compile",
        )
    })?;
    text.push('\n');
    fs::write(path, text).map_err(|err| {
        NextFrameError::new(
            NextFrameErrorCode::CompileFailed,
            format!("write render source failed: {err}"),
            "next step · check output directory permissions",
        )
    })
}

fn map_project_error(err: ProjectError) -> NextFrameError {
    let code = match &err {
        ProjectError::ValidationFailed(_) | ProjectError::SlugInvalid { .. } => {
            NextFrameErrorCode::CompositionInvalid
        }
        ProjectError::StorageFailed(_) => NextFrameErrorCode::CompileFailed,
    };
    NextFrameError::new(
        code,
        err.to_string(),
        "next step · rerun capy nextframe validate",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::{CrateAdapter, recorder_export_command, recorder_snapshot_command};
    use crate::ports::{CompositionArtifact, NextFrameProjectPort};

    #[test]
    fn validates_composition_with_components() -> Result<(), Box<dyn std::error::Error>> {
        let dir = fixture_project("validate")?;
        let artifact = artifact(&dir);

        let report = CrateAdapter::default().validate(&artifact)?;

        assert!(report.ok);
        assert_eq!(report.command[0], "nf-project");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&report.stdout)?["ok"],
            true
        );
        fs::remove_dir_all(project_parent(&dir)?)?;
        Ok(())
    }

    #[test]
    fn compiles_render_source_with_nf_project() -> Result<(), Box<dyn std::error::Error>> {
        let dir = fixture_project("compile")?;
        let artifact = artifact(&dir);
        let out = dir.join("render_source.json");

        let report = CrateAdapter::default().compile(&artifact, &out)?;

        assert!(report.ok);
        assert_eq!(report.output, out);
        let source: serde_json::Value = serde_json::from_str(&fs::read_to_string(&out)?)?;
        assert_eq!(source["schema_version"], "nf.render_source.v1");
        assert_eq!(source["tracks"].as_array().map(Vec::len), Some(1));
        fs::remove_dir_all(project_parent(&dir)?)?;
        Ok(())
    }

    #[test]
    fn rejects_missing_component() -> Result<(), Box<dyn std::error::Error>> {
        let dir = fixture_project("missing-component")?;
        fs::remove_file(dir.join("components/html.capy-poster.js"))?;
        let artifact = artifact(&dir);

        let report = CrateAdapter::default().validate(&artifact)?;

        assert!(!report.ok);
        let value: serde_json::Value = serde_json::from_str(&report.stdout)?;
        assert_eq!(value["errors"].as_array().map(Vec::len), Some(1));
        fs::remove_dir_all(project_parent(&dir)?)?;
        Ok(())
    }

    #[test]
    fn recorder_snapshot_command_targets_source_api() {
        let command = recorder_snapshot_command(
            Path::new("/tmp/render_source.json"),
            Path::new("/tmp/frame.png"),
            42,
        );

        assert_eq!(command[0], "nf-recorder");
        assert_eq!(command[1], "snapshot-source");
        assert!(command.contains(&"--source".to_string()));
        assert!(command.contains(&"--t-ms".to_string()));
        assert!(command.contains(&"42".to_string()));
        assert!(command.contains(&"/tmp/frame.png".to_string()));
    }

    #[test]
    fn recorder_export_command_carries_fps() {
        let command = recorder_export_command(
            Path::new("/tmp/render_source.json"),
            Path::new("/tmp/out.mp4"),
            24,
        );

        assert_eq!(command[0], "nf-recorder");
        assert_eq!(command[1], "export");
        assert!(command.contains(&"--source".to_string()));
        assert!(command.contains(&"--fps".to_string()));
        assert!(command.contains(&"24".to_string()));
        assert!(command.contains(&"/tmp/out.mp4".to_string()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unit_test_binary_is_not_treated_as_app_bundle() {
        assert!(!super::current_exe_is_macos_app_bundle());
    }

    #[test]
    fn reports_invalid_json() -> Result<(), Box<dyn std::error::Error>> {
        let dir = fixture_project("invalid-json")?;
        fs::write(dir.join("composition.json"), "{")?;
        let artifact = artifact(&dir);

        let err = match CrateAdapter::default().validate(&artifact) {
            Ok(_) => return Err("invalid JSON should fail".into()),
            Err(err) => err,
        };

        assert_eq!(err.body.code, "COMPOSITION_INVALID");
        fs::remove_dir_all(project_parent(&dir)?)?;
        Ok(())
    }

    fn fixture_project(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "capy-nextframe-crate-adapter-{label}-{}-{}",
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
                "schema": "nextframe.composition.v2",
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
