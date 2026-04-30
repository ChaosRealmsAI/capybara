#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import { initialQueue } from "./verify-ai-clip-suggestion-fixtures.mjs";
import { proposalAcceptEval, proposalGenerateEval, proposalLoadEval, proposalRejectEval, proposalSaveFeedbackEval } from "./verify-video-clip-proposal-evals.mjs";
import { verifyProposalEvidencePage, writeProposalEvidencePage, writeProposalManifest } from "./verify-video-clip-proposal-report.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-video-clip-proposal.mjs [spec/versions/v0.52]

Use when:
  Verify the v0.52 feedback-aware clip proposal diff loop end to end on a real CEF desktop.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.52.
  Requires target/debug/capy, local ffmpeg/ffprobe, macOS launchctl, CEF shell harness, ImageMagick magick, and Playwright for evidence-page browser check.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/video-clip-proposal-project.
  Imports two local WebM videos, seeds .capy/video-clip-queue.json, writes semantics and feedback through Project Core, launches an isolated debug shell id, generates proposal diff, rejects it, regenerates it, accepts it, and opens <version>/evidence/index.html.

Evidence outputs:
  video-clip-proposal-diff.json
  video-clip-proposal-queue-before-proposal.json
  video-clip-proposal-queue-after-proposal.json
  video-clip-proposal-queue-after-reject.json
  video-clip-proposal-queue-after-accept.json
  video-clip-proposal-summary.json
  video-clip-proposal-*-state.json
  video-clip-proposal-*-desktop.png

Pitfalls:
  Proposal generation must not mutate .capy/video-clip-queue.json.
  Only explicit accept may update the queue; reject must preserve the original queue ids.
  This verifies deterministic local proposal logic, not paid model interpretation.
  State JSON is the primary proof; PNGs are state-derived because CEF app-view capture is unstable on this surface.

Next step:
  Review <version>/evidence/index.html and <version>/evidence/assets/video-clip-proposal-summary.json.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.52");
const versionId = path.basename(versionDir);
const evidenceDir = path.join(versionDir, "evidence");
const assetsDir = path.join(evidenceDir, "assets");
const projectDir = path.join(assetsDir, "video-clip-proposal-project");
const mediaDir = path.join(projectDir, "media");
const initialQueuePath = path.join(assetsDir, "video-clip-proposal-initial-queue.json");
const projectProposalManifest = path.join(projectDir, ".capy", "video-clip-proposal.json");
const uiComposition = "__queue_only__";
const logs = [];
const openInstanceIds = [];
let currentSocket = "";
let shellBundleReady = false;

rmSync(assetsDir, { recursive: true, force: true });
mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(path.join(root, "target/debug/capy")), "missing target/debug/capy");
  command("ffmpeg", ["-version"], "video-clip-proposal-ffmpeg-version.log");
  command("ffprobe", ["-version"], "video-clip-proposal-ffprobe-version.log");
  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  generateVideo(path.join(mediaDir, "camera-a-wide.webm"), "testsrc2=size=640x360:rate=30", 4, "video-clip-proposal-source-a-generate.log");
  generateVideo(path.join(mediaDir, "camera-b-close.webm"), "smptebars=size=480x270:rate=24", 5, "video-clip-proposal-source-b-generate.log");
  capyJson(["project", "init", "--project", projectDir, "--name", "v0.52 Video Clip Proposal Project"], "video-clip-proposal-project-init.json");
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importA = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-a-wide.webm", "--title", "Camera A wide"], "video-clip-proposal-import-a.json");
  const importB = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-b-close.webm", "--title", "Camera B close"], "video-clip-proposal-import-b.json");
  writeJson("video-clip-proposal-initial-queue.json", initialQueue(importA, importB));
  capyJson(["project", "clip-queue", "write", "--project", projectDir, "--manifest", initialQueuePath], "video-clip-proposal-initial-queue-write.json");
  capyJson(["project", "clip-queue", "analyze", "--project", projectDir], "video-clip-proposal-cli-semantics.json");
  openShell("main", "video-clip-proposal-open-main.log");

  const loadedState = capyJson(["devtools", "--eval", proposalLoadEval(projectDir, uiComposition)], "video-clip-proposal-loaded-state.json", capyEnv());
  assertSemantics(loadedState.semantics, 2, "loaded");
  assertNoPageErrors(loadedState, "loaded");

  const savedState = capyJson(["devtools", "--eval", proposalSaveFeedbackEval("这段不适合开场")], "video-clip-proposal-feedback-saved-state.json", capyEnv());
  assertFeedback(savedState.feedback, "saved");
  const feedbackCli = capyJson(["project", "clip-queue", "feedbacks", "--project", projectDir], "video-clip-proposal-cli-feedbacks.json");
  assertFeedback(feedbackCli, "CLI feedbacks");

  const queueBeforeProposal = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-before-proposal.json");
  const proposedState = capyJson(["devtools", "--eval", proposalGenerateEval()], "video-clip-proposal-generated-state.json", capyEnv());
  assertProposal(proposedState.proposal, "generated");
  assertTextIncludes(proposedState.domProposalText, ["修改提案", "Before", "After"], "proposal DOM");
  copyFileSync(projectProposalManifest, path.join(assetsDir, "video-clip-proposal-diff.json"));
  const queueAfterProposal = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-after-proposal.json");
  assertQueueIds(queueBeforeProposal, queueAfterProposal, "proposal generation mutated queue");

  const rejectedState = capyJson(["devtools", "--eval", proposalRejectEval()], "video-clip-proposal-rejected-state.json", capyEnv());
  assertDecision(rejectedState.proposal, "rejected");
  const queueAfterReject = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-after-reject.json");
  assertQueueIds(queueBeforeProposal, queueAfterReject, "reject mutated queue");

  const acceptedState = capyJson(["devtools", "--eval", proposalAcceptEval()], "video-clip-proposal-accepted-state.json", capyEnv());
  assertDecision(acceptedState.proposal, "accepted");
  const queueAfterAccept = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-after-accept.json");
  assertAcceptedQueue(queueAfterAccept);
  capyJson(["project", "clip-queue", "proposal-current", "--project", projectDir], "video-clip-proposal-current-accepted.json");

  const summary = writeSummary({ loadedState, savedState, proposedState, rejectedState, acceptedState, feedbackCli, queueBeforeProposal, queueAfterReject, queueAfterAccept });
  writeProposalEvidencePage({ evidenceDir, logs, summary });
  writeProposalManifest({ evidenceDir, summary });
  await verifyProposalEvidencePage({ evidenceDir, assetsDir });
  command("open", [path.join(evidenceDir, "index.html")], "evidence-open.log");
  logs.push({ command: "verdict", ok: true });
  writeLogs();
  shutdown();
  console.log(JSON.stringify({ ok: true, assets: assetsDir, proposal: path.join(assetsDir, "video-clip-proposal-diff.json") }, null, 2));
} catch (error) {
  logs.push({ command: "verdict", ok: false, error: error instanceof Error ? error.message : String(error) });
  writeLogs();
  try { shutdown(); } catch {}
  console.error(JSON.stringify({ ok: false, error: error instanceof Error ? error.message : String(error), assets: assetsDir }, null, 2));
  process.exit(1);
}

function generateVideo(out, source, seconds, evidenceName) {
  command("ffmpeg", ["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", source, "-t", String(seconds), "-c:v", "libvpx", "-b:v", "1200k", "-pix_fmt", "yuv420p", out], evidenceName);
  assert(existsSync(out), `source video missing: ${out}`);
}

function openShell(phase, evidenceName) {
  const instanceId = `${versionId.replace(/[^A-Za-z0-9]+/g, "-")}-video-clip-proposal-${phase}`;
  currentSocket = `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
  openInstanceIds.push(instanceId);
  const args = ["--id", instanceId, "--project", "demo", "--replace"];
  if (shellBundleReady) args.push("--skip-build");
  const env = { ...process.env, CAPYBARA_SOCKET: currentSocket, CAPY_VERIFY_VERSION_DIR: versionDir, CAPY_VERIFY_ASSETS: assetsDir, CAPY_VERIFY_OPEN_PROJECT: "demo", CAPY_LAUNCH_LABEL: launchLabel(instanceId) };
  launchShell(args, evidenceName, env);
  const ps = capyJson(["ps"], `video-clip-proposal-${phase}-ps.json`, capyEnv());
  assert(Number(ps.count || 0) > 0, `${phase} shell did not open a window`);
  shellBundleReady = true;
}

function command(cmd, args, evidenceName, options = {}) {
  const started = Date.now();
  const stdout = execFileSync(cmd, args, { cwd: root, env: options.env || process.env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 128 * 1024 * 1024, timeout: options.timeout || 120_000 });
  if (evidenceName) writeFileSync(path.join(assetsDir, evidenceName), stdout);
  logs.push({ command: [cmd, ...args].join(" "), evidence: evidenceName || null, elapsed_ms: Date.now() - started, ok: true });
  return stdout;
}

function launchShell(args, evidenceName, env) {
  try { command("scripts/open-debug-shell.sh", args, evidenceName, { env }); } catch (error) {
    const message = `${String(error?.stdout || "")}${String(error?.stderr || "")}\n${error instanceof Error ? error.message : String(error)}\n`;
    writeFileSync(path.join(assetsDir, evidenceName), message);
    logs.push({ command: ["scripts/open-debug-shell.sh", ...args].join(" "), evidence: evidenceName, ok: false, error: message });
    throw error;
  }
}

function optionalCommandResult(cmd, args, evidenceName, options = {}) {
  try { command(cmd, args, evidenceName, options); return { ok: true }; } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    writeFileSync(path.join(assetsDir, evidenceName), evidenceName.endsWith(".json") ? `${JSON.stringify({ ok: false, error: message }, null, 2)}\n` : `${message}\n`);
    logs.push({ command: [cmd, ...args].join(" "), evidence: evidenceName, ok: false, error: message });
    return { ok: false, error: message };
  }
}

function capyJson(args, evidenceName, env = process.env) {
  const value = JSON.parse(command("target/debug/capy", args, evidenceName, { env }));
  writeJson(evidenceName, value);
  writeStateImage(value);
  return value;
}

function writeJson(name, value) {
  writeFileSync(path.join(assetsDir, name), `${JSON.stringify(value, null, 2)}\n`);
}

function writeStateImage(value) {
  const imageName = {
    loaded: "video-clip-proposal-loaded-desktop.png",
    "feedback-saved": "video-clip-proposal-feedback-saved-desktop.png",
    "proposal-generated": "video-clip-proposal-generated-desktop.png",
    "proposal-rejected": "video-clip-proposal-rejected-desktop.png",
    "proposal-accepted": "video-clip-proposal-accepted-desktop.png"
  }[value?.stage];
  if (!imageName) return;
  const svg = path.join(assetsDir, imageName.replace(/\.png$/, ".svg"));
  const panels = [
    ["Clip queue", value.domQueueText || ""],
    ["修改提案", value.domProposalText || "尚未生成提案"],
    ["AI 建议", value.domSuggestionText || "尚未生成建议"]
  ];
  writeFileSync(svg, stateSvg(value.stage, panels));
  execFileSync("magick", [svg, path.join(assetsDir, imageName)], { cwd: root, stdio: ["ignore", "pipe", "pipe"] });
}

function stateSvg(stage, panels) {
  const panelSvg = panels.map(([title, text], index) => {
    const x = 24 + index * 312;
    return `<g><rect x="${x}" y="104" width="${index === 2 ? 288 : 286}" height="452" rx="14" fill="#fff" stroke="#d8dee8"/><text x="${x + 20}" y="140" font-size="20" font-weight="700" fill="#0f172a">${xml(title)}</text>${textLines(text, x + 20, 172, index === 2 ? 248 : 246)}</g>`;
  }).join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="600" viewBox="0 0 960 600"><rect width="960" height="600" fill="#f6f8fb"/><text x="36" y="54" font-size="28" font-weight="700" fill="#101827">Capybara · 片段反馈修改提案</text><text x="38" y="82" font-size="14" fill="#64748b">state-derived evidence · CEF DOM/state · stage=${xml(stage || "")}</text>${panelSvg}</svg>`;
}

function textLines(text, x, y, width) {
  const approx = Math.max(10, Math.floor(width / 8));
  const words = String(text || "").replace(/\s+/g, " ").slice(0, 1400).split(" ");
  const lines = [];
  let line = "";
  for (const word of words) {
    const next = line ? `${line} ${word}` : word;
    if (next.length > approx && line) { lines.push(line); line = word; } else { line = next; }
    if (lines.length >= 17) break;
  }
  if (line && lines.length < 18) lines.push(line);
  return lines.map((line, index) => `<text x="${x}" y="${y + index * 22}" font-size="14" fill="#334155">${xml(line)}</text>`).join("");
}

function writeSummary({ loadedState, savedState, proposedState, rejectedState, acceptedState, feedbackCli, queueBeforeProposal, queueAfterReject, queueAfterAccept }) {
  const summary = {
    version: versionId,
    verdict: "passed",
    project: projectDir,
    feedback: feedbackCli,
    proposal: proposedState.proposal,
    reject_decision: rejectedState.proposal?.decision || null,
    accept_decision: acceptedState.proposal?.decision || null,
    queue_before_proposal: queueBeforeProposal.items || [],
    queue_after_reject: queueAfterReject.items || [],
    queue_after_accept: queueAfterAccept.items || [],
    states: { loaded: summarizeState(loadedState), saved: summarizeState(savedState), proposed: summarizeState(proposedState), rejected: summarizeState(rejectedState), accepted: summarizeState(acceptedState) }
  };
  writeJson("video-clip-proposal-summary.json", summary);
  return summary;
}

function summarizeState(state) {
  return { stage: state.stage, queue_count: state.queue?.length || 0, semantic_count: state.semantics?.items?.length || 0, feedback_count: state.feedback?.items?.length || 0, proposal_status: state.proposal?.status || "", change_count: state.proposal?.changes?.length || 0, layout: state.layout, console_errors: state.consoleErrors || [], page_errors: state.pageErrors || [] };
}

function assertSemantics(manifest, minCount, label) {
  assert(manifest?.schema_version === "capy.project-video-clip-semantics.v1", `${label} semantics schema mismatch`);
  assert((manifest.items || []).length >= minCount, `${label} missing semantic items`);
}

function assertFeedback(manifest, label) {
  assert(manifest?.schema_version === "capy.project-video-clip-feedback.v1", `${label} feedback schema mismatch`);
  assert((manifest.items || []).some(item => item.feedback === "这段不适合开场" && item.queue_item_id === "queue-initial-camera-a"), `${label} missing segment feedback`);
}

function assertProposal(proposal, label) {
  assert(proposal?.schema_version === "capy.project-video-clip-proposal.v1", `${label} proposal schema mismatch`);
  assert(proposal.status === "proposed", `${label} proposal status mismatch`);
  assert((proposal.changes || []).some(change => change.action === "deprioritize" && change.before_sequence === 1 && change.after_sequence === 2), `${label} missing deprioritize change`);
  assert((proposal.changes || []).some(change => change.feedback_reason && change.semantic_reason), `${label} missing feedback/semantic reasons`);
}

function assertDecision(proposal, status) {
  assert(proposal?.status === status, `proposal decision status should be ${status}`);
  assert(proposal?.decision?.decision === (status === "accepted" ? "accept" : "reject"), `proposal decision payload should be ${status}`);
}

function assertQueueIds(before, after, message) {
  assert(JSON.stringify((before.items || []).map(item => item.id)) === JSON.stringify((after.items || []).map(item => item.id)), message);
}

function assertAcceptedQueue(queue) {
  const ids = (queue.items || []).map(item => item.id);
  assert(ids[0] === "queue-initial-camera-b" && ids[1] === "queue-initial-camera-a", `accept did not update queue order: ${ids.join(",")}`);
}

function assertNoPageErrors(state, label) {
  const errors = [...(state.consoleErrors || []), ...(state.pageErrors || [])];
  assert(errors.length === 0, `${label} has page/console errors: ${JSON.stringify(errors)}`);
}

function assertTextIncludes(text, parts, label) {
  for (const part of parts) assert(String(text || "").includes(part), `${label} missing text: ${part}`);
}

function capyEnv() {
  return { ...process.env, CAPYBARA_SOCKET: currentSocket };
}

function launchLabel(instanceId) {
  return `com.capybara.debug.${instanceId}`;
}

function shutdown() {
  for (const instanceId of [...openInstanceIds].reverse()) {
    optionalCommandResult("target/debug/capy", ["quit"], `video-clip-proposal-${instanceId}-quit.json`, { timeout: 10_000, env: { ...process.env, CAPYBARA_SOCKET: `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock` } });
    optionalCommandResult("launchctl", ["remove", launchLabel(instanceId)], `video-clip-proposal-${instanceId}-launchctl-remove.log`);
  }
}

function writeLogs() {
  writeFileSync(path.join(assetsDir, "video-clip-proposal-command-log.json"), `${JSON.stringify(logs, null, 2)}\n`);
}

function xml(value) {
  return String(value ?? "").replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
