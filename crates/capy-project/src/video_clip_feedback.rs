use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::package::{
    CAPY_DIR, ProjectPackage, ProjectPackageError, ProjectPackageResult, now_ms, read_json,
};
use crate::video_clip_queue::ProjectVideoClipQueueItemV1;
use crate::video_clip_semantics::clip_semantic_key;

pub const VIDEO_CLIP_FEEDBACK_SCHEMA_VERSION: &str = "capy.project-video-clip-feedback.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipFeedbackManifestV1 {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub updated_at: u64,
    pub source_queue_count: usize,
    #[serde(default)]
    pub items: Vec<ProjectVideoClipFeedbackItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipFeedbackItemV1 {
    pub id: String,
    pub clip_key: String,
    pub queue_item_id: String,
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
    pub feedback: String,
    pub feedback_kind: String,
    pub recommendation_effect: String,
    pub created_at: u64,
    pub updated_at: u64,
}

impl ProjectPackage {
    pub fn video_clip_feedback(&self) -> ProjectPackageResult<ProjectVideoClipFeedbackManifestV1> {
        let project = self.project_manifest()?;
        let path = self.video_clip_feedback_path();
        if !path.exists() {
            return Ok(ProjectVideoClipFeedbackManifestV1 {
                schema_version: VIDEO_CLIP_FEEDBACK_SCHEMA_VERSION.to_string(),
                project_id: project.id,
                project_name: project.name,
                updated_at: project.updated_at,
                source_queue_count: 0,
                items: Vec::new(),
            });
        }
        let mut manifest: ProjectVideoClipFeedbackManifestV1 =
            read_json(&path, "read project video clip feedback")?;
        if manifest.schema_version.trim().is_empty() {
            manifest.schema_version = VIDEO_CLIP_FEEDBACK_SCHEMA_VERSION.to_string();
        }
        if manifest.project_id.trim().is_empty() {
            manifest.project_id = project.id;
        }
        if manifest.project_name.trim().is_empty() {
            manifest.project_name = project.name;
        }
        manifest.items = normalize_feedback_items(manifest.items);
        Ok(manifest)
    }

    pub fn record_video_clip_feedback(
        &self,
        queue_item_id: &str,
        feedback: &str,
    ) -> ProjectPackageResult<ProjectVideoClipFeedbackManifestV1> {
        let queue = self.video_clip_queue()?;
        let queue_item = queue
            .items
            .iter()
            .find(|item| item.id == queue_item_id)
            .ok_or_else(|| {
                ProjectPackageError::Invalid(format!(
                    "video clip feedback queue_item_id not found: {queue_item_id}"
                ))
            })?;
        let key = queue_item_clip_key(queue_item);
        let existing = self.video_clip_feedback()?;
        let mut items = existing
            .items
            .into_iter()
            .filter(|item| item.clip_key != key && item.queue_item_id != queue_item_id)
            .collect::<Vec<_>>();
        let trimmed = feedback.trim();
        if !trimmed.is_empty() {
            let now = now_ms();
            let created_at = existing_created_at(self, &key, queue_item_id).unwrap_or(now);
            items.push(feedback_from_queue_item(
                queue_item, trimmed, created_at, now,
            ));
        }
        self.write_video_clip_feedback_manifest(items, queue.items.len())
    }

    fn write_video_clip_feedback_manifest(
        &self,
        items: Vec<ProjectVideoClipFeedbackItemV1>,
        source_queue_count: usize,
    ) -> ProjectPackageResult<ProjectVideoClipFeedbackManifestV1> {
        let project = self.project_manifest()?;
        let now = now_ms();
        let manifest = ProjectVideoClipFeedbackManifestV1 {
            schema_version: VIDEO_CLIP_FEEDBACK_SCHEMA_VERSION.to_string(),
            project_id: project.id,
            project_name: project.name,
            updated_at: now,
            source_queue_count,
            items: normalize_feedback_items(items),
        };
        self.write_json(&self.video_clip_feedback_path(), &manifest)?;
        self.touch_project_manifest()?;
        Ok(manifest)
    }

    fn video_clip_feedback_path(&self) -> std::path::PathBuf {
        self.root().join(CAPY_DIR).join("video-clip-feedback.json")
    }
}

pub(crate) fn queue_item_clip_key(item: &ProjectVideoClipQueueItemV1) -> String {
    clip_semantic_key(
        &item.composition_path,
        &item.clip_id,
        item.start_ms,
        item.end_ms,
    )
}

fn feedback_from_queue_item(
    item: &ProjectVideoClipQueueItemV1,
    feedback: &str,
    created_at: u64,
    updated_at: u64,
) -> ProjectVideoClipFeedbackItemV1 {
    let clip_key = queue_item_clip_key(item);
    let (feedback_kind, recommendation_effect) = classify_feedback(feedback);
    ProjectVideoClipFeedbackItemV1 {
        id: feedback_id(&clip_key),
        clip_key,
        queue_item_id: item.id.clone(),
        composition_path: item.composition_path.clone(),
        render_source_path: item.render_source_path.clone(),
        clip_id: item.clip_id.clone(),
        track_id: item.track_id.clone(),
        scene: item.scene.clone(),
        start_ms: item.start_ms,
        end_ms: item.end_ms,
        duration_ms: item
            .duration_ms
            .max(item.end_ms.saturating_sub(item.start_ms))
            .max(1),
        source_video: item.source_video.clone(),
        feedback: feedback.to_string(),
        feedback_kind,
        recommendation_effect,
        created_at,
        updated_at,
    }
}

fn existing_created_at(package: &ProjectPackage, key: &str, queue_item_id: &str) -> Option<u64> {
    package
        .video_clip_feedback()
        .ok()?
        .items
        .into_iter()
        .find(|item| item.clip_key == key || item.queue_item_id == queue_item_id)
        .map(|item| item.created_at)
}

fn normalize_feedback_items(
    items: Vec<ProjectVideoClipFeedbackItemV1>,
) -> Vec<ProjectVideoClipFeedbackItemV1> {
    let mut items = items
        .into_iter()
        .filter(|item| !item.feedback.trim().is_empty())
        .map(|item| {
            let (feedback_kind, recommendation_effect) = classify_feedback(&item.feedback);
            ProjectVideoClipFeedbackItemV1 {
                id: if item.id.trim().is_empty() {
                    feedback_id(&item.clip_key)
                } else {
                    item.id
                },
                duration_ms: item
                    .duration_ms
                    .max(item.end_ms.saturating_sub(item.start_ms))
                    .max(1),
                feedback_kind,
                recommendation_effect,
                ..item
            }
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| {
        a.queue_item_id
            .cmp(&b.queue_item_id)
            .then_with(|| a.clip_key.cmp(&b.clip_key))
    });
    items
}

fn classify_feedback(feedback: &str) -> (String, String) {
    let lower = feedback.to_lowercase();
    let compact = lower.replace(char::is_whitespace, "");
    if compact.contains("不适合开场")
        || compact.contains("不要开场")
        || compact.contains("别放开场")
        || compact.contains("不放开场")
        || lower.contains("not opening")
        || lower.contains("not opener")
        || lower.contains("avoid opening")
    {
        return (
            "opening_reject".to_string(),
            "deprioritize_opening".to_string(),
        );
    }
    if compact.contains("适合开场")
        || compact.contains("开场好")
        || lower.contains("good opener")
        || lower.contains("use as opening")
    {
        return ("opening_prefer".to_string(), "prefer_opening".to_string());
    }
    ("general".to_string(), "cite_context".to_string())
}

fn feedback_id(clip_key: &str) -> String {
    format!("feedback-fnv1a64-{:016x}", fnv1a64(clip_key.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectPackage;
    use serde_json::json;
    use std::error::Error;
    use std::fs;

    #[test]
    fn video_clip_feedback_records_and_clears_segment_feedback() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let project = dir.path().join("demo");
        let composition = project.join(".capy/video-compositions/art_a/compositions/main.json");
        fs::create_dir_all(composition.parent().ok_or("composition parent")?)?;
        fs::write(&composition, "{}\n")?;
        let package = ProjectPackage::init(&project, Some("Feedback Project".to_string()))?;
        package.write_video_clip_queue(vec![ProjectVideoClipQueueItemV1 {
            id: "queue-a".to_string(),
            sequence: 1,
            composition_path: composition.display().to_string(),
            render_source_path: String::new(),
            clip_id: "source".to_string(),
            track_id: "video".to_string(),
            scene: "Camera A opening detail".to_string(),
            start_ms: 500,
            end_ms: 1_700,
            duration_ms: 1_200,
            source_video: Some(json!({ "filename": "camera-a-wide.webm" })),
            suggestion_id: None,
            suggestion_reason: None,
            semantic_ref: None,
            semantic_summary: None,
            semantic_tags: Vec::new(),
            semantic_reason: None,
            updated_at: 0,
        }])?;

        let manifest = package.record_video_clip_feedback("queue-a", "这段不适合开场")?;
        assert_eq!(manifest.schema_version, VIDEO_CLIP_FEEDBACK_SCHEMA_VERSION);
        assert_eq!(manifest.items.len(), 1);
        assert_eq!(manifest.items[0].queue_item_id, "queue-a");
        assert_eq!(manifest.items[0].feedback_kind, "opening_reject");
        assert_eq!(
            manifest.items[0].recommendation_effect,
            "deprioritize_opening"
        );

        let reopened = ProjectPackage::open(&project)?.video_clip_feedback()?;
        assert_eq!(reopened.items[0].feedback, "这段不适合开场");
        let cleared = package.record_video_clip_feedback("queue-a", " ")?;
        assert!(cleared.items.is_empty());
        Ok(())
    }
}
