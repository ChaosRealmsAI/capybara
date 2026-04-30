use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::model::{PackagePaths, SourceMeta, rel_path};

pub(super) fn write_preview(paths: &PackagePaths, report: &Value) -> Result<PathBuf, String> {
    let mut frames = fs::read_dir(&paths.rgba_frames_dir)
        .map_err(|err| format!("read {} failed: {err}", paths.rgba_frames_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
        .collect::<Vec<_>>();
    frames.sort();
    let frame_js = frames
        .iter()
        .map(|path| format!("\"../{}\"", rel_path(&paths.root, path)))
        .collect::<Vec<_>>()
        .join(",");
    let verdict = report
        .get("verdict")
        .and_then(Value::as_str)
        .unwrap_or("draft");
    let metrics = report.get("metrics").cloned().unwrap_or(Value::Null);
    let metrics_text = serde_json::to_string_pretty(&metrics).map_err(|err| err.to_string())?;
    let html = format!(
        r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>动态抠像预览</title>
<style>
body {{ margin: 0; font-family: "PingFang SC", system-ui, sans-serif; background: #fffaf0; color: #1c1917; }}
main {{ max-width: 1180px; margin: 0 auto; padding: 24px; }}
header {{ display: flex; justify-content: space-between; gap: 16px; align-items: end; flex-wrap: wrap; }}
h1 {{ margin: 0; font-size: clamp(28px, 5vw, 52px); line-height: 1; }}
.stage {{ margin-top: 18px; min-height: 560px; border: 1px solid rgba(28,25,23,.14); border-radius: 24px; display: grid; place-items: center; overflow: hidden; background: #08111f; }}
.stage.light {{ background: #ffffff; }}
.stage.photo {{ background: linear-gradient(135deg, #30485f, #d9c3a5 55%, #41576b); }}
.stage.game {{ background: linear-gradient(#8fd1ff 0 48%, #79b660 48% 100%); }}
.stage img {{ max-width: min(92vw, 880px); width: 75%; height: auto; object-fit: contain; }}
.controls {{ display: flex; gap: 8px; flex-wrap: wrap; margin: 16px 0; }}
button {{ border: 1px solid rgba(28,25,23,.16); background: white; border-radius: 999px; padding: 10px 14px; cursor: pointer; }}
button[aria-pressed="true"] {{ background: #ede7ff; color: #5b21b6; font-weight: 700; }}
.grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }}
.card {{ border: 1px solid rgba(28,25,23,.14); background: rgba(255,255,255,.7); border-radius: 18px; padding: 16px; }}
pre {{ white-space: pre-wrap; overflow-wrap: anywhere; font: 12px "SF Mono", monospace; }}
@media (max-width: 760px) {{ .grid {{ grid-template-columns: 1fr; }} .stage {{ min-height: 420px; }} .stage img {{ width: 92%; }} }}
</style>
</head>
<body>
<main>
<header><div><h1>动态透明资产预览</h1><p>黑、白、照片、游戏背景都要看，不能只看浅色棚拍底。</p></div><strong id="verdict">verdict: {verdict}</strong></header>
<div class="controls" aria-label="背景和播放控制">
<button data-bg="deep" aria-pressed="true">深色</button>
<button data-bg="light" aria-pressed="false">白色</button>
<button data-bg="photo" aria-pressed="false">照片感</button>
<button data-bg="game" aria-pressed="false">游戏场景</button>
<button id="play" aria-pressed="true">播放</button>
</div>
<section id="stage" class="stage" data-testid="motion-stage"><img id="frame" alt="透明人物动画帧"></section>
<section class="grid">
<div class="card"><h2>质量指标</h2><pre>{metrics_text}</pre></div>
<div class="card"><h2>导出文件</h2><p><a href="../manifest.json">manifest.json</a> · <a href="../atlas/walk.png">atlas/walk.png</a> · <a href="../video/preview.webm">preview.webm</a></p></div>
</section>
</main>
<script>
const frames = [{frame_js}];
let index = 0;
let playing = true;
const img = document.querySelector("#frame");
const stage = document.querySelector("#stage");
function draw() {{
  if (frames.length > 0) img.src = frames[index % frames.length];
  document.documentElement.dataset.frame = String(index % Math.max(frames.length, 1));
  if (playing) index += 1;
}}
setInterval(draw, 1000 / 24);
draw();
document.querySelectorAll("[data-bg]").forEach((button) => {{
  button.addEventListener("click", () => {{
    document.querySelectorAll("[data-bg]").forEach((item) => item.setAttribute("aria-pressed", String(item === button)));
    stage.className = "stage" + (button.dataset.bg === "deep" ? "" : " " + button.dataset.bg);
    document.documentElement.dataset.background = button.dataset.bg;
  }});
}});
document.querySelector("#play").addEventListener("click", (event) => {{
  playing = !playing;
  event.currentTarget.setAttribute("aria-pressed", String(playing));
  event.currentTarget.textContent = playing ? "播放" : "暂停";
}});
document.documentElement.dataset.background = "deep";
</script>
</body>
</html>
"##
    );
    let path = paths.qa_dir.join("preview.html");
    write_text(&path, &html)?;
    Ok(path)
}

pub(super) fn write_evidence_index(
    index_path: &Path,
    package_root: &Path,
    source: &Path,
    meta: &SourceMeta,
    report: &Value,
    command_json: &Value,
) -> Result<(), String> {
    let index_dir = index_path.parent().unwrap_or_else(|| Path::new("."));
    let package_rel = package_root
        .strip_prefix(index_dir)
        .unwrap_or(package_root)
        .to_string_lossy()
        .replace('\\', "/");
    let verdict = report
        .get("verdict")
        .and_then(Value::as_str)
        .unwrap_or("draft");
    let metrics = report.get("metrics").cloned().unwrap_or(Value::Null);
    let warnings = report.get("warnings").cloned().unwrap_or(Value::Null);
    let notes = report.get("notes").cloned().unwrap_or(Value::Null);
    let metrics_text = serde_json::to_string_pretty(&metrics).map_err(|err| err.to_string())?;
    let warnings_text = serde_json::to_string_pretty(&warnings).map_err(|err| err.to_string())?;
    let notes_text = serde_json::to_string_pretty(&notes).map_err(|err| err.to_string())?;
    let command_text = serde_json::to_string_pretty(command_json).map_err(|err| err.to_string())?;
    let html = format!(
        r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>v0.32 动态抠像验收证据</title>
<style>
body {{ margin: 0; background: #fffaf0; color: #1c1917; font-family: "PingFang SC", system-ui, sans-serif; }}
main {{ max-width: 1180px; margin: 0 auto; padding: 28px; }}
h1 {{ margin: 0 0 8px; font-size: clamp(30px, 5vw, 58px); line-height: 1; }}
.hero, .card {{ border: 1px solid rgba(28,25,23,.14); border-radius: 22px; background: rgba(255,255,255,.72); padding: 18px; margin: 16px 0; }}
.hero {{ display: grid; grid-template-columns: 1fr 1fr; gap: 18px; align-items: start; }}
img {{ width: 100%; border-radius: 16px; background: #061225; }}
pre {{ white-space: pre-wrap; overflow-wrap: anywhere; font: 12px "SF Mono", monospace; }}
.verdict {{ display: inline-block; padding: 8px 12px; border-radius: 999px; background: #e9f9ee; color: #116236; font-weight: 800; }}
@media (max-width: 760px) {{ .hero {{ grid-template-columns: 1fr; }} }}
</style>
</head>
<body>
<main>
<p class="verdict">verdict: {verdict}</p>
<h1>v0.32 动态抠像验收证据</h1>
<p>源视频：{source}</p>
<section class="hero">
  <div><h2>源视频 storyboard</h2><img src="{package_rel}/source/contact.jpg" alt="源视频关键帧"></div>
  <div><h2>抠像抽样</h2><img src="{package_rel}/qa/contact-deep.png" alt="深色背景抽样抠像"></div>
</section>
<section class="card">
  <h2>真实预览入口</h2>
  <p><a href="{package_rel}/qa/preview.html">打开多背景动态预览</a> · <a href="{package_rel}/manifest.json">manifest.json</a> · <a href="{package_rel}/qa/report.json">QA report</a></p>
  <p>{width}x{height} · {fps:.3}fps · {frames} frames · {duration:.3}s</p>
</section>
<section class="card"><h2>质量指标</h2><pre>{metrics_text}</pre></section>
<section class="card"><h2>警告</h2><pre>{warnings_text}</pre></section>
<section class="card"><h2>说明</h2><pre>{notes_text}</pre></section>
<section class="card"><h2>命令结果</h2><pre>{command_text}</pre></section>
</main>
</body>
</html>
"##,
        source = source.display(),
        width = meta.width,
        height = meta.height,
        fps = meta.fps,
        frames = meta.frame_count,
        duration = meta.duration_sec,
    );
    write_text(index_path, &html)
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
}
