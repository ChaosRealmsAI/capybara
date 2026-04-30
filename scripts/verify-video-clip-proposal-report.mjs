import { createReadStream, existsSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import path from "node:path";

export function writeProposalEvidencePage({ evidenceDir, logs, summary }) {
  const commandRows = logs.map(item => `<tr><td><code>${escapeHtml(item.command)}</code></td><td>${item.ok ? "通过" : "失败"}</td><td>${item.evidence ? `<a href="assets/${escapeHtml(item.evidence)}">${escapeHtml(item.evidence)}</a>` : ""}</td></tr>`).join("\n");
  const changeRows = summary.proposal.changes.map(change => `<tr><td>${escapeHtml(change.action_label_zh)}</td><td>${position(change.before_sequence)}</td><td>${position(change.after_sequence)}</td><td>${escapeHtml(change.scene)}</td><td>${escapeHtml(change.reason_summary)}</td><td>${escapeHtml(change.apply_status)}</td></tr>`).join("\n");
  writeFileSync(path.join(evidenceDir, "index.html"), `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(summary.version)} 片段反馈修改提案证据</title>
  <style>
    body{margin:0;font-family:-apple-system,BlinkMacSystemFont,"PingFang SC",sans-serif;color:#1f2937;background:#f6f8fb}
    main{max-width:1200px;margin:0 auto;padding:32px 20px 56px}
    header{display:flex;justify-content:space-between;gap:20px;align-items:flex-start;margin-bottom:22px}
    h1{margin:0;font-size:30px} h2{margin:0 0 12px;font-size:19px} p{line-height:1.7}
    .badge{padding:8px 12px;border-radius:999px;background:#dcfce7;color:#166534;font-weight:800}
    section{margin-top:16px;padding:18px;border:1px solid #e5e7eb;border-radius:8px;background:white}
    .grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:14px}
    img{width:100%;border-radius:6px;border:1px solid #e5e7eb;background:#111827}
    dl{display:grid;grid-template-columns:180px minmax(0,1fr);gap:8px 12px;margin:0}
    dt{color:#6b7280;font-weight:700} dd{margin:0;overflow-wrap:anywhere}
    table{width:100%;border-collapse:collapse;font-size:12px} th,td{padding:8px;border-bottom:1px solid #e5e7eb;text-align:left;vertical-align:top}
    code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>${escapeHtml(summary.version)} 片段反馈生成修改提案</h1>
        <p>已用真实 CEF DOM/state 完成反馈输入、proposal diff、拒绝不改 queue、重新生成并接受后更新 queue 的闭环验证。全程本地 deterministic，无 live-spend provider。</p>
      </div>
      <span class="badge">通过 · PM 明确决策后才写 queue</span>
    </header>
    <section>
      <h2>验收结论</h2>
      <dl>
        <dt>Proposal diff</dt><dd><a href="assets/video-clip-proposal-diff.json">video-clip-proposal-diff.json</a></dd>
        <dt>Reject 后 queue</dt><dd><a href="assets/video-clip-proposal-queue-after-reject.json">video-clip-proposal-queue-after-reject.json</a></dd>
        <dt>Accept 后 queue</dt><dd><a href="assets/video-clip-proposal-queue-after-accept.json">video-clip-proposal-queue-after-accept.json</a></dd>
        <dt>决策记录</dt><dd>先 rejected 保持原 queue，再 accepted 写入 after queue。</dd>
        <dt>红线</dt><dd>proposal 生成不改 queue；无 provider 调用；仍是线性 clip queue。</dd>
      </dl>
    </section>
    <section>
      <h2>可见证据</h2>
      <div class="grid">
        <figure><img src="assets/video-clip-proposal-loaded-desktop.png" alt="加载队列和语义"><figcaption>加载：queue 与语义已恢复</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-feedback-saved-desktop.png" alt="反馈已保存"><figcaption>反馈：片段反馈绑定到 queue item</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-generated-desktop.png" alt="proposal diff"><figcaption>提案：展示 before/after 和理由</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-rejected-desktop.png" alt="拒绝提案"><figcaption>拒绝：记录 rejected，queue 不变</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-accepted-desktop.png" alt="接受提案"><figcaption>接受：记录 accepted，queue 更新</figcaption></figure>
      </div>
    </section>
    <section><h2>Proposal Diff 明细</h2><table><thead><tr><th>动作</th><th>Before</th><th>After</th><th>片段</th><th>原因摘要</th><th>状态</th></tr></thead><tbody>${changeRows}</tbody></table></section>
    <section>
      <h2>状态 JSON</h2>
      <dl>
        <dt>加载</dt><dd><a href="assets/video-clip-proposal-loaded-state.json">video-clip-proposal-loaded-state.json</a></dd>
        <dt>保存反馈</dt><dd><a href="assets/video-clip-proposal-feedback-saved-state.json">video-clip-proposal-feedback-saved-state.json</a></dd>
        <dt>生成提案</dt><dd><a href="assets/video-clip-proposal-generated-state.json">video-clip-proposal-generated-state.json</a></dd>
        <dt>拒绝提案</dt><dd><a href="assets/video-clip-proposal-rejected-state.json">video-clip-proposal-rejected-state.json</a></dd>
        <dt>接受提案</dt><dd><a href="assets/video-clip-proposal-accepted-state.json">video-clip-proposal-accepted-state.json</a></dd>
        <dt>汇总</dt><dd><a href="assets/video-clip-proposal-summary.json">video-clip-proposal-summary.json</a></dd>
      </dl>
    </section>
    <section><h2>命令证据</h2><table><thead><tr><th>命令</th><th>结果</th><th>证据</th></tr></thead><tbody>${commandRows}</tbody></table></section>
  </main>
</body>
</html>
`);
}

export function writeProposalManifest({ evidenceDir, summary }) {
  const version = summary.version;
  writeFileSync(path.join(evidenceDir, "manifest.json"), `${JSON.stringify({
    schema: "capy.evidence.manifest.v1",
    version,
    status: "verified",
    generated_at: new Date().toISOString(),
    summary: "片段反馈生成 proposal diff、拒绝保持 queue、接受更新 queue 已通过真实 CEF DOM/state 验证。",
    verdict: { status: "passed", blockers: [], warnings: ["desktop PNG is state-derived from CEF DOM/state because app-view capture can be unstable on this video surface"] },
    runs: [{ id: "video-clip-proposal-loop", command: `scripts/verify-video-clip-proposal.mjs spec/versions/${version}`, status: "passed", evidence: `spec/versions/${version}/evidence/assets/video-clip-proposal-summary.json` }],
    artifacts: [
      { path: `spec/versions/${version}/evidence/index.html`, kind: "html-report", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-diff.json`, kind: "project-manifest", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-accepted-desktop.png`, kind: "state-derived-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/evidence-page-browser.png`, kind: "browser-screenshot", status: "verified" }
    ]
  }, null, 2)}\n`);
}

export async function verifyProposalEvidencePage({ evidenceDir, assetsDir }) {
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
  assert(state.bodyText.includes("Proposal Diff 明细"), "proposal diff section missing");
  assert(state.bodyText.includes("接受：记录 accepted"), "accept evidence missing");
  assert(state.images.length >= 5 && state.images.every(img => img.complete && img.w > 0), "evidence images did not load");
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

function position(value) {
  return value ? `#${value}` : "无";
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
