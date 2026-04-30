use std::fs;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};

use super::model::{
    AnimationAction, FixtureAction, FixtureKind, FixtureVisual, GameAssetPack, SourceJob,
    normalize_rel,
};

const BG: Rgba<u8> = Rgba([224, 224, 224, 255]);
const TRANSPARENT: Rgba<u8> = Rgba([0, 0, 0, 0]);
const FRAME: u32 = 160;

pub(super) fn ensure_pack_dirs(root: &Path) -> Result<(), String> {
    for dir in [
        "prompts",
        "raw",
        "transparent",
        "frames",
        "spritesheets",
        "qa",
        "preview",
    ] {
        fs::create_dir_all(root.join(dir))
            .map_err(|err| format!("create {} failed: {err}", root.join(dir).display()))?;
    }
    Ok(())
}

pub(super) fn write_prompt(root: &Path, job: &SourceJob) -> Result<(), String> {
    write_text(&root.join(job.prompt_path), job.prompt)
}

pub(super) fn write_fixture_source(root: &Path, job: &SourceJob) -> Result<(), String> {
    let path = root.join(job.output_path);
    let image = render_fixture(job.visual);
    save_image(&path, &image)
}

pub(super) fn build_pack_outputs(pack: &mut GameAssetPack, root: &Path) -> Result<(), String> {
    for asset in &mut pack.assets {
        if asset.actions.is_empty() {
            let raw_path = root.join(&asset.raw_path);
            let transparent_path = root.join(&asset.transparent_path);
            if !transparent_path.is_file() {
                gray_to_alpha(&raw_path, &transparent_path)?;
            }
            continue;
        }
        for action in &mut asset.actions {
            split_action_strip(root, action)?;
        }
        if asset.transparent_path.trim().is_empty() {
            continue;
        }
        let first_frame = asset
            .actions
            .first()
            .and_then(|action| action.frame_paths.first())
            .ok_or_else(|| format!("asset {} has no generated frame", asset.id))?;
        copy_file(root.join(first_frame), root.join(&asset.transparent_path))?;
    }
    pack.refresh_counts();
    write_contact_sheet(pack, root)?;
    write_preview(pack, root)?;
    write_report(pack, root, "built")?;
    Ok(())
}

pub(super) fn write_pack_json(pack: &GameAssetPack, root: &Path) -> Result<PathBuf, String> {
    let path = root.join("pack.json");
    let text = serde_json::to_string_pretty(pack).map_err(|err| err.to_string())?;
    write_text(&path, &format!("{text}\n"))?;
    Ok(path)
}

pub(super) fn write_report(
    pack: &GameAssetPack,
    root: &Path,
    verdict: &str,
) -> Result<PathBuf, String> {
    let report = json!({
        "schema": "capy.game_assets.report.v1",
        "verdict": verdict,
        "pack_id": pack.id,
        "mode": pack.mode,
        "asset_count": pack.assets.len(),
        "frame_count": pack.frame_count(),
        "spritesheet_count": pack.spritesheet_count(),
        "preview_html": pack.outputs.preview_html,
        "contact_sheet": pack.outputs.contact_sheet,
    });
    let path = root.join(&pack.outputs.report_json);
    let text = serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?;
    write_text(&path, &format!("{text}\n"))?;
    Ok(path)
}

pub(super) fn verify_pack(pack: &GameAssetPack, root: &Path) -> Value {
    let mut missing = Vec::new();
    check_file(root, "pack.json", &mut missing);
    check_file(root, &pack.outputs.preview_html, &mut missing);
    check_file(root, &pack.outputs.contact_sheet, &mut missing);
    check_file(root, &pack.outputs.report_json, &mut missing);
    for asset in &pack.assets {
        check_file(root, &asset.prompt_path, &mut missing);
        check_file(root, &asset.raw_path, &mut missing);
        check_file(root, &asset.transparent_path, &mut missing);
        for action in &asset.actions {
            check_file(root, &action.source_path, &mut missing);
            check_file(root, &action.spritesheet_path, &mut missing);
            for frame in &action.frame_paths {
                check_file(root, frame, &mut missing);
            }
        }
    }
    let frame_count = pack.frame_count();
    let spritesheet_count = pack.spritesheet_count();
    let passed = missing.is_empty()
        && pack.schema == super::model::PACK_SCHEMA
        && pack.assets.len() >= 5
        && frame_count >= 16
        && spritesheet_count >= 4;
    json!({
        "schema": "capy.game_assets.verify.v1",
        "verdict": if passed { "passed" } else { "failed" },
        "pack_id": pack.id,
        "pack_path": normalize_rel(root.join("pack.json")),
        "asset_count": pack.assets.len(),
        "frame_count": frame_count,
        "spritesheet_count": spritesheet_count,
        "missing": missing,
        "preview_html": normalize_rel(root.join(&pack.outputs.preview_html)),
        "contact_sheet": normalize_rel(root.join(&pack.outputs.contact_sheet)),
    })
}

fn split_action_strip(root: &Path, action: &mut AnimationAction) -> Result<(), String> {
    let frame_count = action.frame_paths.len() as u32;
    if frame_count == 0 {
        return Err(format!("action {} has no frame paths", action.id));
    }
    let source = image::open(root.join(&action.source_path))
        .map_err(|err| format!("open {} failed: {err}", action.source_path))?
        .to_rgba8();
    let cell_width = source.width().max(frame_count) / frame_count;
    let cell_height = source.height();
    let mut frames = Vec::new();
    for index in 0..frame_count {
        let x = index * cell_width;
        let crop = image::imageops::crop_imm(&source, x, 0, cell_width, cell_height).to_image();
        let resized = image::imageops::resize(&crop, FRAME, FRAME, FilterType::Nearest);
        let alpha = remove_gray_background(&resized);
        let path = root.join(&action.frame_paths[index as usize]);
        save_image(&path, &alpha)?;
        frames.push(alpha);
    }
    let sheet = horizontal_sheet(&frames);
    save_image(&root.join(&action.spritesheet_path), &sheet)?;
    Ok(())
}

fn gray_to_alpha(raw_path: &Path, transparent_path: &Path) -> Result<(), String> {
    let image = image::open(raw_path)
        .map_err(|err| format!("open {} failed: {err}", raw_path.display()))?
        .to_rgba8();
    let resized = image::imageops::resize(&image, FRAME, FRAME, FilterType::Nearest);
    let alpha = remove_gray_background(&resized);
    save_image(transparent_path, &alpha)
}

fn write_contact_sheet(pack: &GameAssetPack, root: &Path) -> Result<PathBuf, String> {
    let thumbs = collect_contact_images(pack);
    let cols = 6;
    let rows = ((thumbs.len() as u32).saturating_add(cols - 1)).max(1) / cols;
    let mut sheet = RgbaImage::from_pixel(cols * 120, rows * 120, Rgba([245, 247, 242, 255]));
    for (index, rel_path) in thumbs.iter().enumerate() {
        let img = image::open(root.join(rel_path))
            .map_err(|err| format!("open contact image {rel_path} failed: {err}"))?
            .to_rgba8();
        let thumb = image::imageops::resize(&img, 96, 96, FilterType::Nearest);
        let col = index as u32 % cols;
        let row = index as u32 / cols;
        overlay(&mut sheet, &thumb, col * 120 + 12, row * 120 + 12);
    }
    let path = root.join(&pack.outputs.contact_sheet);
    save_image(&path, &sheet)?;
    Ok(path)
}

fn write_preview(pack: &GameAssetPack, root: &Path) -> Result<PathBuf, String> {
    let mut cards = String::new();
    for asset in &pack.assets {
        cards.push_str(&format!(
            r#"<article class="card"><img src="../{}" alt=""><h2>{}</h2><p>{}</p></article>"#,
            asset.transparent_path, asset.name, asset.notes
        ));
    }
    let mut actions = String::new();
    for asset in &pack.assets {
        for action in &asset.actions {
            actions.push_str(&format!(
                r#"<figure><img src="../{}" alt=""><figcaption>{} / {}</figcaption></figure>"#,
                action.spritesheet_path, asset.name, action.name
            ));
        }
    }
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
body {{ margin: 0; font: 14px system-ui, sans-serif; background: #f5f7f2; color: #172019; }}
main {{ max-width: 1120px; margin: 0 auto; padding: 24px; }}
header {{ display: flex; justify-content: space-between; gap: 16px; align-items: end; }}
h1 {{ margin: 0; font-size: 26px; }}
.meta {{ color: #526156; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: 12px; }}
.card, figure {{ margin: 0; border: 1px solid #cad7cc; background: white; border-radius: 8px; padding: 12px; }}
.card img {{ width: 100%; aspect-ratio: 1; object-fit: contain; background: #edf2ea; }}
figure img {{ width: 100%; image-rendering: pixelated; background: #edf2ea; }}
figcaption, p {{ color: #526156; }}
</style>
</head>
<body>
<main>
<header><div><h1>{title}</h1><div class="meta">{count} assets · {frames} frames · {mode}</div></div><a href="../pack.json">pack.json</a></header>
<h2>Assets</h2>
<section class="grid">{cards}</section>
<h2>Spritesheets</h2>
<section class="grid">{actions}</section>
</main>
</body>
</html>
"#,
        title = pack.title,
        count = pack.assets.len(),
        frames = pack.frame_count(),
        mode = pack.mode,
    );
    let path = root.join(&pack.outputs.preview_html);
    write_text(&path, &html)?;
    Ok(path)
}

fn collect_contact_images(pack: &GameAssetPack) -> Vec<String> {
    let mut images = Vec::new();
    for asset in &pack.assets {
        images.push(asset.transparent_path.clone());
        for action in &asset.actions {
            images.extend(action.frame_paths.iter().cloned());
        }
    }
    images
}

fn render_fixture(visual: FixtureVisual) -> RgbaImage {
    let width = FRAME * visual.frames.max(1);
    let mut image = RgbaImage::from_pixel(
        width,
        FRAME,
        if visual.transparent { TRANSPARENT } else { BG },
    );
    for frame in 0..visual.frames.max(1) {
        draw_subject(&mut image, visual.kind, visual.action, frame, frame * FRAME);
    }
    image
}

fn draw_subject(
    image: &mut RgbaImage,
    kind: FixtureKind,
    action: FixtureAction,
    frame: u32,
    offset_x: u32,
) {
    match kind {
        FixtureKind::Hero => draw_hero(image, action, frame, offset_x),
        FixtureKind::Enemy => draw_enemy(image, frame, offset_x),
        FixtureKind::Blade => draw_blade(image, offset_x),
        FixtureKind::Herb => draw_herb(image, offset_x),
        FixtureKind::Chest => draw_chest(image, offset_x),
    }
}

fn draw_hero(image: &mut RgbaImage, action: FixtureAction, frame: u32, ox: u32) {
    let bob = match action {
        FixtureAction::Idle => frame % 2,
        FixtureAction::Run => (frame % 2) * 3,
        FixtureAction::Attack => 0,
        FixtureAction::Anchor | FixtureAction::Loop => 1,
    };
    let cx = ox
        + 80
        + if matches!(action, FixtureAction::Run) {
            frame * 2
        } else {
            0
        };
    let cy = 82 - bob;
    draw_circle(image, cx, cy - 42, 16, Rgba([233, 202, 144, 255]));
    draw_rect(image, cx - 20, cy - 25, 40, 50, Rgba([56, 112, 69, 255]));
    draw_rect(image, cx - 28, cy - 17, 56, 28, Rgba([35, 82, 55, 255]));
    draw_rect(image, cx - 8, cy + 25, 8, 34, Rgba([67, 54, 45, 255]));
    draw_rect(image, cx + 6, cy + 25, 8, 34, Rgba([67, 54, 45, 255]));
    draw_circle(image, cx - 6, cy - 45, 3, Rgba([22, 35, 30, 255]));
    draw_circle(image, cx + 8, cy - 45, 3, Rgba([22, 35, 30, 255]));
    if matches!(action, FixtureAction::Attack) {
        let reach = 24 + frame * 10;
        draw_rect(
            image,
            cx + 16,
            cy - 28,
            reach,
            5,
            Rgba([205, 220, 194, 255]),
        );
        draw_rect(image, cx + 12, cy - 32, 10, 13, Rgba([89, 63, 42, 255]));
    } else {
        draw_rect(image, cx + 15, cy - 24, 7, 34, Rgba([126, 92, 54, 255]));
        draw_rect(image, cx + 20, cy - 30, 5, 42, Rgba([202, 218, 195, 255]));
    }
}

fn draw_enemy(image: &mut RgbaImage, frame: u32, ox: u32) {
    let cx = ox + 80;
    let cy = 90 + (frame % 2) * 2;
    draw_circle(image, cx, cy - 18, 34, Rgba([73, 91, 58, 255]));
    draw_rect(image, cx - 24, cy - 4, 48, 38, Rgba([85, 67, 50, 255]));
    draw_circle(image, cx - 10, cy - 22, 4, Rgba([150, 245, 128, 255]));
    draw_circle(image, cx + 10, cy - 22, 4, Rgba([150, 245, 128, 255]));
    draw_rect(image, cx - 34, cy - 42, 12, 34, Rgba([62, 76, 47, 255]));
    draw_rect(image, cx + 22, cy - 42, 12, 34, Rgba([62, 76, 47, 255]));
    draw_rect(image, cx - 20, cy + 28, 9, 28, Rgba([73, 54, 39, 255]));
    draw_rect(image, cx + 12, cy + 28, 9, 28, Rgba([73, 54, 39, 255]));
}

fn draw_blade(image: &mut RgbaImage, ox: u32) {
    for i in 0..58 {
        draw_rect(
            image,
            ox + 54 + i,
            42 + i / 2,
            5,
            5,
            Rgba([200, 218, 206, 255]),
        );
    }
    draw_rect(image, ox + 48, 92, 52, 10, Rgba([52, 103, 63, 255]));
    draw_rect(image, ox + 72, 96, 10, 34, Rgba([91, 65, 42, 255]));
}

fn draw_herb(image: &mut RgbaImage, ox: u32) {
    let cx = ox + 80;
    draw_rect(image, cx - 3, 70, 6, 54, Rgba([52, 103, 63, 255]));
    draw_circle(image, cx - 18, 80, 20, Rgba([76, 159, 76, 255]));
    draw_circle(image, cx + 18, 82, 20, Rgba([92, 178, 86, 255]));
    draw_circle(image, cx, 62, 18, Rgba([104, 190, 95, 255]));
    draw_circle(image, cx + 16, 58, 5, Rgba([98, 140, 230, 255]));
}

fn draw_chest(image: &mut RgbaImage, ox: u32) {
    draw_rect(image, ox + 42, 70, 76, 48, Rgba([114, 72, 42, 255]));
    draw_rect(image, ox + 48, 60, 64, 24, Rgba([137, 91, 52, 255]));
    draw_rect(image, ox + 42, 88, 76, 8, Rgba([73, 49, 33, 255]));
    draw_rect(image, ox + 75, 82, 12, 18, Rgba([112, 210, 114, 255]));
    draw_rect(image, ox + 38, 66, 84, 6, Rgba([184, 151, 82, 255]));
}

fn horizontal_sheet(frames: &[RgbaImage]) -> RgbaImage {
    let width = FRAME * frames.len().max(1) as u32;
    let mut sheet = RgbaImage::from_pixel(width, FRAME, TRANSPARENT);
    for (index, frame) in frames.iter().enumerate() {
        overlay(&mut sheet, frame, index as u32 * FRAME, 0);
    }
    sheet
}

fn remove_gray_background(image: &RgbaImage) -> RgbaImage {
    let mut out = image.clone();
    for pixel in out.pixels_mut() {
        let [r, g, b, _a] = pixel.0;
        if r.abs_diff(224) <= 3 && g.abs_diff(224) <= 3 && b.abs_diff(224) <= 3 {
            *pixel = TRANSPARENT;
        }
    }
    out
}

fn overlay(base: &mut RgbaImage, overlay: &RgbaImage, x: u32, y: u32) {
    for oy in 0..overlay.height() {
        for ox in 0..overlay.width() {
            let px = overlay.get_pixel(ox, oy);
            if px.0[3] == 0 {
                continue;
            }
            let tx = x + ox;
            let ty = y + oy;
            if tx < base.width() && ty < base.height() {
                base.put_pixel(tx, ty, *px);
            }
        }
    }
}

fn draw_rect(image: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    let max_x = x.saturating_add(w).min(image.width());
    let max_y = y.saturating_add(h).min(image.height());
    for py in y..max_y {
        for px in x..max_x {
            image.put_pixel(px, py, color);
        }
    }
}

fn draw_circle(image: &mut RgbaImage, cx: u32, cy: u32, radius: u32, color: Rgba<u8>) {
    let start_x = cx.saturating_sub(radius);
    let end_x = cx
        .saturating_add(radius)
        .min(image.width().saturating_sub(1));
    let start_y = cy.saturating_sub(radius);
    let end_y = cy
        .saturating_add(radius)
        .min(image.height().saturating_sub(1));
    let r2 = (radius * radius) as i64;
    for y in start_y..=end_y {
        for x in start_x..=end_x {
            let dx = x as i64 - cx as i64;
            let dy = y as i64 - cy as i64;
            if dx * dx + dy * dy <= r2 {
                image.put_pixel(x, y, color);
            }
        }
    }
}

fn check_file(root: &Path, rel: &str, missing: &mut Vec<String>) {
    if rel.trim().is_empty() || !root.join(rel).is_file() {
        missing.push(rel.to_string());
    }
}

fn save_image(path: &Path, image: &RgbaImage) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    image
        .save(path)
        .map_err(|err| format!("save {} failed: {err}", path.display()))
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn copy_file(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<(), String> {
    let to = to.as_ref();
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::copy(from.as_ref(), to)
        .map(|_| ())
        .map_err(|err| format!("copy {} failed: {err}", from.as_ref().display()))
}
