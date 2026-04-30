use super::*;

#[test]
fn clip_proposal_composition_contains_selected_scene_only() -> Result<(), Box<dyn std::error::Error>>
{
    let root = unique_dir("clip-proposal");
    let project = root.join("demo");
    fs::create_dir_all(project.join("components"))?;
    fs::create_dir_all(project.join("compositions"))?;
    fs::write(
        project.join("components/html.capy-title.js"),
        "export function mount() {}\nexport function update() {}\n",
    )?;
    let composition = project.join("compositions/main.json");
    write_json(
        &composition,
        &json!({
            "schema": "capy.timeline.composition.v2",
            "schema_version": "capy.composition.v2",
            "id": "demo",
            "name": "Demo",
            "viewport": { "w": 1920, "h": 1080, "ratio": "16:9" },
            "theme": "default",
            "clips": [
                {
                    "id": "intro",
                    "name": "Intro",
                    "duration": "2s",
                    "tracks": [{
                        "id": "stage",
                        "kind": "component",
                        "component": "html.capy-title",
                        "items": [{ "id": "headline", "params": { "title": "Intro" } }]
                    }]
                },
                {
                    "id": "export",
                    "name": "Export",
                    "duration": "3s",
                    "tracks": [{
                        "id": "stage",
                        "kind": "component",
                        "component": "html.capy-title",
                        "items": [{ "id": "headline", "params": { "title": "Export" } }]
                    }]
                }
            ]
        }),
    )?;

    let out = write_clip_proposal_composition(
        &composition,
        &json!({ "range": { "clip_id": "export", "start_ms": 2000, "end_ms": 5000 } }),
        "job-test",
    )?;
    let clipped = read_json(&out)?;

    assert_eq!(clipped["duration_ms"], json!(3000));
    assert_eq!(clipped["clips"].as_array().map(Vec::len), Some(1));
    assert_eq!(clipped["clips"][0]["id"], json!("export"));
    assert!(
        out.display()
            .to_string()
            .contains("exports/clip-proposals/job-test/compositions/main.json")
    );
    let proposal_root = out
        .parent()
        .and_then(Path::parent)
        .ok_or("missing proposal root")?;
    assert!(
        proposal_root
            .join("components/html.capy-title.js")
            .is_file()
    );
    let _ = fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn clip_proposal_preserves_source_video_range() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_dir("video-range");
    let project = root.join("demo");
    fs::create_dir_all(project.join("compositions"))?;
    let video = project.join("source.mp4");
    fs::write(&video, "placeholder")?;
    let composition = project.join("compositions/main.json");
    write_json(
        &composition,
        &json!({
            "schema": "capy.timeline.composition.v2",
            "id": "real-video",
            "name": "Real Video",
            "clips": [{
                "id": "source",
                "name": "source.mp4",
                "duration": "4s",
                "tracks": [{
                    "id": "video",
                    "kind": "video",
                    "params": { "src": format!("file://{}", video.display()) }
                }]
            }]
        }),
    )?;

    let out = write_clip_proposal_composition(
        &composition,
        &json!({ "range": { "clip_id": "source", "start_ms": 1000, "end_ms": 3000 } }),
        "job-video",
    )?;
    let clipped = read_json(&out)?;

    assert_eq!(clipped["duration_ms"], json!(2000));
    assert_eq!(
        clipped["clips"][0]["tracks"][0]["params"]["source_start_ms"],
        json!(1000)
    );
    assert_eq!(
        clipped["clips"][0]["tracks"][0]["params"]["source_end_ms"],
        json!(3000)
    );
    let _ = fs::remove_dir_all(root);
    Ok(())
}

fn unique_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "capy-timeline-editor-{label}-{}-{}",
        std::process::id(),
        timestamp_millis()
    ))
}

fn timestamp_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
