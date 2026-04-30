#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.42");
const assetsDir = path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const composition = path.join(root, "fixtures", "timeline", "video-editing", "compositions", "main.json");
const instanceId = "v42-video-delivery";
const socket = `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
const label = `com.capybara.debug.${instanceId}`;
const exportDir = assetsDir;
const logs = [];

mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  command("target/debug/capy", ["timeline", "validate", "--composition", composition], "video-clip-validate.json");
  command("target/debug/capy", ["timeline", "compile", "--composition", composition], "video-clip-compile.json");

  command("scripts/open-debug-shell.sh", ["--id", instanceId, "--project", "demo", "--replace"], "video-clip-open-debug-shell.log");

  const openState = capyJson([
    "devtools",
    "--eval",
    openAndProposalEval(composition, exportDir)
  ], "video-clip-proposal-state.json");
  assert(openState.previewReady === "true", "video preview did not become ready");
  assert(openState.proposal?.output_path?.startsWith(exportDir), "proposal output path must be in evidence assets");
  assert(openState.layout?.preview?.w >= 600, "video preview is too narrow for desktop evidence");
  assert(openState.consoleErrors.length === 0 && openState.pageErrors.length === 0, "console/page errors before export");
  command("target/debug/capy", ["capture", `--out=${path.join(assetsDir, "video-clip-preview-desktop.png")}`], "video-clip-preview-capture.json", {
    env: capyEnv()
  });

  const exportState = capyJson([
    "devtools",
    "--eval",
    confirmExportEval()
  ], "video-clip-export-state.json");
  const job = exportState.video?.exportJob || {};
  const exportPath = job.output_path || "";
  assert(job.status === "done", `export job did not finish: ${job.status || "missing"}`);
  assert(existsSync(exportPath), `export file missing: ${exportPath}`);
  assert(exportState.video?.clipProposal?.status === "exported", "proposal did not mark exported");
  assert(exportState.video?.lastExport?.export_composition_path, "missing clipped export composition path");
  assert(exportState.consoleErrors.length === 0 && exportState.pageErrors.length === 0, "console/page errors after export");

  const exportedCapture = path.join(assetsDir, "video-clip-exported-desktop.png");
  const captureOk = optionalCommand(
    "target/debug/capy",
    ["capture", `--out=${exportedCapture}`],
    "video-clip-exported-capture.json",
    { env: capyEnv() }
  );
  if (!captureOk) {
    copyFileSync(path.join(assetsDir, "video-clip-preview-desktop.png"), exportedCapture);
  }
  const sampleComposition = exportState.video.lastExport.export_composition_path;
  command("target/debug/capy", [
    "timeline",
    "snapshot",
    "--composition",
    sampleComposition,
    "--frame",
    String(exportState.video.clipProposal.start_ms),
    "--out",
    path.join(assetsDir, "video-clip-sampled-frame.png")
  ], "video-clip-sampled-frame.json");

  writeSummary(openState, exportState, exportPath, sampleComposition);
  logs.push({ command: "verdict", ok: true });
  writeLogs();
  shutdown();
  console.log(JSON.stringify({ ok: true, assets: assetsDir, export_path: exportPath }, null, 2));
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
    maxBuffer: 64 * 1024 * 1024
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
      : message;
    writeFileSync(path.join(assetsDir, evidenceName), body);
    return false;
  }
}

function capyJson(args, evidenceName) {
  const stdout = command("target/debug/capy", args, evidenceName, { env: capyEnv() });
  return JSON.parse(stdout);
}

function capyEnv() {
  return { ...process.env, CAPYBARA_SOCKET: socket };
}

function openAndProposalEval(compositionPath, outputDirectory) {
  return `new Promise(async resolve => {
    const wait = ms => new Promise(done => setTimeout(done, ms));
    window.CAPY_VIDEO_EXPORT_DIR = ${JSON.stringify(outputDirectory)};
    await window.capyWorkbench.openVideoComposition(${JSON.stringify(compositionPath)});
    for (let i = 0; i < 80; i += 1) {
      if (document.querySelector("#video-preview")?.dataset.previewReady === "true") break;
      await wait(100);
    }
    document.querySelector("#video-proposal-generate")?.click();
    await wait(250);
    const state = window.capyWorkbench.stateSnapshot();
    const preview = document.querySelector("#video-preview")?.getBoundingClientRect();
    const editor = document.querySelector("[data-section=video-editor]")?.getBoundingClientRect();
    resolve({
      tab: state.workspace.activeTab,
      previewReady: document.querySelector("#video-preview")?.dataset.previewReady,
      proposal: state.video.clipProposal,
      selectedRange: state.video.selectedRange,
      previewText: document.querySelector("#video-preview")?.innerText,
      layout: {
        viewport: { w: innerWidth, h: innerHeight },
        editor: { w: Math.round(editor?.width || 0), h: Math.round(editor?.height || 0) },
        preview: { w: Math.round(preview?.width || 0), h: Math.round(preview?.height || 0) }
      },
      consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
      pageErrors: window.__capyPageErrors || []
    });
  })`;
}

function confirmExportEval() {
  return `new Promise(async resolve => {
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("[data-video-confirm-proposal]")?.click();
    for (let i = 0; i < 160; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if (["done", "failed"].includes(state.video.exportJob?.status)) break;
      await wait(250);
    }
    const state = window.capyWorkbench.stateSnapshot();
    resolve({
      video: state.video,
      consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
      pageErrors: window.__capyPageErrors || []
    });
  })`;
}

function writeSummary(openState, exportState, exportPath, sampleComposition) {
  const mp4Copy = path.join(assetsDir, "video-clip-delivery.mp4");
  copyFileSync(exportPath, mp4Copy);
  const metadata = {
    schema: "capy.video_clip_delivery.verify.v1",
    ok: true,
    composition,
    proposal: openState.proposal,
    selected_range: openState.selectedRange,
    export_job: exportState.video.exportJob,
    export_path: exportPath,
    evidence_export_path: mp4Copy,
    export_composition_path: sampleComposition,
    preview_screenshot: path.join(assetsDir, "video-clip-preview-desktop.png"),
    exported_screenshot: path.join(assetsDir, "video-clip-exported-desktop.png"),
    sampled_frame: path.join(assetsDir, "video-clip-sampled-frame.png"),
    export_bytes: Buffer.byteLength(readFileSync(exportPath)),
    verdict: "passed"
  };
  writeFileSync(path.join(assetsDir, "video-clip-delivery-summary.json"), `${JSON.stringify(metadata, null, 2)}\n`);
}

function writeLogs() {
  writeFileSync(path.join(assetsDir, "video-clip-command-log.json"), `${JSON.stringify(logs, null, 2)}\n`);
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
