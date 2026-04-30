use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use super::CutoutArgs;
use super::model::{self, PackagePaths, SourceMeta};

pub(super) fn prepare_output_dir(out: &Path, overwrite: bool) -> Result<(), String> {
    if out.exists() {
        if !overwrite {
            return Err(format!(
                "{} already exists; pass --overwrite",
                out.display()
            ));
        }
        fs::remove_dir_all(out).map_err(|err| format!("remove {} failed: {err}", out.display()))?;
    }
    fs::create_dir_all(out).map_err(|err| format!("create {} failed: {err}", out.display()))
}

pub(super) fn probe_source(input: &Path, paths: &PackagePaths) -> Result<SourceMeta, String> {
    let output = run_output(
        Command::new("ffprobe")
            .args([
                "-hide_banner",
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(input),
        "ffprobe source",
    )?;
    let value: Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("parse ffprobe JSON failed: {err}"))?;
    write_bytes(&paths.logs_dir.join("ffprobe.json"), &output.stdout)?;
    write_json(&paths.source_dir.join("metadata.json"), &value)?;
    source_meta(&value)
}

pub(super) fn extract_frames(args: &CutoutArgs, paths: &PackagePaths) -> Result<(), String> {
    let pattern = paths.source_frames_dir.join("frame_%06d.png");
    let mut command = Command::new("ffmpeg");
    command
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(&args.input);
    if let Some(max_frames) = args.max_frames {
        command.args(["-vframes", &max_frames.to_string()]);
    }
    command.arg("-vsync").arg("0").arg(pattern);
    run_logged(
        &mut command,
        &paths.logs_dir.join("extract-frames.log"),
        "extract source frames",
    )
}

pub(super) fn write_source_contact(input: &Path, paths: &PackagePaths) -> Result<(), String> {
    let mut command = Command::new("ffmpeg");
    command
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-vf", "fps=1,scale=320:-1,tile=4x2", "-frames:v", "1"])
        .arg(paths.source_dir.join("contact.jpg"));
    run_logged(
        &mut command,
        &paths.logs_dir.join("source-contact.log"),
        "write source contact sheet",
    )
}

pub(super) fn normalize_frame_count(source_frames_dir: &Path) -> Result<u32, String> {
    let count = fs::read_dir(source_frames_dir)
        .map_err(|err| format!("read {} failed: {err}", source_frames_dir.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("png"))
        .count();
    if count == 0 {
        return Err("ffmpeg extracted zero frames".to_string());
    }
    Ok(count as u32)
}

pub(super) fn write_cutout_manifest(
    paths: &PackagePaths,
    frame_count: u32,
) -> Result<PathBuf, String> {
    let items = (1..=frame_count)
        .map(|index| {
            json!({
                "key": format!("frame_{index:06}"),
                "label": format!("Frame {index:06}"),
                "source": paths.source_frames_dir.join(format!("frame_{index:06}.png"))
            })
        })
        .collect::<Vec<_>>();
    let manifest = json!({
        "schema": "capy.cutout.batch.v1",
        "items": items
    });
    let path = paths.tmp_dir.join("cutout-batch.json");
    write_json(&path, &manifest)?;
    Ok(path)
}

pub(super) fn run_cutout_batch(
    args: &CutoutArgs,
    paths: &PackagePaths,
    manifest: &Path,
) -> Result<Value, String> {
    let mut command_args = vec![
        "cutout".to_string(),
        "batch".to_string(),
        "--manifest".to_string(),
        manifest.display().to_string(),
        "--out-dir".to_string(),
        paths.tmp_dir.join("focus").display().to_string(),
        "--report".to_string(),
        paths
            .logs_dir
            .join("cutout-batch-report.json")
            .display()
            .to_string(),
        "--mask-max-side".to_string(),
        args.mask_max_side.to_string(),
    ];
    if args.full_res_mask {
        command_args.push("--full-res-mask".to_string());
    }
    let refs = command_args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_self_output(&refs)?;
    write_bytes(
        &paths.logs_dir.join("cutout-batch.stdout.json"),
        &output.stdout,
    )?;
    write_bytes(
        &paths.logs_dir.join("cutout-batch.stderr.log"),
        &output.stderr,
    )?;
    serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("parse cutout batch JSON failed: {err}"))
}

pub(super) fn normalize_cutout_outputs(
    paths: &PackagePaths,
    frame_count: u32,
) -> Result<(), String> {
    for index in 1..=frame_count {
        let key = format!("frame_{index:06}");
        copy_file(
            &paths
                .tmp_dir
                .join("focus/outputs")
                .join(format!("{key}-focus.png")),
            &paths.rgba_frames_dir.join(format!("{key}.png")),
        )?;
        copy_file(
            &paths
                .tmp_dir
                .join("focus/masks")
                .join(format!("{key}-mask.png")),
            &paths.masks_dir.join(format!("{key}.png")),
        )?;
    }
    Ok(())
}

pub(super) fn export_videos(paths: &PackagePaths, meta: &SourceMeta) -> Result<(), String> {
    let fps = format!("{:.6}", meta.fps.max(1.0));
    run_logged(
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-framerate",
                &fps,
                "-i",
            ])
            .arg(paths.rgba_frames_dir.join("frame_%06d.png"))
            .args([
                "-c:v",
                "libvpx-vp9",
                "-pix_fmt",
                "yuva420p",
                "-auto-alt-ref",
                "0",
                "-crf",
                "20",
                "-b:v",
                "0",
                "-y",
            ])
            .arg(paths.video_dir.join("preview.webm")),
        &paths.logs_dir.join("export-preview-webm.log"),
        "export WebM alpha preview",
    )?;
    run_logged(
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-framerate",
                &fps,
                "-i",
            ])
            .arg(paths.source_frames_dir.join("frame_%06d.png"))
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-y"])
            .arg(paths.video_dir.join("rgb.mp4")),
        &paths.logs_dir.join("export-rgb-mp4.log"),
        "export RGB MP4",
    )?;
    run_logged(
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-framerate",
                &fps,
                "-i",
            ])
            .arg(paths.masks_dir.join("frame_%06d.png"))
            .args([
                "-vf",
                "format=gray",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-crf",
                "18",
                "-y",
            ])
            .arg(paths.video_dir.join("alpha.mp4")),
        &paths.logs_dir.join("export-alpha-mp4.log"),
        "export alpha-mask MP4",
    )
}

pub(super) fn verify_manifest(manifest_path: &Path) -> Result<Value, String> {
    let root = manifest_path
        .parent()
        .ok_or_else(|| format!("{} has no parent", manifest_path.display()))?;
    let manifest = read_json(manifest_path)?;
    let report = read_json(&root.join("qa/report.json"))?;
    let mut missing = Vec::new();
    for rel in [
        "manifest.json",
        "qa/report.json",
        "qa/preview.html",
        "qa/contact-deep.png",
        "atlas/walk.png",
        "atlas/walk.json",
        "video/preview.webm",
        "video/rgb.mp4",
        "video/alpha.mp4",
    ] {
        if !root.join(rel).is_file() {
            missing.push(rel.to_string());
        }
    }
    let frame_count = manifest
        .pointer("/source/frame_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let rgba_count = count_pngs(&root.join("frames/rgba"));
    let mask_count = count_pngs(&root.join("masks"));
    let passed = missing.is_empty()
        && frame_count > 0
        && rgba_count == frame_count
        && mask_count == frame_count
        && manifest.get("schema").and_then(Value::as_str) == Some(model::MANIFEST_SCHEMA)
        && report.get("schema").and_then(Value::as_str) == Some(model::QA_SCHEMA);
    Ok(json!({
        "schema": "capy.motion.verify.v1",
        "verdict": if passed { "passed" } else { "failed" },
        "manifest": manifest_path,
        "missing": missing,
        "frame_count": frame_count,
        "rgba_frames": rgba_count,
        "masks": mask_count,
        "qa_verdict": report.get("verdict").cloned().unwrap_or(Value::Null),
        "preview_html": root.join("qa/preview.html")
    }))
}

pub(super) fn command_available(program: &str, args: &[&str]) -> Value {
    match Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(output) => json!({ "ok": output.status.success(), "program": program }),
        Err(err) => json!({ "ok": false, "program": program, "error": err.to_string() }),
    }
}

pub(super) fn run_self_json(args: &[&str]) -> Result<Value, String> {
    let output = run_self_output(args)?;
    serde_json::from_slice(&output.stdout).map_err(|err| format!("parse capy JSON failed: {err}"))
}

pub(super) fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    fs::write(path, format!("{text}\n"))
        .map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn source_meta(value: &Value) -> Result<SourceMeta, String> {
    let streams = value
        .get("streams")
        .and_then(Value::as_array)
        .ok_or("ffprobe JSON missing streams")?;
    let video = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))
        .ok_or("ffprobe JSON has no video stream")?;
    let audio = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));
    let fps = parse_rate(
        video
            .get("avg_frame_rate")
            .and_then(Value::as_str)
            .unwrap_or("0/1"),
    );
    let duration_sec = video
        .get("duration")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<f64>().ok())
        .or_else(|| {
            value
                .pointer("/format/duration")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<f64>().ok())
        })
        .unwrap_or(0.0);
    let frame_count = video
        .get("nb_frames")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(|| (duration_sec * fps).round() as u32);
    Ok(SourceMeta {
        width: video.get("width").and_then(Value::as_u64).unwrap_or(0) as u32,
        height: video.get("height").and_then(Value::as_u64).unwrap_or(0) as u32,
        fps,
        duration_sec,
        frame_count,
        video_codec: video
            .get("codec_name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        audio_codec: audio
            .and_then(|stream| stream.get("codec_name"))
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn count_pngs(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("png"))
        .count() as u64
}

fn parse_rate(value: &str) -> f64 {
    if let Some((num, den)) = value.split_once('/') {
        let num = num.parse::<f64>().unwrap_or(0.0);
        let den = den.parse::<f64>().unwrap_or(1.0);
        if den == 0.0 { 0.0 } else { num / den }
    } else {
        value.parse::<f64>().unwrap_or(0.0)
    }
}

fn run_logged(command: &mut Command, log_path: &Path, label: &str) -> Result<(), String> {
    let output = run_output(command, label)?;
    let mut log = Vec::new();
    log.extend_from_slice(b"stdout:\n");
    log.extend_from_slice(&output.stdout);
    log.extend_from_slice(b"\nstderr:\n");
    log.extend_from_slice(&output.stderr);
    write_bytes(log_path, &log)
}

fn run_output(command: &mut Command, label: &str) -> Result<std::process::Output, String> {
    let output = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("{label} failed to start: {err}"))?;
    if output.status.success() {
        return Ok(output);
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(format!(
        "{label} failed: {}",
        if stderr.is_empty() { stdout } else { stderr }
    ))
}

fn run_self_output(args: &[&str]) -> Result<std::process::Output, String> {
    let exe =
        std::env::current_exe().map_err(|err| format!("resolve current capy exe failed: {err}"))?;
    run_output(Command::new(exe).args(args), "run capy subcommand")
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {} failed: {err}", path.display()))
}

fn write_bytes(path: &Path, value: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::write(path, value).map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|err| format!("copy {} to {} failed: {err}", from.display(), to.display()))
}
