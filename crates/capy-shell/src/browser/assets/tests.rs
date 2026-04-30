use super::*;

#[test]
fn percent_decode_rejects_invalid_escape() {
    assert!(percent_decode("bad%zz").is_err());
}

#[test]
fn frontend_url_escapes_project_query_value() {
    let url = frontend_url("http://127.0.0.1:1", "demo project/alpha", 2.0);
    assert_eq!(
        url,
        "http://127.0.0.1:1/index.html?project=demo%20project/alpha&dpr=2.000"
    );
}

#[test]
fn wasm_assets_use_wasm_mime_type() {
    assert_eq!(
        mime_for_path(Path::new("capy_canvas_web_bg.wasm")),
        "application/wasm"
    );
}

#[test]
fn fixture_assets_are_served_from_workspace_root() -> Result<(), Box<dyn std::error::Error>> {
    let frontend = project_root()?.join("frontend/capy-app");
    let response = frontend_response(&frontend, "/fixtures/poster/v1/single-poster.json")?;
    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "application/json; charset=utf-8");
    let text = String::from_utf8(response.body)?;
    assert!(text.contains("\"schema\": \"capy.poster.document.v1\""));
    assert!(text.contains("\"title\": \"AI Design Poster\""));
    Ok(())
}

#[test]
fn fixture_route_rejects_path_escape() -> Result<(), Box<dyn std::error::Error>> {
    let response = workspace_fixture_response("fixtures/../frontend/capy-app/index.html")?;
    assert_eq!(response.status, 403);
    Ok(())
}

#[test]
fn target_assets_are_served_from_workspace_root() -> Result<(), Box<dyn std::error::Error>> {
    let root = project_root()?;
    let path = root.join("target/capy-asset-route-test.json");
    std::fs::create_dir_all(path.parent().ok_or("missing parent")?)?;
    std::fs::write(&path, br#"{"ok":true}"#)?;
    let frontend = root.join("frontend/capy-app");
    let response = frontend_response(&frontend, "/target/capy-asset-route-test.json")?;
    assert_eq!(response.status, 200);
    assert_eq!(response.content_type, "application/json; charset=utf-8");
    let text = String::from_utf8(response.body)?;
    assert!(text.contains("\"ok\":true"));
    let _remove_result = std::fs::remove_file(path);
    Ok(())
}

#[test]
fn source_project_root_is_a_frontend_candidate() {
    let candidates = frontend_root_candidates();
    assert!(
        candidates
            .iter()
            .any(|path| path.ends_with("frontend/capy-app"))
    );
}
