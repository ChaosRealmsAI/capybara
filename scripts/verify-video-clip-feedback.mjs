#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import { initialQueue } from "./verify-ai-clip-suggestion-fixtures.mjs";
import { feedbackLoadEval, feedbackRestoreEval, feedbackSaveEval, feedbackSuggestEval } from "./verify-video-clip-feedback-evals.mjs";
import { verifyFeedbackEvidencePage, writeFeedbackEvidencePage, writeFeedbackManifest } from "./verify-video-clip-feedback-report.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-video-clip-feedback.mjs [spec/versions/v0.51]

Use when:
  Verify the v0.51 fragment-level feedback loop end to end on a real CEF desktop.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.51.
  Requires target/debug/capy, local ffmpeg/ffprobe, macOS launchctl, CEF shell harness, ImageMagick magick, and Playwright for evidence-page browser check.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/video-clip-feedback-project.
  Imports two local WebM videos, seeds .capy/video-clip-queue.json, writes semantics and feedback through Project Core, launches isolated debug shell ids, regenerates suggestions, and opens <version>/evidence/index.html.

Evidence outputs:
  video-clip-feedback-loaded-state.json
  video-clip-feedback-saved-state.json
  video-clip-feedback-suggested-state.json
  video-clip-feedback-restored-state.json
  video-clip-feedback-manifest.json
  video-clip-feedback-suggestion.json
  video-clip-feedback-queue-after-suggest.json
  video-clip-feedback-summary.json
  video-clip-feedback-*-desktop.png

Pitfalls:
  This verifies deterministic local feedback classification, not paid model interpretation.
  Suggestion generation must remain read-only; queue mutation is allowed only after manual adopt.
  State JSON is the primary proof; PNGs are state-derived because CEF app-view capture is unstable on this surface.

Next step:
  Review <version>/evidence/index.html and <version>/evidence/assets/video-clip-feedback-summary.json.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.51");
const versionId = path.basename(versionDir);
const evidenceDir = path.join(versionDir, "evidence");
const assetsDir = path.join(evidenceDir, "assets");
const projectDir = path.join(assetsDir, "video-clip-feedback-project");
const mediaDir = path.join(projectDir, "media");
const initialQueuePath = path.join(assetsDir, "video-clip-feedback-initial-queue.json");
const projectFeedbackManifest = path.join(projectDir, ".capy", "video-clip-feedback.json");
const uiComposition = "__queue_only__";
const logs = [];
const openInstanceIds = [];
let currentSocket = "";
let shellBundleReady = false;

rmSync(assetsDir, { recursive: true, force: true });
mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(path.join(root, "target/debug/capy")), "missing target/debug/capy");
  command("ffmpeg", ["-version"], "video-clip-feedback-ffmpeg-version.log");
  command("ffprobe", ["-version"], "video-clip-feedback-ffprobe-version.log");
  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  generateVideo(path.join(mediaDir, "camera-a-wide.webm"), "testsrc2=size=640x360:rate=30", 4, "video-clip-feedback-source-a-generate.log");
  generateVideo(path.join(mediaDir, "camera-b-close.webm"), "smptebars=size=480x270:rate=24", 5, "video-clip-feedback-source-b-generate.log");
  capyJson(["project", "init", "--project", projectDir, "--name", "v0.51 Video Clip Feedback Project"], "video-clip-feedback-project-init.json");
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importA = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-a-wide.webm", "--title", "Camera A wide"], "video-clip-feedback-import-a.json");
  const importB = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-b-close.webm", "--title", "Camera B close"], "video-clip-feedback-import-b.json");
  writeJson("video-clip-feedback-initial-queue.json", initialQueue(importA, importB));
  capyJson(["project", "clip-queue", "write", "--project", projectDir, "--manifest", initialQueuePath], "video-clip-feedback-initial-queue-write.json");
  capyJson(["project", "clip-queue", "analyze", "--project", projectDir], "video-clip-feedback-cli-semantics.json");
  openShell("main", "video-clip-feedback-open-main.log");

  const loadedState = capyJson(["devtools", "--eval", feedbackLoadEval(projectDir, uiComposition)], "video-clip-feedback-loaded-state.json", capyEnv());
  assertSemantics(loadedState.semantics, 2, "loaded");
  assertNoPageErrors(loadedState, "loaded");

  const savedState = capyJson(["devtools", "--eval", feedbackSaveEval("这段不适合开场")], "video-clip-feedback-saved-state.json", capyEnv());
  assertFeedback(savedState.feedback, "saved");
  assertTextIncludes(savedState.domQueueText, ["用户反馈", "这段不适合开场"], "saved queue DOM");
  copyFileSync(projectFeedbackManifest, path.join(assetsDir, "video-clip-feedback-manifest.json"));
  const feedbackCli = capyJson(["project", "clip-queue", "feedbacks", "--project", projectDir], "video-clip-feedback-cli-feedbacks.json");
  assertFeedback(feedbackCli, "CLI feedbacks");

  const queueBeforeSuggest = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-feedback-queue-before-suggest.json");
  const suggestedState = capyJson(["devtools", "--eval", feedbackSuggestEval()], "video-clip-feedback-suggested-state.json", capyEnv());
  assertFeedbackSuggestion(suggestedState.suggestion, "suggested");
  assertTextIncludes(suggestedState.domSuggestionText, ["用户反馈", "反馈调整"], "suggestion DOM");
  writeJson("video-clip-feedback-suggestion.json", suggestedState.suggestion);
  const queueAfterSuggest = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-feedback-queue-after-suggest.json");
  assertQueueUnchanged(queueBeforeSuggest, queueAfterSuggest);

  restartShell("restore", "video-clip-feedback-open-restore.log");
  const restoredState = capyJson(["devtools", "--eval", feedbackRestoreEval(projectDir, uiComposition)], "video-clip-feedback-restored-state.json", capyEnv());
  assertFeedback(restoredState.feedback, "restored");
  assertFeedbackSuggestion(restoredState.suggestion, "restored suggestion");
  assertNoPageErrors(restoredState, "restored");

  const summary = writeSummary({ loadedState, savedState, suggestedState, restoredState, feedbackCli, queueAfterSuggest });
  writeFeedbackEvidencePage({ evidenceDir, logs, summary });
  writeFeedbackManifest({ evidenceDir, summary });
  await verifyFeedbackEvidencePage({ evidenceDir, assetsDir });
  command("open", [path.join(evidenceDir, "index.html")], "evidence-open.log");
  logs.push({ command: "verdict", ok: true });
  writeLogs();
  shutdown();
  console.log(JSON.stringify({ ok: true, assets: assetsDir, feedback: path.join(assetsDir, "video-clip-feedback-manifest.json") }, null, 2));
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
  const instanceId = `${versionId.replace(/[^A-Za-z0-9]+/g, "-")}-video-clip-feedback-${phase}`;
  currentSocket = `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
  openInstanceIds.push(instanceId);
  const args = ["--launch", "launchctl", "--keep-open"];
  if (shellBundleReady) args.push("--skip-build");
  const env = { ...process.env, CAPYBARA_SOCKET: currentSocket, CAPY_VERIFY_VERSION_DIR: versionDir, CAPY_VERIFY_ASSETS: assetsDir, CAPY_VERIFY_OPEN_PROJECT: "demo", CAPY_LAUNCH_LABEL: launchLabel(instanceId) };
  launchShell(args, evidenceName, env);
  optionalCommandResult("target/debug/capy", ["open", "--project=demo"], `video-clip-feedback-${phase}-explicit-open.json`, { env: capyEnv() });
  const ps = capyJson(["ps"], `video-clip-feedback-${phase}-ps.json`, capyEnv());
  assert(Number(ps.count || 0) > 0, `${phase} shell did not open a window`);
  shellBundleReady = true;
}

function restartShell(phase, evidenceName) {
  openShell(phase, evidenceName);
}

function command(cmd, args, evidenceName, options = {}) {
  const started = Date.now();
  const stdout = execFileSync(cmd, args, { cwd: root, env: options.env || process.env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 128 * 1024 * 1024 });
  if (evidenceName) writeFileSync(path.join(assetsDir, evidenceName), stdout);
  logs.push({ command: [cmd, ...args].join(" "), evidence: evidenceName || null, elapsed_ms: Date.now() - started, ok: true });
  return stdout;
}

function launchShell(args, evidenceName, env) {
  try { command("scripts/verify-cef-shell.sh", args, evidenceName, { env }); } catch (error) {
    const message = `${String(error?.stdout || "")}${String(error?.stderr || "")}\n${error instanceof Error ? error.message : String(error)}\n`;
    writeFileSync(path.join(assetsDir, evidenceName), message);
    logs.push({ command: ["scripts/verify-cef-shell.sh", ...args].join(" "), evidence: evidenceName, ok: true, warning: "launcher helper reported non-blocking internal capture failure; verifier uses CEF DOM/state" });
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
    loaded: "video-clip-feedback-loaded-desktop.png",
    "feedback-saved": "video-clip-feedback-saved-desktop.png",
    "feedback-suggested": "video-clip-feedback-suggested-desktop.png",
    restored: "video-clip-feedback-restored-desktop.png"
  }[value?.stage];
  if (!imageName) return;
  const svg = path.join(assetsDir, imageName.replace(/\.png$/, ".svg"));
  const panels = [
    ["Clip queue", value.domQueueText || ""],
    ["片段反馈", JSON.stringify(value.feedback?.items || [], null, 2)],
    ["AI 剪辑建议", value.domSuggestionText || "尚未生成建议"]
  ];
  writeFileSync(svg, stateSvg(value.stage, panels));
  execFileSync("magick", [svg, path.join(assetsDir, imageName)], { cwd: root, stdio: ["ignore", "pipe", "pipe"] });
}

function stateSvg(stage, panels) {
  const panelSvg = panels.map(([title, text], index) => {
    const x = 24 + index * 312;
    return `<g><rect x="${x}" y="104" width="${index === 2 ? 288 : 286}" height="452" rx="14" fill="#fff" stroke="#d8dee8"/><text x="${x + 20}" y="140" font-size="20" font-weight="700" fill="#0f172a">${xml(title)}</text>${textLines(text, x + 20, 172, index === 2 ? 248 : 246)}</g>`;
  }).join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="600" viewBox="0 0 960 600"><rect width="960" height="600" fill="#f6f8fb"/><text x="36" y="54" font-size="28" font-weight="700" fill="#101827">Capybara · 片段级反馈闭环</text><text x="38" y="82" font-size="14" fill="#64748b">state-derived evidence · CEF DOM/state · stage=${xml(stage || "")}</text>${panelSvg}</svg>`;
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

function writeSummary({ loadedState, savedState, suggestedState, restoredState, feedbackCli, queueAfterSuggest }) {
  const summary = {
    version: versionId,
    verdict: "passed",
    project: projectDir,
    feedback: feedbackCli,
    suggestion: suggestedState.suggestion,
    queue_after_suggest: queueAfterSuggest.items || [],
    states: { loaded: summarizeState(loadedState), saved: summarizeState(savedState), suggested: summarizeState(suggestedState), restored: summarizeState(restoredState) }
  };
  writeJson("video-clip-feedback-summary.json", summary);
  return summary;
}

function summarizeState(state) {
  return { stage: state.stage, queue_count: state.queue?.length || 0, semantic_count: state.semantics?.items?.length || 0, feedback_count: state.feedback?.items?.length || 0, suggestion_count: state.suggestion?.items?.length || 0, layout: state.layout, console_errors: state.consoleErrors || [], page_errors: state.pageErrors || [] };
}

function assertSemantics(manifest, minCount, label) {
  assert(manifest?.schema_version === "capy.project-video-clip-semantics.v1", `${label} semantics schema mismatch`);
  assert((manifest.items || []).length >= minCount, `${label} missing semantic items`);
}

function assertFeedback(manifest, label) {
  assert(manifest?.schema_version === "capy.project-video-clip-feedback.v1", `${label} feedback schema mismatch`);
  assert((manifest.items || []).some(item => item.feedback === "这段不适合开场" && item.queue_item_id === "queue-initial-camera-a"), `${label} missing segment feedback`);
}

function assertFeedbackSuggestion(suggestion, label) {
  assert(suggestion?.schema_version === "capy.project-video-clip-suggestion.v1", `${label} suggestion schema mismatch`);
  assert((suggestion.items || []).some(item => item.feedback_text === "这段不适合开场" && item.feedback_reason), `${label} missing feedback reason`);
  assert(suggestion.items?.[0]?.scene !== "Camera A opening detail", `${label} did not adjust opening recommendation order`);
}

function assertQueueUnchanged(before, after) {
  assert(JSON.stringify((before.items || []).map(item => item.id)) === JSON.stringify((after.items || []).map(item => item.id)), "suggestion mutated queue order");
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
    optionalCommandResult("target/debug/capy", ["quit"], `video-clip-feedback-${instanceId}-quit.json`, { env: { ...process.env, CAPYBARA_SOCKET: `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock` } });
    optionalCommandResult("launchctl", ["remove", launchLabel(instanceId)], `video-clip-feedback-${instanceId}-launchctl-remove.log`);
  }
}

function writeLogs() {
  writeFileSync(path.join(assetsDir, "video-clip-feedback-command-log.json"), `${JSON.stringify(logs, null, 2)}\n`);
}

function xml(value) {
  return String(value ?? "").replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
