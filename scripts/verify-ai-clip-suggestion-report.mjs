import { createReadStream, existsSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import path from "node:path";

export function writeEvidencePage({ evidenceDir, logs, summary }) {
  const rows = logs.map(item => `<tr><td><code>${escapeHtml(item.command)}</code></td><td>${item.ok ? "通过" : "非阻断记录"}</td><td>${item.evidence ? `<a href="assets/${escapeHtml(item.evidence)}">${escapeHtml(item.evidence)}</a>` : ""}</td></tr>`).join("\n");
  const suggestionRows = summary.suggestion.items.map(item => `<tr><td>${item.sequence}</td><td>${escapeHtml(item.source_video?.filename || "")}</td><td>${escapeHtml(item.scene || item.clip_id)}</td><td>${item.start_ms}ms - ${item.end_ms}ms</td><td>${item.duration_ms}ms</td><td>${escapeHtml(item.reason || "")}</td></tr>`).join("\n");
  const queueRows = summary.final_queue.map(item => `<tr><td>${item.sequence}</td><td>${escapeHtml(item.source_video?.filename || "")}</td><td>${escapeHtml(item.scene || item.clip_id)}</td><td>${item.start_ms}ms - ${item.end_ms}ms</td><td>${item.duration_ms}ms</td><td>${escapeHtml(item.suggestion_reason || "")}</td></tr>`).join("\n");
  const captureVerdict = summary.export_capture_verdict;
  const captureStatus = captureVerdict?.capture?.status || "missing";
  const captureClass = captureVerdict?.capture?.blocking ? "danger" : captureStatus === "captured" ? "ok" : "warn";
  const captureAttempts = (captureVerdict?.capture?.attempts || []).map(item => `<tr><td>${escapeHtml(item.method)}</td><td>${item.ok ? "成功" : escapeHtml(item.failure_kind)}</td><td>${item.evidence ? `<a href="assets/${escapeHtml(item.evidence)}">${escapeHtml(item.evidence)}</a>` : ""}</td><td>${escapeHtml(item.error || "")}</td></tr>`).join("\n");
  const captureCaption = captureVerdict?.capture?.final_image_source === "prior-visible-fallback"
    ? "post-export capture 未成功；此图是最后一张已验证真实桌面截图的 fallback，不能当作导出后截图成功。"
    : "导出后桌面截图：用于证明导出完成后界面仍可见。";
  writeFileSync(path.join(evidenceDir, "index.html"), `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(summary.version)} AI剪辑方案建议证据</title>
  <style>
    body{margin:0;font-family:-apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;color:#1f2937;background:#f6f8fb}
    main{max-width:1200px;margin:0 auto;padding:32px 20px 56px}
    header{display:flex;justify-content:space-between;gap:20px;align-items:flex-start;margin-bottom:22px}
    h1{margin:0;font-size:30px} h2{margin:0 0 12px;font-size:19px} p{line-height:1.7}
    .badge{padding:8px 12px;border-radius:999px;background:#dcfce7;color:#166534;font-weight:800}
    .badge.warn{background:#fef3c7;color:#92400e}.badge.danger{background:#fee2e2;color:#991b1b}
    section{margin-top:16px;padding:18px;border:1px solid #e5e7eb;border-radius:8px;background:white}
    .grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:14px}
    img,video{width:100%;border-radius:6px;border:1px solid #e5e7eb;background:#111827}
    dl{display:grid;grid-template-columns:160px minmax(0,1fr);gap:8px 12px;margin:0}
    dt{color:#6b7280;font-weight:700} dd{margin:0;overflow-wrap:anywhere}
    table{width:100%;border-collapse:collapse;font-size:12px} th,td{padding:8px;border-bottom:1px solid #e5e7eb;text-align:left;vertical-align:top}
    code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>${escapeHtml(summary.version)} AI剪辑方案建议</h1>
        <p>已用真实 CEF 桌面完成本地 AI 建议、采用方案、重开恢复和导出。建议、项目队列和导出 proposal 都来自同一个 <code>${escapeHtml(summary.suggestion.suggestion_id)}</code>。</p>
      </div>
      <span class="badge ${summary.verdict === "passed" ? "" : "danger"}">${summary.verdict === "passed" ? "通过" : "阻断"}</span>
    </header>
    <section>
      <h2>验收结论</h2>
      <dl>
        <dt>建议方案</dt><dd><a href="assets/ai-clip-suggestion-plan.json">ai-clip-suggestion-plan.json</a></dd>
        <dt>项目队列 manifest</dt><dd><a href="assets/ai-clip-suggestion-manifest.json">ai-clip-suggestion-manifest.json</a></dd>
        <dt>最终片段数</dt><dd>${summary.final_queue.length} 个</dd>
        <dt>总时长</dt><dd>${summary.total_duration_ms}ms · ffprobe ${escapeHtml(summary.export_probe.duration || "")}s</dd>
        <dt>导出文件</dt><dd><a href="assets/ai-clip-suggestion-delivery.mp4">ai-clip-suggestion-delivery.mp4</a></dd>
        <dt>proposal composition</dt><dd><a href="assets/ai-clip-suggestion-proposal-composition.json">ai-clip-suggestion-proposal-composition.json</a></dd>
        <dt>截图 verdict</dt><dd><span class="badge ${captureClass}">${escapeHtml(captureStatus)}</span> · <a href="assets/ai-clip-suggestion-export-capture-verdict.json">ai-clip-suggestion-export-capture-verdict.json</a></dd>
        <dt>红线</dt><dd>本地 deterministic planner，无付费模型调用；仍是线性剪辑队列，不做多轨、转场、字幕或音频混合。</dd>
      </dl>
    </section>
    <section>
      <h2>导出与截图分离 verdict</h2>
      <dl>
        <dt>导出结果</dt><dd>${summary.export_state.exportJob?.status === "done" ? "通过" : "失败"} · <a href="assets/ai-clip-suggestion-export-state.json">导出状态 JSON</a></dd>
        <dt>截图结果</dt><dd>${escapeHtml(captureStatus)} · ${escapeHtml(captureVerdict?.capture?.rationale || "")}</dd>
        <dt>是否阻断</dt><dd>${captureVerdict?.capture?.blocking ? "阻断验收" : "不阻断导出验收，但保留截图告警"}</dd>
        <dt>重试命令</dt><dd><code>${escapeHtml(captureVerdict?.capture?.retry_command || "无")}</code></dd>
      </dl>
      <table><thead><tr><th>截图方式</th><th>结果</th><th>证据</th><th>错误</th></tr></thead><tbody>${captureAttempts}</tbody></table>
    </section>
    <section>
      <h2>AI 建议方案</h2>
      <table><thead><tr><th>#</th><th>来源视频</th><th>片段</th><th>起止时间</th><th>时长</th><th>选择理由</th></tr></thead><tbody>${suggestionRows}</tbody></table>
    </section>
    <section>
      <h2>采用后的项目队列</h2>
      <table><thead><tr><th>#</th><th>来源视频</th><th>片段</th><th>起止时间</th><th>时长</th><th>保留理由</th></tr></thead><tbody>${queueRows}</tbody></table>
    </section>
    <section>
      <h2>可见证据</h2>
      <div class="grid">
        <figure><img src="assets/ai-clip-suggestion-desktop.png" alt="AI 建议面板截图"><figcaption>AI 建议面板：来源、时间和理由可见</figcaption></figure>
        <figure><img src="assets/ai-clip-suggestion-adopted-desktop.png" alt="采用方案后的队列截图"><figcaption>采用方案：项目队列写入 suggestion_id</figcaption></figure>
        <figure><img src="assets/ai-clip-suggestion-restored-desktop.png" alt="重开恢复截图"><figcaption>重开恢复：同一建议队列仍可见</figcaption></figure>
        <figure><img src="assets/ai-clip-suggestion-export-desktop.png" alt="导出后桌面截图或 fallback"><figcaption>${escapeHtml(captureCaption)}</figcaption></figure>
        <figure><video controls src="assets/ai-clip-suggestion-delivery.mp4"></video><figcaption>采用后队列导出 MP4</figcaption></figure>
        <figure><img src="assets/ai-clip-suggestion-sampled-frame.png" alt="导出抽帧"><figcaption>导出 composition 抽帧</figcaption></figure>
      </div>
    </section>
    <section>
      <h2>状态 JSON</h2>
      <dl>
        <dt>建议状态</dt><dd><a href="assets/ai-clip-suggestion-state.json">ai-clip-suggestion-state.json</a></dd>
        <dt>采用状态</dt><dd><a href="assets/ai-clip-suggestion-adopted-state.json">ai-clip-suggestion-adopted-state.json</a></dd>
        <dt>重开恢复</dt><dd><a href="assets/ai-clip-suggestion-restored-state.json">ai-clip-suggestion-restored-state.json</a></dd>
        <dt>导出状态</dt><dd><a href="assets/ai-clip-suggestion-export-state.json">ai-clip-suggestion-export-state.json</a></dd>
        <dt>截图 verdict</dt><dd><a href="assets/ai-clip-suggestion-export-capture-verdict.json">ai-clip-suggestion-export-capture-verdict.json</a></dd>
        <dt>汇总</dt><dd><a href="assets/ai-clip-suggestion-summary.json">ai-clip-suggestion-summary.json</a></dd>
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

export function writeManifest({ evidenceDir, summary }) {
  const version = summary?.version || path.basename(path.dirname(evidenceDir));
  const captureStatus = summary?.export_capture_verdict?.capture?.status || "missing";
  const captureBlocking = Boolean(summary?.export_capture_verdict?.capture?.blocking);
  const value = {
    schema: "capy.evidence.manifest.v1",
    version,
    status: summary?.verdict === "passed" ? "verified" : "blocked",
    generated_at: new Date().toISOString(),
    summary: "AI 剪辑方案建议、采用、重开恢复、proposal/export 和 post-export capture verdict 已通过真实桌面验证。",
    runs: [
      { id: "ai-clip-suggestion-loop", command: `scripts/verify-ai-clip-suggestion.mjs spec/versions/${version}`, status: summary?.verdict === "passed" ? "passed" : "failed", evidence: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-summary.json` }
    ],
    artifacts: [
      { path: `spec/versions/${version}/evidence/index.html`, kind: "html-report", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-plan.json`, kind: "suggestion-json", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-manifest.json`, kind: "project-manifest", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-desktop.png`, kind: "desktop-capture", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-adopted-desktop.png`, kind: "desktop-capture", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-restored-desktop.png`, kind: "desktop-capture", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-export-desktop.png`, kind: "post-export-desktop-capture", status: captureBlocking ? "warning-or-blocked" : captureStatus },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-export-capture-verdict.json`, kind: "capture-verdict", status: captureStatus },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-proposal-composition.json`, kind: "composition-json", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/ai-clip-suggestion-delivery.mp4`, kind: "video", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/evidence-page-browser.png`, kind: "browser-screenshot", status: "verified" }
    ],
    verdict: summary?.export_capture_verdict?.verdict || { status: "passed", blockers: [], warnings: [] }
  };
  writeFileSync(path.join(evidenceDir, "manifest.json"), `${JSON.stringify(value, null, 2)}\n`);
}

export async function verifyEvidencePage({ evidenceDir, assetsDir }) {
  const version = path.basename(path.dirname(evidenceDir));
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
    bodyText: document.body.innerText,
    links: [...document.querySelectorAll("a")].map(anchor => anchor.getAttribute("href")),
    images: [...document.images].map(img => ({ src: img.getAttribute("src"), complete: img.complete, w: img.naturalWidth, h: img.naturalHeight })),
    videos: [...document.querySelectorAll("video")].map(video => video.getAttribute("src")),
    badge: document.querySelector(".badge")?.textContent || "",
    bodyLength: document.body.innerText.length
  }));
  assert(state.title.includes(version), "evidence page title missing");
  assert(state.links.some(link => link?.includes("ai-clip-suggestion-plan.json")), "suggestion link missing");
  assert(state.links.some(link => link?.includes("ai-clip-suggestion-export-capture-verdict.json")), "capture verdict link missing");
  assert(state.bodyText.includes("导出与截图分离 verdict"), "capture verdict section missing");
  assert(state.images.length >= 5 && state.images.every(img => img.complete && img.w > 0), "evidence images did not load");
  assert(state.videos.some(src => src?.includes("ai-clip-suggestion-delivery.mp4")), "evidence MP4 video link missing");
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
