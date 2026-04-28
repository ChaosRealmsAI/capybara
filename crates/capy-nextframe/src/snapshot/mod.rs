pub mod binary;
pub mod embedded;
pub mod report;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use report::{SnapshotError, SnapshotMode, SnapshotReport, SnapshotRequest};

use self::report::{SnapshotFailure, SnapshotSuccess};

pub fn snapshot(req: SnapshotRequest) -> SnapshotReport {
    let trace_id = trace_id();
    let composition_path = absolute_path(&req.composition_path);
    let render_source_path = render_source_path(&composition_path);
    let snapshot_path = req
        .out
        .as_deref()
        .map(absolute_path)
        .unwrap_or_else(|| default_snapshot_path(&composition_path, req.frame_ms));

    if !composition_path.is_file() {
        return failure(
            trace_id,
            composition_path,
            render_source_path,
            snapshot_path,
            req.frame_ms,
            SnapshotError::new(
                "COMPOSITION_NOT_FOUND",
                "$.composition_path",
                "composition file does not exist",
                "next step · run capy nextframe compose-poster",
            ),
        );
    }
    if !render_source_path.is_file() {
        return failure(
            trace_id,
            composition_path,
            render_source_path,
            snapshot_path,
            req.frame_ms,
            SnapshotError::new(
                "RENDER_SOURCE_MISSING",
                "$.render_source_path",
                "render_source.json is missing for this composition",
                "next step · run capy nextframe compile --composition <path>",
            ),
        );
    }

    match binary::snapshot_with_binary(&render_source_path, &snapshot_path, req.frame_ms) {
        binary::BinarySnapshot::Snapshotted => metrics_success(
            trace_id,
            composition_path,
            render_source_path,
            snapshot_path,
            req.frame_ms,
            SnapshotMode::Binary,
        ),
        binary::BinarySnapshot::Failed(error) => failure(
            trace_id,
            composition_path,
            render_source_path,
            snapshot_path,
            req.frame_ms,
            error,
        ),
        binary::BinarySnapshot::Missing if req.strict_binary => failure(
            trace_id,
            composition_path,
            render_source_path,
            snapshot_path,
            req.frame_ms,
            SnapshotError::new(
                "NEXTFRAME_NOT_FOUND",
                "$.binary",
                "nf-recorder was not found on PATH or CAPY_NF_RECORDER",
                "next step · install nf-recorder or rerun without --strict-binary",
            ),
        ),
        binary::BinarySnapshot::Missing => {
            match embedded::snapshot_embedded(&render_source_path, &snapshot_path) {
                Ok(metrics) => SnapshotReport::success(SnapshotSuccess {
                    trace_id,
                    composition_path,
                    render_source_path,
                    snapshot_path,
                    frame_ms: req.frame_ms,
                    snapshot_mode: SnapshotMode::Embedded,
                    width: metrics.width,
                    height: metrics.height,
                    byte_size: metrics.byte_size,
                }),
                Err(error) => failure(
                    trace_id,
                    composition_path,
                    render_source_path,
                    snapshot_path,
                    req.frame_ms,
                    error,
                ),
            }
        }
    }
}

fn metrics_success(
    trace_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    snapshot_path: PathBuf,
    frame_ms: u64,
    snapshot_mode: SnapshotMode,
) -> SnapshotReport {
    match embedded::read_png_metrics(&snapshot_path) {
        Ok(metrics) => SnapshotReport::success(SnapshotSuccess {
            trace_id,
            composition_path,
            render_source_path,
            snapshot_path,
            frame_ms,
            snapshot_mode,
            width: metrics.width,
            height: metrics.height,
            byte_size: metrics.byte_size,
        }),
        Err(error) => failure(
            trace_id,
            composition_path,
            render_source_path,
            snapshot_path,
            frame_ms,
            error,
        ),
    }
}

fn failure(
    trace_id: String,
    composition_path: PathBuf,
    render_source_path: PathBuf,
    snapshot_path: PathBuf,
    frame_ms: u64,
    error: SnapshotError,
) -> SnapshotReport {
    SnapshotReport::failure(SnapshotFailure {
        trace_id,
        composition_path,
        render_source_path,
        snapshot_path,
        frame_ms,
        errors: vec![error],
    })
}

fn default_snapshot_path(composition_path: &Path, frame_ms: u64) -> PathBuf {
    composition_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("snapshots")
        .join(format!("frame-{frame_ms}.png"))
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
    format!("snapshot-{millis}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{SnapshotRequest, snapshot};

    #[test]
    fn reports_missing_render_source() -> Result<(), Box<dyn std::error::Error>> {
        let dir = unique_dir("missing-render-source")?;
        let composition = dir.join("composition.json");
        fs::write(&composition, "{}")?;

        let report = snapshot(SnapshotRequest {
            composition_path: composition,
            frame_ms: 0,
            out: None,
            strict_binary: false,
        });

        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "RENDER_SOURCE_MISSING");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn reports_missing_composition() {
        let report = snapshot(SnapshotRequest {
            composition_path: PathBuf::from("/definitely/not/composition.json"),
            frame_ms: 0,
            out: None,
            strict_binary: false,
        });

        assert!(!report.ok);
        assert_eq!(report.errors[0].code, "COMPOSITION_NOT_FOUND");
    }

    fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "capy-nextframe-snapshot-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis()
        ));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}
