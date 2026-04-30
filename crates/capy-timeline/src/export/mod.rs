pub mod embedded;
pub mod report;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use report::{ExportCompositionRequest, ExportError, ExportKind, ExportReport, ExportStatus};

use self::report::{ExportFailure, ExportMode, ExportSuccess};
use crate::adapters::timeline_engine::TimelineAdapter;
use crate::ports::{CompositionArtifact, ExportOptions, TimelineRecorderPort};

pub fn export_composition(req: ExportCompositionRequest) -> ExportReport {
    let composition_path = absolute_path(&req.composition_path);
    let render_source_path = render_source_path(&composition_path);
    let output_path = req
        .out
        .as_deref()
        .map(absolute_path)
        .unwrap_or_else(|| default_output_path(&composition_path, req.kind));
    let context = ExportContext {
        trace_id: trace_id(),
        job_id: job_id(),
        composition_path,
        render_source_path,
        output_path,
        kind: req.kind,
        fps: req.fps,
        profile: normalized_profile(&req.profile),
        resolution: normalized_resolution(req.resolution.as_deref()),
        parallel: req.parallel.unwrap_or(0),
        strict_recorder: req.strict_recorder,
    };

    if !context.composition_path.is_file() {
        return failure(
            context,
            0,
            0,
            None,
            ExportError::new(
                "COMPOSITION_NOT_FOUND",
                "$.composition_path",
                "composition file does not exist",
                "next step · run capy timeline compose-poster",
            ),
        );
    }
    if !context.render_source_path.is_file() {
        return failure(
            context,
            0,
            0,
            None,
            ExportError::new(
                "RENDER_SOURCE_MISSING",
                "$.render_source_path",
                "render_source.json is missing for this composition",
                "next step · run capy timeline compile --composition <path>",
            ),
        );
    }

    let duration_ms = embedded::read_duration_ms(&context.render_source_path).unwrap_or(0);
    let frame_count = embedded::frame_count(duration_ms, req.fps).unwrap_or(0);
    let artifact = artifact_for_path(&context.composition_path);
    let recorder_result = TimelineAdapter::default().export(
        &artifact,
        &context.output_path,
        ExportOptions {
            profile: context.profile.clone(),
            fps: req.fps,
            resolution: requested_resolution(&context),
            parallel: req.parallel,
            strict_recorder: req.strict_recorder,
        },
    );
    match recorder_result {
        Ok(_) => return metrics_success(context, duration_ms, frame_count, ExportMode::Crate),
        Err(error) if context.strict_recorder => {
            return failure(
                context,
                duration_ms,
                frame_count,
                Some(ExportMode::Crate),
                ExportError::new(
                    error.body.code,
                    "$.recorder",
                    error.body.message,
                    error.body.hint,
                ),
            );
        }
        Err(_) => {}
    }

    match embedded::export_embedded(
        &context.render_source_path,
        &context.output_path,
        context.fps,
        &context.job_id,
    ) {
        Ok(metrics) => ExportReport::success(ExportSuccess {
            trace_id: context.trace_id,
            job_id: context.job_id,
            composition_path: context.composition_path,
            render_source_path: context.render_source_path,
            output_path: context.output_path,
            kind: context.kind,
            duration_ms: metrics.duration_ms,
            fps: context.fps,
            profile: context.profile,
            resolution: context.resolution,
            parallel: context.parallel,
            strict_recorder: context.strict_recorder,
            frame_count: metrics.frame_count,
            byte_size: metrics.byte_size,
            export_mode: ExportMode::Embedded,
        }),
        Err(error) => failure(
            context,
            duration_ms,
            frame_count,
            Some(ExportMode::Embedded),
            error,
        ),
    }
}

#[derive(Debug, Clone)]
struct ExportContext {
    trace_id: String,
    job_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    output_path: PathBuf,
    kind: ExportKind,
    fps: u32,
    profile: String,
    resolution: String,
    parallel: usize,
    strict_recorder: bool,
}

fn metrics_success(
    context: ExportContext,
    duration_ms: u64,
    frame_count: u64,
    export_mode: ExportMode,
) -> ExportReport {
    match fs::metadata(&context.output_path) {
        Ok(metadata) => ExportReport::success(ExportSuccess {
            trace_id: context.trace_id,
            job_id: context.job_id,
            composition_path: context.composition_path,
            render_source_path: context.render_source_path,
            output_path: context.output_path,
            kind: context.kind,
            duration_ms,
            fps: context.fps,
            profile: context.profile,
            resolution: context.resolution,
            parallel: context.parallel,
            strict_recorder: context.strict_recorder,
            frame_count,
            byte_size: metadata.len(),
            export_mode,
        }),
        Err(err) => failure(
            context,
            duration_ms,
            frame_count,
            Some(export_mode),
            ExportError::new(
                "EXPORT_FAILED",
                "$.output_path",
                format!("read export metadata failed: {err}"),
                "next step · rerun capy timeline export",
            ),
        ),
    }
}

fn failure(
    context: ExportContext,
    duration_ms: u64,
    frame_count: u64,
    export_mode: Option<ExportMode>,
    error: ExportError,
) -> ExportReport {
    ExportReport::failure(ExportFailure {
        trace_id: context.trace_id,
        job_id: context.job_id,
        composition_path: context.composition_path,
        render_source_path: context.render_source_path,
        output_path: context.output_path,
        kind: context.kind,
        duration_ms,
        fps: context.fps,
        profile: context.profile,
        resolution: context.resolution,
        parallel: context.parallel,
        strict_recorder: context.strict_recorder,
        frame_count,
        export_mode,
        errors: vec![error],
    })
}

fn default_output_path(composition_path: &Path, kind: ExportKind) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("exports")
        .join(format!("export.{}", kind.as_str()))
}

fn artifact_for_path(composition_path: &Path) -> CompositionArtifact {
    let composition_id = composition_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("composition")
        .to_string();
    let composition_dir = composition_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let project_root =
        if composition_dir.file_name().and_then(|name| name.to_str()) == Some("compositions") {
            composition_dir
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or(composition_dir)
        } else {
            composition_dir
        };
    let project_slug = project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("capy-timeline")
        .to_string();

    CompositionArtifact {
        project_slug,
        composition_id,
        project_root,
        composition_path: composition_path.to_path_buf(),
        component_paths: Vec::new(),
    }
}

fn render_source_path(composition_path: &Path) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("render_source.json")
}

fn normalized_profile(raw: &str) -> String {
    let value = raw.trim();
    if value.is_empty() {
        "draft".to_string()
    } else {
        value.to_string()
    }
}

fn normalized_resolution(raw: Option<&str>) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("auto")
        .to_string()
}

fn requested_resolution(context: &ExportContext) -> Option<String> {
    (context.resolution != "auto").then(|| context.resolution.clone())
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
    format!("export-{}-{}", timestamp_millis(), std::process::id())
}

fn job_id() -> String {
    format!("exp-{}-{}", timestamp_millis(), std::process::id())
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{ExportCompositionRequest, ExportKind, export_composition};

    #[test]
    fn reports_missing_composition() {
        let report = export_composition(ExportCompositionRequest {
            composition_path: PathBuf::from("/definitely/not/composition.json"),
            kind: ExportKind::Mp4,
            out: None,
            fps: 30,
            profile: "draft".to_string(),
            resolution: None,
            parallel: None,
            strict_recorder: false,
        });

        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "COMPOSITION_NOT_FOUND");
    }

    #[test]
    fn reports_missing_render_source() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("missing-render-source")?;
        let composition = dir.join("composition.json");
        fs::write(&composition, "{}")?;

        let report = export_composition(ExportCompositionRequest {
            composition_path: composition,
            kind: ExportKind::Mp4,
            out: None,
            fps: 30,
            profile: "draft".to_string(),
            resolution: None,
            parallel: None,
            strict_recorder: false,
        });

        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "RENDER_SOURCE_MISSING");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-timeline-export-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}
