use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::ArtifactKind;
use crate::package::{ProjectPackage, ProjectPackageResult, now_ms};
use crate::video_clip_feedback::{
    ProjectVideoClipFeedbackItemV1, ProjectVideoClipFeedbackManifestV1, queue_item_clip_key,
};
use crate::video_clip_queue::{ProjectVideoClipQueueItemV1, VIDEO_CLIP_QUEUE_SCHEMA_VERSION};
use crate::video_clip_semantics::{
    ProjectVideoClipSemanticItemV1, ProjectVideoClipSemanticsManifestV1, clip_semantic_key,
};

pub const VIDEO_CLIP_SUGGESTION_SCHEMA_VERSION: &str = "capy.project-video-clip-suggestion.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipSuggestionV1 {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub suggestion_id: String,
    pub planner: String,
    pub generated_at: u64,
    pub source_video_count: usize,
    pub existing_queue_count: usize,
    pub rationale: String,
    #[serde(default)]
    pub items: Vec<ProjectVideoClipSuggestionItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipSuggestionItemV1 {
    pub id: String,
    pub sequence: u64,
    pub composition_path: String,
    #[serde(default)]
    pub render_source_path: String,
    pub clip_id: String,
    #[serde(default)]
    pub track_id: String,
    pub scene: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_video: Option<Value>,
    pub reason: String,
    pub suggestion_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct VideoCandidate {
    artifact_id: String,
    title: String,
    source_path: String,
    filename: String,
    duration_ms: u64,
    width: u64,
    height: u64,
    composition_path: String,
}

impl ProjectPackage {
    pub fn suggest_video_clip_queue(&self) -> ProjectPackageResult<ProjectVideoClipSuggestionV1> {
        let project = self.project_manifest()?;
        let queue = self.video_clip_queue()?;
        let videos = self.video_candidates()?;
        let semantics_manifest = self.video_clip_semantics().ok();
        let semantics = semantic_map(semantics_manifest.as_ref());
        let feedback_manifest = self.video_clip_feedback().ok();
        let feedback = feedback_map(feedback_manifest.as_ref());
        let basis = json!({
            "schema_version": VIDEO_CLIP_QUEUE_SCHEMA_VERSION,
            "project_id": project.id,
            "videos": videos.iter().map(|item| json!({
                "artifact_id": item.artifact_id,
                "source_path": item.source_path,
                "composition_path": item.composition_path,
                "duration_ms": item.duration_ms
            })).collect::<Vec<_>>(),
            "queue": queue.items.iter().map(|item| json!({
                "id": item.id,
                "sequence": item.sequence,
                "composition_path": item.composition_path,
                "start_ms": item.start_ms,
                "end_ms": item.end_ms,
                "scene": item.scene
            })).collect::<Vec<_>>()
            ,
            "feedback": feedback_manifest.as_ref().map(|manifest| manifest.items.iter().map(|item| json!({
                "clip_key": item.clip_key,
                "queue_item_id": item.queue_item_id,
                "feedback": item.feedback,
                "effect": item.recommendation_effect
            })).collect::<Vec<_>>()).unwrap_or_default()
        });
        let suggestion_id = format!(
            "sug-fnv1a64-{:016x}",
            fnv1a64(serde_json::to_string(&basis).unwrap_or_default().as_bytes())
        );
        let mut items = Vec::new();
        let mut queued_items = queue.items.iter().take(4).collect::<Vec<_>>();
        queued_items.sort_by_key(|item| feedback_sort_key(item, &feedback));
        for item in queued_items {
            let key = queue_item_clip_key(item);
            let semantic = semantics.get(&key);
            let feedback_item = feedback.get(&key);
            items.push(suggestion_from_queue_item(
                item,
                &suggestion_id,
                items.len() + 1,
                semantic.copied(),
                feedback_item.copied(),
            ));
        }
        for video in &videos {
            if items.len() >= 4 {
                break;
            }
            if items
                .iter()
                .any(|item| item.composition_path == video.composition_path)
            {
                continue;
            }
            items.push(suggestion_from_video(
                video,
                &suggestion_id,
                items.len() + 1,
                &semantics,
            ));
        }
        if items.is_empty() {
            for video in videos.iter().take(4) {
                items.push(suggestion_from_video(
                    video,
                    &suggestion_id,
                    items.len() + 1,
                    &semantics,
                ));
            }
        }
        Ok(ProjectVideoClipSuggestionV1 {
            schema_version: VIDEO_CLIP_SUGGESTION_SCHEMA_VERSION.to_string(),
            project_id: project.id,
            project_name: project.name,
            suggestion_id,
            planner: "local-deterministic-video-clip-planner".to_string(),
            generated_at: now_ms(),
            source_video_count: videos.len(),
            existing_queue_count: queue.items.len(),
            rationale: if queue.items.is_empty() {
                "基于项目视频素材自动选择短片段，先形成可采用的线性剪辑队列。".to_string()
            } else if feedback
                .values()
                .any(|item| !item.feedback.trim().is_empty())
            {
                "保留原始 queue 作为 PM 已认可上下文，同时引用片段反馈调整本次只读建议排序；不会自动改写 queue。"
                    .to_string()
            } else {
                "保留已持久化队列的顺序作为 PM 已认可上下文，并补充项目中尚未覆盖的视频素材。"
                    .to_string()
            },
            items,
        })
    }

    fn video_candidates(&self) -> ProjectPackageResult<Vec<VideoCandidate>> {
        let mut videos = self
            .artifacts()?
            .artifacts
            .into_iter()
            .filter(|artifact| artifact.kind == ArtifactKind::Video)
            .filter_map(|artifact| {
                let import = artifact
                    .provenance
                    .as_ref()?
                    .get("video_import")?
                    .as_object()?;
                let composition_path = import
                    .get("composition_path")
                    .and_then(Value::as_str)
                    .or_else(|| artifact.output_refs.first().map(String::as_str))?;
                Some(VideoCandidate {
                    artifact_id: artifact.id,
                    title: artifact.title,
                    source_path: artifact.source_path,
                    filename: import
                        .get("filename")
                        .and_then(Value::as_str)
                        .unwrap_or("video")
                        .to_string(),
                    duration_ms: import
                        .get("duration_ms")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    width: import.get("width").and_then(Value::as_u64).unwrap_or(0),
                    height: import.get("height").and_then(Value::as_u64).unwrap_or(0),
                    composition_path: composition_path.to_string(),
                })
            })
            .collect::<Vec<_>>();
        videos.sort_by(|a, b| {
            a.filename
                .cmp(&b.filename)
                .then_with(|| a.artifact_id.cmp(&b.artifact_id))
        });
        Ok(videos)
    }
}

fn suggestion_from_queue_item(
    item: &ProjectVideoClipQueueItemV1,
    suggestion_id: &str,
    index: usize,
    semantic: Option<&ProjectVideoClipSemanticItemV1>,
    feedback: Option<&ProjectVideoClipFeedbackItemV1>,
) -> ProjectVideoClipSuggestionItemV1 {
    let duration_ms = item
        .duration_ms
        .max(item.end_ms.saturating_sub(item.start_ms))
        .max(1);
    let mut suggestion = ProjectVideoClipSuggestionItemV1 {
        id: format!("{suggestion_id}-{:02}", index),
        sequence: index as u64,
        composition_path: item.composition_path.clone(),
        render_source_path: item.render_source_path.clone(),
        clip_id: item.clip_id.clone(),
        track_id: item.track_id.clone(),
        scene: if item.scene.trim().is_empty() {
            format!("建议片段 {index}")
        } else {
            item.scene.clone()
        },
        start_ms: item.start_ms,
        end_ms: item.start_ms.saturating_add(duration_ms),
        duration_ms,
        source_video: item.source_video.clone(),
        reason: format!("保留持久化队列第 {index} 段，延续 PM 已经整理过的项目节奏。"),
        suggestion_id: suggestion_id.to_string(),
        semantic_ref: None,
        semantic_summary: None,
        semantic_tags: Vec::new(),
        semantic_reason: None,
        feedback_ref: None,
        feedback_text: None,
        feedback_kind: None,
        feedback_effect: None,
        feedback_reason: None,
    };
    apply_semantic_reason(&mut suggestion, semantic);
    apply_feedback_reason(&mut suggestion, feedback);
    suggestion
}

fn suggestion_from_video(
    video: &VideoCandidate,
    suggestion_id: &str,
    index: usize,
    semantics: &BTreeMap<String, &ProjectVideoClipSemanticItemV1>,
) -> ProjectVideoClipSuggestionItemV1 {
    let source_duration = video.duration_ms.max(1);
    let target_duration = source_duration.min(1_800).max(source_duration.min(1_000));
    let start_ms = if index == 1 {
        0
    } else if source_duration > target_duration.saturating_mul(2) {
        source_duration / 3
    } else {
        source_duration.saturating_sub(target_duration)
    };
    let end_ms = start_ms
        .saturating_add(target_duration)
        .min(source_duration);
    let duration_ms = end_ms.saturating_sub(start_ms).max(1);
    let semantic = semantics.get(&clip_semantic_key(
        &video.composition_path,
        "source",
        start_ms,
        end_ms,
    ));
    let mut suggestion = ProjectVideoClipSuggestionItemV1 {
        id: format!("{suggestion_id}-{:02}", index),
        sequence: index as u64,
        composition_path: video.composition_path.clone(),
        render_source_path: String::new(),
        clip_id: "source".to_string(),
        track_id: "video".to_string(),
        scene: format!("{} · AI 建议片段", video.title),
        start_ms,
        end_ms,
        duration_ms,
        source_video: Some(json!({
            "artifact_id": video.artifact_id,
            "src": video.source_path,
            "filename": video.filename,
            "duration_ms": video.duration_ms,
            "width": video.width,
            "height": video.height
        })),
        reason: "补充项目中尚未覆盖的视频素材，让方案能体现多来源素材。".to_string(),
        suggestion_id: suggestion_id.to_string(),
        semantic_ref: None,
        semantic_summary: None,
        semantic_tags: Vec::new(),
        semantic_reason: None,
        feedback_ref: None,
        feedback_text: None,
        feedback_kind: None,
        feedback_effect: None,
        feedback_reason: None,
    };
    apply_semantic_reason(&mut suggestion, semantic.copied());
    suggestion
}

fn apply_semantic_reason(
    suggestion: &mut ProjectVideoClipSuggestionItemV1,
    semantic: Option<&ProjectVideoClipSemanticItemV1>,
) {
    let Some(semantic) = semantic else {
        return;
    };
    suggestion.reason = semantic.recommendation.clone();
    suggestion.semantic_ref = Some(semantic.id.clone());
    suggestion.semantic_summary = Some(semantic.summary_zh.clone());
    suggestion.semantic_tags = semantic.tags.clone();
    suggestion.semantic_reason = Some(format!(
        "{} 摘要：{}",
        semantic.recommendation, semantic.summary_zh
    ));
}

fn apply_feedback_reason(
    suggestion: &mut ProjectVideoClipSuggestionItemV1,
    feedback: Option<&ProjectVideoClipFeedbackItemV1>,
) {
    let Some(feedback) = feedback else {
        return;
    };
    let feedback_text = feedback.feedback.trim();
    if feedback_text.is_empty() {
        return;
    }
    let reason = if feedback.recommendation_effect == "deprioritize_opening" {
        format!("用户反馈：{feedback_text}。建议不把该片段作为开场首位，仍等待 PM 手动采用。")
    } else if feedback.recommendation_effect == "prefer_opening" {
        format!("用户反馈：{feedback_text}。建议优先保留该片段作为开场候选，仍等待 PM 手动采用。")
    } else {
        format!("用户反馈：{feedback_text}。本地建议已把该反馈作为片段取舍上下文。")
    };
    suggestion.reason = if suggestion.reason.trim().is_empty() {
        reason.clone()
    } else {
        format!("{} {}", suggestion.reason, reason)
    };
    suggestion.feedback_ref = Some(feedback.id.clone());
    suggestion.feedback_text = Some(feedback_text.to_string());
    suggestion.feedback_kind = Some(feedback.feedback_kind.clone());
    suggestion.feedback_effect = Some(feedback.recommendation_effect.clone());
    suggestion.feedback_reason = Some(reason);
}

fn semantic_map(
    manifest: Option<&ProjectVideoClipSemanticsManifestV1>,
) -> BTreeMap<String, &ProjectVideoClipSemanticItemV1> {
    let mut map = BTreeMap::new();
    if let Some(manifest) = manifest {
        for item in &manifest.items {
            map.insert(item.clip_key.clone(), item);
        }
    }
    map
}

fn feedback_map(
    manifest: Option<&ProjectVideoClipFeedbackManifestV1>,
) -> BTreeMap<String, &ProjectVideoClipFeedbackItemV1> {
    let mut map = BTreeMap::new();
    if let Some(manifest) = manifest {
        for item in &manifest.items {
            map.insert(item.clip_key.clone(), item);
        }
    }
    map
}

fn feedback_sort_key(
    item: &ProjectVideoClipQueueItemV1,
    feedback: &BTreeMap<String, &ProjectVideoClipFeedbackItemV1>,
) -> (u8, u64) {
    let penalty = feedback
        .get(&queue_item_clip_key(item))
        .map(|item| {
            if item.recommendation_effect == "deprioritize_opening" {
                1
            } else {
                0
            }
        })
        .unwrap_or(0);
    (penalty, item.sequence)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
