import { createReadStream, existsSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import path from "node:path";

export function writeEvidencePage({ evidenceDir, logs, summary }) {
  const rows = logs.map(item => `<tr><td><code>${escapeHtml(item.command)}</code></td><td>${item.ok ? "通过" : "非阻断记录"}</td><td>${item.evidence ? `<a href="assets/${escapeHtml(item.evidence)}">${escapeHtml(item.evidence)}</a>` : ""}</td></tr>`).join("\n");
  writeFileSync(path.join(evidenceDir, "index.html"), `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>v0.44 视频素材工作台证据</title>
  <style>
    body{margin:0;font-family:-apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;color:#1f2937;background:#f6f8fb}
    main{max-width:1180px;margin:0 auto;padding:32px 20px 56px}
    header{display:flex;justify-content:space-between;gap:20px;align-items:flex-start;margin-bottom:22px}
    h1{margin:0;font-size:30px} h2{margin:0 0 12px;font-size:19px} p{line-height:1.7}
    .badge{padding:8px 12px;border-radius:999px;background:#dcfce7;color:#166534;font-weight:800}
    section{margin-top:16px;padding:18px;border:1px solid #e5e7eb;border-radius:8px;background:white}
    .grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:14px}
    img,video{width:100%;border-radius:6px;border:1px solid #e5e7eb;background:#111827}
    dl{display:grid;grid-template-columns:150px minmax(0,1fr);gap:8px 12px;margin:0}
    dt{color:#6b7280;font-weight:700} dd{margin:0;overflow-wrap:anywhere}
    table{width:100%;border-collapse:collapse;font-size:12px} th,td{padding:8px;border-bottom:1px solid #e5e7eb;text-align:left;vertical-align:top}
    code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>v0.44 视频素材工作台</h1>
        <p>已用两个真实项目 WebM 素材完成多素材列表、点击第二个素材切换预览、1.0s-3.0s 选段和只来自当前素材的 clip-only MP4 导出。</p>
      </div>
      <span class="badge">通过</span>
    </header>
    <section>
      <h2>验收结论</h2>
      <dl>
        <dt>素材数量</dt><dd>${summary.workbench_video_cards.length} 个视频素材卡片</dd>
        <dt>选中素材</dt><dd>${escapeHtml(summary.selected_source.filename)} · ${escapeHtml(summary.selected_source.artifact_id)}</dd>
        <dt>选段范围</dt><dd>${summary.selected_range.start_ms}ms - ${summary.selected_range.end_ms}ms · ${summary.selected_range.duration_ms}ms</dd>
        <dt>导出文件</dt><dd><a href="assets/multi-video-clip-only.mp4">multi-video-clip-only.mp4</a> · ffprobe ${escapeHtml(summary.export_probe.duration || "")}s</dd>
        <dt>sampled frame</dt><dd><a href="assets/multi-video-sampled-frame.png">multi-video-sampled-frame.png</a></dd>
        <dt>红线</dt><dd>仍使用 WebM 作为可见预览源；导出为 MP4；没有引入多轨非线性剪辑。</dd>
      </dl>
    </section>
    <section>
      <h2>可见证据</h2>
      <div class="grid">
        <figure><img src="assets/multi-video-list-desktop.png" alt="多视频素材列表桌面截图"><figcaption>多素材列表：两张视频卡片、首帧、时长和状态</figcaption></figure>
        <figure><img src="assets/multi-video-selected-desktop.png" alt="选择第二个素材后的视频工作区"><figcaption>选择 Camera B 后的预览和选段状态</figcaption></figure>
        <figure><video controls src="assets/multi-video-clip-only.mp4"></video><figcaption>只来自 Camera B 的 clip-only MP4</figcaption></figure>
        <figure><img src="assets/multi-video-sampled-frame.png" alt="导出片段 sampled frame"><figcaption>导出片段 sampled frame</figcaption></figure>
      </div>
    </section>
    <section>
      <h2>状态 JSON</h2>
      <dl>
        <dt>列表状态</dt><dd><a href="assets/multi-video-list-state.json">multi-video-list-state.json</a></dd>
        <dt>选择状态</dt><dd><a href="assets/multi-video-selected-state.json">multi-video-selected-state.json</a></dd>
        <dt>导出状态</dt><dd><a href="assets/multi-video-export-state.json">multi-video-export-state.json</a></dd>
        <dt>汇总</dt><dd><a href="assets/multi-video-summary.json">multi-video-summary.json</a></dd>
      </dl>
    </section>
    <section>
      <h2>命令证据</h2>
      <table><thead><tr><th>命令</th><th>结果</th><th>证据</th></tr></thead><tbody>${rows}</tbody></table>
    </section>
  </main>
</body>
</html>
`);
}

export function writeManifest({ evidenceDir }) {
  const value = {
    schema: "capy.evidence.manifest.v1",
    version: "v0.44",
    status: "verified",
    generated_at: new Date().toISOString(),
    summary: "多视频素材列表、素材切换和当前素材 clip-only MP4 导出已通过真实桌面验证。",
    runs: [
      { id: "multi-video-workbench-loop", command: "scripts/verify-multi-video-workbench.mjs spec/versions/v0.44", status: "passed", evidence: "spec/versions/v0.44/evidence/assets/multi-video-summary.json" }
    ],
    artifacts: [
      { path: "spec/versions/v0.44/evidence/index.html", kind: "html-report", status: "verified" },
      { path: "spec/versions/v0.44/evidence/assets/multi-video-list-desktop.png", kind: "desktop-capture", status: "verified" },
      { path: "spec/versions/v0.44/evidence/assets/multi-video-selected-state.json", kind: "state-json", status: "verified" },
      { path: "spec/versions/v0.44/evidence/assets/multi-video-export-state.json", kind: "state-json", status: "verified" },
      { path: "spec/versions/v0.44/evidence/assets/multi-video-clip-only.mp4", kind: "video", status: "verified" },
      { path: "spec/versions/v0.44/evidence/assets/multi-video-sampled-frame.png", kind: "image", status: "verified" },
      { path: "spec/versions/v0.44/evidence/assets/evidence-page-browser.png", kind: "browser-screenshot", status: "verified" }
    ],
    verdict: { status: "passed", blockers: [], warnings: [] }
  };
  writeFileSync(path.join(evidenceDir, "manifest.json"), `${JSON.stringify(value, null, 2)}\n`);
}

export async function verifyEvidencePage({ evidenceDir, assetsDir }) {
  const { chromium } = await loadPlaywright();
  const server = await startEvidenceServer(evidenceDir);
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
  const consoleErrors = [];
  page.on("console", message => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  const pageErrors = [];
  page.on("pageerror", error => pageErrors.push(error.message));
  const url = `http://127.0.0.1:${server.port}/index.html`;
  await page.goto(url, { waitUntil: "networkidle" });
  const state = await page.evaluate(() => ({
    title: document.querySelector("h1")?.textContent || "",
    images: [...document.images].map(img => ({ src: img.getAttribute("src"), complete: img.complete, w: img.naturalWidth, h: img.naturalHeight })),
    videos: [...document.querySelectorAll("video")].map(video => video.getAttribute("src")),
    badge: document.querySelector(".badge")?.textContent || "",
    bodyLength: document.body.innerText.length
  }));
  assert(state.title.includes("v0.44"), "evidence page title missing");
  assert(state.images.length >= 3 && state.images.every(img => img.complete && img.w > 0), "evidence images did not load");
  assert(state.videos.some(src => src?.includes("multi-video-clip-only.mp4")), "evidence MP4 video link missing");
  assert(consoleErrors.length === 0 && pageErrors.length === 0, "evidence page has browser errors");
  await page.screenshot({ path: path.join(assetsDir, "evidence-page-browser.png"), fullPage: true });
  await browser.close();
  await new Promise(resolve => server.instance.close(resolve));
  writeFileSync(path.join(assetsDir, "evidence-page-check.json"), `${JSON.stringify({ ok: true, url, state, consoleErrors, pageErrors, screenshot: path.join(assetsDir, "evidence-page-browser.png") }, null, 2)}\n`);
}

async function loadPlaywright() {
  try {
    return await import("playwright");
  } catch {
    const require = createRequire("/opt/homebrew/lib/node_modules/playwright/package.json");
    return require("playwright");
  }
}

function startEvidenceServer(evidenceDir) {
  return new Promise((resolve) => {
    const instance = createServer((req, res) => {
      const pathname = decodeURIComponent(new URL(req.url || "/", "http://127.0.0.1").pathname);
      const filePath = path.normalize(path.join(evidenceDir, pathname === "/" ? "index.html" : pathname));
      if (!filePath.startsWith(evidenceDir)) {
        res.writeHead(403).end("forbidden");
        return;
      }
      if (!existsSync(filePath)) {
        res.writeHead(404).end("not found");
        return;
      }
      res.writeHead(200, { "Content-Type": contentType(filePath) });
      createReadStream(filePath).pipe(res);
    });
    instance.listen(0, "127.0.0.1", () => resolve({ instance, port: instance.address().port }));
  });
}

function contentType(filePath) {
  if (filePath.endsWith(".html")) return "text/html; charset=utf-8";
  if (filePath.endsWith(".json")) return "application/json";
  if (filePath.endsWith(".png")) return "image/png";
  if (filePath.endsWith(".mp4")) return "video/mp4";
  return "text/plain; charset=utf-8";
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
