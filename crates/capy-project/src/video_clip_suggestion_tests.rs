use crate::{
    ArtifactKind, ArtifactRefV1, ProjectPackage, ProjectVideoClipQueueItemV1,
    VIDEO_CLIP_SUGGESTION_SCHEMA_VERSION,
};
use serde_json::json;
use std::error::Error;
use std::fs;

#[test]
fn video_clip_suggestion_uses_project_videos_and_existing_queue() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let project = dir.path().join("demo");
    let package = ProjectPackage::init(&project, Some("Suggestion Project".to_string()))?;
    let composition_a = write_video_artifact(&package, "art_a", "camera-a.webm", 4_000)?;
    let composition_b = write_video_artifact(&package, "art_b", "camera-b.webm", 5_000)?;
    package.write_video_clip_queue(vec![
        queue_item(
            "queue-a",
            composition_a,
            "Camera A existing opener",
            500,
            1_700,
        ),
        queue_item(
            "queue-b",
            composition_b,
            "Camera B existing closeup",
            1_000,
            3_000,
        ),
    ])?;
    package.analyze_video_clip_semantics()?;

    let suggestion = package.suggest_video_clip_queue()?;
    assert_eq!(
        suggestion.schema_version,
        VIDEO_CLIP_SUGGESTION_SCHEMA_VERSION
    );
    assert_eq!(suggestion.source_video_count, 2);
    assert_eq!(suggestion.existing_queue_count, 2);
    assert_eq!(suggestion.items.len(), 2);
    assert!(suggestion.items.iter().all(|item| !item.reason.is_empty()));
    assert!(suggestion.items.iter().all(|item| {
        item.semantic_reason
            .as_deref()
            .unwrap_or("")
            .contains("摘要")
    }));
    assert_eq!(suggestion.items[0].scene, "Camera A existing opener");
    assert_eq!(suggestion.items[1].scene, "Camera B existing closeup");
    assert!(suggestion.suggestion_id.starts_with("sug-fnv1a64-"));
    Ok(())
}

#[test]
fn video_clip_suggestion_cites_feedback_without_mutating_queue() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let project = dir.path().join("demo");
    let package = ProjectPackage::init(&project, Some("Feedback Suggestion".to_string()))?;
    let composition_a = write_video_artifact(&package, "art_a", "camera-a.webm", 4_000)?;
    let composition_b = write_video_artifact(&package, "art_b", "camera-b.webm", 5_000)?;
    package.write_video_clip_queue(vec![
        queue_item(
            "queue-a",
            composition_a,
            "Camera A opening detail",
            500,
            1_700,
        ),
        queue_item(
            "queue-b",
            composition_b,
            "Camera B product closeup",
            1_000,
            2_500,
        ),
    ])?;
    package.analyze_video_clip_semantics()?;
    package.record_video_clip_feedback("queue-a", "这段不适合开场")?;

    let suggestion = package.suggest_video_clip_queue()?;
    assert_eq!(suggestion.items[0].scene, "Camera B product closeup");
    let feedback_item = suggestion
        .items
        .iter()
        .find(|item| item.scene == "Camera A opening detail")
        .ok_or("feedback item should remain in suggestion")?;
    assert_eq!(
        feedback_item.feedback_text.as_deref(),
        Some("这段不适合开场")
    );
    assert!(
        feedback_item
            .feedback_reason
            .as_deref()
            .unwrap_or("")
            .contains("不把该片段作为开场首位")
    );
    let queue = package.video_clip_queue()?;
    assert_eq!(queue.items[0].id, "queue-a");
    Ok(())
}

fn queue_item(
    id: &str,
    composition_path: String,
    scene: &str,
    start_ms: u64,
    end_ms: u64,
) -> ProjectVideoClipQueueItemV1 {
    ProjectVideoClipQueueItemV1 {
        id: id.to_string(),
        sequence: 1,
        composition_path,
        render_source_path: String::new(),
        clip_id: "source".to_string(),
        track_id: "video".to_string(),
        scene: scene.to_string(),
        start_ms,
        end_ms,
        duration_ms: end_ms.saturating_sub(start_ms),
        source_video: Some(json!({ "filename": format!("{id}.webm") })),
        suggestion_id: None,
        suggestion_reason: None,
        semantic_ref: None,
        semantic_summary: None,
        semantic_tags: Vec::new(),
        semantic_reason: None,
        updated_at: 0,
    }
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
    let composition_path = format!(".capy/video-compositions/{artifact_id}/compositions/main.json");
    let composition_abs = root.join(&composition_path);
    fs::create_dir_all(composition_abs.parent().ok_or("composition parent")?)?;
    fs::write(&composition_abs, "{}\n")?;
    let mut registry = package.artifacts()?;
    registry.artifacts.push(ArtifactRefV1 {
        id: artifact_id.to_string(),
        kind: ArtifactKind::Video,
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
