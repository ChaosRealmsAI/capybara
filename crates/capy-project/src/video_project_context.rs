use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::{
    ArtifactKind, ArtifactRefV1, DesignLanguageAssetV1, DesignLanguageSummaryV1, ProjectManifestV1,
};
use crate::package::{ProjectPackage, ProjectPackageResult, now_ms};
use crate::video_clip_proposal_history::{fnv1a64, queue_hash};
use crate::video_clip_proposal_types::{
    ProjectVideoClipProposalConflictV1, ProjectVideoClipProposalHistoryEntryV1,
};
use crate::video_clip_queue::ProjectVideoClipQueueItemV1;

pub const VIDEO_PROJECT_CONTEXT_SCHEMA_VERSION: &str = "capy.video-project-context.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectContextPackageV1 {
    pub schema_version: String,
    pub package_id: String,
    pub project_summary: VideoProjectSummaryV1,
    pub anchor_artifact: VideoProjectAnchorArtifactV1,
    #[serde(default)]
    pub source_media: Vec<VideoProjectMediaSourceV1>,
    pub clip_queue: VideoProjectClipQueueContextV1,
    pub proposal_history: VideoProjectProposalHistoryContextV1,
    pub design_constraints: VideoProjectDesignConstraintsV1,
    pub safety: VideoProjectContextSafetyV1,
    pub generated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectSummaryV1 {
    pub project_id: String,
    pub project_name: String,
    pub source_media_count: usize,
    pub queue_item_count: usize,
    pub proposal_history_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectAnchorArtifactV1 {
    pub artifact_id: String,
    pub artifact_kind: ArtifactKind,
    pub title: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectMediaSourceV1 {
    pub artifact_id: String,
    pub title: String,
    pub source_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poster_frame_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectClipQueueContextV1 {
    pub schema_version: String,
    pub current_queue_hash: String,
    pub item_count: usize,
    #[serde(default)]
    pub items: Vec<ProjectVideoClipQueueItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectProposalHistoryContextV1 {
    pub schema_version: String,
    pub entry_count: usize,
    #[serde(default)]
    pub status_counts: BTreeMap<String, usize>,
    #[serde(default)]
    pub entries: Vec<ProjectVideoClipProposalHistoryEntryV1>,
    #[serde(default)]
    pub conflicts: Vec<VideoProjectProposalConflictSummaryV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectProposalConflictSummaryV1 {
    pub proposal_id: String,
    pub revision: u64,
    pub status: String,
    pub conflict: ProjectVideoClipProposalConflictV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectDesignConstraintsV1 {
    pub design_language_ref: String,
    pub summary: DesignLanguageSummaryV1,
    #[serde(default)]
    pub assets: Vec<DesignLanguageAssetV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProjectContextSafetyV1 {
    pub safe_for_next_ai_input: bool,
    pub no_queue_write: bool,
    pub proposal_history_read_only: bool,
    pub queue_write_policy_zh: String,
    pub safe_next_input_note_zh: String,
    #[serde(default)]
    pub red_lines_zh: Vec<String>,
}

impl ProjectPackage {
    pub(crate) fn build_video_project_context(
        &self,
        project: &ProjectManifestV1,
        anchor: &ArtifactRefV1,
        artifacts: &[ArtifactRefV1],
        design_summary: &DesignLanguageSummaryV1,
        design_assets: &[DesignLanguageAssetV1],
    ) -> ProjectPackageResult<Option<VideoProjectContextPackageV1>> {
        let source_media = artifacts
            .iter()
            .filter(|artifact| artifact.kind == ArtifactKind::Video)
            .map(video_media_source)
            .collect::<Vec<_>>();
        let queue = self.video_clip_queue()?;
        let history = self.video_clip_proposal_history()?;
        if source_media.is_empty() && queue.items.is_empty() && history.entries.is_empty() {
            return Ok(None);
        }
        let current_queue_hash = queue_hash(&queue.items);
        let status_counts = status_counts(&history.entries);
        let conflicts = history
            .entries
            .iter()
            .filter_map(|entry| {
                entry
                    .conflict
                    .clone()
                    .map(|conflict| VideoProjectProposalConflictSummaryV1 {
                        proposal_id: entry.proposal_id.clone(),
                        revision: entry.revision,
                        status: entry.status.clone(),
                        conflict,
                    })
            })
            .collect::<Vec<_>>();
        let package_id = video_project_context_id(
            project,
            anchor,
            &source_media,
            &current_queue_hash,
            &history.entries,
            design_summary,
        );
        Ok(Some(VideoProjectContextPackageV1 {
            schema_version: VIDEO_PROJECT_CONTEXT_SCHEMA_VERSION.to_string(),
            package_id,
            project_summary: VideoProjectSummaryV1 {
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                source_media_count: source_media.len(),
                queue_item_count: queue.items.len(),
                proposal_history_count: history.entries.len(),
            },
            anchor_artifact: VideoProjectAnchorArtifactV1 {
                artifact_id: anchor.id.clone(),
                artifact_kind: anchor.kind.clone(),
                title: anchor.title.clone(),
                source_path: anchor.source_path.clone(),
            },
            source_media,
            clip_queue: VideoProjectClipQueueContextV1 {
                schema_version: queue.schema_version,
                current_queue_hash,
                item_count: queue.items.len(),
                items: queue.items,
            },
            proposal_history: VideoProjectProposalHistoryContextV1 {
                schema_version: history.schema_version,
                entry_count: history.entries.len(),
                status_counts,
                entries: history.entries,
                conflicts,
            },
            design_constraints: VideoProjectDesignConstraintsV1 {
                design_language_ref: design_summary.design_language_ref.clone(),
                summary: design_summary.clone(),
                assets: design_assets.to_vec(),
            },
            safety: VideoProjectContextSafetyV1 {
                safe_for_next_ai_input: true,
                no_queue_write: true,
                proposal_history_read_only: true,
                queue_write_policy_zh: "context build 只读取素材、queue、history 和设计约束；不会接受、重写或清空 queue。只有当前 proposal 的显式 accept 且 base_queue_hash 匹配时才允许写 queue。".to_string(),
                safe_next_input_note_zh: "该包可直接作为下一轮 AI 输入：AI 应基于 current_queue_hash 和 proposal history 判断上下文，遇到 conflicted 历史必须重新生成当前 proposal，而不是套用旧 proposal。".to_string(),
                red_lines_zh: vec![
                    "不自动接受、重写或清空 clip queue 与 proposal history".to_string(),
                    "不通过历史 proposal 绕过 base/current queue hash 冲突检测".to_string(),
                    "不扩展为多轨 NLE、字幕、转场、音频混合或导出工作流".to_string(),
                ],
            },
            generated_at: now_ms(),
        }))
    }
}

fn video_media_source(artifact: &ArtifactRefV1) -> VideoProjectMediaSourceV1 {
    let video = artifact
        .provenance
        .as_ref()
        .and_then(|value| value.get("video_import"))
        .unwrap_or(&Value::Null);
    VideoProjectMediaSourceV1 {
        artifact_id: artifact.id.clone(),
        title: artifact.title.clone(),
        source_path: artifact.source_path.clone(),
        filename: string_field(video, "filename"),
        duration_ms: u64_field(video, "duration_ms"),
        width: u64_field(video, "width"),
        height: u64_field(video, "height"),
        fps: video.get("fps").and_then(Value::as_f64),
        byte_size: u64_field(video, "byte_size"),
        poster_frame_path: string_field(video, "poster_frame_path"),
        composition_path: string_field(video, "composition_path"),
    }
}

fn status_counts(entries: &[ProjectVideoClipProposalHistoryEntryV1]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts.entry(entry.status.clone()).or_insert(0) += 1;
    }
    counts
}

fn video_project_context_id(
    project: &ProjectManifestV1,
    anchor: &ArtifactRefV1,
    source_media: &[VideoProjectMediaSourceV1],
    current_queue_hash: &str,
    history: &[ProjectVideoClipProposalHistoryEntryV1],
    design_summary: &DesignLanguageSummaryV1,
) -> String {
    let basis = json!({
        "project_id": project.id,
        "anchor_artifact_id": anchor.id,
        "source_media": source_media.iter().map(|item| (&item.artifact_id, &item.source_path)).collect::<Vec<_>>(),
        "current_queue_hash": current_queue_hash,
        "history": history.iter().map(|entry| json!({
            "proposal_id": entry.proposal_id,
            "revision": entry.revision,
            "status": entry.status,
            "base_queue_hash": entry.base_queue_hash,
            "current_queue_hash": entry.current_queue_hash,
            "decision": entry.decision.as_ref().map(|decision| decision.decision.as_str()),
            "conflict": entry.conflict.as_ref().map(|conflict| (&conflict.conflict_type, &conflict.base_queue_hash, &conflict.current_queue_hash))
        })).collect::<Vec<_>>(),
        "design_language_ref": design_summary.design_language_ref
    });
    format!(
        "vpctx-fnv1a64-{:016x}",
        fnv1a64(serde_json::to_string(&basis).unwrap_or_default().as_bytes())
    )
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}
