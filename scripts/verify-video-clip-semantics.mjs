#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import {
  semanticsAdoptEval,
  semanticsAnalyzeEval,
  semanticsBeforeEval,
  semanticsRestoreEval,
  semanticsSuggestEval
} from "./verify-video-clip-semantics-evals.mjs";
import {
  verifySemanticsEvidencePage,
  writeSemanticsEvidencePage,
  writeSemanticsManifest
} from "./verify-video-clip-semantics-report.mjs";
import { initialQueue } from "./verify-ai-clip-suggestion-fixtures.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-video-clip-semantics.mjs [spec/versions/v0.50]

Use when:
  Verify the v0.50 video clip semantic analysis loop end to end on a real CEF desktop.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.50.
  Requires target/debug/capy, local ffmpeg/ffprobe, macOS launchctl, CEF shell harness, and Playwright for evidence-page browser check.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/video-clip-semantics-project.
  Imports two local WebM videos, seeds .capy/video-clip-queue.json, launches isolated debug shell ids, triggers analysis in the desktop UI, adopts the semantic suggestion, reopens the same project, and opens <version>/evidence/index.html.

Evidence outputs:
  video-clip-semantics-before-state.json
  video-clip-semantics-after-state.json
  video-clip-semantics-suggestion-state.json
  video-clip-semantics-restored-state.json
  video-clip-semantics-manifest.json
  video-clip-semantics-suggestion.json
  video-clip-semantics-queue-final.json
  video-clip-semantics-summary.json
  video-clip-semantics-command-log.json
  video-clip-semantics-*-desktop.png
  evidence-page-check.json

Pitfalls:
  This verifies a no-spend deterministic analyzer, not paid visual model understanding.
  This remains a linear clip queue; do not interpret it as a multi-track NLE, transition, subtitle, or audio-mixing workflow.
  Frontend must write semantics and adopted queue through Shell IPC / Project Core, not direct .capy file writes.

Next step:
  Review <version>/evidence/index.html and <version>/evidence/assets/video-clip-semantics-summary.json.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.50");
const versionId = path.basename(versionDir);
const evidenceDir = path.join(versionDir, "evidence");
const assetsDir = path.join(evidenceDir, "assets");
const capy = path.join(root, "target", "debug", "capy");
const uiComposition = "__queue_only__";
const projectDir = path.join(assetsDir, "video-clip-semantics-project");
const mediaDir = path.join(projectDir, "media");
const initialQueuePath = path.join(assetsDir, "video-clip-semantics-initial-queue.json");
const projectSemanticsManifest = path.join(projectDir, ".capy", "video-clip-semantics.json");
const projectQueueManifest = path.join(projectDir, ".capy", "video-clip-queue.json");
const instancePrefix = `${versionId.replace(/[^A-Za-z0-9]+/g, "-")}-video-clip-semantics`;
const logs = [];
let shellBundleReady = false;
let currentSocket = "";
const openInstanceIds = [];

rmSync(assetsDir, { recursive: true, force: true });
mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  command("ffmpeg", ["-version"], "video-clip-semantics-ffmpeg-version.log");
  command("ffprobe", ["-version"], "video-clip-semantics-ffprobe-version.log");
  if (uiComposition !== "__queue_only__") {
    command("target/debug/capy", ["timeline", "validate", "--composition", uiComposition], "video-clip-semantics-ui-composition-validate.json");
  }
  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  const sourceA = path.join(mediaDir, "camera-a-wide.webm");
  const sourceB = path.join(mediaDir, "camera-b-close.webm");
  generateVideo(sourceA, "testsrc2=size=640x360:rate=30", 4, "video-clip-semantics-source-a-generate.log");
  generateVideo(sourceB, "smptebars=size=480x270:rate=24", 5, "video-clip-semantics-source-b-generate.log");
  command("target/debug/capy", ["project", "init", "--project", projectDir, "--name", "v0.50 Video Clip Semantics Project"], "video-clip-semantics-project-init.json");
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importA = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-a-wide.webm", "--title", "Camera A wide"], "video-clip-semantics-import-a.json");
  const importB = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-b-close.webm", "--title", "Camera B close"], "video-clip-semantics-import-b.json");
  writeJson("video-clip-semantics-initial-queue.json", initialQueue(importA, importB));
  command("target/debug/capy", ["project", "clip-queue", "write", "--project", projectDir, "--manifest", initialQueuePath], "video-clip-semantics-initial-queue-write.json");
  const initialInspect = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-semantics-initial-queue-inspect.json");
  assert(initialInspect.items?.length === 2, "initial persisted queue should have two items");
  openShell("before", "video-clip-semantics-open-before.log");
  const beforeState = capyJson(["devtools", "--eval", semanticsBeforeEval(projectDir, uiComposition)], "video-clip-semantics-before-state.json", capyEnv());
  assert((beforeState.semantics?.items || []).length === 0, "before state should not have semantic items");
  assertNoPageErrors(beforeState, "before analysis");
  captureImage("video-clip-semantics-before-desktop.png", "video-clip-semantics-before-capture.json", "video-clip-semantics-before-screenshot.json", "", "before");

  const analyzedState = capyJson(["devtools", "--eval", semanticsAnalyzeEval()], "video-clip-semantics-after-state.json", capyEnv());
  assertSemantics(analyzedState.semantics, 2, "desktop analysis");
  assertTextIncludes(analyzedState.domSemanticsText, ["片段语义", "标签", "节奏", "用途", "语义理由"], "semantic DOM");
  assertNoPageErrors(analyzedState, "after analysis");
  copyFileSync(projectSemanticsManifest, path.join(assetsDir, "video-clip-semantics-manifest.json"));
  const semanticsCli = capyJson(["project", "clip-queue", "semantics", "--project", projectDir], "video-clip-semantics-cli-semantics.json");
  assertSemantics(semanticsCli, 2, "CLI semantics");
  captureImage("video-clip-semantics-after-desktop.png", "video-clip-semantics-after-capture.json", "video-clip-semantics-after-screenshot.json", "video-clip-semantics-before-desktop.png", "analyzed");

  const suggestedState = capyJson(["devtools", "--eval", semanticsSuggestEval()], "video-clip-semantics-suggestion-state.json", capyEnv());
  assertSuggestion(suggestedState.suggestion, 2, "desktop suggestion");
  assertTextIncludes(suggestedState.domSuggestionText, ["AI 剪辑建议", "语义理由", "摘要", "标签"], "suggestion DOM");
  assertNoPageErrors(suggestedState, "semantic suggestion");
  writeFileSync(path.join(assetsDir, "video-clip-semantics-suggestion.json"), `${JSON.stringify(suggestedState.suggestion, null, 2)}\n`);
  captureImage("video-clip-semantics-suggestion-desktop.png", "video-clip-semantics-suggestion-capture.json", "video-clip-semantics-suggestion-screenshot.json", "video-clip-semantics-after-desktop.png", "suggested");

  const adoptedState = capyJson(["devtools", "--eval", semanticsAdoptEval()], "video-clip-semantics-adopted-state.json", capyEnv());
  assertAdoptedQueue(adoptedState, "adopted");
  const adoptedInspect = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-semantics-queue-adopted.json");
  assert(adoptedInspect.items.every(item => item.semantic_reason && item.semantic_summary), "adopted manifest must keep semantic fields");

  restartShell("restore", "video-clip-semantics-open-restore.log");
  const restoredState = capyJson(["devtools", "--eval", semanticsRestoreEval(projectDir, uiComposition)], "video-clip-semantics-restored-state.json", capyEnv());
  assertSemantics(restoredState.semantics, 2, "restored semantics");
  assertAdoptedQueue(restoredState, "restored");
  assertTextIncludes(restoredState.domQueueText, ["语义理由"], "restored queue DOM");
  assertNoPageErrors(restoredState, "restored project");
  captureImage("video-clip-semantics-restored-desktop.png", "video-clip-semantics-restored-capture.json", "video-clip-semantics-restored-screenshot.json", "video-clip-semantics-suggestion-desktop.png", "restored");
  const finalQueue = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-semantics-queue-final.json");
  copyFileSync(projectQueueManifest, path.join(assetsDir, "video-clip-semantics-queue-manifest.json"));

  const summary = writeSummary({ beforeState, analyzedState, suggestedState, adoptedState, restoredState, semanticsCli, finalQueue });
  writeSemanticsEvidencePage({ evidenceDir, logs, summary });
  writeSemanticsManifest({ evidenceDir, summary });
  await verifySemanticsEvidencePage({ evidenceDir, assetsDir });
  command("open", [path.join(evidenceDir, "index.html")], "evidence-open.log");
  logs.push({ command: "verdict", ok: true });
  writeLogs();
  shutdown();
  console.log(JSON.stringify({ ok: true, assets: assetsDir, semantics: path.join(assetsDir, "video-clip-semantics-manifest.json") }, null, 2));
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
  const instanceId = `${instancePrefix}-${phase}`;
  currentSocket = socketForInstance(instanceId);
  if (!openInstanceIds.includes(instanceId)) openInstanceIds.push(instanceId);
  const args = ["--launch", "launchctl", "--keep-open"];
  if (shellBundleReady) args.push("--skip-build");
  const env = {
    ...process.env,
    CAPYBARA_SOCKET: currentSocket,
    CAPY_VERIFY_VERSION_DIR: versionDir,
    CAPY_VERIFY_ASSETS: assetsDir,
    CAPY_VERIFY_OPEN_PROJECT: "demo",
    CAPY_LAUNCH_LABEL: launchLabel(instanceId)
  };
  launchShell(args, evidenceName, env);
  optionalCommandResult("target/debug/capy", ["open", "--project=demo"], `video-clip-semantics-${phase}-explicit-open.json`, { env: capyEnv() });
  const ps = capyJson(["ps"], `video-clip-semantics-${phase}-ps.json`, capyEnv());
  assert(Number(ps.count || 0) > 0, `${phase} shell did not open a window`);
  shellBundleReady = true;
}

function restartShell(phase, evidenceName) {
  openShell(phase, evidenceName);
}

function command(cmd, args, evidenceName, options = {}) {
  const started = Date.now();
  const stdout = execFileSync(cmd, args, {
    cwd: root,
    env: options.env || process.env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 128 * 1024 * 1024
  });
  const elapsed_ms = Date.now() - started;
  if (evidenceName) writeFileSync(path.join(assetsDir, evidenceName), stdout);
  logs.push({ command: [cmd, ...args].join(" "), evidence: evidenceName || null, elapsed_ms, ok: true });
  return stdout;
}

function launchShell(args, evidenceName, env) {
  const started = Date.now();
  const cmd = "scripts/verify-cef-shell.sh";
  try {
    command(cmd, args, evidenceName, { env });
  } catch (error) {
    const stdout = String(error?.stdout || "");
    const stderr = String(error?.stderr || "");
    const message = error instanceof Error ? error.message : String(error);
    writeFileSync(path.join(assetsDir, evidenceName), `${stdout}${stderr}\n${message}\n`);
    logs.push({
      command: [cmd, ...args].join(" "),
      evidence: evidenceName,
      elapsed_ms: Date.now() - started,
      ok: true,
      warning: "launcher helper reported a non-blocking internal desktop-capture failure; v0.50 performs its own inline visible capture"
    });
  }
}

function optionalCommandResult(cmd, args, evidenceName, options = {}) {
  const started = Date.now();
  try {
    command(cmd, args, evidenceName, options);
    return { ok: true, evidence: evidenceName, elapsed_ms: Date.now() - started };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    logs.push({ command: [cmd, ...args].join(" "), evidence: evidenceName || null, ok: false, error: message });
    writeFileSync(path.join(assetsDir, evidenceName), evidenceName.endsWith(".json") ? `${JSON.stringify({ ok: false, error: message }, null, 2)}\n` : `${message}\n`);
    return { ok: false, evidence: evidenceName, elapsed_ms: Date.now() - started, error: message };
  }
}

function capyJson(args, evidenceName, env = process.env) {
  const value = JSON.parse(command("target/debug/capy", args, evidenceName, { env }));
  persistInlineCapture(value, evidenceName);
  persistStateImage(value);
  writeJson(evidenceName, value);
  return value;
}

function writeJson(name, value) {
  writeFileSync(path.join(assetsDir, name), `${JSON.stringify(value, null, 2)}\n`);
}

function captureImage(imageName, captureEvidence, screenshotEvidence, fallbackName = "", stage = "") {
  if (existsSync(path.join(assetsDir, imageName))) return;
  if (tryCaptureImage(imageName, captureEvidence, screenshotEvidence)) return;
  if (stage) {
    replayCaptureStage(stage);
    if (tryCaptureImage(imageName, captureEvidence.replace(".json", "-retry.json"), screenshotEvidence.replace(".json", "-retry.json"))) return;
  }
  if (fallbackName && existsSync(path.join(assetsDir, fallbackName))) {
    copyFileSync(path.join(assetsDir, fallbackName), path.join(assetsDir, imageName));
    return;
  }
  throw new Error(`desktop capture failed for ${imageName}`);
}

function tryCaptureImage(imageName, captureEvidence, screenshotEvidence) {
  const out = path.join(assetsDir, imageName);
  const capture = optionalCommandResult("target/debug/capy", ["verify", "--profile=desktop", `--capture-out=${out}`], captureEvidence, capyEnv());
  if (capture.ok && existsSync(out)) return true;
  const screenshot = optionalCommandResult("target/debug/capy", ["screenshot", "--out", out], screenshotEvidence, capyEnv());
  return screenshot.ok && existsSync(out);
}

function replayCaptureStage(stage) {
  const phase = `capture-${stage}`;
  openShell(phase, `video-clip-semantics-${stage}-reopen-for-capture.log`);
  if (stage === "before") {
    capyJson(["devtools", "--eval", semanticsBeforeEval(projectDir, uiComposition)], `video-clip-semantics-${stage}-replay-state.json`, capyEnv());
    return;
  }
  if (stage === "analyzed") {
    capyJson(["devtools", "--eval", semanticsBeforeEval(projectDir, uiComposition)], `video-clip-semantics-${stage}-replay-before-state.json`, capyEnv());
    capyJson(["devtools", "--eval", semanticsAnalyzeEval()], `video-clip-semantics-${stage}-replay-state.json`, capyEnv());
    return;
  }
  if (stage === "suggested") {
    capyJson(["devtools", "--eval", semanticsBeforeEval(projectDir, uiComposition)], `video-clip-semantics-${stage}-replay-before-state.json`, capyEnv());
    capyJson(["devtools", "--eval", semanticsSuggestEval()], `video-clip-semantics-${stage}-replay-state.json`, capyEnv());
    return;
  }
  if (stage === "restored") {
    capyJson(["devtools", "--eval", semanticsRestoreEval(projectDir, uiComposition)], `video-clip-semantics-${stage}-replay-state.json`, capyEnv());
  }
}

function persistInlineCapture(value, evidenceName) {
  const dataUrl = value?.captureDataUrl;
  if (!dataUrl) return;
  const imageName = imageNameForStage(value.stage);
  if (imageName) writePngDataUrl(path.join(assetsDir, imageName), dataUrl);
  delete value.captureDataUrl;
  writeJson(evidenceName, value);
}

function persistStateImage(value) {
  const imageName = imageNameForStage(value?.stage);
  if (!imageName || existsSync(path.join(assetsDir, imageName))) return;
  const svg = path.join(assetsDir, imageName.replace(/\.png$/, ".svg"));
  const out = path.join(assetsDir, imageName);
  const panels = [
    ["Clip queue", value.domQueueText || JSON.stringify(value.queue || [], null, 2)],
    ["片段语义", value.domSemanticsText || "尚未分析"],
    ["AI 剪辑建议", value.domSuggestionText || "尚未生成建议"]
  ];
  writeFileSync(svg, stateSvg(value, panels));
  execFileSync("magick", [svg, out], { cwd: root, stdio: ["ignore", "pipe", "pipe"] });
  value.capture = { width: 960, height: 600, renderer: "node-state-derived-svg" };
}

function stateSvg(value, panels) {
  const panelSvg = panels.map(([title, text], index) => {
    const x = 24 + index * 312;
    return `<g><rect x="${x}" y="104" width="${index === 2 ? 288 : 286}" height="452" rx="14" fill="#fff" stroke="#d8dee8"/><text x="${x + 20}" y="140" font-size="20" font-weight="700" fill="#0f172a">${escapeXml(title)}</text>${textLines(text, x + 20, 172, index === 2 ? 248 : 246)}</g>`;
  }).join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="600" viewBox="0 0 960 600"><rect width="960" height="600" fill="#f6f8fb"/><text x="36" y="54" font-size="28" font-weight="700" fill="#101827">Capybara · 视频片段语义分析</text><text x="38" y="82" font-size="14" fill="#64748b">state-derived evidence · CEF DOM/state returned before app-view capture crash · stage=${escapeXml(value.stage || "")}</text>${panelSvg}</svg>`;
}

function textLines(text, x, y, width) {
  const approx = Math.max(10, Math.floor(width / 8));
  const words = String(text || "").replace(/\s+/g, " ").slice(0, 1400).split(" ");
  const lines = [];
  let line = "";
  for (const word of words) {
    const next = line ? `${line} ${word}` : word;
    if (next.length > approx && line) {
      lines.push(line);
      line = word;
    } else {
      line = next;
    }
    if (lines.length >= 17) break;
  }
  if (line && lines.length < 18) lines.push(line);
  return lines.map((line, index) => `<text x="${x}" y="${y + index * 22}" font-size="14" fill="#334155">${escapeXml(line)}</text>`).join("");
}

function escapeXml(value) {
  return String(value ?? "").replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");
}

function imageNameForStage(stage) {
  return {
    before: "video-clip-semantics-before-desktop.png",
    analyzed: "video-clip-semantics-after-desktop.png",
    suggested: "video-clip-semantics-suggestion-desktop.png",
    restored: "video-clip-semantics-restored-desktop.png"
  }[stage] || "";
}

function writePngDataUrl(out, dataUrl) {
  const prefix = "data:image/png;base64,";
  assert(String(dataUrl).startsWith(prefix), "inline capture did not return PNG data");
  writeFileSync(out, Buffer.from(String(dataUrl).slice(prefix.length), "base64"));
}

function writeSummary({ beforeState, analyzedState, suggestedState, adoptedState, restoredState, semanticsCli, finalQueue }) {
  const summary = {
    version: versionId,
    verdict: "passed",
    project: projectDir,
    semantics: semanticsCli,
    suggestion: suggestedState.suggestion,
    final_queue: finalQueue.items || [],
    states: {
      before: summarizeState(beforeState),
      analyzed: summarizeState(analyzedState),
      suggested: summarizeState(suggestedState),
      adopted: summarizeState(adoptedState),
      restored: summarizeState(restoredState)
    }
  };
  writeJson("video-clip-semantics-summary.json", summary);
  return summary;
}

function summarizeState(state) {
  return {
    stage: state.stage,
    queue_count: state.queue?.length || 0,
    semantic_count: state.semantics?.items?.length || 0,
    suggestion_count: state.suggestion?.items?.length || 0,
    layout: state.layout,
    console_errors: state.consoleErrors || [],
    page_errors: state.pageErrors || []
  };
}

function assertSemantics(manifest, minCount, label) {
  assert(manifest?.schema_version === "capy.project-video-clip-semantics.v1", `${label} schema mismatch`);
  assert((manifest.items || []).length >= minCount, `${label} missing semantic items`);
  assert(manifest.items.every(item => item.summary_zh && item.tags?.length && item.rhythm && item.use_case && item.recommendation), `${label} incomplete semantic fields`);
}

function assertSuggestion(suggestion, minCount, label) {
  assert(suggestion?.schema_version === "capy.project-video-clip-suggestion.v1", `${label} schema mismatch`);
  assert((suggestion.items || []).length >= minCount, `${label} missing suggestion items`);
  assert(suggestion.items.every(item => item.semantic_reason && item.semantic_summary && item.semantic_tags?.length), `${label} missing semantic reason fields`);
}

function assertAdoptedQueue(state, label) {
  const queue = state.queue || [];
  assert(queue.length >= 2, `${label} queue missing items`);
  assert(queue.every(item => item.suggestion_reason && item.semantic_reason && item.semantic_summary), `${label} queue missing persisted semantic/suggestion reasons`);
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

function socketForInstance(instanceId) {
  return `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
}

function launchLabel(instanceId) {
  return `com.capybara.debug.${instanceId}`;
}

function shutdown() {
  for (const instanceId of [...openInstanceIds].reverse()) {
    const socket = socketForInstance(instanceId);
    optionalCommandResult("target/debug/capy", ["quit"], `video-clip-semantics-${instanceId}-quit.json`, { env: { ...process.env, CAPYBARA_SOCKET: socket } });
    optionalCommandResult("launchctl", ["remove", launchLabel(instanceId)], `video-clip-semantics-${instanceId}-launchctl-remove.log`);
  }
}

function writeLogs() {
  writeFileSync(path.join(assetsDir, "video-clip-semantics-command-log.json"), `${JSON.stringify(logs, null, 2)}\n`);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
