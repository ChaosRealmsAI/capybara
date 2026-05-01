use std::path::PathBuf;

use serde_json::Value;

use crate::package::{CAPY_DIR, ProjectPackage, ProjectPackageResult, read_json};
use crate::video_clip_proposal_types::{
    ProjectVideoClipProposalHistoryEntryV1, ProjectVideoClipProposalHistoryV1,
    ProjectVideoClipProposalV1, VIDEO_CLIP_PROPOSAL_HISTORY_SCHEMA_VERSION,
};
use crate::video_clip_queue::ProjectVideoClipQueueItemV1;

impl ProjectPackage {
    pub fn video_clip_proposal_history(
        &self,
    ) -> ProjectPackageResult<ProjectVideoClipProposalHistoryV1> {
        let project = self.project_manifest()?;
        let path = self.video_clip_proposal_history_path();
        if !path.exists() {
            return Ok(ProjectVideoClipProposalHistoryV1 {
                schema_version: VIDEO_CLIP_PROPOSAL_HISTORY_SCHEMA_VERSION.to_string(),
                project_id: project.id,
                project_name: project.name,
                updated_at: 0,
                entries: Vec::new(),
            });
        }
        let mut history = read_json::<ProjectVideoClipProposalHistoryV1>(
            &path,
            "read project video clip proposal history",
        )?;
        if history.project_id.trim().is_empty() {
            history.project_id = project.id;
        }
        if history.project_name.trim().is_empty() {
            history.project_name = project.name;
        }
        Ok(history)
    }

    pub(crate) fn video_clip_proposal_path(&self) -> PathBuf {
        self.root().join(CAPY_DIR).join("video-clip-proposal.json")
    }

    pub(crate) fn next_video_clip_proposal_revision(&self) -> u64 {
        let current = self
            .video_clip_proposal()
            .map(|proposal| proposal.revision)
            .unwrap_or(0);
        let history = self
            .video_clip_proposal_history()
            .map(|history| {
                history
                    .entries
                    .iter()
                    .map(|entry| entry.revision)
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        current.max(history).saturating_add(1).max(1)
    }

    pub(crate) fn upsert_video_clip_proposal_history(
        &self,
        proposal: &ProjectVideoClipProposalV1,
    ) -> ProjectPackageResult<ProjectVideoClipProposalHistoryV1> {
        let mut history = self.video_clip_proposal_history()?;
        history.schema_version = VIDEO_CLIP_PROPOSAL_HISTORY_SCHEMA_VERSION.to_string();
        history.project_id = proposal.project_id.clone();
        history.project_name = proposal.project_name.clone();
        history.updated_at = proposal.decided_at.unwrap_or(proposal.generated_at);
        let entry = proposal_history_entry(proposal);
        let existing = history.entries.iter().position(|candidate| {
            candidate.proposal_id == entry.proposal_id && candidate.revision == entry.revision
        });
        if let Some(index) = existing {
            history.entries[index] = entry;
        } else {
            history.entries.push(entry);
        }
        history
            .entries
            .sort_by_key(|entry| (entry.generated_at, entry.revision));
        self.write_json(&self.video_clip_proposal_history_path(), &history)?;
        Ok(history)
    }

    fn video_clip_proposal_history_path(&self) -> PathBuf {
        self.root()
            .join(CAPY_DIR)
            .join("video-clip-proposal-history.json")
    }
}

pub(crate) fn proposal_base_queue_hash(proposal: &ProjectVideoClipProposalV1) -> String {
    if proposal.base_queue_hash.trim().is_empty() {
        queue_hash(&proposal.before_queue)
    } else {
        proposal.base_queue_hash.clone()
    }
}

pub(crate) fn queue_hash(items: &[ProjectVideoClipQueueItemV1]) -> String {
    let basis = items.iter().map(queue_basis).collect::<Vec<_>>();
    format!(
        "queue-fnv1a64-{:016x}",
        fnv1a64(serde_json::to_string(&basis).unwrap_or_default().as_bytes())
    )
}

pub(crate) fn queue_basis(item: &ProjectVideoClipQueueItemV1) -> Value {
    serde_json::json!({
        "id": item.id,
        "sequence": item.sequence,
        "composition_path": item.composition_path,
        "clip_id": item.clip_id,
        "start_ms": item.start_ms,
        "end_ms": item.end_ms
    })
}

fn proposal_history_entry(
    proposal: &ProjectVideoClipProposalV1,
) -> ProjectVideoClipProposalHistoryEntryV1 {
    ProjectVideoClipProposalHistoryEntryV1 {
        proposal_id: proposal.proposal_id.clone(),
        revision: proposal.revision,
        source_suggestion_id: proposal.source_suggestion_id.clone(),
        planner: proposal.planner.clone(),
        status: proposal.status.clone(),
        generated_at: proposal.generated_at,
        decided_at: proposal.decided_at,
        base_queue_hash: proposal_base_queue_hash(proposal),
        current_queue_hash: proposal.current_queue_hash.clone(),
        rationale: proposal.rationale.clone(),
        safety_note: proposal.safety_note.clone(),
        before_queue_count: proposal.before_queue.len(),
        after_queue_count: proposal.after_queue.len(),
        changes: proposal.changes.clone(),
        decision: proposal.decision.clone(),
        conflict: proposal.conflict.clone(),
    }
}

pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
