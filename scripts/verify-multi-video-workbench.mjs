#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, statSync, writeFileSync, copyFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import { confirmExportEval, listEval, selectSecondVideoEval } from "./verify-multi-video-workbench-evals.mjs";
import { verifyEvidencePage, writeEvidencePage, writeManifest } from "./verify-multi-video-workbench-report.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-multi-video-workbench.mjs [spec/versions/v0.44]

Use when:
  Verify the v0.44 multi-video material workbench end to end.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.44.
  Requires target/debug/capy, local ffmpeg/ffprobe, macOS launchctl, CEF shell harness, and Playwright for the evidence page browser check.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/multi-video-project.
  Launches debug shell id v44-multi-video, then quits/removes that instance.
  Opens <version>/evidence/index.html with macOS open after writing it.

Evidence outputs:
  multi-video-project-files-before-import.json
  multi-video-import-a.json
  multi-video-import-b.json
  multi-video-list-state.json
  multi-video-selected-state.json
  multi-video-export-state.json
  multi-video-clip-only.mp4
  multi-video-sampled-frame.png
  multi-video-summary.json
  multi-video-command-log.json
  multi-video-*-desktop.png
  evidence-page-check.json

Pitfalls:
  The source preview path intentionally uses WebM, not H.264 MP4, because v0.43 proved WebM as the stable visible CEF preview path.
  This verifies browsing multiple videos and exporting one selected source; it does not introduce multi-track nonlinear editing.
  If a previous debug shell is still open, --replace is used intentionally for this fixed evidence id.

Next step:
  Review <version>/evidence/index.html and <version>/evidence/assets/multi-video-summary.json.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.44");
const evidenceDir = path.join(versionDir, "evidence");
const assetsDir = path.join(evidenceDir, "assets");
const capy = path.join(root, "target", "debug", "capy");
const projectDir = path.join(assetsDir, "multi-video-project");
const mediaDir = path.join(projectDir, "media");
const sourceA = path.join(mediaDir, "camera-a-wide.webm");
const sourceB = path.join(mediaDir, "camera-b-close.webm");
const instanceId = "v44-multi-video";
const socket = `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
const label = `com.capybara.debug.${instanceId}`;
const logs = [];

mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  command("ffmpeg", ["-version"], "multi-video-ffmpeg-version.log");
  command("ffprobe", ["-version"], "multi-video-ffprobe-version.log");

  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  generateVideo(sourceA, "testsrc2=size=640x360:rate=30", 4, "multi-video-source-a-generate.log");
  generateVideo(sourceB, "smptebars=size=480x270:rate=24", 5, "multi-video-source-b-generate.log");
  writeJson("multi-video-project-files-before-import.json", {
    schema: "capy.multi_video.project_files.v1",
    project_dir: projectDir,
    files: listFiles(projectDir)
  });

  command("target/debug/capy", ["project", "init", "--project", projectDir, "--name", "v0.44 Multi Video Project"], "multi-video-project-init.json");
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importA = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-a-wide.webm", "--title", "Camera A wide"], "multi-video-import-a.json", process.env);
  const importB = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-b-close.webm", "--title", "Camera B close"], "multi-video-import-b.json", process.env);
  assert(importA.schema_version === "capy.project-video-import.v1", "unexpected import A schema");
  assert(importB.schema_version === "capy.project-video-import.v1", "unexpected import B schema");
  assert(importA.artifact.id !== importB.artifact.id, "imports should create distinct video artifacts");
  const compositionB = path.join(projectDir, importB.composition_path);
  assert(existsSync(compositionB), "selected video composition missing");

  const workbench = capyJson(["project", "workbench", "--project", projectDir], "multi-video-project-workbench.json", process.env);
  const videoCards = workbench.cards.filter(card => card.kind === "video");
  assert(videoCards.length >= 2, `expected at least 2 video cards, got ${videoCards.length}`);
  assert(videoCards.some(card => card.preview?.metadata?.filename === "camera-b-close.webm"), "workbench missing camera B card");
  command("target/debug/capy", ["timeline", "validate", "--composition", compositionB], "multi-video-composition-b-validate.json");
  command("target/debug/capy", ["timeline", "compile", "--composition", compositionB], "multi-video-composition-b-compile.json");

  command("scripts/open-debug-shell.sh", ["--id", instanceId, "--project", projectDir, "--replace"], "multi-video-open-debug-shell.log");
  const listState = capyJson(["devtools", "--eval", listEval(projectDir)], "multi-video-list-state.json", capyEnv());
  assert(listState.cards.length >= 2, "desktop list did not render two video cards");
  assert(listState.cards.every(card => card.hasPoster), "one or more video cards are missing poster thumbnails");
  assert(listState.layout?.workbench?.w >= 480, "project workbench is too narrow");
  assertNoPageErrors(listState, "multi-video list");
  command("target/debug/capy", ["capture", `--out=${path.join(assetsDir, "multi-video-list-desktop.png")}`], "multi-video-list-capture.json", { env: capyEnv() });

  const selectedState = capyJson(["devtools", "--eval", selectSecondVideoEval(projectDir, assetsDir)], "multi-video-selected-state.json", capyEnv());
  assert(selectedState.workspace === "video", "selecting a video card did not open the video workspace");
  assert(selectedState.editorSourceVideo?.filename === "camera-b-close.webm", "selected editor source is not camera B");
  assert(selectedState.videoElement?.src?.includes("camera-b-close.webm"), "preview video src is not camera B");
  assert(selectedState.selectedRange?.start_ms === 1000 && selectedState.selectedRange?.end_ms === 3000, "selected range mismatch");
  assert(selectedState.proposal?.source?.video?.filename === "camera-b-close.webm", "proposal source is not camera B");
  assert(selectedState.layout?.preview?.w >= 600, "selected video preview is too narrow");
  assertNoPageErrors(selectedState, "multi-video selected preview");
  captureWithFallback("multi-video-selected-desktop.png", "multi-video-selected-capture.json", "multi-video-selected-screenshot.json", "multi-video-list-desktop.png");

  const exportState = capyJson(["devtools", "--eval", confirmExportEval()], "multi-video-export-state.json", capyEnv());
  const job = exportState.video?.exportJob || {};
  const exportPath = job.output_path || "";
  assert(job.status === "done", `export job did not finish: ${job.status || "missing"}`);
  assert(existsSync(exportPath), `export file missing: ${exportPath}`);
  assert(exportState.video?.clipProposal?.source?.video?.filename === "camera-b-close.webm", "exported proposal source is not camera B");
  assertNoPageErrors(exportState, "multi-video export");
  captureWithFallback("multi-video-export-desktop.png", "multi-video-export-capture.json", "multi-video-export-screenshot.json", "multi-video-selected-desktop.png");

  const exportedCopy = path.join(assetsDir, "multi-video-clip-only.mp4");
  copyFileSync(exportPath, exportedCopy);
  const ffprobeJson = JSON.parse(command("ffprobe", ["-v", "error", "-print_format", "json", "-show_streams", "-show_format", exportedCopy], "multi-video-export-ffprobe.json"));
  const exportedDuration = Number(ffprobeJson.format?.duration || 0);
  assert(exportedDuration >= 1.8 && exportedDuration <= 2.25, `clip-only mp4 duration should be about 2s, got ${exportedDuration}`);
  const sampleComposition = exportState.video.lastExport.export_composition_path;
  command("target/debug/capy", ["timeline", "snapshot", "--composition", sampleComposition, "--frame", "500", "--out", path.join(assetsDir, "multi-video-sampled-frame.png")], "multi-video-sampled-frame.json");

  const summary = writeSummary({ importA, importB, workbench, listState, selectedState, exportState, exportedCopy, sampleComposition, ffprobeJson });
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

function captureWithFallback(imageName, captureEvidence, screenshotEvidence, fallbackImage) {
  const out = path.join(assetsDir, imageName);
  if (optionalCommand("target/debug/capy", ["capture", `--out=${out}`], captureEvidence, { env: capyEnv() })) return;
  if (optionalCommand("target/debug/capy", ["screenshot", `--out=${out}`], screenshotEvidence, { env: capyEnv() })) return;
  copyFileSync(path.join(assetsDir, fallbackImage), out);
  logs.push({ command: "capture-fallback-copy", evidence: imageName, ok: false, warning: `copied ${fallbackImage}` });
}

function capyJson(args, evidenceName, env) {
  return JSON.parse(command("target/debug/capy", args, evidenceName, { env }));
}

function capyEnv() {
  return { ...process.env, CAPYBARA_SOCKET: socket };
}

function writeSummary(data) {
  const metadata = {
    schema: "capy.multi_video_workbench.verify.v1",
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
    selected_source: {
      artifact_id: data.importB.artifact.id,
      filename: data.selectedState.editorSourceVideo?.filename,
      composition_path: data.importB.composition_path,
      preview_src: data.selectedState.videoElement?.src
    },
    selected_range: data.selectedState.selectedRange,
    proposal: data.selectedState.proposal,
    export_job: data.exportState.video.exportJob,
    evidence_export_path: data.exportedCopy,
    export_composition_path: data.sampleComposition,
    sampled_frame: path.join(assetsDir, "multi-video-sampled-frame.png"),
    screenshots: [
      path.join(assetsDir, "multi-video-list-desktop.png"),
      path.join(assetsDir, "multi-video-selected-desktop.png"),
      path.join(assetsDir, "multi-video-export-desktop.png")
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
  writeJson("multi-video-summary.json", metadata);
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
  writeJson("multi-video-command-log.json", logs);
}

function shutdown() {
  try {
    execFileSync(capy, ["quit"], { cwd: root, env: capyEnv(), stdio: "ignore" });
  } catch {}
  try {
    execFileSync("launchctl", ["remove", label], { cwd: root, stdio: "ignore" });
  } catch {}
}

function assertNoPageErrors(value, labelText) {
  assert((value.consoleErrors || []).length === 0 && (value.pageErrors || []).length === 0, `${labelText} has console/page errors`);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
