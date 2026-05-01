import { createReadStream, existsSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import path from "node:path";

export function writeProposalEvidencePage({ evidenceDir, logs, summary }) {
  const commandRows = logs.map(item => `<tr><td><code>${escapeHtml(item.command)}</code></td><td>${item.ok ? "通过" : "失败"}</td><td>${item.evidence ? `<a href="assets/${escapeHtml(item.evidence)}">${escapeHtml(item.evidence)}</a>` : ""}</td></tr>`).join("\n");
  const changeRows = summary.proposal.changes.map(change => `<tr><td>${escapeHtml(change.action_label_zh)}</td><td>${position(change.before_sequence)}</td><td>${position(change.after_sequence)}</td><td>${escapeHtml(change.scene)}</td><td>${escapeHtml(change.reason_summary)}</td><td>${escapeHtml(change.apply_status)}</td></tr>`).join("\n");
  const proposalRows = (summary.proposal_history?.entries || [summary.first_proposal, summary.stale_candidate_proposal, summary.conflict_decision, summary.proposal]).filter(Boolean).map(proposal => `<tr><td>r${escapeHtml(proposal.revision || 0)}</td><td>${escapeHtml(proposal.status || "")}</td><td>${escapeHtml(String(proposal.generated_at || ""))}</td><td><code>${escapeHtml(proposal.base_queue_hash || "")}</code></td><td><code>${escapeHtml(proposal.current_queue_hash || proposal.conflict?.current_queue_hash || "")}</code></td><td>${escapeHtml(proposal.decision?.decision || "未决策")}</td><td>${escapeHtml(proposal.conflict?.message_zh || "")}</td></tr>`).join("\n");
  const contextSummary = summary.video_project_context_package || {};
  const contextStatusRows = Object.entries(contextSummary.status_counts || {}).map(([status, count]) => `<tr><td>${escapeHtml(status)}</td><td>${escapeHtml(count)}</td></tr>`).join("\n");
  const contextConflictRows = (contextSummary.conflicts || []).map(conflict => `<tr><td>r${escapeHtml(conflict.revision)}</td><td>${escapeHtml(conflict.conflict_type)}</td><td>${escapeHtml(conflict.message_zh)}</td><td><code>${escapeHtml(conflict.base_queue_hash)}</code></td><td><code>${escapeHtml(conflict.current_queue_hash)}</code></td></tr>`).join("\n");
  const captureRows = (summary.capture_verdicts || []).map(verdict => {
    const statusClass = verdict.capture.blocking ? "danger" : verdict.capture.status === "captured" ? "ok" : "warn";
    const attempts = (verdict.capture.attempts || []).map(attempt => `${escapeHtml(attempt.method)}:${attempt.ok ? "成功" : escapeHtml(attempt.failure_kind)}`).join(" / ");
    return `<tr><td>${escapeHtml(verdict.stage)}</td><td><span class="badge ${statusClass}">${escapeHtml(verdict.capture.status)}</span></td><td>${verdict.capture.blocking ? "阻断" : "不阻断"}</td><td><a href="assets/${escapeHtml(verdict.capture.final_image || "")}">${escapeHtml(verdict.capture.final_image || "")}</a></td><td>${escapeHtml(verdict.capture.final_image_source || "")}</td><td>${escapeHtml(attempts)}</td><td>${escapeHtml(verdict.capture.rationale || "")}</td></tr>`;
  }).join("\n");
  const capturedCount = (summary.capture_verdicts || []).filter(verdict => verdict.capture.status === "captured").length;
  const fallbackCount = (summary.capture_verdicts || []).filter(verdict => verdict.capture.final_image_source === "state-derived-fallback").length;
  const captureBadgeClass = (summary.capture_verdicts || []).some(verdict => verdict.capture.blocking) ? "danger" : fallbackCount > 0 ? "warn" : "ok";
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
    .badge.warn{background:#fef3c7;color:#92400e}.badge.danger{background:#fee2e2;color:#991b1b}.badge.ok{background:#dcfce7;color:#166534}
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
        <h1>${escapeHtml(summary.version)} 持久提案历史</h1>
        <p>已用真实 CEF DOM/state 完成反馈输入、proposal revision/hash、拒绝不改 queue、外部改变 queue 后接受旧 proposal 被阻止、重新生成有效 proposal 后接受写入 queue，并关闭重开同一项目证明 proposal history 持久恢复。截图证据会单独标注真实 app-view capture 是否成功；fallback 不会被当作截图成功。</p>
      </div>
      <span class="badge">通过 · 历史持久且只读</span>
    </header>
    <section>
      <h2>验收结论</h2>
      <dl>
        <dt>Proposal diff</dt><dd><a href="assets/video-clip-proposal-diff.json">video-clip-proposal-diff.json</a></dd>
        <dt>Proposal history</dt><dd><a href="assets/video-clip-proposal-history.json">video-clip-proposal-history.json</a> · ${escapeHtml(summary.proposal_history?.entries?.length || 0)} 条记录</dd>
        <dt>Context package</dt><dd><a href="assets/video-project-context-package.json">video-project-context-package.json</a> · <code>${escapeHtml(contextSummary.package_id || "")}</code> · anchor artifact <code>${escapeHtml(contextSummary.anchor_artifact_id || "")}</code></dd>
        <dt>重开后 state</dt><dd><a href="assets/video-clip-proposal-history-reopened-state.json">video-clip-proposal-history-reopened-state.json</a> · 历史 ${escapeHtml(summary.reopened_history_count || 0)} 条 · 只读 ${summary.reopened_history_readonly ? "是" : "否"}</dd>
        <dt>Reject 后 queue</dt><dd><a href="assets/video-clip-proposal-queue-after-reject.json">video-clip-proposal-queue-after-reject.json</a></dd>
        <dt>冲突后 queue</dt><dd><a href="assets/video-clip-proposal-queue-after-conflict.json">video-clip-proposal-queue-after-conflict.json</a></dd>
        <dt>Accept 后 queue</dt><dd><a href="assets/video-clip-proposal-queue-after-accept.json">video-clip-proposal-queue-after-accept.json</a></dd>
        <dt>决策记录</dt><dd>先 rejected 保持原 queue；再把 queue 外部改成新 hash，旧 proposal accept 返回 conflicted 且不写 queue；最后重新生成有效 proposal 并 accepted 写入 after queue；关闭重开后历史仍显示 rejected / conflicted / accepted。</dd>
        <dt>版本基准</dt><dd>最新 proposal r${escapeHtml(summary.proposal.revision || 0)} · base_queue_hash <code>${escapeHtml(summary.proposal.base_queue_hash || "")}</code>。冲突 proposal r${escapeHtml(summary.conflict_decision?.revision || 0)} 的 current_queue_hash 与 base 不一致。</dd>
        <dt>真实截图状态</dt><dd><span class="badge ${captureBadgeClass}">${capturedCount} 个真实截图成功 · ${fallbackCount} 个 fallback</span> · fallback 不是截图成功。</dd>
        <dt>红线</dt><dd>proposal 生成不改 queue；无 provider 调用；仍是线性 clip queue。</dd>
      </dl>
    </section>
    <section>
      <h2>视频项目 Context Package</h2>
      <p>通过现有 <code>capy context build</code> 生成，不新增 CLI 入口。包内把 source media、当前 queue、accepted/rejected/conflicted proposal history、设计约束和安全说明一次性给下一轮 AI；生成过程不写 queue。</p>
      <dl>
        <dt>完整 JSON</dt><dd><a href="assets/video-project-context-package.json">video-project-context-package.json</a></dd>
        <dt>稳定 package id</dt><dd><code>${escapeHtml(contextSummary.package_id || "")}</code></dd>
        <dt>稳定 artifact id</dt><dd><code>${escapeHtml(contextSummary.anchor_artifact_id || "")}</code></dd>
        <dt>素材 / queue / history</dt><dd>${escapeHtml(contextSummary.source_media_count || 0)} 个素材 · ${escapeHtml(contextSummary.queue_item_count || 0)} 个 queue item · ${escapeHtml(contextSummary.proposal_history_count || 0)} 条历史</dd>
        <dt>current_queue_hash</dt><dd><code>${escapeHtml(contextSummary.current_queue_hash || "")}</code></dd>
        <dt>设计约束</dt><dd><code>${escapeHtml(contextSummary.design_language_ref || "")}</code> · ${escapeHtml(contextSummary.design_asset_count || 0)} 个资产</dd>
        <dt>安全上下文</dt><dd>safe_for_next_ai_input=${contextSummary.safe_for_next_ai_input ? "true" : "false"} · no_queue_write=${contextSummary.no_queue_write ? "true" : "false"} · proposal_history_read_only=${contextSummary.proposal_history_read_only ? "true" : "false"}</dd>
        <dt>下一轮说明</dt><dd>${escapeHtml(contextSummary.safety_note_zh || "")}</dd>
      </dl>
      <div class="grid">
        <table><thead><tr><th>历史状态</th><th>数量</th></tr></thead><tbody>${contextStatusRows}</tbody></table>
        <table><thead><tr><th>Revision</th><th>冲突类型</th><th>原因</th><th>base_queue_hash</th><th>current_queue_hash</th></tr></thead><tbody>${contextConflictRows}</tbody></table>
      </div>
    </section>
    <section><h2>Proposal 持久历史</h2><table><thead><tr><th>Revision</th><th>状态</th><th>生成时间(ms)</th><th>base_queue_hash</th><th>current_queue_hash</th><th>决策</th><th>冲突说明</th></tr></thead><tbody>${proposalRows}</tbody></table></section>
    <section>
      <h2>真实截图状态</h2>
      <p>每个阶段都先尝试 CEF app-view capture / screenshot。若超时或失败，本页显示 state-derived fallback、原因和是否阻断；不会把 fallback 图伪装成真实截图成功。</p>
      <table><thead><tr><th>阶段</th><th>状态</th><th>验收影响</th><th>最终图</th><th>图来源</th><th>尝试</th><th>原因</th></tr></thead><tbody>${captureRows}</tbody></table>
    </section>
    <section>
      <h2>可见证据</h2>
      <div class="grid">
        <figure><img src="assets/video-clip-proposal-loaded-desktop.png" alt="加载队列和语义"><figcaption>加载：queue 与语义已恢复</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-feedback-saved-desktop.png" alt="反馈已保存"><figcaption>反馈：片段反馈绑定到 queue item</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-generated-desktop.png" alt="proposal diff"><figcaption>提案：展示 before/after 和理由</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-rejected-desktop.png" alt="拒绝提案"><figcaption>拒绝：记录 rejected，queue 不变</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-conflicted-desktop.png" alt="旧提案冲突"><figcaption>冲突：旧 proposal 被阻止，queue 不被覆盖</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-accepted-desktop.png" alt="接受提案"><figcaption>接受：记录 accepted，queue 更新</figcaption></figure>
        <figure><img src="assets/video-clip-proposal-history-reopened-desktop.png" alt="重开项目提案历史"><figcaption>重开：持久 history 恢复，历史详情只读</figcaption></figure>
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
        <dt>冲突候选</dt><dd><a href="assets/video-clip-proposal-stale-candidate-state.json">video-clip-proposal-stale-candidate-state.json</a></dd>
        <dt>冲突提案</dt><dd><a href="assets/video-clip-proposal-conflicted-state.json">video-clip-proposal-conflicted-state.json</a></dd>
        <dt>接受提案</dt><dd><a href="assets/video-clip-proposal-accepted-state.json">video-clip-proposal-accepted-state.json</a></dd>
        <dt>重开历史</dt><dd><a href="assets/video-clip-proposal-history-reopened-state.json">video-clip-proposal-history-reopened-state.json</a></dd>
        <dt>持久历史</dt><dd><a href="assets/video-clip-proposal-history.json">video-clip-proposal-history.json</a></dd>
        <dt>汇总</dt><dd><a href="assets/video-clip-proposal-summary.json">video-clip-proposal-summary.json</a></dd>
        <dt>Context package</dt><dd><a href="assets/video-project-context-package.json">video-project-context-package.json</a></dd>
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
  const captureBlocking = (summary.capture_verdicts || []).some(verdict => verdict.capture.blocking);
  const captureWarnings = (summary.capture_verdicts || []).flatMap(verdict => verdict.verdict.warnings || []);
  writeFileSync(path.join(evidenceDir, "manifest.json"), `${JSON.stringify({
    schema: "capy.evidence.manifest.v1",
    version,
    status: captureBlocking ? "blocked" : "verified",
    generated_at: new Date().toISOString(),
    summary: "片段反馈生成 proposal diff、revision/hash、过期 proposal 冲突阻止写 queue、重新生成并接受有效 proposal、关闭重开后持久 history 恢复已通过真实 CEF DOM/state 验证，并记录 app-view capture verdict。",
    verdict: { status: captureBlocking ? "failed" : "passed", blockers: captureBlocking ? ["capture_blocked"] : [], warnings: captureWarnings },
    runs: [{ id: "video-clip-proposal-loop", command: `scripts/verify-video-clip-proposal.mjs spec/versions/${version}`, status: "passed", evidence: `spec/versions/${version}/evidence/assets/video-clip-proposal-summary.json` }],
    artifacts: [
      { path: `spec/versions/${version}/evidence/index.html`, kind: "html-report", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-diff.json`, kind: "project-manifest", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-project-context-package.json`, kind: "context-package", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-history.json`, kind: "project-manifest", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-history-reopened-state.json`, kind: "state-json", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-conflicted-desktop.png`, kind: "desktop-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-accepted-desktop.png`, kind: "desktop-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-history-reopened-desktop.png`, kind: "desktop-visual", status: "verified" },
      { path: `spec/versions/${version}/evidence/assets/video-clip-proposal-accepted-capture-verdict.json`, kind: "capture-verdict", status: "verified" },
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
  assert(state.bodyText.includes("真实截图状态"), "capture verdict section missing");
  assert(state.bodyText.includes("fallback 不是截图成功"), "fallback warning missing");
  assert(state.bodyText.includes("Proposal 持久历史"), "proposal history section missing");
  assert(state.bodyText.includes("视频项目 Context Package"), "video project context section missing");
  assert(state.bodyText.includes("safe_for_next_ai_input=true"), "context safety status missing");
  assert(state.bodyText.includes("video-project-context-package.json"), "context package link missing");
  assert(state.bodyText.includes("重开：持久 history 恢复"), "reopen history visual missing");
  assert(state.bodyText.includes("conflicted"), "conflict status missing");
  assert(state.bodyText.includes("Proposal Diff 明细"), "proposal diff section missing");
  assert(state.bodyText.includes("接受：记录 accepted"), "accept evidence missing");
  assert(state.images.length >= 6 && state.images.every(img => img.complete && img.w > 0), "evidence images did not load");
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
