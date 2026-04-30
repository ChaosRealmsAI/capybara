#!/usr/bin/env node
if (process.argv.includes("--help") || process.argv.includes("-h")) {
  const scriptName = process.argv[1] || "scripts/verify-*.mjs";
  console.log("Usage: node " + scriptName + " [script-specific args]\n\nUse when: AI runs a version-specific browser, DOM, state, or contract verification script listed by BDD, status.json, evidence notes, or capy help harness.\n\nRequired params: script-specific; inspect the owning version status/evidence entry that names this script. Many scripts default to their version directory.\n\nState effects: may start local browser work, interact with Capybara test hooks, and write screenshots, state, or logs under spec/versions/<version>/evidence/assets/ or target/.\n\nPitfalls: do not run by filename guessing; first read target/debug/capy help harness and the owning version status. This generic help describes the family, while the script body owns exact assertions.\n\nNext step: rerun without --help only after BDD/status names this script, then add outputs to evidence/index.html.\n");
  process.exit(0);
}
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.35-canvas-artifact-nodes");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const projectSource = path.resolve(process.env.CAPY_VERIFY_PROJECT_SOURCE || "fixtures/project/html-context");
const projectRoot = path.resolve(process.env.CAPY_VERIFY_PROJECT || path.join(assetsDir, "project-html-context"));
const capy = path.join(root, "target", "debug", "capy");
const resultPath = path.join(assetsDir, "canvas-artifact-nodes-state.json");
const canvasShotPath = path.join(assetsDir, "canvas-artifact-nodes-canvas.png");
const desktopShotPath = path.join(assetsDir, "canvas-artifact-nodes-desktop.png");

mkdirSync(assetsDir, { recursive: true });
if (!process.env.CAPY_VERIFY_PROJECT && existsSync(projectRoot)) rmSync(projectRoot, { recursive: true, force: true });
if (!existsSync(projectRoot)) cpSync(projectSource, projectRoot, { recursive: true });

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
  capyJson(["open", "--project", projectRoot]);

  report.checks.ready = waitFor("artifact canvas nodes", () =>
    capyJson(["devtools", "--eval", readinessProbe(projectRoot)])
  , (data) => data?.status === "ready" && data?.surfaceNodeCount >= 3 && data?.artifactNodeCount >= 3);

  report.checks.layout = capyJson(["devtools", "--eval", layoutProbe()]);
  assert(report.checks.layout.canvasRect.width > 520, "canvas region must be visible and wide");
  assert(report.checks.layout.labelCount >= 3, "artifact node labels must render");
  assertNoErrors(report.checks.layout, "layout");

  const htmlPath = path.join(projectRoot, "web", "index.html");
  const beforeHash = sha256(htmlPath);
  report.checks.selection = capyJson(["devtools", "--eval", selectLandingProbe()]);
  assert(report.checks.selection.selected?.artifact_ref?.source_path === "web/index.html", "Landing HTML node must expose source path");
  assert(report.checks.selection.projectSelectedArtifactId === report.checks.selection.selected.artifact_ref.artifact_id, "project selection must follow canvas selection");
  assert(report.checks.selection.contextText.includes("web/index.html"), "Planner context must include artifact source path");
  assertNoErrors(report.checks.selection, "selection");

  report.checks.geometry = capyJson(["devtools", "--eval", geometryProbe()]);
  assert(report.checks.geometry.movedDistance > 40, "drag must move the canvas node");
  assert(report.checks.geometry.resizedDelta > 40, "resize must change canvas node size");
  assertNoErrors(report.checks.geometry, "geometry");

  const surfaceNodes = JSON.parse(readFileSync(path.join(projectRoot, ".capy", "surface-nodes.json"), "utf8"));
  const surfaceNode = surfaceNodes.nodes.find((node) => node.id === report.checks.geometry.surfaceNodeId);
  assert(surfaceNode, "surface node file must contain selected node");
  assert(close(surfaceNode.geometry.x, report.checks.geometry.after.bounds.x), "persisted x must match canvas");
  assert(close(surfaceNode.geometry.w, report.checks.geometry.after.bounds.w), "persisted width must match canvas");
  report.checks.persistence = { surface_nodes_path: ".capy/surface-nodes.json", surfaceNode, source_hash_unchanged: sha256(htmlPath) === beforeHash };
  assert(report.checks.persistence.source_hash_unchanged, "drag/resize must not change source artifact file");

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

function readinessProbe(expectedProject) {
  const expected = JSON.stringify(expectedProject);
  return `(() => {
    const state = window.capyWorkbench?.stateSnapshot?.();
    const blocks = state?.blocks || [];
    return {
      status: state?.projectPackage?.status || null,
      path: state?.projectPackage?.path || null,
      expectedProject: ${expected},
      surfaceNodeCount: state?.projectPackage?.surfaceNodeCount || 0,
      artifactNodeCount: blocks.filter((node) => node.content_kind === 'project_artifact').length,
      selectedArtifactId: state?.projectPackage?.selectedArtifactId || null,
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
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
      canvasRect: rect('[data-section="canvas-host"]'),
      plannerRect: rect('[data-section="planner-chat"]'),
      labelCount: document.querySelectorAll('[data-capy-component-kind="project-artifact"]').length,
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function selectLandingProbe() {
  return `new Promise((resolve) => {
    const pick = () => (window.capyWorkbench?.stateSnapshot?.().blocks || [])
      .find((node) => node.artifact_ref?.source_path === 'web/index.html');
    const node = pick();
    if (!node) return resolve({ error: 'Landing HTML artifact node not found' });
    window.capyWorkbench.selectNode(node.id);
    setTimeout(() => {
      const state = window.capyWorkbench.stateSnapshot();
      resolve({
        selected: state.canvas.selectedNode,
        projectSelectedArtifactId: state.projectPackage.selectedArtifactId,
        contextText: state.planner.contextText || '',
        pageErrors: window.__capyPageErrors || [],
        consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
      });
    }, 500);
  })`;
}

function geometryProbe() {
  return `new Promise((resolve) => {
    const state = window.capyWorkbench.stateSnapshot();
    const before = state.canvas.selectedNode;
    const bounds = before?.bounds || before?.geometry;
    if (!before?.id || !bounds) return resolve({ error: 'no selected artifact node' });
    window.capyWorkbench.moveNodeById(before.id, bounds.x + 88, bounds.y + 44);
    setTimeout(() => {
      const moved = window.capyWorkbench.stateSnapshot().canvas.selectedNode;
      const movedBounds = moved.bounds || moved.geometry;
      window.capyWorkbench.resizeNodeById(moved.id, movedBounds.x, movedBounds.y, movedBounds.w + 96, movedBounds.h + 52);
      setTimeout(() => {
        const after = window.capyWorkbench.stateSnapshot().canvas.selectedNode;
        const afterBounds = after.bounds || after.geometry;
        resolve({
          surfaceNodeId: after.artifact_ref?.surface_node_id,
          before: { id: before.id, bounds },
          after: { id: after.id, bounds: afterBounds },
          movedDistance: Math.hypot(afterBounds.x - bounds.x, afterBounds.y - bounds.y),
          resizedDelta: Math.hypot(afterBounds.w - bounds.w, afterBounds.h - bounds.h),
          pageErrors: window.__capyPageErrors || [],
          consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
        });
      }, 900);
    }, 220);
  })`;
}

function capyJson(args) {
  const started = Date.now();
  const stdout = execFileSync(capy, args, { cwd: root, env: process.env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 24 * 1024 * 1024 });
  const parsed = JSON.parse(stdout);
  report.commands.push({ cmd: ["target/debug/capy", ...args].join(" "), elapsed_ms: Date.now() - started, output_summary: summarize(parsed) });
  return parsed;
}

function waitFor(label, producer, predicate) {
  let lastError = null;
  for (let attempt = 1; attempt <= 80; attempt += 1) {
    try {
      const value = producer();
      if (predicate(value)) return { label, attempt, value: summarize(value) };
      lastError = new Error(`${label} not ready on attempt ${attempt}`);
    } catch (error) {
      lastError = error;
    }
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
  }
  throw lastError || new Error(`${label} did not become ready`);
}

function sha256(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function close(a, b) {
  return Math.abs(Number(a || 0) - Number(b || 0)) <= 1;
}

function assertNoErrors(value, label) {
  assert((value.pageErrors || []).length === 0, `${label} page errors must be empty`);
  assert((value.consoleErrors || []).length === 0, `${label} console errors must be empty`);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function summarize(value) {
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).slice(0, 8).map((key) => [key, value[key]]));
  }
  return value;
}

function writeReport() {
  writeFileSync(resultPath, `${JSON.stringify(report, null, 2)}\n`);
}
