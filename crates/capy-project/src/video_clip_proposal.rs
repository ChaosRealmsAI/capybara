use std::collections::BTreeMap;

use crate::package::{
    ProjectPackage, ProjectPackageError, ProjectPackageResult, now_ms, read_json,
};
use crate::video_clip_feedback::queue_item_clip_key;
use crate::video_clip_proposal_history::{
    fnv1a64, proposal_base_queue_hash, queue_basis, queue_hash,
};
use crate::video_clip_proposal_types::{
    ProjectVideoClipProposalChangeV1, ProjectVideoClipProposalConflictV1,
    ProjectVideoClipProposalDecisionResultV1, ProjectVideoClipProposalDecisionV1,
    ProjectVideoClipProposalV1, VIDEO_CLIP_PROPOSAL_DECISION_SCHEMA_VERSION,
    VIDEO_CLIP_PROPOSAL_SCHEMA_VERSION,
};
use crate::video_clip_queue::ProjectVideoClipQueueItemV1;
use crate::video_clip_suggestion::{
    ProjectVideoClipSuggestionItemV1, VIDEO_CLIP_SUGGESTION_SCHEMA_VERSION,
};

impl ProjectPackage {
    pub fn generate_video_clip_proposal(&self) -> ProjectPackageResult<ProjectVideoClipProposalV1> {
        let project = self.project_manifest()?;
        let queue = self.video_clip_queue()?;
        let suggestion = self.suggest_video_clip_queue()?;
        if suggestion.schema_version != VIDEO_CLIP_SUGGESTION_SCHEMA_VERSION {
            return Err(ProjectPackageError::Invalid(format!(
                "unexpected video clip suggestion schema: {}",
                suggestion.schema_version
            )));
        }
        let now = now_ms();
        let revision = self.next_video_clip_proposal_revision();
        let after_queue = suggestion
            .items
            .iter()
            .map(|item| {
                suggestion_to_queue_item(item, &suggestion.suggestion_id, &queue.items, now)
            })
            .collect::<Vec<_>>();
        let base_queue_hash = queue_hash(&queue.items);
        let basis = serde_json::json!({
            "project_id": project.id,
            "suggestion_id": suggestion.suggestion_id,
            "revision": revision,
            "base_queue_hash": base_queue_hash,
            "before": queue.items.iter().map(queue_basis).collect::<Vec<_>>(),
            "after": after_queue.iter().map(queue_basis).collect::<Vec<_>>()
        });
        let proposal_id = format!(
            "proposal-fnv1a64-{:016x}",
            fnv1a64(serde_json::to_string(&basis).unwrap_or_default().as_bytes())
        );
        let changes = build_changes(&proposal_id, &queue.items, &after_queue, &suggestion.items);
        let proposal = ProjectVideoClipProposalV1 {
            schema_version: VIDEO_CLIP_PROPOSAL_SCHEMA_VERSION.to_string(),
            project_id: project.id,
            project_name: project.name,
            proposal_id,
            revision,
            source_suggestion_id: suggestion.suggestion_id,
            planner: "local-deterministic-video-clip-proposal-planner".to_string(),
            status: "proposed".to_string(),
            generated_at: now,
            decided_at: None,
            base_queue_hash: base_queue_hash.clone(),
            current_queue_hash: Some(base_queue_hash),
            rationale: proposal_rationale(&changes),
            safety_note: "生成 proposal diff 只写提案状态，不会修改 .capy/video-clip-queue.json；只有 PM 接受且 base_queue_hash 仍匹配当前 queue 后才更新 queue。".to_string(),
            before_queue: queue.items,
            after_queue,
            changes,
            decision: None,
            conflict: None,
        };
        self.write_json(&self.video_clip_proposal_path(), &proposal)?;
        self.upsert_video_clip_proposal_history(&proposal)?;
        self.touch_project_manifest()?;
        Ok(proposal)
    }

    pub fn video_clip_proposal(&self) -> ProjectPackageResult<ProjectVideoClipProposalV1> {
        let path = self.video_clip_proposal_path();
        if !path.exists() {
            return Err(ProjectPackageError::Invalid(
                "video clip proposal not found; run project clip-queue proposal first".to_string(),
            ));
        }
        read_json(&path, "read project video clip proposal")
    }

    pub fn decide_video_clip_proposal(
        &self,
        proposal_id: &str,
        decision: &str,
        reason: &str,
    ) -> ProjectPackageResult<ProjectVideoClipProposalDecisionResultV1> {
        self.decide_video_clip_proposal_for_revision(proposal_id, None, decision, reason)
    }

    pub fn decide_video_clip_proposal_for_revision(
        &self,
        proposal_id: &str,
        expected_revision: Option<u64>,
        decision: &str,
        reason: &str,
    ) -> ProjectPackageResult<ProjectVideoClipProposalDecisionResultV1> {
        let mut proposal = self.video_clip_proposal()?;
        if proposal.proposal_id != proposal_id {
            return Err(ProjectPackageError::Invalid(format!(
                "proposal id mismatch: expected {}, got {proposal_id}",
                proposal.proposal_id
            )));
        }
        if let Some(expected_revision) = expected_revision {
            if proposal.revision != expected_revision {
                return Err(ProjectPackageError::Invalid(format!(
                    "proposal revision mismatch: expected r{}, got r{expected_revision}",
                    proposal.revision
                )));
            }
        }
        let normalized = normalize_decision(decision)?;
        let now = now_ms();
        let current_queue = self.video_clip_queue()?;
        let base_queue_hash = proposal_base_queue_hash(&proposal);
        let current_queue_hash = queue_hash(&current_queue.items);
        proposal.base_queue_hash = base_queue_hash.clone();
        proposal.current_queue_hash = Some(current_queue_hash.clone());
        let mut queue_manifest = None;
        let mut queue_updated = false;
        if normalized == "accept" {
            if current_queue_hash == base_queue_hash {
                let manifest = self.write_video_clip_queue(proposal.after_queue.clone())?;
                proposal.after_queue = manifest.items.clone();
                queue_manifest = Some(manifest);
                queue_updated = true;
            } else {
                proposal.conflict = Some(ProjectVideoClipProposalConflictV1 {
                    conflict_type: "queue_changed_since_proposal".to_string(),
                    message_zh: "该 proposal 已过期：当前剪辑 queue 与生成提案时的 base_queue_hash 不一致，请重新生成 proposal。".to_string(),
                    base_queue_hash: base_queue_hash.clone(),
                    current_queue_hash: current_queue_hash.clone(),
                    detected_at: now,
                });
            }
        }
        proposal.status = if queue_updated {
            "accepted"
        } else if normalized == "accept" {
            "conflicted"
        } else {
            "rejected"
        }
        .to_string();
        proposal.decided_at = Some(now);
        proposal.decision = Some(ProjectVideoClipProposalDecisionV1 {
            decision: normalized.to_string(),
            reason: if reason.trim().is_empty() {
                default_decision_reason(normalized)
            } else {
                reason.trim().to_string()
            },
            decided_at: now,
            queue_updated,
        });
        proposal.changes = proposal
            .changes
            .into_iter()
            .map(|mut change| {
                change.apply_status = if queue_updated && change.applicable {
                    "applied".to_string()
                } else if queue_updated {
                    "not_applicable".to_string()
                } else if normalized == "accept" {
                    "conflicted".to_string()
                } else {
                    "rejected".to_string()
                };
                change
            })
            .collect();
        self.write_json(&self.video_clip_proposal_path(), &proposal)?;
        self.upsert_video_clip_proposal_history(&proposal)?;
        self.touch_project_manifest()?;
        Ok(ProjectVideoClipProposalDecisionResultV1 {
            schema_version: VIDEO_CLIP_PROPOSAL_DECISION_SCHEMA_VERSION.to_string(),
            proposal,
            queue_manifest,
        })
    }
}

fn suggestion_to_queue_item(
    item: &ProjectVideoClipSuggestionItemV1,
    suggestion_id: &str,
    before_queue: &[ProjectVideoClipQueueItemV1],
    now: u64,
) -> ProjectVideoClipQueueItemV1 {
    let key = suggestion_clip_key(item);
    let existing = before_queue
        .iter()
        .find(|before| queue_item_clip_key(before) == key);
    ProjectVideoClipQueueItemV1 {
        id: existing
            .map(|before| before.id.clone())
            .unwrap_or_else(|| item.id.clone()),
        sequence: item.sequence,
        composition_path: item.composition_path.clone(),
        render_source_path: item.render_source_path.clone(),
        clip_id: if item.clip_id.trim().is_empty() {
            "source".to_string()
        } else {
            item.clip_id.clone()
        },
        track_id: item.track_id.clone(),
        scene: item.scene.clone(),
        start_ms: item.start_ms,
        end_ms: item.end_ms,
        duration_ms: item
            .duration_ms
            .max(item.end_ms.saturating_sub(item.start_ms))
            .max(1),
        source_video: item.source_video.clone(),
        suggestion_id: Some(suggestion_id.to_string()),
        suggestion_reason: Some(item.reason.clone()).filter(|value| !value.trim().is_empty()),
        semantic_ref: item.semantic_ref.clone(),
        semantic_summary: item.semantic_summary.clone(),
        semantic_tags: item.semantic_tags.clone(),
        semantic_reason: item.semantic_reason.clone(),
        updated_at: now,
    }
}

fn build_changes(
    proposal_id: &str,
    before_queue: &[ProjectVideoClipQueueItemV1],
    after_queue: &[ProjectVideoClipQueueItemV1],
    suggestion_items: &[ProjectVideoClipSuggestionItemV1],
) -> Vec<ProjectVideoClipProposalChangeV1> {
    let before_by_key = before_queue
        .iter()
        .map(|item| (queue_item_clip_key(item), item))
        .collect::<BTreeMap<_, _>>();
    let before_by_sequence = before_queue
        .iter()
        .map(|item| (item.sequence, item))
        .collect::<BTreeMap<_, _>>();
    let suggestion_by_key = suggestion_items
        .iter()
        .map(|item| (suggestion_clip_key(item), item))
        .collect::<BTreeMap<_, _>>();
    let mut changes = after_queue
        .iter()
        .enumerate()
        .map(|(index, after)| {
            let key = queue_item_clip_key(after);
            let before = before_by_key.get(&key).copied();
            let suggestion = suggestion_by_key.get(&key).copied();
            let fallback_before =
                before.or_else(|| before_by_sequence.get(&after.sequence).copied());
            let action = change_action(before, after, suggestion);
            proposal_change(
                proposal_id,
                index + 1,
                action,
                before,
                fallback_before,
                Some(after),
                suggestion,
            )
        })
        .collect::<Vec<_>>();
    let after_keys = after_queue
        .iter()
        .map(queue_item_clip_key)
        .collect::<std::collections::BTreeSet<_>>();
    for before in before_queue {
        if after_keys.contains(&queue_item_clip_key(before)) {
            continue;
        }
        changes.push(proposal_change(
            proposal_id,
            changes.len() + 1,
            "replace",
            Some(before),
            Some(before),
            None,
            None,
        ));
    }
    changes
}

fn proposal_change(
    proposal_id: &str,
    index: usize,
    action: &str,
    exact_before: Option<&ProjectVideoClipQueueItemV1>,
    display_before: Option<&ProjectVideoClipQueueItemV1>,
    after: Option<&ProjectVideoClipQueueItemV1>,
    suggestion: Option<&ProjectVideoClipSuggestionItemV1>,
) -> ProjectVideoClipProposalChangeV1 {
    let reference = after.or(display_before).or(exact_before);
    let queue_item_id = reference.map(|item| item.id.clone()).unwrap_or_default();
    let clip_key = reference.map(queue_item_clip_key).unwrap_or_default();
    let scene = reference
        .map(|item| item.scene.clone())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| suggestion.map(|item| item.scene.clone()))
        .unwrap_or_else(|| "片段".to_string());
    ProjectVideoClipProposalChangeV1 {
        id: format!("{proposal_id}-change-{index:02}"),
        action: action.to_string(),
        action_label_zh: action_label(action).to_string(),
        before_sequence: exact_before.map(|item| item.sequence),
        after_sequence: after.map(|item| item.sequence),
        queue_item_id,
        clip_key,
        scene,
        reason_summary: reason_summary(action, suggestion),
        feedback_ref: suggestion.and_then(|item| item.feedback_ref.clone()),
        feedback_text: suggestion.and_then(|item| item.feedback_text.clone()),
        feedback_reason: suggestion.and_then(|item| item.feedback_reason.clone()),
        semantic_ref: suggestion.and_then(|item| item.semantic_ref.clone()),
        semantic_reason: suggestion.and_then(|item| item.semantic_reason.clone()),
        applicable: after.is_some(),
        apply_status: "pending".to_string(),
        before_item: display_before.cloned(),
        after_item: after.cloned(),
    }
}

fn change_action(
    before: Option<&ProjectVideoClipQueueItemV1>,
    after: &ProjectVideoClipQueueItemV1,
    suggestion: Option<&ProjectVideoClipSuggestionItemV1>,
) -> &'static str {
    let Some(before) = before else {
        return "replace";
    };
    if before.sequence == after.sequence {
        return "keep";
    }
    if after.sequence > before.sequence
        && suggestion
            .and_then(|item| item.feedback_effect.as_deref())
            .is_some_and(|effect| effect == "deprioritize_opening")
    {
        return "deprioritize";
    }
    "reorder"
}

fn action_label(action: &str) -> &'static str {
    match action {
        "keep" => "保留",
        "deprioritize" => "降权",
        "replace" => "替换",
        "reorder" => "重排",
        _ => "调整",
    }
}

fn reason_summary(action: &str, suggestion: Option<&ProjectVideoClipSuggestionItemV1>) -> String {
    if let Some(feedback) = suggestion.and_then(|item| item.feedback_reason.as_deref()) {
        return feedback.to_string();
    }
    if let Some(semantic) = suggestion.and_then(|item| item.semantic_reason.as_deref()) {
        return semantic.to_string();
    }
    if let Some(reason) = suggestion.map(|item| item.reason.as_str()) {
        if !reason.trim().is_empty() {
            return reason.to_string();
        }
    }
    match action {
        "keep" => "保留当前片段，队列位置不变。".to_string(),
        "deprioritize" => "根据反馈降低该片段在开场位置的优先级。".to_string(),
        "replace" => "用更匹配当前反馈和语义的片段替换该位置。".to_string(),
        "reorder" => "根据反馈和语义理由调整该片段的队列位置。".to_string(),
        _ => "根据本地 deterministic planner 调整。".to_string(),
    }
}

fn proposal_rationale(changes: &[ProjectVideoClipProposalChangeV1]) -> String {
    let mut counts = BTreeMap::<&str, usize>::new();
    for change in changes {
        *counts.entry(change.action.as_str()).or_default() += 1;
    }
    format!(
        "本地 proposal diff 基于当前 queue、片段语义和 PM 反馈生成：保留 {} 条、降权 {} 条、重排 {} 条、替换 {} 条。等待 PM 明确接受或拒绝。",
        counts.get("keep").copied().unwrap_or(0),
        counts.get("deprioritize").copied().unwrap_or(0),
        counts.get("reorder").copied().unwrap_or(0),
        counts.get("replace").copied().unwrap_or(0)
    )
}

fn normalize_decision(decision: &str) -> ProjectPackageResult<&'static str> {
    match decision.trim().to_lowercase().as_str() {
        "accept" | "accepted" => Ok("accept"),
        "reject" | "rejected" => Ok("reject"),
        other => Err(ProjectPackageError::Invalid(format!(
            "unsupported video clip proposal decision: {other}; expected accept or reject"
        ))),
    }
}

fn default_decision_reason(decision: &str) -> String {
    if decision == "accept" {
        "PM 接受 proposal diff，允许写回剪辑队列。".to_string()
    } else {
        "PM 拒绝 proposal diff，保持原剪辑队列。".to_string()
    }
}

fn suggestion_clip_key(item: &ProjectVideoClipSuggestionItemV1) -> String {
    format!(
        "{}|{}|{}|{}",
        item.composition_path.trim(),
        item.clip_id.trim(),
        item.start_ms,
        item.end_ms
    )
}
