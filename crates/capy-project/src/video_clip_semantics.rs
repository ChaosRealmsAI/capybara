use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::ArtifactKind;
use crate::package::{CAPY_DIR, ProjectPackage, ProjectPackageResult, now_ms, read_json};
use crate::video_clip_queue::ProjectVideoClipQueueItemV1;

pub const VIDEO_CLIP_SEMANTICS_SCHEMA_VERSION: &str = "capy.project-video-clip-semantics.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipSemanticsManifestV1 {
    pub schema_version: String,
    pub project_id: String,
    pub project_name: String,
    pub analyzer: String,
    pub updated_at: u64,
    pub source_video_count: usize,
    pub source_queue_count: usize,
    #[serde(default)]
    pub items: Vec<ProjectVideoClipSemanticItemV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectVideoClipSemanticItemV1 {
    pub id: String,
    pub sequence: u64,
    pub clip_key: String,
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
    pub summary_zh: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub rhythm: String,
    pub use_case: String,
    pub recommendation: String,
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
    pub fn video_clip_semantics(
        &self,
    ) -> ProjectPackageResult<ProjectVideoClipSemanticsManifestV1> {
        let project = self.project_manifest()?;
        let path = self.video_clip_semantics_path();
        if !path.exists() {
            return Ok(ProjectVideoClipSemanticsManifestV1 {
                schema_version: VIDEO_CLIP_SEMANTICS_SCHEMA_VERSION.to_string(),
                project_id: project.id,
                project_name: project.name,
                analyzer: local_analyzer_name(),
                updated_at: project.updated_at,
                source_video_count: 0,
                source_queue_count: 0,
                items: Vec::new(),
            });
        }
        let mut manifest: ProjectVideoClipSemanticsManifestV1 =
            read_json(&path, "read project video clip semantics")?;
        if manifest.schema_version.trim().is_empty() {
            manifest.schema_version = VIDEO_CLIP_SEMANTICS_SCHEMA_VERSION.to_string();
        }
        if manifest.project_id.trim().is_empty() {
            manifest.project_id = project.id;
        }
        if manifest.project_name.trim().is_empty() {
            manifest.project_name = project.name;
        }
        if manifest.analyzer.trim().is_empty() {
            manifest.analyzer = local_analyzer_name();
        }
        manifest.items = normalize_semantic_items(manifest.items);
        Ok(manifest)
    }

    pub fn analyze_video_clip_semantics(
        &self,
    ) -> ProjectPackageResult<ProjectVideoClipSemanticsManifestV1> {
        let project = self.project_manifest()?;
        let queue = self.video_clip_queue()?;
        let videos = self.semantic_video_candidates()?;
        let mut items = Vec::new();
        if queue.items.is_empty() {
            for video in &videos {
                items.push(semantic_from_video(video, items.len() + 1));
            }
        } else {
            for item in &queue.items {
                items.push(semantic_from_queue_item(item, items.len() + 1));
            }
        }
        let manifest = ProjectVideoClipSemanticsManifestV1 {
            schema_version: VIDEO_CLIP_SEMANTICS_SCHEMA_VERSION.to_string(),
            project_id: project.id,
            project_name: project.name,
            analyzer: local_analyzer_name(),
            updated_at: now_ms(),
            source_video_count: videos.len(),
            source_queue_count: queue.items.len(),
            items,
        };
        self.write_json(&self.video_clip_semantics_path(), &manifest)?;
        self.touch_project_manifest()?;
        Ok(manifest)
    }

    fn semantic_video_candidates(&self) -> ProjectPackageResult<Vec<VideoCandidate>> {
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

    fn video_clip_semantics_path(&self) -> std::path::PathBuf {
        self.root().join(CAPY_DIR).join("video-clip-semantics.json")
    }
}

pub(crate) fn clip_semantic_key(
    composition_path: &str,
    clip_id: &str,
    start_ms: u64,
    end_ms: u64,
) -> String {
    format!(
        "{}|{}|{}|{}",
        composition_path.trim(),
        clip_id.trim(),
        start_ms,
        end_ms
    )
}

fn semantic_from_queue_item(
    item: &ProjectVideoClipQueueItemV1,
    index: usize,
) -> ProjectVideoClipSemanticItemV1 {
    let duration_ms = item
        .duration_ms
        .max(item.end_ms.saturating_sub(item.start_ms))
        .max(1);
    let filename = filename_from_source_video(item.source_video.as_ref())
        .unwrap_or_else(|| "项目视频".to_string());
    let scene = nonempty(&item.scene).unwrap_or_else(|| format!("片段 {index}"));
    let rhythm = rhythm_for_duration(duration_ms);
    let use_case = use_case_for(&scene, index);
    let tags = tags_for(&scene, &filename, duration_ms, index);
    let summary = format!(
        "第 {index} 段来自 {filename}，聚焦“{scene}”，时长 {}，适合承担{}。",
        format_duration(duration_ms),
        use_case
    );
    let recommendation = format!(
        "语义理由：这段的核心内容是“{scene}”，节奏判断为{rhythm}，建议用作{}。",
        use_case
    );
    ProjectVideoClipSemanticItemV1 {
        id: semantic_id(
            &item.composition_path,
            &item.clip_id,
            item.start_ms,
            item.end_ms,
        ),
        sequence: index as u64,
        clip_key: clip_semantic_key(
            &item.composition_path,
            &item.clip_id,
            item.start_ms,
            item.end_ms,
        ),
        composition_path: item.composition_path.clone(),
        render_source_path: item.render_source_path.clone(),
        clip_id: item.clip_id.clone(),
        track_id: item.track_id.clone(),
        scene,
        start_ms: item.start_ms,
        end_ms: item.end_ms,
        duration_ms,
        source_video: item.source_video.clone(),
        summary_zh: summary,
        tags,
        rhythm,
        use_case,
        recommendation,
    }
}

fn semantic_from_video(video: &VideoCandidate, index: usize) -> ProjectVideoClipSemanticItemV1 {
    let source_duration = video.duration_ms.max(1);
    let duration_ms = source_duration.min(1_800).max(source_duration.min(1_000));
    let start_ms = if index == 1 {
        0
    } else {
        source_duration.saturating_sub(duration_ms)
    };
    let end_ms = start_ms.saturating_add(duration_ms).min(source_duration);
    let scene = format!("{} · 项目视频片段", video.title);
    let rhythm = rhythm_for_duration(duration_ms);
    let use_case = use_case_for(&scene, index);
    let tags = tags_for(&scene, &video.filename, duration_ms, index);
    let summary = format!(
        "第 {index} 段来自 {}，基于项目视频元数据抽取 {} 片段，适合承担{}。",
        video.filename,
        format_duration(duration_ms),
        use_case
    );
    let recommendation = format!(
        "语义理由：{} 是尚未进入队列的项目素材，可作为{}补充方案覆盖面。",
        video.filename, use_case
    );
    ProjectVideoClipSemanticItemV1 {
        id: semantic_id(&video.composition_path, "source", start_ms, end_ms),
        sequence: index as u64,
        clip_key: clip_semantic_key(&video.composition_path, "source", start_ms, end_ms),
        composition_path: video.composition_path.clone(),
        render_source_path: String::new(),
        clip_id: "source".to_string(),
        track_id: "video".to_string(),
        scene,
        start_ms,
        end_ms,
        duration_ms: end_ms.saturating_sub(start_ms).max(1),
        source_video: Some(json!({
            "artifact_id": video.artifact_id,
            "src": video.source_path,
            "filename": video.filename,
            "duration_ms": video.duration_ms,
            "width": video.width,
            "height": video.height
        })),
        summary_zh: summary,
        tags,
        rhythm,
        use_case,
        recommendation,
    }
}

fn normalize_semantic_items(
    items: Vec<ProjectVideoClipSemanticItemV1>,
) -> Vec<ProjectVideoClipSemanticItemV1> {
    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let duration_ms = item
                .duration_ms
                .max(item.end_ms.saturating_sub(item.start_ms))
                .max(1);
            ProjectVideoClipSemanticItemV1 {
                sequence: (index + 1) as u64,
                clip_key: if item.clip_key.trim().is_empty() {
                    clip_semantic_key(
                        &item.composition_path,
                        &item.clip_id,
                        item.start_ms,
                        item.end_ms,
                    )
                } else {
                    item.clip_key
                },
                duration_ms,
                ..item
            }
        })
        .collect()
}

fn tags_for(scene: &str, filename: &str, duration_ms: u64, index: usize) -> Vec<String> {
    let mut tags = Vec::new();
    let lower = format!("{} {}", scene, filename).to_lowercase();
    if index == 1 || lower.contains("open") || lower.contains("wide") {
        tags.push("开场".to_string());
    }
    if lower.contains("wide") || lower.contains("全景") {
        tags.push("全景".to_string());
    }
    if lower.contains("close") || lower.contains("detail") || lower.contains("product") {
        tags.push("产品细节".to_string());
    }
    if lower.contains("camera") {
        tags.push("多机位".to_string());
    }
    tags.push(if duration_ms <= 1_500 {
        "快节奏".to_string()
    } else if duration_ms <= 3_000 {
        "说明段".to_string()
    } else {
        "铺垫".to_string()
    });
    tags.sort();
    tags.dedup();
    tags
}

fn rhythm_for_duration(duration_ms: u64) -> String {
    if duration_ms <= 1_500 {
        "快节奏 hook，可用于抓住开头注意力".to_string()
    } else if duration_ms <= 3_000 {
        "中速说明，适合承接信息和产品证明".to_string()
    } else {
        "慢速铺垫，适合建立背景或收束情绪".to_string()
    }
}

fn use_case_for(scene: &str, index: usize) -> String {
    let lower = scene.to_lowercase();
    if index == 1 || lower.contains("open") || lower.contains("opening") {
        "开场吸引注意".to_string()
    } else if lower.contains("close") || lower.contains("detail") || lower.contains("product") {
        "产品细节证明".to_string()
    } else {
        "承接段落或转场补充".to_string()
    }
}

fn filename_from_source_video(source: Option<&Value>) -> Option<String> {
    source?
        .get("filename")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn semantic_id(composition_path: &str, clip_id: &str, start_ms: u64, end_ms: u64) -> String {
    let key = clip_semantic_key(composition_path, clip_id, start_ms, end_ms);
    format!("sem-fnv1a64-{:016x}", fnv1a64(key.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn format_duration(ms: u64) -> String {
    let seconds = ms as f64 / 1000.0;
    format!("{seconds:.1}s")
}

fn local_analyzer_name() -> String {
    "local-deterministic-video-clip-semantic-analyzer".to_string()
}

fn nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProjectPackage, ProjectVideoClipQueueItemV1};
    use std::error::Error;
    use std::fs;

    #[test]
    fn video_clip_semantics_persist_queue_analysis() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let project = dir.path().join("demo");
        let package = ProjectPackage::init(&project, Some("Semantics Project".to_string()))?;
        let composition = project.join(".capy/video-compositions/art_a/compositions/main.json");
        fs::create_dir_all(composition.parent().ok_or("composition parent")?)?;
        fs::write(&composition, "{}\n")?;
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

        let manifest = package.analyze_video_clip_semantics()?;
        assert_eq!(manifest.schema_version, VIDEO_CLIP_SEMANTICS_SCHEMA_VERSION);
        assert_eq!(manifest.source_queue_count, 1);
        assert_eq!(manifest.items.len(), 1);
        assert!(
            manifest.items[0]
                .summary_zh
                .contains("Camera A opening detail")
        );
        assert!(manifest.items[0].tags.contains(&"开场".to_string()));
        assert!(manifest.items[0].recommendation.contains("语义理由"));

        let reopened = ProjectPackage::open(&project)?.video_clip_semantics()?;
        assert_eq!(reopened.items[0].clip_key, manifest.items[0].clip_key);
        Ok(())
    }
}
