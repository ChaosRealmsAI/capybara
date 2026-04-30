#!/usr/bin/env node
if (process.argv.includes("--help") || process.argv.includes("-h")) {
  const scriptName = process.argv[1] || "scripts/verify-*.mjs";
  console.log("Usage: node " + scriptName + " [script-specific args]\n\nUse when: AI runs a version-specific browser, DOM, state, or contract verification script listed by BDD, status.json, evidence notes, or capy help harness.\n\nRequired params: script-specific; inspect the owning version status/evidence entry that names this script. Many scripts default to their version directory.\n\nState effects: may start local browser work, interact with Capybara test hooks, and write screenshots, state, or logs under spec/versions/<version>/evidence/assets/ or target/.\n\nPitfalls: do not run by filename guessing; first read target/debug/capy help harness and the owning version status. This generic help describes the family, while the script body owns exact assertions.\n\nNext step: rerun without --help only after BDD/status names this script, then add outputs to evidence/index.html.\n");
  process.exit(0);
}
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.8-canvas-image-tool");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const resultPath = path.join(assetsDir, "canvas-image-tool-verifier.json");
const capturePath = path.join(assetsDir, "canvas-image-workbench.png");
const prompt = [
  "Scene: Warm product design board with soft natural light.",
  "Subject: One polished hero image for a Capybara design workspace.",
  "Important details: refined color blocks, premium composition, tangible design asset.",
  "Use case: Canvas image node for design exploration.",
  "Constraints: No text, no watermark, no UI chrome."
].join(" ");

mkdirSync(assetsDir, { recursive: true });

const report = {
  ok: false,
  started_at: new Date().toISOString(),
  socket: process.env.CAPYBARA_SOCKET || null,
  commands: [],
  checks: {}
};

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);

  report.checks.ready = waitFor("canvas ready", () =>
    capyJson(["canvas", "snapshot"])
  , (data) => data?.canvas?.ready === true);

  const before = capyJson(["canvas", "snapshot"]);
  report.checks.before = summarizeCanvas(before);

  const cliResult = capyJson([
    "canvas",
    "generate-image",
    "--dry-run",
    "--out",
    assetsDir,
    "--name",
    "canvas-cli-dry-run",
    prompt
  ]);
  report.checks.cli_dry_run = trimGeneratedResult(cliResult);
  assert(cliResult.ok === true, "CLI dry-run must report ok=true");
  assert(cliResult.inserted?.inserted_node?.content_kind === "image", "CLI dry-run must select inserted image node");
  assert(
    cliResult.inserted.inserted_node.source_path?.endsWith("canvas-cli-dry-run.png"),
    "CLI dry-run inserted node must expose fixture source_path"
  );

  const afterCli = capyJson(["canvas", "snapshot"]);
  report.checks.after_cli = summarizeCanvas(afterCli);
  assert(afterCli.canvas.nodeCount > before.canvas.nodeCount, "CLI dry-run must increase node count");

  report.checks.layout = capyJson(["devtools", "--eval", layoutProbe()]);
  assert(report.checks.layout.toolRect.width > 220, "right-side image tool must be visible");
  assert(report.checks.layout.pageErrors.length === 0, "page errors before UI tool run must be empty");
  assert(report.checks.layout.consoleErrors.length === 0, "console errors before UI tool run must be empty");

  report.checks.ui_dry_run = capyJson([
    "devtools",
    "--eval",
    "window.capyWorkbench.verifyCanvasImageTool()"
  ]);
  assert(report.checks.ui_dry_run.passed === true, "UI dry-run tool must insert an image node");
  assert(report.checks.ui_dry_run.selected_node?.content_kind === "image", "UI dry-run selected node must be image");
  assert(report.checks.ui_dry_run.pageErrors.length === 0, "page errors after UI tool run must be empty");
  assert(report.checks.ui_dry_run.consoleErrors.length === 0, "console errors after UI tool run must be empty");

  const afterUi = capyJson(["canvas", "snapshot"]);
  report.checks.after_ui = summarizeCanvas(afterUi);
  assert(afterUi.canvas.nodeCount > afterCli.canvas.nodeCount, "UI dry-run must increase node count");

  report.checks.capture = capyJson(["capture", "--out", capturePath]);
  assert(report.checks.capture.bytes > 100000, "app-view capture must be non-empty");
  assert(statSync(capturePath).size > 100000, "app-view capture file must be non-empty");

  report.ok = true;
  report.finished_at = new Date().toISOString();
  writeReport();
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  report.ok = false;
  report.error = error instanceof Error ? error.message : String(error);
  report.finished_at = new Date().toISOString();
  writeReport();
  console.error(JSON.stringify(report, null, 2));
  process.exit(1);
}

function capyJson(args) {
  const started = Date.now();
  const stdout = execFileSync(capy, args, {
    cwd: root,
    env: process.env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 24 * 1024 * 1024
  });
  const elapsed_ms = Date.now() - started;
  const parsed = JSON.parse(stdout);
  report.commands.push({
    cmd: ["target/debug/capy", ...args.map(maskLongArg)].join(" "),
    elapsed_ms,
    output_summary: summarize(parsed)
  });
  return parsed;
}

function waitFor(label, producer, predicate) {
  let lastError = null;
  for (let attempt = 1; attempt <= 50; attempt += 1) {
    try {
      const value = producer();
      if (predicate(value)) {
        return { label, attempt, value: summarizeCanvas(value) };
      }
      lastError = new Error(`${label} not ready on attempt ${attempt}`);
    } catch (error) {
      lastError = error;
    }
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
  }
  throw lastError || new Error(`${label} did not become ready`);
}

function layoutProbe() {
  return `(() => {
    const rect = (selector) => {
      const el = document.querySelector(selector);
      if (!el) return { found: false, width: 0, height: 0 };
      const box = el.getBoundingClientRect();
      return { found: true, x: box.x, y: box.y, width: box.width, height: box.height };
    };
    return {
      canvasRect: rect('[data-section="canvas-host"]'),
      plannerRect: rect('[data-section="planner-chat"]'),
      toolRect: rect('[data-section="canvas-image-tool"]'),
      toolStatus: document.querySelector('#image-tool-status')?.textContent || '',
      labels: document.querySelectorAll('[data-node-id]').length,
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function summarizeCanvas(value) {
  const selected = value?.canvas?.selectedNode || null;
  return {
    ready: value?.canvas?.ready,
    nodeCount: value?.canvas?.nodeCount,
    selectedId: value?.selectedId,
    selectedNode: selected ? {
      id: selected.id,
      title: selected.title,
      content_kind: selected.content_kind,
      source_path: selected.source_path,
      generation_provider: selected.generation_provider,
      mime: selected.mime
    } : null
  };
}

function trimGeneratedResult(value) {
  return {
    ok: value.ok,
    kind: value.kind,
    mode: value.mode,
    provider: value.provider,
    image_path: value.image_path,
    inserted_node: value.inserted?.inserted_node ? {
      id: value.inserted.inserted_node.id,
      content_kind: value.inserted.inserted_node.content_kind,
      source_path: value.inserted.inserted_node.source_path,
      generation_provider: value.inserted.inserted_node.generation_provider
    } : null
  };
}

function summarize(value) {
  if (value && typeof value === "object") {
    const keys = Object.keys(value).slice(0, 8);
    return Object.fromEntries(keys.map((key) => [key, key === "image_base64" ? "<base64>" : value[key]]));
  }
  return value;
}

function maskLongArg(value) {
  if (typeof value === "string" && value.length > 140) {
    return `${value.slice(0, 120)}...`;
  }
  return value;
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function writeReport() {
  writeFileSync(resultPath, `${JSON.stringify(report, null, 2)}\n`);
}
