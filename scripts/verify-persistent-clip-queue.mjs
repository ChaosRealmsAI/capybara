#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import {
  queueCreateAndPersistEval,
  queueExportEval,
  queueModifyAndPersistEval,
  queueRestoreEval
} from "./verify-persistent-clip-queue-evals.mjs";
import {
  verifyEvidencePage,
  writeEvidencePage,
  writeManifest
} from "./verify-persistent-clip-queue-report.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-persistent-clip-queue.mjs [spec/versions/v0.47]

Use when:
  Verify the v0.47 project-level persistent clip queue end to end.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.47.
  Requires target/debug/capy, cargo wef, local ffmpeg/ffprobe, macOS launchctl, CEF shell harness, and Playwright for the evidence page browser check.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/persistent-clip-queue-project.
  Builds/stages the desktop app, launches isolated debug shell ids for create/restore/final restore, then quits/removes those instances.
  Writes and inspects <project>/.capy/video-clip-queue.json.
  Opens <version>/evidence/index.html with macOS open after writing it.

Evidence outputs:
  persistent-clip-queue-import-a.json
  persistent-clip-queue-import-b.json
  persistent-clip-queue-created-state.json
  persistent-clip-queue-restored-state.json
  persistent-clip-queue-modified-state.json
  persistent-clip-queue-final-restored-state.json
  persistent-clip-queue-export-state.json
  persistent-clip-queue-manifest.json
  persistent-clip-queue-proposal-composition.json
  persistent-clip-queue-delivery.mp4
  persistent-clip-queue-sampled-frame.png
  persistent-clip-queue-summary.json
  persistent-clip-queue-command-log.json
  persistent-clip-queue-*-desktop.png
  evidence-page-check.json

Pitfalls:
  This verifies a persisted linear clip queue, not a multi-track nonlinear editor.
  The frontend must not write .capy directly; manifest writes go through Shell IPC and capy-project.
  Each reopen phase uses a distinct debug shell id so the persisted project manifest is verified without depending on same-process CEF restart behavior.

Next step:
  Review <version>/evidence/index.html and <version>/evidence/assets/persistent-clip-queue-summary.json.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.47");
const evidenceDir = path.join(versionDir, "evidence");
const assetsDir = path.join(evidenceDir, "assets");
const capy = path.join(root, "target", "debug", "capy");
const projectDir = path.join(assetsDir, "persistent-clip-queue-project");
const mediaDir = path.join(projectDir, "media");
const sourceA = path.join(mediaDir, "camera-a-wide.webm");
const sourceB = path.join(mediaDir, "camera-b-close.webm");
const projectManifest = path.join(projectDir, ".capy", "video-clip-queue.json");
const instancePrefix = "v47-persistent-clip-queue";
const logs = [];
let shellBundleReady = false;
let currentSocket = "";
const openInstanceIds = [];

mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  command("ffmpeg", ["-version"], "persistent-clip-queue-ffmpeg-version.log");
  command("ffprobe", ["-version"], "persistent-clip-queue-ffprobe-version.log");

  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  generateVideo(sourceA, "testsrc2=size=640x360:rate=30", 4, "persistent-clip-queue-source-a-generate.log");
  generateVideo(sourceB, "smptebars=size=480x270:rate=24", 5, "persistent-clip-queue-source-b-generate.log");
  writeJson("persistent-clip-queue-project-files-before-import.json", {
    schema: "capy.persistent_clip_queue.project_files.v1",
    project_dir: projectDir,
    files: listFiles(projectDir)
  });

  command("target/debug/capy", ["project", "init", "--project", projectDir, "--name", "v0.47 Persistent Clip Queue Project"], "persistent-clip-queue-project-init.json");
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importA = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-a-wide.webm", "--title", "Camera A wide"], "persistent-clip-queue-import-a.json", process.env);
  const importB = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-b-close.webm", "--title", "Camera B close"], "persistent-clip-queue-import-b.json", process.env);
  assert(importA.schema_version === "capy.project-video-import.v1", "unexpected import A schema");
  assert(importB.schema_version === "capy.project-video-import.v1", "unexpected import B schema");
  const workbench = capyJson(["project", "workbench", "--project", projectDir], "persistent-clip-queue-project-workbench.json", process.env);
  assert(workbench.cards.filter(card => card.kind === "video").length >= 2, "workbench missing two video cards");
  command("target/debug/capy", ["timeline", "validate", "--composition", path.join(projectDir, importA.composition_path)], "persistent-clip-queue-composition-a-validate.json");
  command("target/debug/capy", ["timeline", "validate", "--composition", path.join(projectDir, importB.composition_path)], "persistent-clip-queue-composition-b-validate.json");
  command("target/debug/capy", ["timeline", "compile", "--composition", path.join(projectDir, importA.composition_path)], "persistent-clip-queue-composition-a-compile.json");
  command("target/debug/capy", ["timeline", "compile", "--composition", path.join(projectDir, importB.composition_path)], "persistent-clip-queue-composition-b-compile.json");

  openShell("create", "persistent-clip-queue-open-create.log");
  const createdState = capyJson(["devtools", "--eval", queueCreateAndPersistEval(projectDir, assetsDir)], "persistent-clip-queue-created-state.json", capyEnv());
  assertQueueState(createdState, 3, "created");
  assert(createdState.persistStatus === "saved", "created queue did not report saved status");
  assert(existsSync(projectManifest), `queue manifest missing: ${projectManifest}`);
  const createdManifest = inspectManifest("persistent-clip-queue-manifest-created.json");
  assert(createdManifest.items?.length === 3, "created manifest should have three items");
  assertNoPageErrors(createdState, "created queue");
  captureWithFallback("persistent-clip-queue-created-desktop.png", "persistent-clip-queue-created-capture.json", "persistent-clip-queue-created-screenshot.json");

  restartShell("restore", "persistent-clip-queue-open-restore.log");
  const restoredState = capyJson(["devtools", "--eval", queueRestoreEval(projectDir, "restored")], "persistent-clip-queue-restored-state.json", capyEnv());
  assertQueueState(restoredState, 3, "restored");
  assert(restoredState.queue[0]?.source_video?.filename === "camera-a-wide.webm", "restored queue item 1 should be camera A");
  assert(restoredState.queue[1]?.source_video?.filename === "camera-b-close.webm", "restored queue item 2 should be camera B");
  assertNoPageErrors(restoredState, "restored queue");
  captureWithFallback("persistent-clip-queue-restored-desktop.png", "persistent-clip-queue-restored-capture.json", "persistent-clip-queue-restored-screenshot.json", "persistent-clip-queue-created-desktop.png");

  const modifiedState = capyJson(["devtools", "--eval", queueModifyAndPersistEval()], "persistent-clip-queue-modified-state.json", capyEnv());
  assertQueueState(modifiedState, 3, "modified");
  assert(modifiedState.queue[0]?.source_video?.filename === "camera-b-close.webm", "modified queue should start with camera B");
  assert(modifiedState.queue.every(item => item.scene !== "Camera B temporary tail"), "removed temporary tail must not remain");
  assert(modifiedState.queue.some(item => item.scene === "Camera A ending detail"), "new Camera A ending detail missing");
  const modifiedManifest = inspectManifest("persistent-clip-queue-manifest-modified.json");
  assert(modifiedManifest.items?.length === 3, "modified manifest should have three items");
  assert(modifiedManifest.items?.[0]?.source_video?.filename === "camera-b-close.webm", "modified manifest should start with camera B");
  assertNoPageErrors(modifiedState, "modified queue");
  captureWithFallback("persistent-clip-queue-modified-desktop.png", "persistent-clip-queue-modified-capture.json", "persistent-clip-queue-modified-screenshot.json", "persistent-clip-queue-restored-desktop.png");

  restartShell("final", "persistent-clip-queue-open-final-restore.log");
  const finalRestoredState = capyJson(["devtools", "--eval", queueRestoreEval(projectDir, "final-restored")], "persistent-clip-queue-final-restored-state.json", capyEnv());
  assertQueueState(finalRestoredState, 3, "final restored");
  assert(finalRestoredState.queue[0]?.source_video?.filename === "camera-b-close.webm", "final restored queue should start with camera B");
  assert(finalRestoredState.queue.some(item => item.scene === "Camera A ending detail"), "final restored queue missing new item");
  assert(finalRestoredState.queue.every(item => item.scene !== "Camera B temporary tail"), "final restored queue must not revive removed item");
  assertNoPageErrors(finalRestoredState, "final restored queue");
  captureWithFallback("persistent-clip-queue-final-restored-desktop.png", "persistent-clip-queue-final-restored-capture.json", "persistent-clip-queue-final-restored-screenshot.json", "persistent-clip-queue-modified-desktop.png");

  const finalManifest = inspectManifest("persistent-clip-queue-manifest-inspect-final.json");
  copyFileSync(projectManifest, path.join(assetsDir, "persistent-clip-queue-manifest.json"));
  assert(finalManifest.items?.length === 3, "final manifest should have three items");
  assert(queueTotalDuration(finalManifest.items) === 4800, "final manifest duration should be 4800ms");

  const exportState = capyJson(["devtools", "--eval", queueExportEval()], "persistent-clip-queue-export-state.json", capyEnv());
  assertQueueState(exportState, 3, "exported");
  assert(exportState.proposal?.kind === "video-clip-queue-proposal", "proposal kind mismatch");
  assert(exportState.proposal?.clip_count === 3, "proposal should contain three clips");
  assert(exportState.exportJob?.status === "done", `export job did not finish: ${exportState.exportJob?.status || "missing"}`);
  assert(existsSync(exportState.exportJob.output_path), `export file missing: ${exportState.exportJob.output_path}`);
  assertNoPageErrors(exportState, "persistent queue export");
  captureWithFallback("persistent-clip-queue-export-desktop.png", "persistent-clip-queue-export-capture.json", "persistent-clip-queue-export-screenshot.json", "persistent-clip-queue-final-restored-desktop.png");

  const exportedCopy = path.join(assetsDir, "persistent-clip-queue-delivery.mp4");
  copyFileSync(exportState.exportJob.output_path, exportedCopy);
  const exportComposition = exportState.lastExport.export_composition_path;
  copyFileSync(exportComposition, path.join(assetsDir, "persistent-clip-queue-proposal-composition.json"));
  const proposalJson = JSON.parse(command("cat", [exportComposition], "persistent-clip-queue-proposal-composition-read.json"));
  assert(proposalJson.delivery?.kind === "video-clip-queue-proposal", "proposal composition delivery kind mismatch");
  assert(proposalJson.clips?.length === 3, "proposal composition should have three clips");
  const ffprobeJson = JSON.parse(command("ffprobe", ["-v", "error", "-print_format", "json", "-show_streams", "-show_format", exportedCopy], "persistent-clip-queue-export-ffprobe.json"));
  const exportedDuration = Number(ffprobeJson.format?.duration || 0);
  assert(exportedDuration >= 4.4 && exportedDuration <= 5.2, `persistent queue mp4 duration should be about 4.8s, got ${exportedDuration}`);
  command("target/debug/capy", ["timeline", "snapshot", "--composition", exportComposition, "--frame", "3000", "--out", path.join(assetsDir, "persistent-clip-queue-sampled-frame.png")], "persistent-clip-queue-sampled-frame.json");

  const summary = writeSummary({ importA, importB, workbench, createdState, restoredState, modifiedState, finalRestoredState, finalManifest, exportState, exportedCopy, exportComposition, proposalJson, ffprobeJson });
  writeEvidencePage({ evidenceDir, logs, summary });
  writeManifest({ evidenceDir });
  await verifyEvidencePage({ evidenceDir, assetsDir });
  command("open", [path.join(evidenceDir, "index.html")], "evidence-open.log");
  logs.push({ command: "verdict", ok: true });
  writeLogs();
  shutdown();
  console.log(JSON.stringify({ ok: true, assets: assetsDir, manifest: path.join(assetsDir, "persistent-clip-queue-manifest.json"), export_path: exportedCopy }, null, 2));
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

function captureWithFallback(imageName, captureEvidence, screenshotEvidence, fallbackImage = null) {
  const out = path.join(assetsDir, imageName);
  if (optionalCommand("target/debug/capy", ["capture", `--out=${out}`], captureEvidence, { env: capyEnv() })) return;
  optionalCommand("target/debug/capy", ["screenshot", `--out=${out}`], screenshotEvidence, { env: capyEnv() });
  if (fallbackImage && existsSync(path.join(assetsDir, fallbackImage))) {
    copyFileSync(path.join(assetsDir, fallbackImage), out);
    logs.push({ command: "capture-fallback-copy", evidence: imageName, ok: false, warning: `copied ${fallbackImage}` });
  }
}

function capyJson(args, evidenceName, env) {
  return JSON.parse(command("target/debug/capy", args, evidenceName, { env }));
}

function inspectManifest(evidenceName) {
  return capyJson(["project", "clip-queue", "inspect", "--project", projectDir], evidenceName, process.env);
}

function capyEnv() {
  return { ...process.env, CAPYBARA_SOCKET: currentSocket };
}

function writeSummary(data) {
  const finalQueue = data.finalManifest.items;
  const metadata = {
    schema: "capy.persistent_clip_queue.verify.v1",
    ok: true,
    project_dir: projectDir,
    manifest_path: projectManifest,
    sources: [
      { title: "Camera A wide", path: sourceA, bytes: statSync(sourceA).size, artifact_id: data.importA.artifact.id },
      { title: "Camera B close", path: sourceB, bytes: statSync(sourceB).size, artifact_id: data.importB.artifact.id }
    ],
    workbench_video_cards: data.workbench.cards.filter(card => card.kind === "video").map(card => ({
      id: card.id,
      title: card.title,
      status: card.status,
      filename: card.preview?.metadata?.filename,
      duration_ms: card.preview?.metadata?.duration_ms,
      poster_frame_path: card.preview?.poster_frame_path,
      composition_path: card.preview?.composition_path
    })),
    created_queue: data.createdState.queue,
    restored_queue: data.restoredState.queue,
    modified_queue: data.modifiedState.queue,
    final_queue: finalQueue,
    total_duration_ms: queueTotalDuration(finalQueue),
    final_manifest: data.finalManifest,
    proposal: data.exportState.proposal,
    export_job: data.exportState.exportJob,
    evidence_export_path: data.exportedCopy,
    export_composition_path: data.exportComposition,
    proposal_delivery: data.proposalJson.delivery,
    sampled_frame: path.join(assetsDir, "persistent-clip-queue-sampled-frame.png"),
    screenshots: [
      path.join(assetsDir, "persistent-clip-queue-created-desktop.png"),
      path.join(assetsDir, "persistent-clip-queue-restored-desktop.png"),
      path.join(assetsDir, "persistent-clip-queue-modified-desktop.png"),
      path.join(assetsDir, "persistent-clip-queue-final-restored-desktop.png"),
      path.join(assetsDir, "persistent-clip-queue-export-desktop.png")
    ],
    export_probe: {
      duration: data.ffprobeJson.format?.duration || null,
      streams: (data.ffprobeJson.streams || []).map(stream => ({
        codec_type: stream.codec_type,
        width: stream.width || null,
        height: stream.height || null,
        duration: stream.duration || null
      }))
    },
    verdict: "passed"
  };
  writeJson("persistent-clip-queue-summary.json", metadata);
  return metadata;
}

function listFiles(dir) {
  const result = [];
  const visit = current => {
    for (const name of execFileSync("find", [current, "-maxdepth", "1", "-mindepth", "1"], { encoding: "utf8" }).trim().split("\n").filter(Boolean)) {
      const rel = path.relative(dir, name);
      const info = statSync(name);
      result.push({ path: rel, kind: info.isDirectory() ? "dir" : "file", bytes: info.isFile() ? info.size : 0 });
      if (info.isDirectory()) visit(name);
    }
  };
  visit(dir);
  return result.sort((left, right) => left.path.localeCompare(right.path));
}

function writeJson(name, value) {
  writeFileSync(path.join(assetsDir, name), `${JSON.stringify(value, null, 2)}\n`);
}

function writeLogs() {
  writeJson("persistent-clip-queue-command-log.json", logs);
}

function shutdown() {
  for (const instanceId of [...openInstanceIds].reverse()) {
    const socket = socketForInstance(instanceId);
    try {
      execFileSync(capy, ["quit"], { cwd: root, env: { ...process.env, CAPYBARA_SOCKET: socket }, stdio: "ignore" });
    } catch {}
    try {
      execFileSync("launchctl", ["remove", labelForInstance(instanceId)], { cwd: root, stdio: "ignore" });
    } catch {}
    rmSync(socket, { force: true });
  }
}

function socketForInstance(instanceId) {
  return `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
}

function labelForInstance(instanceId) {
  return `com.capybara.debug.${instanceId}`;
}

function assertQueueState(value, expectedCount, labelText) {
  assert(value.workspace === "video", `${labelText}: video workspace not active`);
  assert((value.queue || []).length === expectedCount, `${labelText}: expected ${expectedCount} queue items, got ${(value.queue || []).length}`);
  assert((value.domQueue || []).length === expectedCount, `${labelText}: expected ${expectedCount} DOM queue cards`);
  assert(value.layout?.deliveryPanel?.w >= 600, `${labelText}: delivery panel is too narrow`);
}

function assertNoPageErrors(value, labelText) {
  assert((value.consoleErrors || []).length === 0 && (value.pageErrors || []).length === 0, `${labelText} has console/page errors`);
}

function queueTotalDuration(queue) {
  return (queue || []).reduce((total, item) => total + Number(item.duration_ms || 0), 0);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
