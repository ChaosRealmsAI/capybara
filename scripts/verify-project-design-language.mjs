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
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.36-project-design-language");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const resultPath = path.join(assetsDir, "design-language-desktop-state.json");
const canvasShotPath = path.join(assetsDir, "design-language-canvas.png");
const desktopShotPath = path.join(assetsDir, "design-language-desktop.png");

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

  report.checks.ready = waitFor(
    "project design language",
    () => capyJson(["devtools", "--eval", readinessProbe()]),
    (data) => data?.visible === true && data?.designVisible === true && data?.tokenCount === "1"
  );

  report.checks.layout = capyJson(["devtools", "--eval", layoutProbe()]);
  assert(report.checks.layout.designSummaryRect.width > 260, "design summary must be visible");
  assert(report.checks.layout.projectPanelRect.width > 300, "project panel must be visible");
  assert(report.checks.layout.canvasRect.width > 400, "canvas region must stay usable");
  assert(report.checks.layout.pageErrors.length === 0, "page errors must be empty");
  assert(report.checks.layout.consoleErrors.length === 0, "console errors must be empty");

  report.checks.selection = capyJson(["devtools", "--eval", selectionProbe()]);
  assert(report.checks.selection.before !== report.checks.selection.after, "card click must change selected card");
  assert(report.checks.selection.designRef.startsWith("dlpkg-fnv1a64-"), "summary must expose stable design ref");
  assert(report.checks.selection.pageErrors.length === 0, "page errors after selection must be empty");

  report.checks.generation = capyJson(["devtools", "--eval", generationProbe()]);
  assert(report.checks.generation.run?.status === "planned", "dry-run generation must return planned status");
  assert(report.checks.generation.run?.provider === "fixture", "visible verification must use no-spend fixture provider");
  assert(report.checks.generation.run?.design_language_ref?.startsWith("dlpkg-fnv1a64-"), "run must record design_language_ref");
  assert(report.checks.generation.stateGeneration?.run?.design_language_ref === report.checks.generation.run.design_language_ref, "state generation must retain same design_language_ref");
  assert(report.checks.generation.pageErrors.length === 0, "page errors after generation must be empty");
  assert(report.checks.generation.consoleErrors.length === 0, "console errors after generation must be empty");

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
    const summary = document.querySelector('#project-design-language-summary');
    return {
      visible: !document.querySelector('#project-workbench')?.hidden,
      designVisible: summary ? !summary.hidden : false,
      designRef: summary?.dataset?.designLanguageRef || '',
      tokenCount: summary?.querySelector('dd')?.textContent?.trim() || '',
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
      projectPanelRect: rect('#project-package-panel'),
      designSummaryRect: rect('#project-design-language-summary'),
      designText: document.querySelector('#project-design-language-summary')?.textContent || '',
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function selectionProbe() {
  return `new Promise((resolve) => {
    const before = window.capyWorkbench.stateSnapshot().projectPackage.selectedCardId;
    const cards = Array.from(document.querySelectorAll('[data-project-card-id]'));
    const poster = cards.find((card) => card.dataset.projectCardKind === 'poster' && card.dataset.projectCardId !== before);
    const target = poster || cards.find((card) => card.dataset.projectCardId !== before) || cards[0];
    target?.click();
    setTimeout(() => {
      const state = window.capyWorkbench.stateSnapshot();
      resolve({
        before,
        after: state.projectPackage.selectedCardId,
        designRef: document.querySelector('#project-design-language-summary')?.dataset?.designLanguageRef || '',
        designText: document.querySelector('#project-design-language-summary')?.textContent || '',
        pageErrors: window.__capyPageErrors || [],
        consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
      });
    }, 350);
  })`;
}

function generationProbe() {
  return `new Promise((resolve) => {
    window.capyWorkbench.generateSelectedProjectArtifact({
      dryRun: true,
      live: false,
      provider: 'fixture',
      prompt: 'Make the selected artifact follow the active design language.'
    })
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
