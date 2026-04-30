#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.9-poster-json-renderer");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const fixture = path.join(root, "fixtures", "poster", "sample-poster.json");
const resultPath = path.join(assetsDir, "poster-canvas-verifier.json");
const capturePath = path.join(assetsDir, "poster-canvas-workbench.png");

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
  assert(existsSync(fixture), `missing poster fixture: ${fixture}`);

  report.checks.ready = waitFor("canvas ready", () =>
    capyJson(["canvas", "snapshot"])
  , (data) => data?.canvas?.ready === true);

  const before = capyJson(["canvas", "snapshot"]);
  report.checks.before = summarizeCanvas(before);

  report.checks.load_poster = capyJson([
    "canvas",
    "load-poster",
    "--path",
    fixture,
    "--title",
    "Verifier poster",
    "--x",
    "360",
    "--y",
    "118"
  ]);
  assert(report.checks.load_poster.ok === true, "load-poster must report ok=true");
  assert(report.checks.load_poster.content_kind === "poster", "load-poster must select a poster node");
  assert(report.checks.load_poster.render_state === "rendered", "poster render state must be rendered");
  assert(report.checks.load_poster.poster_state?.layer_count >= 3, "poster state must include layers");
  assert(
    report.checks.load_poster.poster_state?.generated_assets?.[0]?.task_id === "task_demo_poster_001",
    "poster provenance task id must survive in state"
  );

  report.checks.dom_initial = capyJson(["devtools", "--eval", domProbe()]);
  assert(report.checks.dom_initial.overlayCount >= 1, "poster overlay must exist in DOM");
  assert(report.checks.dom_initial.stageCount >= 1, "poster stage must exist in DOM");
  assert(report.checks.dom_initial.headline.includes("CERAMIC"), "initial headline must render");
  assert(report.checks.dom_initial.pageErrors.length === 0, "page errors before interaction must be empty");
  assert(report.checks.dom_initial.consoleErrors.length === 0, "console errors before interaction must be empty");

  report.checks.interaction = capyJson([
    "devtools",
    "--eval",
    "window.capyWorkbench.verifyPosterRenderer()"
  ]);
  assert(report.checks.interaction.passed === true, "poster verifier interaction must pass");
  report.checks.visible_poster = capyJson([
    "devtools",
    "--eval",
    visiblePosterProbe(report.checks.interaction.node_id)
  ]);
  assert(report.checks.visible_poster.visibleRatio >= 0.9, "poster overlay must be mostly visible in the canvas panel");
  assert(report.checks.visible_poster.headline.includes("LOCAL"), "focused poster screenshot must contain the edited headline");

  report.checks.state = capyJson([
    "devtools",
    "--eval",
    "window.capyWorkbench.stateSnapshot()"
  ]);
  assert(
    (report.checks.state.poster?.documents || []).some((doc) => doc.render_state === "error-preserved"),
    "state snapshot must preserve last valid poster after invalid JSON"
  );

  report.checks.screenshot = capyJson(["screenshot", "--out", capturePath]);
  assert(report.checks.screenshot.bytes > 100000, "workbench screenshot must be non-empty");
  assert(statSync(capturePath).size > 100000, "workbench screenshot file must be non-empty");

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

function domProbe() {
  return `(() => {
    const overlay = document.querySelector('[data-component="poster-overlay-layer"]');
    const headline = document.querySelector('[data-poster-node-id] [data-layer-id="headline"]');
    const rect = overlay?.getBoundingClientRect();
    return {
      overlayFound: Boolean(overlay),
      overlayCount: document.querySelectorAll('[data-poster-node-id]').length,
      stageCount: document.querySelectorAll('[data-poster-node-id] .poster-stage').length,
      layerCount: document.querySelectorAll('[data-poster-node-id] [data-layer-id]').length,
      headline: headline?.textContent || '',
      overlayRect: rect ? { width: rect.width, height: rect.height } : null,
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function visiblePosterProbe(nodeId) {
  return `(() => {
    const poster = document.querySelector('[data-poster-node-id="${Number(nodeId)}"]');
    const host = document.querySelector('[data-section="canvas-host"]');
    const headline = poster?.querySelector('[data-layer-id="headline"]');
    if (!poster || !host) {
      return { found: false, visibleRatio: 0, headline: '' };
    }
    const rect = poster.getBoundingClientRect();
    const hostRect = host.getBoundingClientRect();
    const overlapWidth = Math.max(0, Math.min(rect.right, hostRect.right) - Math.max(rect.left, hostRect.left));
    const overlapHeight = Math.max(0, Math.min(rect.bottom, hostRect.bottom) - Math.max(rect.top, hostRect.top));
    const area = Math.max(1, rect.width * rect.height);
    return {
      found: true,
      headline: headline?.textContent || '',
      rect: {
        left: Number(rect.left.toFixed(2)),
        top: Number(rect.top.toFixed(2)),
        width: Number(rect.width.toFixed(2)),
        height: Number(rect.height.toFixed(2))
      },
      hostRect: {
        left: Number(hostRect.left.toFixed(2)),
        top: Number(hostRect.top.toFixed(2)),
        width: Number(hostRect.width.toFixed(2)),
        height: Number(hostRect.height.toFixed(2))
      },
      visibleRatio: Number(((overlapWidth * overlapHeight) / area).toFixed(4))
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
      source_path: selected.source_path
    } : null,
    posterDocuments: value?.poster?.documents?.length || 0
  };
}

function summarize(value) {
  if (value && typeof value === "object") {
    const keys = Object.keys(value).slice(0, 8);
    return Object.fromEntries(keys.map((key) => [key, value[key]]));
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
