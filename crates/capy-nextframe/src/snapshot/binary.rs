use std::path::Path;

use crate::adapter::binary::BinaryAdapter;
use crate::config::NextFrameConfig;
use crate::ports::{NextFrameRecorderPort, SnapshotOptions};

use super::report::SnapshotError;

pub enum BinarySnapshot {
    Snapshotted,
    Missing,
    Failed(SnapshotError),
}

pub fn snapshot_with_binary(
    render_source_path: &Path,
    out: &Path,
    frame_ms: u64,
) -> BinarySnapshot {
    let adapter = match BinaryAdapter::new_recorder(NextFrameConfig::default()) {
        Ok(adapter) => adapter,
        Err(err) if err.body.code == "NEXTFRAME_NOT_FOUND" => return BinarySnapshot::Missing,
        Err(err) => {
            return BinarySnapshot::Failed(SnapshotError::new(
                err.body.code,
                "$.binary",
                err.body.message,
                with_next_step(err.body.hint),
            ));
        }
    };
    match adapter.snapshot(
        render_source_path,
        out,
        SnapshotOptions {
            t_ms: frame_ms,
            resolution: None,
        },
    ) {
        Ok(_) => BinarySnapshot::Snapshotted,
        Err(err) if err.body.code == "NEXTFRAME_NOT_FOUND" => BinarySnapshot::Missing,
        Err(err) => BinarySnapshot::Failed(SnapshotError::new(
            err.body.code,
            "$.binary",
            err.body.message,
            with_next_step(err.body.hint),
        )),
    }
}

fn with_next_step(hint: String) -> String {
    if hint.contains("next step ·") {
        hint
    } else {
        format!("next step · {hint}")
    }
}
