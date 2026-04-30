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
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.31-project-workbench-cli-generation");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const resultPath = path.join(assetsDir, "project-workbench-browser-state.json");
const canvasShotPath = path.join(assetsDir, "project-workbench-canvas.png");
const desktopShotPath = path.join(assetsDir, "project-workbench-desktop.png");

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

  report.checks.ready = waitFor("project workbench", () =>
    capyJson(["devtools", "--eval", readinessProbe()])
  , (data) => data?.cards === 6 && data?.visible === true && data?.projectPath);

  report.checks.layout = capyJson(["devtools", "--eval", layoutProbe()]);
  assert(report.checks.layout.cards === 6, "workbench must render six cards");
  assert(report.checks.layout.canvasRect.width > 400, "canvas region must be wide enough");
  assert(report.checks.layout.workbenchRect.width > 300, "workbench region must be visible");
  assert(report.checks.layout.plannerRect.width > 240, "planner region must be visible");
  assert(report.checks.layout.pageErrors.length === 0, "page errors must be empty");

  report.checks.selection = capyJson(["devtools", "--eval", selectionProbe()]);
  assert(report.checks.selection.before !== report.checks.selection.after, "card click must change selected card");
  assert(report.checks.selection.contextText.includes("poster") || report.checks.selection.contextText.includes("json"), "selected context must describe artifact source");

  report.checks.generation = capyJson(["devtools", "--eval", generationProbe()]);
  assert(report.checks.generation.run?.status === "planned", "dry-run generation must return planned status");
  assert(report.checks.generation.stateGeneration?.run?.dry_run === true, "state must retain dry-run generation result");
  assert(report.checks.generation.pageErrors.length === 0, "page errors after generation must be empty");

  report.checks.canvas_screenshot = capyJson(["screenshot", "--region", "canvas", "--out", canvasShotPath]);
  report.checks.desktop_capture = capyJson(["capture", "--out", desktopShotPath]);
  assert(statSync(canvasShotPath).size > 10000, "canvas screenshot must be non-empty");
  assert(statSync(desktopShotPath).size > 100000, "desktop capture must be non-empty");

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
  for (let attempt = 1; attempt <= 60; attempt += 1) {
    try {
      const value = producer();
      if (predicate(value)) return { label, attempt, value };
      lastError = new Error(`${label} not ready on attempt ${attempt}`);
    } catch (error) {
      lastError = error;
    }
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
  }
  throw lastError || new Error(`${label} did not become ready`);
}

function readinessProbe() {
  return `(() => {
    const state = window.capyWorkbench?.stateSnapshot?.();
    const cards = document.querySelectorAll('[data-project-card-kind]').length;
    return {
      visible: !document.querySelector('#project-workbench')?.hidden,
      cards,
      selectedCardId: state?.projectPackage?.selectedCardId || null,
      projectPath: state?.projectPackage?.path || null,
      status: state?.projectPackage?.status || null,
      pageErrors: window.__capyPageErrors || []
    };
  })()`;
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
      workbenchRect: rect('#project-workbench'),
      plannerRect: rect('[data-section="planner-chat"]'),
      cards: document.querySelectorAll('[data-project-card-kind]').length,
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function selectionProbe() {
  return `new Promise((resolve) => {
    const before = window.capyWorkbench.stateSnapshot().projectPackage.selectedCardId;
    const target = document.querySelector('[data-project-card-kind="poster"]') || document.querySelector('[data-project-card-kind]');
    target?.click();
    setTimeout(() => {
      const afterState = window.capyWorkbench.stateSnapshot();
      resolve({
        before,
        after: afterState.projectPackage.selectedCardId,
        contextText: document.querySelector('#project-package-meta')?.textContent || '',
        selectedSummary: document.querySelector('#project-selected-summary')?.textContent || '',
        pageErrors: window.__capyPageErrors || []
      });
    }, 350);
  })`;
}

function generationProbe() {
  return `new Promise((resolve) => {
    window.capyWorkbench.generateSelectedProjectArtifact({ dryRun: true, prompt: 'Make the selected artifact clearer.' })
      .then((result) => {
        setTimeout(() => {
          const state = window.capyWorkbench.stateSnapshot();
          resolve({
            run: result.run,
            stateGeneration: state.projectPackage.generation,
            selectedCardId: state.projectPackage.selectedCardId,
            pageErrors: window.__capyPageErrors || [],
            consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
          });
        }, 250);
      })
      .catch((error) => resolve({ error: String(error), pageErrors: window.__capyPageErrors || [] }));
  })`;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
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
