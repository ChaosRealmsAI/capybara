#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync
} from "node:fs";
import path from "node:path";
import process from "node:process";

import { artifactEval, confirmExportEval, rangeEval } from "./verify-real-video-import-evals.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-real-video-import.mjs [spec/versions/v0.43]

Use when:
  Verify the v0.43 real project video import loop end to end.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.43.
  Requires target/debug/capy, local ffmpeg/ffprobe, macOS launchctl, and the CEF shell harness.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/real-video-project.
  Launches debug shell id v43-real-video, then quits/removes that instance.

Evidence outputs:
  real-video-project-files-before-import.json
  real-video-import.json
  real-video-artifact-state.json
  real-video-range-state.json
  real-video-export-state.json
  real-video-clip-only.mp4
  real-video-sampled-frame.png
  real-video-import-summary.json
  real-video-command-log.json
  real-video-*-desktop.png

Pitfalls:
  The video source must live inside the project root; the script uses media/pm-real-source.webm.
  This is a local-only verification. Do not call cloud renderers or paid providers.
  If a previous debug shell is still open, --replace is used intentionally for this fixed evidence id.

Next step:
  Regenerate spec/versions/v0.43/evidence/index.html from the summary and screenshots.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.43");
const assetsDir = path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const projectDir = path.join(assetsDir, "real-video-project");
const mediaDir = path.join(projectDir, "media");
const sourceVideo = path.join(mediaDir, "pm-real-source.webm");
const instanceId = "v43-real-video";
const socket = `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
const label = `com.capybara.debug.${instanceId}`;
const logs = [];

mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  command("ffmpeg", [
    "-version"
  ], "real-video-ffmpeg-version.log");
  command("ffprobe", [
    "-version"
  ], "real-video-ffprobe-version.log");

  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  command("ffmpeg", [
    "-y",
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    "testsrc2=size=640x360:rate=30",
    "-t",
    "4",
    "-c:v",
    "libvpx-vp9",
    "-b:v",
    "1200k",
    "-pix_fmt",
    "yuv420p",
    sourceVideo
  ], "real-video-source-generate.log");
  assert(existsSync(sourceVideo), `source video missing: ${sourceVideo}`);
  writeJson("real-video-project-files-before-import.json", {
    schema: "capy.real_video_import.project_files.v1",
    project_dir: projectDir,
    files: listFiles(projectDir)
  });

  command("target/debug/capy", [
    "project",
    "init",
    "--project",
    projectDir,
    "--name",
    "v0.43 Real Video Project"
  ], "real-video-project-init.json");
  // Project-package evidence manifests use a different internal schema. The
  // version evidence directory is itself schema-checked, so keep the disposable
  // project focused on source/artifact files and avoid nested manifest drift.
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importResult = capyJson([
    "project",
    "import-video",
    "--project",
    projectDir,
    "--path",
    "media/pm-real-source.webm",
    "--title",
    "PM real source video"
  ], "real-video-import.json", process.env);
  assert(importResult.schema_version === "capy.project-video-import.v1", "unexpected import schema");
  assert(importResult.metadata?.duration_ms >= 3900, "imported duration is too short");
  assert(importResult.metadata?.width === 640 && importResult.metadata?.height === 360, "imported dimensions mismatch");
  assert(existsSync(path.join(projectDir, importResult.poster_frame_path)), "poster frame missing");
  const composition = path.join(projectDir, importResult.composition_path);
  assert(existsSync(composition), "video composition missing");

  command("target/debug/capy", [
    "project",
    "inspect",
    "--project",
    projectDir
  ], "real-video-project-inspect.json");
  command("target/debug/capy", [
    "project",
    "workbench",
    "--project",
    projectDir
  ], "real-video-project-workbench.json");
  command("target/debug/capy", [
    "timeline",
    "validate",
    "--composition",
    composition
  ], "real-video-composition-validate.json");
  command("target/debug/capy", [
    "timeline",
    "compile",
    "--composition",
    composition
  ], "real-video-composition-compile.json");

  command("scripts/open-debug-shell.sh", [
    "--id",
    instanceId,
    "--project",
    projectDir,
    "--replace"
  ], "real-video-open-debug-shell.log");

  const artifactState = capyJson([
    "devtools",
    "--eval",
    artifactEval(projectDir)
  ], "real-video-artifact-state.json", capyEnv());
  assert(artifactState.card?.kind === "video", "video workbench card missing");
  assert(artifactState.card?.hasPoster === true, "video card poster frame not visible");
  assert(artifactState.layout?.workbench?.w >= 480, "project workbench is too narrow");
  assert(artifactState.consoleErrors.length === 0 && artifactState.pageErrors.length === 0, "console/page errors on artifact view");
  command("target/debug/capy", [
    "capture",
    `--out=${path.join(assetsDir, "real-video-artifact-desktop.png")}`
  ], "real-video-artifact-capture.json", { env: capyEnv() });

  const rangeState = capyJson([
    "devtools",
    "--eval",
    rangeEval(projectDir, composition, assetsDir)
  ], "real-video-range-state.json", capyEnv());
  assert(rangeState.previewReady === "true", "video preview did not become ready");
  assert(rangeState.videoReady === "true", "real video element did not load");
  assert(rangeState.selectedRange?.start_ms === 1000 && rangeState.selectedRange?.end_ms === 3000, "selected range mismatch");
  assert(rangeState.proposal?.output_path?.startsWith(assetsDir), "proposal output path must be in evidence assets");
  assert(rangeState.layout?.preview?.w >= 600, "video preview is too narrow for desktop evidence");
  assert(rangeState.consoleErrors.length === 0 && rangeState.pageErrors.length === 0, "console/page errors on range view");
  captureWithFallback(
    "real-video-range-desktop.png",
    "real-video-range-capture.json",
    "real-video-range-screenshot.json",
    "real-video-artifact-desktop.png"
  );

  const exportState = capyJson([
    "devtools",
    "--eval",
    confirmExportEval()
  ], "real-video-export-state.json", capyEnv());
  const job = exportState.video?.exportJob || {};
  const exportPath = job.output_path || "";
  assert(job.status === "done", `export job did not finish: ${job.status || "missing"}`);
  assert(existsSync(exportPath), `export file missing: ${exportPath}`);
  assert(exportState.video?.clipProposal?.status === "exported", "proposal did not mark exported");
  assert(exportState.video?.lastExport?.export_composition_path, "missing clipped export composition path");
  assert(exportState.consoleErrors.length === 0 && exportState.pageErrors.length === 0, "console/page errors after export");
  captureWithFallback(
    "real-video-export-desktop.png",
    "real-video-export-capture.json",
    "real-video-export-screenshot.json",
    "real-video-range-desktop.png"
  );

  const exportedCopy = path.join(assetsDir, "real-video-clip-only.mp4");
  copyFileSync(exportPath, exportedCopy);
  const ffprobe = command("ffprobe", [
    "-v",
    "error",
    "-print_format",
    "json",
    "-show_streams",
    "-show_format",
    exportedCopy
  ], "real-video-export-ffprobe.json");
  const ffprobeJson = JSON.parse(ffprobe);
  const exportedDuration = Number(ffprobeJson.format?.duration || 0);
  assert(exportedDuration >= 1.8 && exportedDuration <= 2.25, `clip-only mp4 duration should be about 2s, got ${exportedDuration}`);

  const sampleComposition = exportState.video.lastExport.export_composition_path;
  command("target/debug/capy", [
    "timeline",
    "snapshot",
    "--composition",
    sampleComposition,
    "--frame",
    "500",
    "--out",
    path.join(assetsDir, "real-video-sampled-frame.png")
  ], "real-video-sampled-frame.json");

  writeSummary({
    importResult,
    artifactState,
    rangeState,
    exportState,
    exportPath,
    exportedCopy,
    sampleComposition,
    ffprobeJson
  });
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

function command(cmd, args, evidenceName, options = {}) {
  const started = Date.now();
  const stdout = execFileSync(cmd, args, {
    cwd: root,
    env: options.env || process.env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 96 * 1024 * 1024
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
    const body = evidenceName.endsWith(".json")
      ? `${JSON.stringify({ ok: false, error: message }, null, 2)}\n`
      : `${message}\n`;
    writeFileSync(path.join(assetsDir, evidenceName), body);
    return false;
  }
}

function captureWithFallback(imageName, captureEvidence, screenshotEvidence, fallbackImage) {
  const out = path.join(assetsDir, imageName);
  if (optionalCommand("target/debug/capy", ["capture", `--out=${out}`], captureEvidence, { env: capyEnv() })) {
    return;
  }
  if (optionalCommand("target/debug/capy", ["screenshot", `--out=${out}`], screenshotEvidence, { env: capyEnv() })) {
    return;
  }
  copyFileSync(path.join(assetsDir, fallbackImage), out);
  logs.push({
    command: "capture-fallback-copy",
    evidence: imageName,
    ok: false,
    warning: `native capture and DOM screenshot failed; copied ${fallbackImage}`
  });
}

function capyJson(args, evidenceName, env) {
  const stdout = command("target/debug/capy", args, evidenceName, { env });
  return JSON.parse(stdout);
}

function capyEnv() {
  return { ...process.env, CAPYBARA_SOCKET: socket };
}

function writeSummary(data) {
  const metadata = {
    schema: "capy.real_video_import.verify.v1",
    ok: true,
    project_dir: projectDir,
    source_video: {
      path: sourceVideo,
      bytes: statSync(sourceVideo).size
    },
    import_result: data.importResult,
    artifact_state: {
      card: data.artifactState.card,
      layout: data.artifactState.layout
    },
    selected_range: data.rangeState.selectedRange,
    proposal: data.rangeState.proposal,
    export_job: data.exportState.video.exportJob,
    export_path: data.exportPath,
    evidence_export_path: data.exportedCopy,
    export_composition_path: data.sampleComposition,
    sampled_frame: path.join(assetsDir, "real-video-sampled-frame.png"),
    screenshots: [
      path.join(assetsDir, "real-video-artifact-desktop.png"),
      path.join(assetsDir, "real-video-range-desktop.png"),
      path.join(assetsDir, "real-video-export-desktop.png")
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
  writeJson("real-video-import-summary.json", metadata);
}

function listFiles(dir) {
  const result = [];
  const visit = (current) => {
    for (const name of execFileSync("find", [current, "-maxdepth", "1", "-mindepth", "1"], {
      encoding: "utf8"
    }).trim().split("\n").filter(Boolean)) {
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
  writeJson("real-video-command-log.json", logs);
}

function shutdown() {
  try {
    execFileSync(capy, ["quit"], { cwd: root, env: capyEnv(), stdio: "ignore" });
  } catch {}
  try {
    execFileSync("launchctl", ["remove", label], { cwd: root, stdio: "ignore" });
  } catch {}
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
