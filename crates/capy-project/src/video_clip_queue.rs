use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::package::{
    CAPY_DIR, ProjectPackage, ProjectPackageError, ProjectPackageResult, now_ms, read_json,
};

pub const VIDEO_CLIP_QUEUE_SCHEMA_VERSION: &str = "capy.project-video-clip-queue.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipQueueManifestV1 {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub updated_at: u64,
    #[serde(default)]
    pub items: Vec<ProjectVideoClipQueueItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipQueueItemV1 {
    pub id: String,
    pub sequence: u64,
    pub composition_path: String,
    #[serde(default)]
    pub render_source_path: String,
    pub clip_id: String,
    #[serde(default)]
    pub track_id: String,
    #[serde(default)]
    pub scene: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_video: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub semantic_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_reason: Option<String>,
    pub updated_at: u64,
}

impl ProjectPackage {
    pub fn video_clip_queue(&self) -> ProjectPackageResult<ProjectVideoClipQueueManifestV1> {
        let manifest = self.project_manifest()?;
        let path = self.video_clip_queue_path();
        if !path.exists() {
            return Ok(ProjectVideoClipQueueManifestV1 {
                schema_version: VIDEO_CLIP_QUEUE_SCHEMA_VERSION.to_string(),
                project_id: manifest.id,
                project_name: manifest.name,
                updated_at: manifest.updated_at,
                items: Vec::new(),
            });
        }
        let mut queue: ProjectVideoClipQueueManifestV1 =
            read_json(&path, "read project video clip queue")?;
        if queue.schema_version.trim().is_empty() {
            queue.schema_version = VIDEO_CLIP_QUEUE_SCHEMA_VERSION.to_string();
        }
        if queue.project_id.trim().is_empty() {
            queue.project_id = manifest.id;
        }
        if queue.project_name.trim().is_empty() {
            queue.project_name = manifest.name;
        }
        queue.items = normalize_items(queue.items);
        Ok(queue)
    }

    pub fn write_video_clip_queue(
        &self,
        items: Vec<ProjectVideoClipQueueItemV1>,
    ) -> ProjectPackageResult<ProjectVideoClipQueueManifestV1> {
        let project = self.project_manifest()?;
        let now = now_ms();
        let items = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| self.normalize_manifest_item(item, index, now))
            .collect::<ProjectPackageResult<Vec<_>>>()?;
        let manifest = ProjectVideoClipQueueManifestV1 {
            schema_version: VIDEO_CLIP_QUEUE_SCHEMA_VERSION.to_string(),
            project_id: project.id,
            project_name: project.name,
            updated_at: now,
            items,
        };
        self.write_json(&self.video_clip_queue_path(), &manifest)?;
        self.touch_project_manifest()?;
        Ok(manifest)
    }

    fn normalize_manifest_item(
        &self,
        item: ProjectVideoClipQueueItemV1,
        index: usize,
        now: u64,
    ) -> ProjectPackageResult<ProjectVideoClipQueueItemV1> {
        let start_ms = item.start_ms;
        let end_ms = item.end_ms.max(start_ms.saturating_add(1));
        let duration_ms = item.duration_ms.max(end_ms.saturating_sub(start_ms)).max(1);
        Ok(ProjectVideoClipQueueItemV1 {
            id: if item.id.trim().is_empty() {
                format!("queue-{}", index + 1)
            } else {
                item.id
            },
            sequence: (index + 1) as u64,
            composition_path: self
                .normalize_project_path(&item.composition_path, "composition_path")?,
            render_source_path: if item.render_source_path.trim().is_empty() {
                String::new()
            } else {
                self.normalize_project_path(&item.render_source_path, "render_source_path")?
            },
            clip_id: if item.clip_id.trim().is_empty() {
                "source".to_string()
            } else {
                item.clip_id
            },
            track_id: item.track_id,
            scene: item.scene,
            start_ms,
            end_ms,
            duration_ms,
            source_video: item.source_video,
            suggestion_id: item.suggestion_id.filter(|value| !value.trim().is_empty()),
            suggestion_reason: item
                .suggestion_reason
                .filter(|value| !value.trim().is_empty()),
            semantic_ref: item.semantic_ref.filter(|value| !value.trim().is_empty()),
            semantic_summary: item
                .semantic_summary
                .filter(|value| !value.trim().is_empty()),
            semantic_tags: item
                .semantic_tags
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect(),
            semantic_reason: item
                .semantic_reason
                .filter(|value| !value.trim().is_empty()),
            updated_at: now,
        })
    }

    fn normalize_project_path(&self, raw: &str, field: &str) -> ProjectPackageResult<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ProjectPackageError::Invalid(format!(
                "video clip queue item {field} is required"
            )));
        }
        let candidate = if Path::new(trimmed).is_absolute() {
            PathBuf::from(trimmed)
        } else {
            self.root().join(trimmed)
        };
        let resolved = fs::canonicalize(&candidate).map_err(|source| ProjectPackageError::Io {
            context: format!("resolve video clip queue {field} {}", candidate.display()),
            source,
        })?;
        if !resolved.starts_with(self.root()) {
            return Err(ProjectPackageError::Invalid(format!(
                "video clip queue {field} must live inside project root: {}",
                resolved.display()
            )));
        }
        resolved
            .strip_prefix(self.root())
            .map_err(|err| {
                ProjectPackageError::Invalid(format!(
                    "video clip queue {field} {} is not project-relative: {err}",
                    resolved.display()
                ))
            })
            .map(|path| path.display().to_string())
    }

    fn video_clip_queue_path(&self) -> PathBuf {
        self.root().join(CAPY_DIR).join("video-clip-queue.json")
    }
}

fn normalize_items(items: Vec<ProjectVideoClipQueueItemV1>) -> Vec<ProjectVideoClipQueueItemV1> {
    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| ProjectVideoClipQueueItemV1 {
            sequence: (index + 1) as u64,
            duration_ms: item
                .duration_ms
                .max(item.end_ms.saturating_sub(item.start_ms))
                .max(1),
            ..item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectPackage;
    use std::error::Error;

    #[test]
    fn video_clip_queue_manifest_round_trips() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let project = dir.path().join("demo");
        let composition = project.join(".capy/video-compositions/art_a/compositions/main.json");
        let render_source =
            project.join(".capy/video-compositions/art_a/compositions/render_source.json");
        let composition_dir = composition
            .parent()
            .ok_or("composition path should have a parent")?;
        fs::create_dir_all(composition_dir)?;
        fs::write(&composition, "{}\n")?;
        fs::write(&render_source, "{}\n")?;

        let package = ProjectPackage::init(&project, Some("Queue Project".to_string()))?;
        let written = package.write_video_clip_queue(vec![ProjectVideoClipQueueItemV1 {
            id: "queue-a".to_string(),
            sequence: 9,
            composition_path: composition.display().to_string(),
            render_source_path: render_source.display().to_string(),
            clip_id: "source".to_string(),
            track_id: "video".to_string(),
            scene: "Camera A".to_string(),
            start_ms: 500,
            end_ms: 2000,
            duration_ms: 1,
            source_video: Some(serde_json::json!({ "filename": "camera-a.webm" })),
            suggestion_id: Some("sug-demo".to_string()),
            suggestion_reason: Some("保留已选开场".to_string()),
            semantic_ref: Some("sem-demo".to_string()),
            semantic_summary: Some("开场产品近景".to_string()),
            semantic_tags: vec!["开场".to_string(), "产品".to_string()],
            semantic_reason: Some("语义理由：适合开场。".to_string()),
            updated_at: 0,
        }])?;
        assert_eq!(written.schema_version, VIDEO_CLIP_QUEUE_SCHEMA_VERSION);
        assert_eq!(written.items[0].sequence, 1);
        assert_eq!(written.items[0].duration_ms, 1500);
        assert_eq!(
            written.items[0].composition_path,
            ".capy/video-compositions/art_a/compositions/main.json"
        );

        let reopened = ProjectPackage::open(&project)?.video_clip_queue()?;
        assert_eq!(reopened.items.len(), 1);
        let source_video = reopened.items[0]
            .source_video
            .as_ref()
            .ok_or("source video metadata should round-trip")?;
        assert_eq!(source_video["filename"], "camera-a.webm");
        assert_eq!(reopened.items[0].suggestion_id.as_deref(), Some("sug-demo"));
        assert_eq!(reopened.items[0].semantic_ref.as_deref(), Some("sem-demo"));
        assert_eq!(reopened.items[0].semantic_tags.len(), 2);
        Ok(())
    }
}
