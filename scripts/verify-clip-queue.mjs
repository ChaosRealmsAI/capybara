#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import { queueAddedEval, queueProposalExportEval, queueReorderedEval } from "./verify-clip-queue-evals.mjs";
import { verifyEvidencePage, writeEvidencePage, writeManifest } from "./verify-clip-queue-report.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-clip-queue.mjs [spec/versions/v0.46]

Use when:
  Verify the v0.46 multi-clip delivery queue end to end.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.46.
  Requires target/debug/capy, local ffmpeg/ffprobe, macOS launchctl, CEF shell harness, and Playwright for the evidence page browser check.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/clip-queue-project.
  Launches debug shell id v46-clip-queue, then quits/removes that instance.
  Opens <version>/evidence/index.html with macOS open after writing it.

Evidence outputs:
  clip-queue-import-a.json
  clip-queue-import-b.json
  clip-queue-added-state.json
  clip-queue-reordered-state.json
  clip-queue-export-state.json
  clip-queue-proposal-composition.json
  clip-queue-delivery.mp4
  clip-queue-sampled-frame.png
  clip-queue-summary.json
  clip-queue-command-log.json
  clip-queue-*-desktop.png
  evidence-page-check.json

Pitfalls:
  This verifies a linear clip queue, not a multi-track nonlinear editor.
  The source preview path intentionally uses WebM, matching the v0.43/v0.44 stable CEF preview path.
  If a previous debug shell is still open, --replace is used intentionally for this fixed evidence id.

Next step:
  Review <version>/evidence/index.html and <version>/evidence/assets/clip-queue-summary.json.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.46");
const evidenceDir = path.join(versionDir, "evidence");
const assetsDir = path.join(evidenceDir, "assets");
const capy = path.join(root, "target", "debug", "capy");
const projectDir = path.join(assetsDir, "clip-queue-project");
const mediaDir = path.join(projectDir, "media");
const sourceA = path.join(mediaDir, "camera-a-wide.webm");
const sourceB = path.join(mediaDir, "camera-b-close.webm");
const instanceId = "v46-clip-queue";
const socket = `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
const label = `com.capybara.debug.${instanceId}`;
const logs = [];

mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  command("ffmpeg", ["-version"], "clip-queue-ffmpeg-version.log");
  command("ffprobe", ["-version"], "clip-queue-ffprobe-version.log");

  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  generateVideo(sourceA, "testsrc2=size=640x360:rate=30", 4, "clip-queue-source-a-generate.log");
  generateVideo(sourceB, "smptebars=size=480x270:rate=24", 5, "clip-queue-source-b-generate.log");
  writeJson("clip-queue-project-files-before-import.json", {
    schema: "capy.clip_queue.project_files.v1",
    project_dir: projectDir,
    files: listFiles(projectDir)
  });

  command("target/debug/capy", ["project", "init", "--project", projectDir, "--name", "v0.46 Clip Queue Project"], "clip-queue-project-init.json");
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importA = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-a-wide.webm", "--title", "Camera A wide"], "clip-queue-import-a.json", process.env);
  const importB = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-b-close.webm", "--title", "Camera B close"], "clip-queue-import-b.json", process.env);
  assert(importA.schema_version === "capy.project-video-import.v1", "unexpected import A schema");
  assert(importB.schema_version === "capy.project-video-import.v1", "unexpected import B schema");
  const workbench = capyJson(["project", "workbench", "--project", projectDir], "clip-queue-project-workbench.json", process.env);
  assert(workbench.cards.filter(card => card.kind === "video").length >= 2, "workbench missing two video cards");
  command("target/debug/capy", ["timeline", "validate", "--composition", path.join(projectDir, importA.composition_path)], "clip-queue-composition-a-validate.json");
  command("target/debug/capy", ["timeline", "validate", "--composition", path.join(projectDir, importB.composition_path)], "clip-queue-composition-b-validate.json");
  command("target/debug/capy", ["timeline", "compile", "--composition", path.join(projectDir, importA.composition_path)], "clip-queue-composition-a-compile.json");
  command("target/debug/capy", ["timeline", "compile", "--composition", path.join(projectDir, importB.composition_path)], "clip-queue-composition-b-compile.json");

  command("scripts/open-debug-shell.sh", ["--id", instanceId, "--project", projectDir, "--replace", "--skip-build"], "clip-queue-open-debug-shell.log");
  const addedState = capyJson(["devtools", "--eval", queueAddedEval(projectDir, assetsDir)], "clip-queue-added-state.json", capyEnv());
  assertQueueState(addedState, 3, "added");
  assert(addedState.queue.some(item => item.source_video?.filename === "camera-a-wide.webm"), "queue missing camera A");
  assert(addedState.queue.some(item => item.source_video?.filename === "camera-b-close.webm"), "queue missing camera B");
  assertNoPageErrors(addedState, "clip queue added");
  captureWithFallback("clip-queue-added-desktop.png", "clip-queue-added-capture.json", "clip-queue-added-screenshot.json");

  const reorderedState = capyJson(["devtools", "--eval", queueReorderedEval()], "clip-queue-reordered-state.json", capyEnv());
  assertQueueState(reorderedState, 2, "reordered");
  assert(reorderedState.queue[0]?.source_video?.filename === "camera-b-close.webm", "camera B should be first after reorder");
  assert(reorderedState.queue[1]?.source_video?.filename === "camera-a-wide.webm", "camera A should be second after removal");
  assert(queueTotalDuration(reorderedState.queue) === 3500, "final queue duration should be 3500ms");
  assertNoPageErrors(reorderedState, "clip queue reordered");
  captureWithFallback("clip-queue-reordered-desktop.png", "clip-queue-reordered-capture.json", "clip-queue-reordered-screenshot.json", "clip-queue-added-desktop.png");

  const exportState = capyJson(["devtools", "--eval", queueProposalExportEval()], "clip-queue-export-state.json", capyEnv());
  assertQueueState(exportState, 2, "exported");
  assert(exportState.proposal?.kind === "video-clip-queue-proposal", "proposal kind mismatch");
  assert(exportState.proposal?.clip_count === 2, "proposal should contain two clips");
  assert(exportState.exportJob?.status === "done", `export job did not finish: ${exportState.exportJob?.status || "missing"}`);
  assert(existsSync(exportState.exportJob.output_path), `export file missing: ${exportState.exportJob.output_path}`);
  assertNoPageErrors(exportState, "clip queue export");
  captureWithFallback("clip-queue-export-desktop.png", "clip-queue-export-capture.json", "clip-queue-export-screenshot.json", "clip-queue-reordered-desktop.png");

  const exportedCopy = path.join(assetsDir, "clip-queue-delivery.mp4");
  copyFileSync(exportState.exportJob.output_path, exportedCopy);
  const exportComposition = exportState.lastExport.export_composition_path;
  copyFileSync(exportComposition, path.join(assetsDir, "clip-queue-proposal-composition.json"));
  const proposalJson = JSON.parse(command("cat", [exportComposition], "clip-queue-proposal-composition-read.json"));
  assert(proposalJson.delivery?.kind === "video-clip-queue-proposal", "proposal composition delivery kind mismatch");
  assert(proposalJson.clips?.length === 2, "proposal composition should have two clips");
  const ffprobeJson = JSON.parse(command("ffprobe", ["-v", "error", "-print_format", "json", "-show_streams", "-show_format", exportedCopy], "clip-queue-export-ffprobe.json"));
  const exportedDuration = Number(ffprobeJson.format?.duration || 0);
  assert(exportedDuration >= 3.2 && exportedDuration <= 3.9, `clip queue mp4 duration should be about 3.5s, got ${exportedDuration}`);
  command("target/debug/capy", ["timeline", "snapshot", "--composition", exportComposition, "--frame", "2500", "--out", path.join(assetsDir, "clip-queue-sampled-frame.png")], "clip-queue-sampled-frame.json");

  const summary = writeSummary({ importA, importB, workbench, addedState, reorderedState, exportState, exportedCopy, exportComposition, proposalJson, ffprobeJson });
  writeEvidencePage({ evidenceDir, logs, summary });
  writeManifest({ evidenceDir });
  await verifyEvidencePage({ evidenceDir, assetsDir });
  command("open", [path.join(evidenceDir, "index.html")], "evidence-open.log");
  logs.push({ command: "verdict", ok: true });
  writeLogs();
  shutdown();
  console.log(JSON.stringify({ ok: true, assets: assetsDir, export_path: exportedCopy }, null, 2));
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

function capyEnv() {
  return { ...process.env, CAPYBARA_SOCKET: socket };
}

function writeSummary(data) {
  const finalQueue = data.reorderedState.queue;
  const metadata = {
    schema: "capy.clip_queue.verify.v1",
    ok: true,
    project_dir: projectDir,
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
    added_queue: data.addedState.queue,
    final_queue: finalQueue,
    total_duration_ms: queueTotalDuration(finalQueue),
    proposal: data.exportState.proposal,
    export_job: data.exportState.exportJob,
    evidence_export_path: data.exportedCopy,
    export_composition_path: data.exportComposition,
    proposal_delivery: data.proposalJson.delivery,
    sampled_frame: path.join(assetsDir, "clip-queue-sampled-frame.png"),
    screenshots: [
      path.join(assetsDir, "clip-queue-added-desktop.png"),
      path.join(assetsDir, "clip-queue-reordered-desktop.png"),
      path.join(assetsDir, "clip-queue-export-desktop.png")
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
  writeJson("clip-queue-summary.json", metadata);
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
  writeJson("clip-queue-command-log.json", logs);
}

function shutdown() {
  try {
    execFileSync(capy, ["quit"], { cwd: root, env: capyEnv(), stdio: "ignore" });
  } catch {}
  try {
    execFileSync("launchctl", ["remove", label], { cwd: root, stdio: "ignore" });
  } catch {}
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
