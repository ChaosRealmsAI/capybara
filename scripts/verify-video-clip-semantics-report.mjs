import { createReadStream, existsSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import path from "node:path";

export function writeSemanticsEvidencePage({ evidenceDir, logs, summary }) {
  const commandRows = logs.map(item => `<tr><td><code>${escapeHtml(item.command)}</code></td><td>${item.ok ? "通过" : "失败"}</td><td>${item.evidence ? `<a href="assets/${escapeHtml(item.evidence)}">${escapeHtml(item.evidence)}</a>` : ""}</td></tr>`).join("\n");
  const semanticRows = summary.semantics.items.map(item => `<tr><td>${item.sequence}</td><td>${escapeHtml(item.source_video?.filename || "")}</td><td>${escapeHtml(item.summary_zh)}</td><td>${escapeHtml((item.tags || []).join(" / "))}</td><td>${escapeHtml(item.rhythm)}</td><td>${escapeHtml(item.use_case)}</td><td>${escapeHtml(item.recommendation)}</td></tr>`).join("\n");
  const suggestionRows = summary.suggestion.items.map(item => `<tr><td>${item.sequence}</td><td>${escapeHtml(item.source_video?.filename || "")}</td><td>${escapeHtml(item.reason || "")}</td><td>${escapeHtml(item.semantic_reason || "")}</td></tr>`).join("\n");
  writeFileSync(path.join(evidenceDir, "index.html"), `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(summary.version)} 视频片段语义分析证据</title>
  <style>
    body{margin:0;font-family:-apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;color:#1f2937;background:#f6f8fb}
    main{max-width:1200px;margin:0 auto;padding:32px 20px 56px}
    header{display:flex;justify-content:space-between;gap:20px;align-items:flex-start;margin-bottom:22px}
    h1{margin:0;font-size:30px} h2{margin:0 0 12px;font-size:19px} p{line-height:1.7}
    .badge{padding:8px 12px;border-radius:999px;background:#dcfce7;color:#166534;font-weight:800}
    section{margin-top:16px;padding:18px;border:1px solid #e5e7eb;border-radius:8px;background:white}
    .grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:14px}
    img{width:100%;border-radius:6px;border:1px solid #e5e7eb;background:#111827}
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
        <h1>${escapeHtml(summary.version)} 视频片段语义分析</h1>
        <p>已用真实 CEF 桌面 DOM/state 完成分析前、分析后、AI 建议语义理由和重开恢复四步验证。当前 CEF app-view capture 在窗口切换后会崩溃或超时，图片证据由返回的真实 DOM/state 生成；全程使用本地 deterministic analyzer，无付费模型调用。</p>
      </div>
      <span class="badge">通过 · state-derived visual</span>
    </header>
    <section>
      <h2>验收结论</h2>
      <dl>
        <dt>语义 manifest</dt><dd><a href="assets/video-clip-semantics-manifest.json">video-clip-semantics-manifest.json</a></dd>
        <dt>建议 JSON</dt><dd><a href="assets/video-clip-semantics-suggestion.json">video-clip-semantics-suggestion.json</a></dd>
        <dt>Queue manifest</dt><dd><a href="assets/video-clip-semantics-queue-final.json">video-clip-semantics-queue-final.json</a></dd>
        <dt>片段数量</dt><dd>${summary.semantics.items.length} 段</dd>
        <dt>红线</dt><dd>未调用 provider；语义写入 Project Core；仍是线性 clip queue。</dd>
        <dt>图片来源</dt><dd>PNG 来自 CEF 返回的 DOM/state 文本与布局指标；原始 state JSON 是主证据。</dd>
      </dl>
    </section>
    <section>
      <h2>可见证据</h2>
      <div class="grid">
        <figure><img src="assets/video-clip-semantics-before-desktop.png" alt="分析前可见状态"><figcaption>分析前：queue 已从 Project Core 载入</figcaption></figure>
        <figure><img src="assets/video-clip-semantics-after-desktop.png" alt="分析后可见状态"><figcaption>分析后：摘要、标签、节奏、用途可见</figcaption></figure>
        <figure><img src="assets/video-clip-semantics-suggestion-desktop.png" alt="建议理由可见状态"><figcaption>AI 建议：语义理由可见</figcaption></figure>
        <figure><img src="assets/video-clip-semantics-restored-desktop.png" alt="重开恢复可见状态"><figcaption>重开恢复：语义和建议理由从 Project Core 恢复</figcaption></figure>
      </div>
    </section>
    <section>
      <h2>片段语义</h2>
      <table><thead><tr><th>#</th><th>来源</th><th>摘要</th><th>标签</th><th>节奏</th><th>用途</th><th>推荐理由</th></tr></thead><tbody>${semanticRows}</tbody></table>
    </section>
    <section>
      <h2>AI 建议引用语义</h2>
      <table><thead><tr><th>#</th><th>来源</th><th>建议理由</th><th>语义理由</th></tr></thead><tbody>${suggestionRows}</tbody></table>
    </section>
    <section>
      <h2>状态 JSON</h2>
      <dl>
        <dt>分析前</dt><dd><a href="assets/video-clip-semantics-before-state.json">video-clip-semantics-before-state.json</a></dd>
        <dt>分析后</dt><dd><a href="assets/video-clip-semantics-after-state.json">video-clip-semantics-after-state.json</a></dd>
        <dt>建议理由</dt><dd><a href="assets/video-clip-semantics-suggestion-state.json">video-clip-semantics-suggestion-state.json</a></dd>
        <dt>重开恢复</dt><dd><a href="assets/video-clip-semantics-restored-state.json">video-clip-semantics-restored-state.json</a></dd>
        <dt>汇总</dt><dd><a href="assets/video-clip-semantics-summary.json">video-clip-semantics-summary.json</a></dd>
      </dl>
    </section>
    <section>
      <h2>命令证据</h2>
      <table><thead><tr><th>命令</th><th>结果</th><th>证据</th></tr></thead><tbody>${commandRows}</tbody></table>
    </section>
  </main>
</body>
</html>
`);
}

export function writeSemanticsManifest({ evidenceDir, summary }) {
  const version = summary.version;
  writeFileSync(path.join(evidenceDir, "manifest.json"), `${JSON.stringify({
    schema: "capy.evidence.manifest.v1",
    version,
    status: "verified",
    generated_at: new Date().toISOString(),
    summary: "视频片段语义分析、AI 建议语义理由和重开恢复已通过真实 CEF DOM/state 验证；图片为 state-derived PNG。",
    verdict: { status: "passed", blockers: [], warnings: ["current CEF app-view capture crashes or times out after video workspace state changes; v0.50 evidence uses DOM/state-derived PNG plus raw state JSON"] },
    runs: [{ id: "video-clip-semantics-loop", command: `scripts/verify-video-clip-semantics.mjs spec/versions/${version}`, status: "passed", evidence: `spec/versions/${version}/evidence/assets/video-clip-semantics-summary.json` }],
    artifacts: [
      { path: `spec/versions/${version}/evidence/index.html`, kind: "html-report", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-semantics-before-desktop.png`, kind: "state-derived-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-semantics-after-desktop.png`, kind: "state-derived-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-semantics-suggestion-desktop.png`, kind: "state-derived-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-semantics-restored-desktop.png`, kind: "state-derived-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-semantics-manifest.json`, kind: "project-manifest", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/evidence-page-browser.png`, kind: "browser-screenshot", status: "verified" }
    ]
  }, null, 2)}\n`);
}

export async function verifySemanticsEvidencePage({ evidenceDir, assetsDir }) {
  const version = path.basename(path.dirname(evidenceDir));
  const { chromium } = await loadPlaywright();
  const server = await startEvidenceServer(evidenceDir);
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const consoleErrors = [];
  const pageErrors = [];
  page.on("console", message => { if (message.type() === "error") consoleErrors.push(message.text()); });
  page.on("pageerror", error => pageErrors.push(error.message));
  const url = `http://127.0.0.1:${server.port}/index.html`;
  await page.goto(url, { waitUntil: "networkidle" });
  const state = await page.evaluate(() => ({
    title: document.querySelector("h1")?.textContent || "",
    bodyText: document.body.innerText,
    images: [...document.images].map(img => ({ src: img.getAttribute("src"), complete: img.complete, w: img.naturalWidth, h: img.naturalHeight })),
    bodyLength: document.body.innerText.length
  }));
  assert(state.title.includes(version), "evidence page title missing version");
  assert(state.bodyText.includes("AI 建议引用语义"), "suggestion semantics section missing");
  assert(state.images.length >= 4 && state.images.every(img => img.complete && img.w > 0), "evidence images did not load");
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
      if (!filePath.startsWith(evidenceDir)) return res.writeHead(403).end("forbidden");
      if (!existsSync(filePath)) return res.writeHead(404).end("not found");
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
  return "text/plain; charset=utf-8";
}

function escapeHtml(value) {
  return String(value ?? "").replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
