pub mod html;
pub mod report;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub use report::{VerifyError, VerifyExportRequest, VerifyReport, VerifyStages};

use crate::compile::{CompileCompositionRequest, compile_composition};
use crate::export::{ExportCompositionRequest, ExportKind, export_composition};
use crate::snapshot::{SnapshotRequest, snapshot};
use crate::validate::{ValidateCompositionRequest, validate_composition};

pub fn verify_export(req: VerifyExportRequest) -> VerifyReport {
    let started = Instant::now();
    let trace_id = trace_id();
    let composition_path = absolute_path(&req.composition_path);
    let evidence_index_html = req
        .out_html
        .as_deref()
        .map(absolute_path)
        .unwrap_or_else(|| default_index_path(&composition_path));
    let evidence_root = evidence_index_html
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| absolute_path(Path::new("evidence")));

    let validate = validate_composition(ValidateCompositionRequest {
        composition_path: composition_path.clone(),
        strict_binary: false,
    });
    let compile = compile_composition(CompileCompositionRequest {
        composition_path: composition_path.clone(),
        strict_binary: false,
    });
    let snapshot = snapshot(SnapshotRequest {
        composition_path: composition_path.clone(),
        frame_ms: 0,
        out: None,
        strict_binary: false,
    });
    let export = export_composition(ExportCompositionRequest {
        composition_path: composition_path.clone(),
        kind: ExportKind::Mp4,
        out: None,
        fps: 30,
        strict_binary: false,
    });

    let title = composition_title(&composition_path);
    let mut report = VerifyReport::new(
        trace_id,
        composition_path,
        evidence_root,
        evidence_index_html,
        VerifyStages {
            validate,
            compile,
            snapshot,
            export,
        },
        started.elapsed().as_millis(),
        title,
    );

    match write_index(&report) {
        Ok(()) => {}
        Err(error) => report.push_error(error),
    }
    report.duration_ms = started.elapsed().as_millis();
    report
}

fn write_index(report: &VerifyReport) -> Result<(), VerifyError> {
    if let Some(parent) = report.evidence_index_html.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            VerifyError::new(
                "EVIDENCE_WRITE_FAILED",
                "$.evidence_root",
                format!("create evidence directory failed: {err}"),
                "next step · choose a writable --out-html path",
            )
        })?;
    }
    let html = html::render_index(report)?;
    fs::write(&report.evidence_index_html, html).map_err(|err| {
        VerifyError::new(
            "EVIDENCE_WRITE_FAILED",
            "$.evidence_index_html",
            format!("write evidence index failed: {err}"),
            "next step · choose a writable --out-html path",
        )
    })
}

fn composition_title(path: &Path) -> String {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| {
            value
                .get("title")
                .and_then(serde_json::Value::as_str)
                .or_else(|| value.get("name").and_then(serde_json::Value::as_str))
                .map(str::to_string)
        })
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "NextFrame Composition".to_string())
}

fn default_index_path(composition_path: &Path) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("evidence")
        .join("index.html")
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
    format!("verify-{millis}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::{VerifyExportRequest, verify_export};

    #[test]
    fn verify_export_writes_index_for_happy_path() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("happy")?;
        let composition = write_composition(&dir, valid_composition())?;

        let report = verify_export(VerifyExportRequest {
            composition_path: composition,
            out_html: None,
        });

        assert!(report.ok);
        assert_eq!(report.stage, "verify-export");
        assert_eq!(report.verdict, "passed");
        assert_eq!(report.stages.validate.track_count, 1);
        assert_eq!(report.stages.compile.compile_mode, "embedded");
        assert!(report.stages.snapshot.snapshot_path.is_file());
        assert!(report.stages.export.output_path.is_file());
        assert!(report.evidence_index_html.is_file());
        let html = fs::read_to_string(&report.evidence_index_html)?;
        assert!(html.contains("stage-card"));
        assert!(html.contains("<video"));
        assert!(html.contains("<img"));
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn verify_export_reports_missing_composition() {
        let report = verify_export(VerifyExportRequest {
            composition_path: PathBuf::from("/definitely/not/composition.json"),
            out_html: None,
        });

        assert!(!report.ok);
        assert_eq!(report.verdict, "failed");
        assert_eq!(
            report.stages.validate.errors[0].code,
            "COMPOSITION_NOT_FOUND"
        );
        assert_eq!(
            report.stages.compile.errors[0].code,
            "COMPOSITION_NOT_FOUND"
        );
    }

    #[test]
    fn verify_export_reports_invalid_composition() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("invalid")?;
        let composition = write_composition(&dir, json!({"tracks": []}))?;

        let report = verify_export(VerifyExportRequest {
            composition_path: composition,
            out_html: None,
        });

        assert!(!report.ok);
        assert_eq!(report.verdict, "failed");
        assert_eq!(report.stages.validate.errors[0].code, "COMPOSITION_INVALID");
        assert_eq!(report.stages.compile.errors[0].code, "INVALID_COMPOSITION");
        assert!(report.evidence_index_html.is_file());
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn verify_export_reports_unwritable_index_path() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("unwritable-index")?;
        let composition = write_composition(&dir, valid_composition())?;
        let out_html = dir.join("snapshots");

        let report = verify_export(VerifyExportRequest {
            composition_path: composition,
            out_html: Some(out_html),
        });

        assert!(!report.ok);
        assert_eq!(report.verdict, "failed");
        assert_eq!(report.errors[0].code, "EVIDENCE_WRITE_FAILED");
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
            "duration_ms": 200,
            "duration": "200ms",
            "viewport": {"w": 1920, "h": 1080, "ratio": "16:9"},
            "theme": "default",
            "tracks": [{
                "id": "track-poster",
                "kind": "component",
                "component": "html.capy-poster",
                "z": 10,
                "time": {"start": "0ms", "end": "200ms"},
                "duration_ms": 200,
                "params": {"poster": {
                    "version": "capy-poster-v0.1",
                    "type": "poster",
                    "canvas": {"width": 1920, "height": 1080, "aspectRatio": "16:9", "background": "#ffffff"},
                    "assets": {},
                    "layers": [{
                        "id": "headline",
                        "type": "text",
                        "text": "Verify Export",
                        "x": 96,
                        "y": 96,
                        "width": 900,
                        "height": 160,
                        "z": 1,
                        "style": {"color": "#111111", "fontSize": 72, "fontWeight": 700}
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
        fs::write(path.clone(), serde_json::to_string_pretty(&value)?)?;
        Ok(path)
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-nextframe-verify-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}
