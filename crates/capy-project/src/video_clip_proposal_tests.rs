use crate::{
    ArtifactKind, ArtifactRefV1, ProjectPackage, ProjectVideoClipQueueItemV1,
    VIDEO_CLIP_PROPOSAL_SCHEMA_VERSION,
};
use serde_json::json;
use std::error::Error;
use std::fs;

#[test]
fn video_clip_proposal_preview_does_not_mutate_queue() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_project()?;
    let package = &fixture.package;
    package.analyze_video_clip_semantics()?;
    package.record_video_clip_feedback("queue-a", "这段不适合开场")?;
    let queue_before = package.video_clip_queue()?;

    let proposal = package.generate_video_clip_proposal()?;

    assert_eq!(proposal.schema_version, VIDEO_CLIP_PROPOSAL_SCHEMA_VERSION);
    assert_eq!(proposal.status, "proposed");
    assert!(proposal.safety_note.contains("不会修改"));
    assert_eq!(proposal.before_queue[0].id, "queue-a");
    assert_eq!(proposal.after_queue[0].id, "queue-b");
    assert!(proposal.changes.iter().any(|change| {
        change.action == "deprioritize"
            && change.before_sequence == Some(1)
            && change.after_sequence == Some(2)
            && change
                .feedback_reason
                .as_deref()
                .unwrap_or("")
                .contains("不把该片段作为开场首位")
    }));
    let queue_after = package.video_clip_queue()?;
    assert_eq!(queue_ids(&queue_before), queue_ids(&queue_after));
    Ok(())
}

#[test]
fn video_clip_proposal_decision_rejects_or_accepts_queue_update() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_project()?;
    let package = &fixture.package;
    package.analyze_video_clip_semantics()?;
    package.record_video_clip_feedback("queue-a", "这段不适合开场")?;
    let original = package.video_clip_queue()?;
    let rejected = package.generate_video_clip_proposal()?;

    let reject_result =
        package.decide_video_clip_proposal(&rejected.proposal_id, "reject", "PM wants original")?;
    assert_eq!(reject_result.proposal.status, "rejected");
    assert!(reject_result.queue_manifest.is_none());
    assert_eq!(
        queue_ids(&original),
        queue_ids(&package.video_clip_queue()?)
    );

    let accepted = package.generate_video_clip_proposal()?;
    let accept_result =
        package.decide_video_clip_proposal(&accepted.proposal_id, "accept", "PM approves")?;
    assert_eq!(accept_result.proposal.status, "accepted");
    assert_eq!(
        queue_ids(&package.video_clip_queue()?),
        vec!["queue-b", "queue-a"]
    );
    assert_eq!(
        queue_ids(
            accept_result
                .queue_manifest
                .as_ref()
                .ok_or("missing queue")?
        ),
        vec!["queue-b", "queue-a"]
    );
    assert!(
        accept_result
            .proposal
            .changes
            .iter()
            .all(|change| change.apply_status == "applied")
    );
    Ok(())
}

#[test]
fn video_clip_proposal_queue_file_is_written_only_by_accept() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_project()?;
    let package = &fixture.package;
    package.analyze_video_clip_semantics()?;
    package.record_video_clip_feedback("queue-a", "这段不适合开场")?;
    let queue_before = queue_manifest_text(package)?;

    let generated = package.generate_video_clip_proposal()?;
    assert_eq!(
        queue_manifest_text(package)?,
        queue_before,
        "proposal generation must not rewrite the queue file"
    );

    let rejected = package.decide_video_clip_proposal(&generated.proposal_id, "reject", "")?;
    assert_eq!(rejected.proposal.status, "rejected");
    assert_eq!(
        queue_manifest_text(package)?,
        queue_before,
        "reject decision must not rewrite the queue file"
    );

    let generated_again = package.generate_video_clip_proposal()?;
    assert_eq!(
        queue_manifest_text(package)?,
        queue_before,
        "regenerating after reject must still leave the queue file unchanged"
    );

    let accepted =
        package.decide_video_clip_proposal(&generated_again.proposal_id, "accept", "")?;
    assert_eq!(accepted.proposal.status, "accepted");
    let queue_after_accept = queue_manifest_text(package)?;
    assert_ne!(
        queue_after_accept, queue_before,
        "accept decision must be the operation that writes the queue file"
    );
    assert_eq!(
        queue_ids(&package.video_clip_queue()?),
        vec!["queue-b", "queue-a"]
    );
    Ok(())
}

fn queue_ids(queue: &crate::ProjectVideoClipQueueManifestV1) -> Vec<&str> {
    queue.items.iter().map(|item| item.id.as_str()).collect()
}

fn queue_manifest_text(package: &ProjectPackage) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(
        package.root().join(".capy/video-clip-queue.json"),
    )?)
}

struct Fixture {
    _dir: tempfile::TempDir,
    package: ProjectPackage,
}

fn fixture_project() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let project = dir.path().join("demo");
    let package = ProjectPackage::init(&project, Some("Proposal Project".to_string()))?;
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
    Ok(Fixture { _dir: dir, package })
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
