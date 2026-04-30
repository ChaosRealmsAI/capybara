use super::*;

#[test]
fn project_artifact_card_exposes_ref_selection_and_resize() {
    let mut state = AppState::new();
    let idx = state.create_project_artifact_card(
        "Landing HTML",
        96.0,
        128.0,
        420.0,
        260.0,
        "proj_demo",
        "surf_art_landing",
        "art_landing",
        "html",
        "src/index.html",
    );
    let shape = &state.shapes[idx];
    assert_eq!(shape.kind, ShapeKind::StickyNote);
    assert_eq!(shape.content_kind(), CanvasContentKind::ProjectArtifact);
    assert_eq!(shape.metadata.status.as_deref(), Some("ready"));
    assert_eq!(
        shape.metadata.source_path.as_deref(),
        Some("src/index.html")
    );
    assert_eq!(
        shape
            .metadata
            .artifact_ref
            .as_ref()
            .map(|artifact| artifact.surface_node_id.as_str()),
        Some("surf_art_landing")
    );
    assert_eq!(state.selected, vec![idx]);

    let snapshot = state.ai_snapshot();
    assert_eq!(
        snapshot.nodes[0].content_kind,
        CanvasContentKind::ProjectArtifact
    );
    assert_eq!(
        snapshot.nodes[0]
            .artifact_ref
            .as_ref()
            .map(|artifact| artifact.source_path.as_str()),
        Some("src/index.html")
    );
    let selection = state.selected_context();
    assert_eq!(
        selection.items[0]
            .artifact_ref
            .as_ref()
            .map(|artifact| artifact.artifact_id.as_str()),
        Some("art_landing")
    );

    let id = state.shapes[idx].id;
    state
        .resize_shape_by_id(id, 160.0, 180.0, 520.0, 310.0)
        .expect("resize by id");
    let resized = &state.shapes[idx];
    assert_eq!(
        (resized.x, resized.y, resized.w, resized.h),
        (160.0, 180.0, 520.0, 310.0)
    );
    assert_eq!(
        resized
            .metadata
            .artifact_ref
            .as_ref()
            .map(|artifact| artifact.source_path.as_str()),
        Some("src/index.html")
    );
}
