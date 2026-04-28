#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.6-canvas-chat-workbench");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const resultPath = path.join(assetsDir, "canvas-chat-verifier.json");
const capturePath = path.join(assetsDir, "canvas-chat-workbench.png");
const openCapturePath = path.join(assetsDir, "capy-cef-open-window.png");
const domCanvasPath = path.join(assetsDir, "canvas-chat-dom-canvas.png");
const domPlannerPath = path.join(assetsDir, "canvas-chat-dom-planner.png");

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

  const seeded = waitFor("canvas seed", () =>
    capyJson(["devtools", "--eval", ensureSelectionProbe()])
  , (data) => data?.canvas?.ready === true && data?.canvas?.nodeCount >= 4 && data?.selectedId);
  report.checks.seeded = seeded;

  report.checks.canvas_ready = capyJson(["state", "--key", "canvas.ready"]);
  assert(report.checks.canvas_ready.value === true, "canvas.ready must be true");

  report.checks.node_count = capyJson(["state", "--key", "canvas.nodeCount"]);
  assert(report.checks.node_count.value >= 4, "canvas.nodeCount must be at least 4");

  report.checks.selected_node = capyJson(["state", "--key", "canvas.selectedNode"]);
  assert(report.checks.selected_node.value?.id, "canvas.selectedNode must have an id");

  report.checks.planner_context = capyJson(["state", "--key", "planner.context"]);
  assert(
    report.checks.planner_context.value?.selected_count >= 1,
    "planner.context must include at least one selected item"
  );

  report.checks.layout = capyJson(["devtools", "--eval", layoutProbe()]);
  assert(report.checks.layout.canvasRect.width > 200, "canvas region must have width");
  assert(report.checks.layout.plannerRect.width > 200, "planner region must have width");
  assert(report.checks.layout.labels >= 4, "canvas labels must be rendered");
  assert(report.checks.layout.pageErrors.length === 0, "page errors must be empty");

  report.checks.interaction = capyJson(["devtools", "--eval", interactionProbe()]);
  assert(
    String(report.checks.interaction.afterSelected) === String(report.checks.interaction.clickedId),
    "clicking a canvas node must update selected id"
  );
  assert(report.checks.interaction.contextTitle, "planner context title must be visible");
  assert(report.checks.interaction.includesCanvasSelection, "composed prompt must include canvas selection");
  assert(report.checks.interaction.pageErrors.length === 0, "page errors after interaction must be empty");

  report.checks.dom_canvas = capyJson(["screenshot", "--region", "canvas", "--out", domCanvasPath]);
  report.checks.dom_planner = capyJson(["screenshot", "--region", "planner", "--out", domPlannerPath]);
  assert(existsSync(openCapturePath), `missing visible desktop capture: ${openCapturePath}`);
  copyFileSync(openCapturePath, capturePath);
  report.checks.visible_capture = {
    source: "scripts/verify-cef-shell.sh --launch open",
    out: capturePath,
    source_out: openCapturePath,
    bytes: statSync(capturePath).size
  };
  assert(report.checks.visible_capture.bytes > 100000, "native window capture must be non-empty");
  assert(statSync(capturePath).size > 100000, "native window capture file must be non-empty");

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
    stdio: ["ignore", "pipe", "pipe"]
  });
  const elapsed_ms = Date.now() - started;
  const parsed = JSON.parse(stdout);
  report.commands.push({
    cmd: ["target/debug/capy", ...args].join(" "),
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
        return { label, attempt, value };
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
      browser: document.documentElement.dataset.capyBrowser,
      readyState: document.readyState,
      canvasRect: rect('[data-section="canvas-host"]'),
      plannerRect: rect('[data-section="planner-chat"]'),
      contextTitle: document.querySelector('#context-title')?.textContent || '',
      labels: document.querySelectorAll('[data-node-id]').length,
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function ensureSelectionProbe() {
  return `(() => {
    const workbench = window.capyWorkbench;
    if (!workbench?.refreshPlannerContext) return null;
    let state = workbench.refreshPlannerContext();
    if (!state?.selectedId && Array.isArray(state?.blocks) && state.blocks[0]) {
      workbench.selectNode(state.blocks[0].id);
      state = workbench.refreshPlannerContext();
    }
    return state;
  })()`;
}

function interactionProbe() {
  return `new Promise((resolve) => {
    setTimeout(() => {
      const before = window.capyWorkbench.stateSnapshot();
      const labels = Array.from(document.querySelectorAll('[data-node-id]'));
      const target = labels.find((node) => !node.classList.contains('is-selected')) || labels[0] || null;
      target?.click();
      setTimeout(() => {
        const after = window.capyWorkbench.stateSnapshot();
        const composed = window.capyWorkbench.composePromptWithContext('Make this more premium');
        resolve({
          beforeSelected: before.selectedId,
          clickedId: target?.dataset.nodeId || null,
          afterSelected: after.selectedId,
          contextTitle: document.querySelector('#context-title')?.textContent || '',
          contextMeta: document.querySelector('#context-meta')?.textContent || '',
          composed,
          includesCanvasSelection: composed.includes('[Canvas selection]') && composed.includes('id='),
          pageErrors: window.__capyPageErrors || [],
          consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
        });
      }, 300);
    }, 250);
  })`;
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function summarize(value) {
  if (value && typeof value === "object") {
    const keys = Object.keys(value).slice(0, 8);
    return Object.fromEntries(keys.map((key) => [key, value[key]]));
  }
  return value;
}

function writeReport() {
  writeFileSync(resultPath, `${JSON.stringify(report, null, 2)}\n`);
}
