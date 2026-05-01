use crate::{
    ArtifactKind, ArtifactRefV1, ContextBuildRequest, ProjectPackage, ProjectVideoClipQueueItemV1,
    VIDEO_CLIP_PROPOSAL_HISTORY_SCHEMA_VERSION, VIDEO_CLIP_PROPOSAL_SCHEMA_VERSION,
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
    assert_eq!(proposal.revision, 1);
    assert!(proposal.base_queue_hash.starts_with("queue-fnv1a64-"));
    assert_eq!(
        proposal.current_queue_hash.as_deref(),
        Some(proposal.base_queue_hash.as_str())
    );
    assert!(proposal.safety_note.contains("不会修改"));
    assert!(proposal.safety_note.contains("base_queue_hash"));
    let history = package.video_clip_proposal_history()?;
    assert_eq!(
        history.schema_version,
        VIDEO_CLIP_PROPOSAL_HISTORY_SCHEMA_VERSION
    );
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].proposal_id, proposal.proposal_id);
    assert_eq!(history.entries[0].status, "proposed");
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

    let reject_result = package.decide_video_clip_proposal_for_revision(
        &rejected.proposal_id,
        Some(rejected.revision),
        "reject",
        "PM wants original",
    )?;
    assert_eq!(reject_result.proposal.status, "rejected");
    assert_eq!(
        package.video_clip_proposal_history()?.entries[0].status,
        "rejected"
    );
    assert!(reject_result.queue_manifest.is_none());
    assert_eq!(
        queue_ids(&original),
        queue_ids(&package.video_clip_queue()?)
    );

    let accepted = package.generate_video_clip_proposal()?;
    assert!(accepted.revision > rejected.revision);
    let accept_result = package.decide_video_clip_proposal_for_revision(
        &accepted.proposal_id,
        Some(accepted.revision),
        "accept",
        "PM approves",
    )?;
    assert_eq!(accept_result.proposal.status, "accepted");
    assert!(
        package
            .video_clip_proposal_history()?
            .entries
            .iter()
            .any(|entry| entry.revision == accepted.revision
                && entry.status == "accepted"
                && entry
                    .decision
                    .as_ref()
                    .is_some_and(|decision| decision.queue_updated))
    );
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

#[test]
fn video_clip_proposal_accept_conflicts_when_queue_hash_changed() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_project()?;
    let package = &fixture.package;
    package.analyze_video_clip_semantics()?;
    package.record_video_clip_feedback("queue-a", "这段不适合开场")?;
    let proposal = package.generate_video_clip_proposal()?;
    let current = package.video_clip_queue()?;
    let externally_changed = vec![current.items[0].clone()];
    package.write_video_clip_queue(externally_changed)?;
    let queue_after_external_change = queue_manifest_text(package)?;

    let result = package.decide_video_clip_proposal_for_revision(
        &proposal.proposal_id,
        Some(proposal.revision),
        "accept",
        "PM clicked stale proposal",
    )?;

    assert_eq!(result.proposal.status, "conflicted");
    assert!(result.queue_manifest.is_none());
    assert_eq!(queue_manifest_text(package)?, queue_after_external_change);
    assert_eq!(queue_ids(&package.video_clip_queue()?), vec!["queue-a"]);
    let conflict = result.proposal.conflict.ok_or("missing conflict")?;
    assert_eq!(conflict.conflict_type, "queue_changed_since_proposal");
    assert_eq!(conflict.base_queue_hash, proposal.base_queue_hash);
    assert_ne!(conflict.current_queue_hash, proposal.base_queue_hash);
    assert!(
        result
            .proposal
            .changes
            .iter()
            .all(|change| change.apply_status == "conflicted")
    );
    let history = package.video_clip_proposal_history()?;
    let conflicted = history
        .entries
        .iter()
        .find(|entry| entry.revision == proposal.revision)
        .ok_or("missing conflicted history entry")?;
    assert_eq!(conflicted.status, "conflicted");
    assert_eq!(
        conflicted
            .conflict
            .as_ref()
            .ok_or("missing history conflict")?
            .conflict_type,
        "queue_changed_since_proposal"
    );
    Ok(())
}

#[test]
fn video_clip_proposal_revision_distinguishes_repeated_generations() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_project()?;
    let package = &fixture.package;
    package.analyze_video_clip_semantics()?;
    package.record_video_clip_feedback("queue-a", "这段不适合开场")?;

    let first = package.generate_video_clip_proposal()?;
    let second = package.generate_video_clip_proposal()?;
    assert_eq!(first.revision, 1);
    assert_eq!(second.revision, 2);
    assert_ne!(first.proposal_id, second.proposal_id);
    assert_eq!(first.base_queue_hash, second.base_queue_hash);

    let error = package
        .decide_video_clip_proposal_for_revision(
            &second.proposal_id,
            Some(first.revision),
            "accept",
            "",
        )
        .err()
        .ok_or("expected stale revision error")?;
    assert!(error.to_string().contains("proposal revision mismatch"));
    assert_eq!(
        queue_ids(&package.video_clip_queue()?),
        vec!["queue-a", "queue-b"]
    );

    let current = package.video_clip_queue()?;
    package.write_video_clip_queue(vec![current.items[1].clone(), current.items[0].clone()])?;
    let third = package.generate_video_clip_proposal()?;
    assert_eq!(third.revision, 3);
    assert_ne!(third.base_queue_hash, first.base_queue_hash);
    Ok(())
}

#[test]
fn video_clip_proposal_history_persists_across_reopen() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_project()?;
    let package = &fixture.package;
    package.analyze_video_clip_semantics()?;
    package.record_video_clip_feedback("queue-a", "这段不适合开场")?;

    let first = package.generate_video_clip_proposal()?;
    package.decide_video_clip_proposal_for_revision(
        &first.proposal_id,
        Some(first.revision),
        "reject",
        "PM rejected first proposal",
    )?;
    let second = package.generate_video_clip_proposal()?;
    let current = package.video_clip_queue()?;
    package.write_video_clip_queue(vec![current.items[0].clone()])?;
    package.decide_video_clip_proposal_for_revision(
        &second.proposal_id,
        Some(second.revision),
        "accept",
        "PM tried stale proposal",
    )?;

    let reopened = ProjectPackage::open(package.root())?;
    let history = reopened.video_clip_proposal_history()?;
    assert_eq!(
        history.schema_version,
        VIDEO_CLIP_PROPOSAL_HISTORY_SCHEMA_VERSION
    );
    assert_eq!(history.entries.len(), 2);
    assert_eq!(history.entries[0].status, "rejected");
    assert_eq!(history.entries[1].status, "conflicted");
    assert_eq!(history.entries[0].changes.len(), first.changes.len());
    assert_eq!(history.entries[1].changes.len(), second.changes.len());
    assert!(history.entries[0].decision.is_some());
    assert!(history.entries[1].conflict.is_some());
    assert_eq!(reopened.video_clip_proposal()?.status, "conflicted");

    let third = reopened.generate_video_clip_proposal()?;
    assert_eq!(third.revision, 3);
    assert_eq!(reopened.video_clip_proposal_history()?.entries.len(), 3);
    Ok(())
}

#[test]
fn context_build_includes_safe_video_project_package() -> Result<(), Box<dyn Error>> {
    let fixture = fixture_project()?;
    let package = &fixture.package;
    fs::create_dir_all(package.root().join("design"))?;
    fs::write(
        package.root().join("design/video-rules.md"),
        "Keep edits linear.",
    )?;
    package.add_design_asset(
        "markdown".to_string(),
        Some("brand-rule".to_string()),
        "design/video-rules.md",
        "Video rules".to_string(),
        Some("Project-level video constraints".to_string()),
    )?;
    package.analyze_video_clip_semantics()?;
    package.record_video_clip_feedback("queue-a", "这段不适合开场")?;

    let first = package.generate_video_clip_proposal()?;
    package.decide_video_clip_proposal_for_revision(
        &first.proposal_id,
        Some(first.revision),
        "reject",
        "PM rejected first proposal",
    )?;
    let stale = package.generate_video_clip_proposal()?;
    let current = package.video_clip_queue()?;
    package.write_video_clip_queue(vec![current.items[0].clone()])?;
    package.decide_video_clip_proposal_for_revision(
        &stale.proposal_id,
        Some(stale.revision),
        "accept",
        "PM clicked stale proposal",
    )?;
    let valid = package.generate_video_clip_proposal()?;
    package.decide_video_clip_proposal_for_revision(
        &valid.proposal_id,
        Some(valid.revision),
        "accept",
        "PM accepts current proposal",
    )?;
    let queue_before_manifest = package.video_clip_queue()?;
    let queue_before = queue_ids(&queue_before_manifest);

    let context = package.build_context(ContextBuildRequest {
        artifact_id: "art_a".to_string(),
        selector: None,
        canvas_node: None,
        json_pointer: None,
    })?;
    let video_context = context
        .video_project_context
        .ok_or("context package should include video project context")?;

    assert!(video_context.package_id.starts_with("vpctx-fnv1a64-"));
    assert_eq!(video_context.anchor_artifact.artifact_id, "art_a");
    assert_eq!(video_context.source_media.len(), 2);
    assert_eq!(video_context.proposal_history.entry_count, 3);
    assert_eq!(video_context.proposal_history.status_counts["rejected"], 1);
    assert_eq!(
        video_context.proposal_history.status_counts["conflicted"],
        1
    );
    assert_eq!(video_context.proposal_history.status_counts["accepted"], 1);
    assert_eq!(video_context.proposal_history.conflicts.len(), 1);
    assert!(
        video_context
            .clip_queue
            .current_queue_hash
            .starts_with("queue-fnv1a64-")
    );
    assert!(video_context.safety.safe_for_next_ai_input);
    assert!(video_context.safety.no_queue_write);
    assert_eq!(video_context.design_constraints.assets.len(), 1);
    assert_eq!(queue_ids(&package.video_clip_queue()?), queue_before);
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
