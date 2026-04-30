use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::ArtifactKind;
use crate::package::{ProjectPackage, ProjectPackageResult, now_ms};
use crate::video_clip_queue::{ProjectVideoClipQueueItemV1, VIDEO_CLIP_QUEUE_SCHEMA_VERSION};

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
        });
        let suggestion_id = format!(
            "sug-fnv1a64-{:016x}",
            fnv1a64(serde_json::to_string(&basis).unwrap_or_default().as_bytes())
        );
        let mut items = Vec::new();
        for item in queue.items.iter().take(4) {
            items.push(suggestion_from_queue_item(
                item,
                &suggestion_id,
                items.len() + 1,
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
            ));
        }
        if items.is_empty() {
            for video in videos.iter().take(4) {
                items.push(suggestion_from_video(
                    video,
                    &suggestion_id,
                    items.len() + 1,
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
) -> ProjectVideoClipSuggestionItemV1 {
    let duration_ms = item
        .duration_ms
        .max(item.end_ms.saturating_sub(item.start_ms))
        .max(1);
    ProjectVideoClipSuggestionItemV1 {
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
    }
}

fn suggestion_from_video(
    video: &VideoCandidate,
    suggestion_id: &str,
    index: usize,
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
    ProjectVideoClipSuggestionItemV1 {
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
    }
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
    use crate::{ProjectPackage, ProjectVideoClipQueueItemV1};
    use std::error::Error;
    use std::fs;

    #[test]
    fn video_clip_suggestion_uses_project_videos_and_existing_queue() -> Result<(), Box<dyn Error>>
    {
        let dir = tempfile::tempdir()?;
        let project = dir.path().join("demo");
        let package = ProjectPackage::init(&project, Some("Suggestion Project".to_string()))?;
        let composition_a = write_video_artifact(&package, "art_a", "camera-a.webm", 4_000)?;
        let composition_b = write_video_artifact(&package, "art_b", "camera-b.webm", 5_000)?;
        package.write_video_clip_queue(vec![
            ProjectVideoClipQueueItemV1 {
                id: "queue-a".to_string(),
                sequence: 9,
                composition_path: composition_a.clone(),
                render_source_path: String::new(),
                clip_id: "source".to_string(),
                track_id: "video".to_string(),
                scene: "Camera A existing opener".to_string(),
                start_ms: 500,
                end_ms: 1_700,
                duration_ms: 1_200,
                source_video: Some(json!({ "filename": "camera-a.webm" })),
                suggestion_id: None,
                suggestion_reason: None,
                updated_at: 0,
            },
            ProjectVideoClipQueueItemV1 {
                id: "queue-b".to_string(),
                sequence: 10,
                composition_path: composition_b.clone(),
                render_source_path: String::new(),
                clip_id: "source".to_string(),
                track_id: "video".to_string(),
                scene: "Camera B existing closeup".to_string(),
                start_ms: 1_000,
                end_ms: 3_000,
                duration_ms: 2_000,
                source_video: Some(json!({ "filename": "camera-b.webm" })),
                suggestion_id: None,
                suggestion_reason: None,
                updated_at: 0,
            },
        ])?;

        let suggestion = package.suggest_video_clip_queue()?;
        assert_eq!(
            suggestion.schema_version,
            VIDEO_CLIP_SUGGESTION_SCHEMA_VERSION
        );
        assert_eq!(suggestion.source_video_count, 2);
        assert_eq!(suggestion.existing_queue_count, 2);
        assert_eq!(suggestion.items.len(), 2);
        assert!(suggestion.items.iter().all(|item| !item.reason.is_empty()));
        assert_eq!(suggestion.items[0].scene, "Camera A existing opener");
        assert_eq!(suggestion.items[1].scene, "Camera B existing closeup");
        assert!(suggestion.suggestion_id.starts_with("sug-fnv1a64-"));
        Ok(())
    }

    fn write_video_artifact(
        package: &ProjectPackage,
        artifact_id: &str,
        filename: &str,
        duration_ms: u64,
    ) -> Result<String, Box<dyn Error>> {
        let root = package.root();
        let media = root.join("media");
        fs::create_dir_all(&media)?;
        fs::write(media.join(filename), "fixture")?;
        let composition_path =
            format!(".capy/video-compositions/{artifact_id}/compositions/main.json");
        let composition_abs = root.join(&composition_path);
        fs::create_dir_all(composition_abs.parent().ok_or("composition parent")?)?;
        fs::write(&composition_abs, "{}\n")?;
        let mut registry = package.artifacts()?;
        registry.artifacts.push(crate::ArtifactRefV1 {
            id: artifact_id.to_string(),
            kind: crate::ArtifactKind::Video,
            title: filename.to_string(),
            source_path: format!("media/{filename}"),
            source_refs: Vec::new(),
            output_refs: vec![composition_path.clone()],
            design_language_refs: Vec::new(),
            asset_refs: Vec::new(),
            provenance: Some(json!({
                "video_import": {
                    "filename": filename,
                    "duration_ms": duration_ms,
                    "width": 640,
                    "height": 360,
                    "byte_size": 7,
                    "composition_path": composition_path
                }
            })),
            evidence_refs: Vec::new(),
            updated_at: 0,
        });
        package.write_artifacts(&registry)?;
        Ok(composition_path)
    }
}
