#!/usr/bin/env node
if (process.argv.includes("--help") || process.argv.includes("-h")) {
  const scriptName = process.argv[1] || "scripts/verify-*.mjs";
  console.log("Usage: node " + scriptName + " [script-specific args]\n\nUse when: AI runs a version-specific browser, DOM, state, or contract verification script listed by BDD, status.json, evidence notes, or capy help harness.\n\nRequired params: script-specific; inspect the owning version status/evidence entry that names this script. Many scripts default to their version directory.\n\nState effects: may start local browser work, interact with Capybara test hooks, and write screenshots, state, or logs under spec/versions/<version>/evidence/assets/ or target/.\n\nPitfalls: do not run by filename guessing; first read target/debug/capy help harness and the owning version status. This generic help describes the family, while the script body owns exact assertions.\n\nNext step: rerun without --help only after BDD/status names this script, then add outputs to evidence/index.html.\n");
  process.exit(0);
}
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.38-precise-selection-context");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const projectRoot = path.join(root, "target", "capy-v38-selection-context-project");
const fixtureRoot = path.join(root, "fixtures", "project", "html-context");
const resultPath = path.join(assetsDir, "selection-context-desktop-state.json");
const canvasShotPath = path.join(assetsDir, "selection-context-canvas.png");
const desktopShotPath = path.join(assetsDir, "selection-context-desktop.png");

mkdirSync(assetsDir, { recursive: true });

const report = {
  ok: false,
  started_at: new Date().toISOString(),
  socket: process.env.CAPYBARA_SOCKET || null,
  project_root: projectRoot,
  commands: [],
  checks: {}
};

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  rmSync(projectRoot, { recursive: true, force: true });
  cpSync(fixtureRoot, projectRoot, { recursive: true });

  report.checks.open = capyJson(["open", `--project=${projectRoot}`]);
  report.checks.ready = waitFor(
    "selection context project",
    () => capyJson(["devtools", "--eval", readinessProbe(projectRoot)]),
    (data) => data?.visible === true && data?.projectPath === projectRoot
  );

  report.checks.html_selection = capyJson(["devtools", "--eval", htmlSelectionProbe()]);
  assert(report.checks.html_selection.context?.selection_context?.kind === "html-section", "HTML selection must resolve to html-section");
  assert(report.checks.html_selection.context?.selection_context?.scope === "sub-artifact", "HTML selection must be sub-artifact scoped");
  assert(report.checks.html_selection.context?.selection_context?.selected_text === "Project Context Draft", "HTML selection must capture headline text");
  assert(report.checks.html_selection.panelVisible === true, "selection context panel must be visible");
  assert(report.checks.html_selection.pageErrors.length === 0, "page errors after HTML selection must be empty");

  report.checks.json_selection = capyJson(["devtools", "--eval", jsonSelectionProbe()]);
  assert(report.checks.json_selection.context?.selection_context?.kind === "json-pointer", "JSON selection must resolve to json-pointer");
  assert(report.checks.json_selection.context?.selection_context?.selected_json === "Launch", "JSON selection must capture selected title");
  assert(report.checks.json_selection.panelVisible === true, "selection context panel must stay visible after JSON selection");
  assert(report.checks.json_selection.pageErrors.length === 0, "page errors after JSON selection must be empty");

  report.checks.fallback_selection = capyJson(["devtools", "--eval", fallbackSelectionProbe()]);
  assert(report.checks.fallback_selection.context?.selection_context?.kind === "file", "unsupported selection must fall back to file context");
  assert(report.checks.fallback_selection.context?.selection_context?.fallback_reason, "fallback context must explain the reason");
  assert(report.checks.fallback_selection.consoleErrors.length === 0, "console errors after fallback selection must be empty");

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
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 24 * 1024 * 1024
  });
  const parsed = JSON.parse(stdout);
  report.commands.push({
    cmd: ["target/debug/capy", ...args].join(" "),
    elapsed_ms: Date.now() - started,
    output_summary: summarize(parsed)
  });
  return parsed;
}

function waitFor(label, producer, predicate) {
  let lastError = null;
  for (let attempt = 1; attempt <= 80; attempt += 1) {
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

function readinessProbe(expectedProject) {
  return `(() => {
    const state = window.capyWorkbench?.stateSnapshot?.();
    return {
      visible: !document.querySelector('#project-workbench')?.hidden,
      projectPath: state?.projectPackage?.path || null,
      expectedProject: ${JSON.stringify(expectedProject)},
      cards: document.querySelectorAll('[data-project-card-kind]').length,
      status: state?.projectPackage?.status || null,
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function htmlSelectionProbe() {
  return `new Promise((resolve) => {
    const web = document.querySelector('[data-project-card-kind="web"]');
    web?.click();
    setTimeout(async () => {
      try {
        const context = await window.capyWorkbench.buildSelectedProjectContext('[data-capy-section="hero-title"]');
        resolve({
          context,
          panelVisible: !document.querySelector('#project-selection-context')?.hidden,
          panelText: document.querySelector('#project-selection-context')?.textContent || '',
          pageErrors: window.__capyPageErrors || [],
          consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
        });
      } catch (error) {
        resolve({ error: String(error), pageErrors: window.__capyPageErrors || [] });
      }
    }, 250);
  })`;
}

function jsonSelectionProbe() {
  return `new Promise((resolve) => {
    const poster = document.querySelector('[data-project-card-kind="poster"]');
    poster?.click();
    setTimeout(async () => {
      try {
        const context = await window.capyWorkbench.buildSelectedProjectContext({ jsonPointer: '/pages/0/title' });
        resolve({
          context,
          panelVisible: !document.querySelector('#project-selection-context')?.hidden,
          panelText: document.querySelector('#project-selection-context')?.textContent || '',
          pageErrors: window.__capyPageErrors || [],
          consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
        });
      } catch (error) {
        resolve({ error: String(error), pageErrors: window.__capyPageErrors || [] });
      }
    }, 250);
  })`;
}

function fallbackSelectionProbe() {
  return `new Promise((resolve) => {
    const image = document.querySelector('[data-artifact-id="art_00000000000000000000000000000002"]');
    image?.click();
    setTimeout(async () => {
      try {
        const context = await window.capyWorkbench.buildSelectedProjectContext('[data-capy-section="hero-title"]');
        resolve({
          context,
          panelVisible: !document.querySelector('#project-selection-context')?.hidden,
          panelText: document.querySelector('#project-selection-context')?.textContent || '',
          pageErrors: window.__capyPageErrors || [],
          consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
        });
      } catch (error) {
        resolve({ error: String(error), pageErrors: window.__capyPageErrors || [] });
      }
    }, 250);
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
