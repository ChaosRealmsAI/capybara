use std::fs;

use capy_project::{ArtifactKind, PatchDocumentV1, ProjectGenerateRequestV1, ProjectPackage};
use serde_json::json;

const PATCH_SCHEMA_VERSION: &str = "capy.patch.v1";

#[test]
fn review_accept_reject_retry_and_undo_are_file_safe() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let project = ProjectPackage::init(temp.path(), Some("Review Test".to_string()))?;
    fs::write(
        temp.path().join("index.html"),
        "<!doctype html><body><h1>Before</h1>\n</body>",
    )?;
    let artifact =
        project.add_artifact(ArtifactKind::Html, "index.html", "Home".to_string(), vec![])?;

    let proposal = project.generate(ProjectGenerateRequestV1 {
        artifact_id: artifact.id.clone(),
        provider: "fixture".to_string(),
        prompt: "Make it clearer".to_string(),
        dry_run: true,
        review: true,
        selector: None,
        canvas_node: None,
        json_pointer: None,
    })?;
    assert_eq!(proposal.run.status, "proposed");
    assert_eq!(
        fs::read_to_string(temp.path().join("index.html"))?,
        "<!doctype html><body><h1>Before</h1>\n</body>"
    );
    assert!(proposal.run.review.is_some());

    let rejected = project.reject_review_run(&proposal.run.id, "test")?;
    assert_eq!(rejected.run.status, "rejected");
    assert_eq!(
        fs::read_to_string(temp.path().join("index.html"))?,
        "<!doctype html><body><h1>Before</h1>\n</body>"
    );

    let retry = project.retry_review_run(&proposal.run.id, "test")?;
    assert_eq!(retry.run.status, "proposed");
    assert_eq!(
        retry
            .run
            .review
            .as_ref()
            .and_then(|review| review.parent_run_id.as_deref()),
        Some(proposal.run.id.as_str())
    );

    let accepted = project.accept_review_run(&retry.run.id, "test")?;
    assert_eq!(accepted.run.status, "accepted");
    let changed = fs::read_to_string(temp.path().join("index.html"))?;
    assert!(changed.contains("Capybara CLI draft"));

    let undone = project.undo_review_run(&retry.run.id, "test")?;
    assert_eq!(undone.run.status, "reverted");
    assert_eq!(
        fs::read_to_string(temp.path().join("index.html"))?,
        "<!doctype html><body><h1>Before</h1>\n</body>"
    );
    Ok(())
}

#[test]
fn stale_source_refuses_accept() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let project = ProjectPackage::init(temp.path(), Some("Review Test".to_string()))?;
    fs::write(temp.path().join("index.html"), "<h1>Before</h1>")?;
    let artifact =
        project.add_artifact(ArtifactKind::Html, "index.html", "Home".to_string(), vec![])?;
    let proposal = project.generate(ProjectGenerateRequestV1 {
        artifact_id: artifact.id,
        provider: "fixture".to_string(),
        prompt: "Make it clearer".to_string(),
        dry_run: true,
        review: true,
        selector: None,
        canvas_node: None,
        json_pointer: None,
    })?;

    fs::write(temp.path().join("index.html"), "<h1>Someone else</h1>")?;
    let error = project.accept_review_run(&proposal.run.id, "test").err();
    assert!(format!("{error:?}").contains("source hash changed"));
    Ok(())
}

#[test]
fn review_requires_single_operation() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let project = ProjectPackage::init(temp.path(), Some("Review Test".to_string()))?;
    let patch = PatchDocumentV1 {
        schema_version: PATCH_SCHEMA_VERSION.to_string(),
        project_id: None,
        input_context_ref: None,
        actor: None,
        operations: Vec::new(),
    };
    let request = ProjectGenerateRequestV1 {
        artifact_id: "missing".to_string(),
        provider: "fixture".to_string(),
        prompt: "Make it clearer".to_string(),
        dry_run: true,
        review: true,
        selector: None,
        canvas_node: None,
        json_pointer: None,
    };

    let error = project
        .record_review_proposal(&request, patch, json!({}), None, None)
        .err();
    assert!(format!("{error:?}").contains("exactly one operation"));
    Ok(())
}
