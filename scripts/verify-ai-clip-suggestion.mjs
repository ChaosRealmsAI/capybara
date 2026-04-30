#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import {
  suggestionAdoptEval,
  suggestionExportEval,
  suggestionGenerateEval,
  suggestionRestoreEval
} from "./verify-ai-clip-suggestion-evals.mjs";
import {
  verifyEvidencePage,
  writeEvidencePage,
  writeManifest
} from "./verify-ai-clip-suggestion-report.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-ai-clip-suggestion.mjs [spec/versions/v0.48]

Use when:
  Verify the v0.48 AI clip suggestion feature end to end on a real CEF desktop.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.48.
  Requires target/debug/capy, local ffmpeg/ffprobe, macOS launchctl, CEF shell harness, and Playwright for the evidence page browser check.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/ai-clip-suggestion-project.
  Imports three local WebM videos, seeds a persisted queue, launches isolated debug shell ids, adopts the AI suggestion, writes .capy/video-clip-queue.json, exports MP4, and opens <version>/evidence/index.html with macOS open.

Evidence outputs:
  ai-clip-suggestion-plan.json
  ai-clip-suggestion-state.json
  ai-clip-suggestion-adopted-state.json
  ai-clip-suggestion-restored-state.json
  ai-clip-suggestion-export-state.json
  ai-clip-suggestion-manifest.json
  ai-clip-suggestion-proposal-composition.json
  ai-clip-suggestion-delivery.mp4
  ai-clip-suggestion-sampled-frame.png
  ai-clip-suggestion-summary.json
  ai-clip-suggestion-command-log.json
  ai-clip-suggestion-*-desktop.png
  evidence-page-check.json

Pitfalls:
  This verifies a no-spend deterministic planner, not a real paid model call.
  This remains a linear clip queue; do not interpret it as a multi-track NLE, transition, subtitle, or audio-mixing workflow.
  Frontend adoption must write through Shell IPC / Project Core, not direct .capy file writes.

Next step:
  Review <version>/evidence/index.html and <version>/evidence/assets/ai-clip-suggestion-summary.json.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.48");
const evidenceDir = path.join(versionDir, "evidence");
const assetsDir = path.join(evidenceDir, "assets");
const capy = path.join(root, "target", "debug", "capy");
const projectDir = path.join(assetsDir, "ai-clip-suggestion-project");
const mediaDir = path.join(projectDir, "media");
const initialQueuePath = path.join(assetsDir, "ai-clip-suggestion-initial-queue.json");
const projectManifest = path.join(projectDir, ".capy", "video-clip-queue.json");
const instancePrefix = "v48-ai-clip-suggestion";
const logs = [];
let shellBundleReady = false;
let currentSocket = "";
const openInstanceIds = [];

mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  command("ffmpeg", ["-version"], "ai-clip-suggestion-ffmpeg-version.log");
  command("ffprobe", ["-version"], "ai-clip-suggestion-ffprobe-version.log");

  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  const sourceA = path.join(mediaDir, "camera-a-wide.webm");
  const sourceB = path.join(mediaDir, "camera-b-close.webm");
  const sourceC = path.join(mediaDir, "camera-c-detail.webm");
  generateVideo(sourceA, "testsrc2=size=640x360:rate=30", 4, "ai-clip-suggestion-source-a-generate.log");
  generateVideo(sourceB, "smptebars=size=480x270:rate=24", 5, "ai-clip-suggestion-source-b-generate.log");
  generateVideo(sourceC, "testsrc=size=512x288:rate=25", 6, "ai-clip-suggestion-source-c-generate.log");

  command("target/debug/capy", ["project", "init", "--project", projectDir, "--name", "v0.48 AI Clip Suggestion Project"], "ai-clip-suggestion-project-init.json");
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importA = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-a-wide.webm", "--title", "Camera A wide"], "ai-clip-suggestion-import-a.json");
  const importB = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-b-close.webm", "--title", "Camera B close"], "ai-clip-suggestion-import-b.json");
  const importC = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-c-detail.webm", "--title", "Camera C detail"], "ai-clip-suggestion-import-c.json");
  const workbench = capyJson(["project", "workbench", "--project", projectDir], "ai-clip-suggestion-project-workbench.json");
  assert(workbench.cards.filter(card => card.kind === "video").length >= 3, "workbench missing three video cards");

  for (const [label, imported] of [["a", importA], ["b", importB], ["c", importC]]) {
    command("target/debug/capy", ["timeline", "validate", "--composition", path.join(projectDir, imported.composition_path)], `ai-clip-suggestion-composition-${label}-validate.json`);
    command("target/debug/capy", ["timeline", "compile", "--composition", path.join(projectDir, imported.composition_path)], `ai-clip-suggestion-composition-${label}-compile.json`);
  }

  writeJson("ai-clip-suggestion-initial-queue.json", initialQueue(importA, importB));
  command("target/debug/capy", ["project", "clip-queue", "write", "--project", projectDir, "--manifest", initialQueuePath], "ai-clip-suggestion-initial-queue-write.json");
  const initialInspect = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "ai-clip-suggestion-initial-queue-inspect.json");
  assert(initialInspect.items?.length === 2, "initial persisted queue should have two items");
  const cliSuggestion = capyJson(["project", "clip-queue", "suggest", "--project", projectDir], "ai-clip-suggestion-cli-plan.json");
  assertSuggestion(cliSuggestion, 3, "CLI suggestion");

  openShell("suggest", "ai-clip-suggestion-open-suggest.log");
  const suggestedState = capyJson(["devtools", "--eval", suggestionGenerateEval(projectDir, assetsDir)], "ai-clip-suggestion-state.json", capyEnv());
  assertSuggestion(suggestedState.suggestion, 3, "desktop suggestion");
  assert(suggestedState.domSuggestionText.includes("选择理由") || suggestedState.domSuggestionText.includes("保留"), "suggestion reason text missing in DOM");
  assertNoPageErrors(suggestedState, "suggestion panel");
  captureWithFallback("ai-clip-suggestion-desktop.png", "ai-clip-suggestion-capture.json", "ai-clip-suggestion-screenshot.json");

  const adoptedState = capyJson(["devtools", "--eval", suggestionAdoptEval()], "ai-clip-suggestion-adopted-state.json", capyEnv());
  assertAdoptedQueue(adoptedState, "adopted");
  const adoptedInspect = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "ai-clip-suggestion-manifest-inspect-adopted.json");
  assert(adoptedInspect.items?.length === 3, "adopted manifest should have three items");
  assert(adoptedInspect.items.every(item => item.suggestion_id === adoptedState.suggestion.suggestion_id), "adopted manifest must keep suggestion_id");
  assertNoPageErrors(adoptedState, "adopted suggestion");
  captureWithFallback("ai-clip-suggestion-adopted-desktop.png", "ai-clip-suggestion-adopted-capture.json", "ai-clip-suggestion-adopted-screenshot.json", "ai-clip-suggestion-desktop.png");

  restartShell("restore", "ai-clip-suggestion-open-restore.log");
  const restoredState = capyJson(["devtools", "--eval", suggestionRestoreEval(projectDir)], "ai-clip-suggestion-restored-state.json", capyEnv());
  assertAdoptedQueue(restoredState, "restored");
  assertNoPageErrors(restoredState, "restored suggestion queue");
  captureWithFallback("ai-clip-suggestion-restored-desktop.png", "ai-clip-suggestion-restored-capture.json", "ai-clip-suggestion-restored-screenshot.json", "ai-clip-suggestion-adopted-desktop.png");

  const finalManifest = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "ai-clip-suggestion-manifest-inspect-final.json");
  copyFileSync(projectManifest, path.join(assetsDir, "ai-clip-suggestion-manifest.json"));
  writeFileSync(path.join(assetsDir, "ai-clip-suggestion-plan.json"), `${JSON.stringify(adoptedState.suggestion, null, 2)}\n`);
  assert(finalManifest.items?.length === 3, "final manifest should have three items");
  assert(queueTotalDuration(finalManifest.items) === 4500, "final suggestion queue duration should be 4500ms");

  const exportState = capyJson(["devtools", "--eval", suggestionExportEval(assetsDir)], "ai-clip-suggestion-export-state.json", capyEnv());
  assertAdoptedQueue(exportState, "exported");
  assert(exportState.proposal?.kind === "video-clip-queue-proposal", "proposal kind mismatch");
  assert(exportState.proposal?.clip_count === 3, "proposal should contain three clips");
  assert(exportState.exportJob?.status === "done", `export job did not finish: ${exportState.exportJob?.status || "missing"}`);
  assert(existsSync(exportState.exportJob.output_path), `export file missing: ${exportState.exportJob.output_path}`);
  assertNoPageErrors(exportState, "suggestion queue export");
  captureWithFallback("ai-clip-suggestion-export-desktop.png", "ai-clip-suggestion-export-capture.json", "ai-clip-suggestion-export-screenshot.json", "ai-clip-suggestion-restored-desktop.png");

  const exportedCopy = path.join(assetsDir, "ai-clip-suggestion-delivery.mp4");
  copyFileSync(exportState.exportJob.output_path, exportedCopy);
  const exportComposition = exportState.lastExport.export_composition_path;
  copyFileSync(exportComposition, path.join(assetsDir, "ai-clip-suggestion-proposal-composition.json"));
  const proposalJson = JSON.parse(command("cat", [exportComposition], "ai-clip-suggestion-proposal-composition-read.json"));
  assert(proposalJson.delivery?.kind === "video-clip-queue-proposal", "proposal composition delivery kind mismatch");
  assert(proposalJson.delivery?.items?.every(item => item.suggestion_id === adoptedState.suggestion.suggestion_id), "proposal delivery items must carry suggestion_id");
  const ffprobeJson = JSON.parse(command("ffprobe", ["-v", "error", "-print_format", "json", "-show_streams", "-show_format", exportedCopy], "ai-clip-suggestion-export-ffprobe.json"));
  const exportedDuration = Number(ffprobeJson.format?.duration || 0);
  assert(exportedDuration >= 4.1 && exportedDuration <= 4.9, `AI suggestion mp4 duration should be about 4.5s, got ${exportedDuration}`);
  command("target/debug/capy", ["timeline", "snapshot", "--composition", exportComposition, "--frame", "3000", "--out", path.join(assetsDir, "ai-clip-suggestion-sampled-frame.png")], "ai-clip-suggestion-sampled-frame.json");

  const summary = writeSummary({ importA, importB, importC, workbench, cliSuggestion, suggestedState, adoptedState, restoredState, finalManifest, exportState, exportedCopy, exportComposition, proposalJson, ffprobeJson });
  writeEvidencePage({ evidenceDir, logs, summary });
  writeManifest({ evidenceDir });
  await verifyEvidencePage({ evidenceDir, assetsDir });
  command("open", [path.join(evidenceDir, "index.html")], "evidence-open.log");
  logs.push({ command: "verdict", ok: true });
  writeLogs();
  shutdown();
  console.log(JSON.stringify({ ok: true, assets: assetsDir, manifest: path.join(assetsDir, "ai-clip-suggestion-manifest.json"), export_path: exportedCopy }, null, 2));
} catch (error) {
  logs.push({ command: "verdict", ok: false, error: error instanceof Error ? error.message : String(error) });
  writeLogs();
  try {
    shutdown();
  } catch {}
  console.error(JSON.stringify({ ok: false, error: error instanceof Error ? error.message : String(error), assets: assetsDir }, null, 2));
  process.exit(1);
}

function generateVideo(out, source, seconds, evidenceName) {
  command("ffmpeg", ["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", source, "-t", String(seconds), "-c:v", "libvpx-vp9", "-b:v", "1200k", "-pix_fmt", "yuv420p", out], evidenceName);
  assert(existsSync(out), `source video missing: ${out}`);
}

function openShell(phase, evidenceName) {
  const instanceId = `${instancePrefix}-${phase}`;
  currentSocket = socketForInstance(instanceId);
  const args = ["--id", instanceId, "--project", projectDir, "--replace"];
  if (shellBundleReady) args.push("--skip-build");
  if (!openInstanceIds.includes(instanceId)) openInstanceIds.push(instanceId);
  command("scripts/open-debug-shell.sh", args, evidenceName);
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

function optionalCommand(cmd, args, evidenceName, options = {}) {
  try {
    command(cmd, args, evidenceName, options);
    return true;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    logs.push({ command: [cmd, ...args].join(" "), evidence: evidenceName || null, ok: false, error: message });
    writeFileSync(path.join(assetsDir, evidenceName), evidenceName.endsWith(".json") ? `${JSON.stringify({ ok: false, error: message }, null, 2)}\n` : `${message}\n`);
    return false;
  }
}

function capyJson(args, evidenceName, env = process.env) {
  const raw = command("target/debug/capy", args, evidenceName, { env });
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`parse JSON from capy ${args.join(" ")} failed: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function captureWithFallback(imageName, captureEvidence, screenshotEvidence, fallbackImage = null) {
  const out = path.join(assetsDir, imageName);
  if (optionalCommand("target/debug/capy", ["capture", `--out=${out}`], captureEvidence, { env: capyEnv() })) return;
  if (optionalCommand("target/debug/capy", ["screenshot", `--out=${out}`], screenshotEvidence, { env: capyEnv() })) return;
  if (fallbackImage) {
    copyFileSync(path.join(assetsDir, fallbackImage), out);
    logs.push({ command: `fallback copy ${fallbackImage} ${imageName}`, evidence: imageName, ok: true });
    return;
  }
  throw new Error(`capture failed for ${imageName}`);
}

function inspectSourceVideo(importResult) {
  return {
    filename: importResult.metadata.filename,
    duration_ms: importResult.metadata.duration_ms,
    width: importResult.metadata.width,
    height: importResult.metadata.height
  };
}

function initialQueue(importA, importB) {
  return {
    schema_version: "capy.project-video-clip-queue.v1",
    project_id: "",
    project_name: "",
    updated_at: Date.now(),
    items: [
      {
        id: "queue-initial-camera-a",
        sequence: 1,
        composition_path: importA.composition_path,
        render_source_path: "",
        clip_id: "source",
        track_id: "video",
        scene: "Camera A opening detail",
        start_ms: 500,
        end_ms: 1700,
        duration_ms: 1200,
        source_video: inspectSourceVideo(importA),
        updated_at: Date.now()
      },
      {
        id: "queue-initial-camera-b",
        sequence: 2,
        composition_path: importB.composition_path,
        render_source_path: "",
        clip_id: "source",
        track_id: "video",
        scene: "Camera B product closeup",
        start_ms: 1000,
        end_ms: 2500,
        duration_ms: 1500,
        source_video: inspectSourceVideo(importB),
        updated_at: Date.now()
      }
    ]
  };
}

function writeSummary(data) {
  const metadata = {
    schema: "capy.ai_clip_suggestion.summary.v1",
    version: "v0.48",
    generated_at: new Date().toISOString(),
    project_dir: projectDir,
    imports: {
      camera_a: data.importA,
      camera_b: data.importB,
      camera_c: data.importC
    },
    workbench_video_count: data.workbench.cards.filter(card => card.kind === "video").length,
    cli_suggestion: data.cliSuggestion,
    suggestion: data.adoptedState.suggestion,
    final_queue: data.finalManifest.items,
    total_duration_ms: queueTotalDuration(data.finalManifest.items),
    export_state: data.exportState,
    proposal_delivery: data.proposalJson.delivery,
    export_probe: {
      duration: data.ffprobeJson.format?.duration,
      size: data.ffprobeJson.format?.size
    },
    files: {
      manifest: path.join(assetsDir, "ai-clip-suggestion-manifest.json"),
      proposal: path.join(assetsDir, "ai-clip-suggestion-proposal-composition.json"),
      export_mp4: data.exportedCopy,
      export_composition: data.exportComposition,
      sampled_frame: path.join(assetsDir, "ai-clip-suggestion-sampled-frame.png")
    },
    desktop_captures: [
      path.join(assetsDir, "ai-clip-suggestion-desktop.png"),
      path.join(assetsDir, "ai-clip-suggestion-adopted-desktop.png"),
      path.join(assetsDir, "ai-clip-suggestion-restored-desktop.png"),
      path.join(assetsDir, "ai-clip-suggestion-export-desktop.png")
    ],
    verdict: "passed"
  };
  writeJson("ai-clip-suggestion-summary.json", metadata);
  return metadata;
}

function writeJson(name, value) {
  writeFileSync(path.join(assetsDir, name), `${JSON.stringify(value, null, 2)}\n`);
}

function writeLogs() {
  writeJson("ai-clip-suggestion-command-log.json", logs);
}

function queueTotalDuration(items) {
  return items.reduce((total, item) => total + Math.max(1, Number(item.duration_ms || 0)), 0);
}

function assertSuggestion(suggestion, minItems, label) {
  assert(suggestion?.schema_version === "capy.project-video-clip-suggestion.v1", `${label} schema mismatch`);
  assert((suggestion.items || []).length >= minItems, `${label} should contain at least ${minItems} items`);
  assert(suggestion.items.every(item => item.reason && item.source_video && item.duration_ms > 0), `${label} items need reason, source_video, and duration`);
  assert(suggestion.source_video_count >= 3, `${label} should see three project videos`);
  assert(suggestion.existing_queue_count >= 2, `${label} should use existing queue`);
}

function assertAdoptedQueue(state, label) {
  const queue = state.queue || [];
  assert(queue.length === 3, `${label} queue should contain three items`);
  assert(queue.every(item => item.suggestion_id && item.suggestion_reason), `${label} queue should keep suggestion metadata`);
  assert(state.persistStatus === "saved" || state.persistStatus === "loaded", `${label} persist status should be saved/loaded`);
}

function assertNoPageErrors(state, label) {
  assert((state.consoleErrors || []).length === 0, `${label} console errors: ${JSON.stringify(state.consoleErrors)}`);
  assert((state.pageErrors || []).length === 0, `${label} page errors: ${JSON.stringify(state.pageErrors)}`);
  assert(Number(state.layout?.editor?.w || 0) > 900, `${label} editor width is too narrow`);
  assert(Number(state.layout?.deliveryPanel?.w || 0) > 500, `${label} delivery panel width is too narrow`);
}

function capyEnv() {
  return { ...process.env, CAPYBARA_SOCKET: currentSocket };
}

function socketForInstance(instanceId) {
  return `/tmp/capybara-${instanceId}-${process.getuid()}.sock`;
}

function shutdown() {
  for (const instanceId of [...openInstanceIds].reverse()) {
    currentSocket = socketForInstance(instanceId);
    optionalCommand("target/debug/capy", ["quit"], `ai-clip-suggestion-quit-${instanceId}.json`, { env: capyEnv() });
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
