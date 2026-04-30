use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{read_png_metrics, snapshot_embedded};

#[test]
fn embedded_snapshot_writes_png_with_viewport_dimensions() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = unique_dir("happy")?;
    let source = dir.join("render_source.json");
    let out = dir.join("frame.png");
    fs::write(&source, serde_json::to_vec_pretty(&render_source())?)?;

    let metrics = snapshot_embedded(&source, &out, 0)?;

    assert!(out.is_file());
    assert!(metrics.byte_size > 0);
    assert_eq!(metrics.width, 320);
    assert_eq!(metrics.height, 180);
    let decoded = read_png_metrics(&out)?;
    assert_eq!(decoded, metrics);
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn embedded_snapshot_defaults_viewport() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("default-viewport")?;
    let source = dir.join("render_source.json");
    let out = dir.join("frame.png");
    let mut value = render_source();
    value["viewport"] = json!({});
    fs::write(&source, serde_json::to_vec_pretty(&value)?)?;

    let metrics = snapshot_embedded(&source, &out, 0)?;

    assert_eq!(metrics.width, 1080);
    assert_eq!(metrics.height, 1080);
    fs::remove_dir_all(dir)?;
    Ok(())
}

#[test]
fn embedded_snapshot_reports_missing_poster() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_dir("missing-poster")?;
    let source = dir.join("render_source.json");
    let out = dir.join("frame.png");
    fs::write(&source, serde_json::to_vec_pretty(&json!({"tracks": []}))?)?;

    let err = snapshot_embedded(&source, &out, 0)
        .err()
        .ok_or("missing poster should fail")?;

    assert_eq!(err.code, "SNAPSHOT_FAILED");
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn render_source() -> serde_json::Value {
    json!({
        "schema_version": "capy.timeline.render_source.v1",
        "viewport": {"w": 320, "h": 180},
        "tracks": [{
            "id": "poster.document",
            "clips": [{
                "params": {
                    "params": {
                        "poster": {
                            "canvas": {"background": "#ffffff"},
                            "assets": {},
                            "layers": [
                                {
                                    "id": "shape",
                                    "type": "shape",
                                    "shape": "rect",
                                    "x": 0,
                                    "y": 0,
                                    "width": 320,
                                    "height": 180,
                                    "style": {"fill": "#f0f0f0"}
                                },
                                {
                                    "id": "headline",
                                    "type": "text",
                                    "text": "Launch",
                                    "x": 20,
                                    "y": 20,
                                    "width": 120,
                                    "height": 40,
                                    "style": {"color": "#111111"}
                                }
                            ]
                        }
                    }
                }
            }]
        }]
    })
}

fn unique_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "capy-timeline-snapshot-embedded-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    ));
    fs::create_dir_all(Path::new(&dir))?;
    Ok(dir)
}
