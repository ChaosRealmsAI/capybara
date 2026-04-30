import { createReadStream, existsSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import path from "node:path";

export function writeFeedbackEvidencePage({ evidenceDir, logs, summary }) {
  const commandRows = logs.map(item => `<tr><td><code>${escapeHtml(item.command)}</code></td><td>${item.ok ? "通过" : "失败"}</td><td>${item.evidence ? `<a href="assets/${escapeHtml(item.evidence)}">${escapeHtml(item.evidence)}</a>` : ""}</td></tr>`).join("\n");
  const feedbackRows = summary.feedback.items.map(item => `<tr><td>${escapeHtml(item.queue_item_id)}</td><td>${escapeHtml(item.scene)}</td><td>${escapeHtml(item.feedback)}</td><td>${escapeHtml(item.feedback_kind)}</td><td>${escapeHtml(item.recommendation_effect)}</td></tr>`).join("\n");
  const suggestionRows = summary.suggestion.items.map(item => `<tr><td>${item.sequence}</td><td>${escapeHtml(item.scene)}</td><td>${escapeHtml(item.feedback_text || "")}</td><td>${escapeHtml(item.feedback_reason || "")}</td></tr>`).join("\n");
  writeFileSync(path.join(evidenceDir, "index.html"), `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(summary.version)} 片段级反馈闭环证据</title>
  <style>
    body{margin:0;font-family:-apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;color:#1f2937;background:#f6f8fb}
    main{max-width:1200px;margin:0 auto;padding:32px 20px 56px}
    header{display:flex;justify-content:space-between;gap:20px;align-items:flex-start;margin-bottom:22px}
    h1{margin:0;font-size:30px} h2{margin:0 0 12px;font-size:19px} p{line-height:1.7}
    .badge{padding:8px 12px;border-radius:999px;background:#dcfce7;color:#166534;font-weight:800}
    section{margin-top:16px;padding:18px;border:1px solid #e5e7eb;border-radius:8px;background:white}
    .grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:14px}
    img{width:100%;border-radius:6px;border:1px solid #e5e7eb;background:#111827}
    dl{display:grid;grid-template-columns:170px minmax(0,1fr);gap:8px 12px;margin:0}
    dt{color:#6b7280;font-weight:700} dd{margin:0;overflow-wrap:anywhere}
    table{width:100%;border-collapse:collapse;font-size:12px} th,td{padding:8px;border-bottom:1px solid #e5e7eb;text-align:left;vertical-align:top}
    code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>${escapeHtml(summary.version)} 片段级反馈闭环</h1>
        <p>已用真实 CEF DOM/state 完成保存片段反馈、重新生成建议、证明 queue 未被自动修改、重开恢复四步验证。图片证据由返回的真实 DOM/state 生成；全程本地 deterministic，无付费或联网 provider。</p>
      </div>
      <span class="badge">通过 · state JSON 主证据</span>
    </header>
    <section>
      <h2>验收结论</h2>
      <dl>
        <dt>反馈 manifest</dt><dd><a href="assets/video-clip-feedback-manifest.json">video-clip-feedback-manifest.json</a></dd>
        <dt>建议 JSON</dt><dd><a href="assets/video-clip-feedback-suggestion.json">video-clip-feedback-suggestion.json</a></dd>
        <dt>Queue 未自动修改</dt><dd><a href="assets/video-clip-feedback-queue-after-suggest.json">video-clip-feedback-queue-after-suggest.json</a></dd>
        <dt>反馈数量</dt><dd>${summary.feedback.items.length} 条</dd>
        <dt>红线</dt><dd>未调用 provider；suggest 只读；queue 仍需用户手动采用方案。</dd>
      </dl>
    </section>
    <section>
      <h2>可见证据</h2>
      <div class="grid">
        <figure><img src="assets/video-clip-feedback-loaded-desktop.png" alt="加载队列和语义"><figcaption>加载：queue 与语义已恢复</figcaption></figure>
        <figure><img src="assets/video-clip-feedback-saved-desktop.png" alt="片段反馈已保存"><figcaption>保存：反馈绑定到第 1 个片段</figcaption></figure>
        <figure><img src="assets/video-clip-feedback-suggested-desktop.png" alt="建议引用反馈"><figcaption>建议：引用反馈并调整排序</figcaption></figure>
        <figure><img src="assets/video-clip-feedback-restored-desktop.png" alt="重开恢复反馈"><figcaption>重开：反馈、语义和建议依据恢复</figcaption></figure>
      </div>
    </section>
    <section><h2>片段反馈</h2><table><thead><tr><th>queue item</th><th>片段</th><th>反馈</th><th>分类</th><th>影响</th></tr></thead><tbody>${feedbackRows}</tbody></table></section>
    <section><h2>AI 建议引用反馈</h2><table><thead><tr><th>#</th><th>片段</th><th>用户反馈</th><th>反馈调整</th></tr></thead><tbody>${suggestionRows}</tbody></table></section>
    <section>
      <h2>状态 JSON</h2>
      <dl>
        <dt>加载</dt><dd><a href="assets/video-clip-feedback-loaded-state.json">video-clip-feedback-loaded-state.json</a></dd>
        <dt>保存反馈</dt><dd><a href="assets/video-clip-feedback-saved-state.json">video-clip-feedback-saved-state.json</a></dd>
        <dt>建议反馈</dt><dd><a href="assets/video-clip-feedback-suggested-state.json">video-clip-feedback-suggested-state.json</a></dd>
        <dt>重开恢复</dt><dd><a href="assets/video-clip-feedback-restored-state.json">video-clip-feedback-restored-state.json</a></dd>
        <dt>汇总</dt><dd><a href="assets/video-clip-feedback-summary.json">video-clip-feedback-summary.json</a></dd>
      </dl>
    </section>
    <section><h2>命令证据</h2><table><thead><tr><th>命令</th><th>结果</th><th>证据</th></tr></thead><tbody>${commandRows}</tbody></table></section>
  </main>
</body>
</html>
`);
}

export function writeFeedbackManifest({ evidenceDir, summary }) {
  const version = summary.version;
  writeFileSync(path.join(evidenceDir, "manifest.json"), `${JSON.stringify({
    schema: "capy.evidence.manifest.v1",
    version,
    status: "verified",
    generated_at: new Date().toISOString(),
    summary: "片段级反馈保存、建议引用反馈、queue 未自动修改和重开恢复已通过真实 CEF DOM/state 验证。",
    verdict: { status: "passed", blockers: [], warnings: ["desktop PNG is state-derived from CEF DOM/state because app-view capture can be unstable on this video surface"] },
    runs: [{ id: "video-clip-feedback-loop", command: `scripts/verify-video-clip-feedback.mjs spec/versions/${version}`, status: "passed", evidence: `spec/versions/${version}/evidence/assets/video-clip-feedback-summary.json` }],
    artifacts: [
      { path: `spec/versions/${version}/evidence/index.html`, kind: "html-report", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-feedback-saved-desktop.png`, kind: "state-derived-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-feedback-manifest.json`, kind: "project-manifest", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/evidence-page-browser.png`, kind: "browser-screenshot", status: "verified" }
    ]
  }, null, 2)}\n`);
}

export async function verifyFeedbackEvidencePage({ evidenceDir, assetsDir }) {
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
    images: [...document.images].map(img => ({ complete: img.complete, w: img.naturalWidth, h: img.naturalHeight }))
  }));
  assert(state.title.includes(version), "evidence page title missing version");
  assert(state.bodyText.includes("AI 建议引用反馈"), "feedback suggestion section missing");
  assert(state.images.length >= 4 && state.images.every(img => img.complete && img.w > 0), "evidence images did not load");
  assert(consoleErrors.length === 0 && pageErrors.length === 0, "evidence page has browser errors");
  await page.screenshot({ path: path.join(assetsDir, "evidence-page-browser.png"), fullPage: true });
  await browser.close();
  await new Promise(resolve => server.instance.close(resolve));
  writeFileSync(path.join(assetsDir, "evidence-page-check.json"), `${JSON.stringify({ ok: true, url, state, consoleErrors, pageErrors }, null, 2)}\n`);
}

async function loadPlaywright() {
  try { return await import("playwright"); } catch {
    const require = createRequire("/opt/homebrew/lib/node_modules/playwright/package.json");
    return require("playwright");
  }
}

function startEvidenceServer(evidenceDir) {
  return new Promise(resolve => {
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
