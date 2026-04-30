#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.34-project-ai-delivery-loop");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const resultPath = path.join(assetsDir, "project-ai-delivery-loop-state.json");
const canvasShotPath = path.join(assetsDir, "project-ai-canvas.png");
const desktopShotPath = path.join(assetsDir, "project-ai-desktop.png");
const sdkResponsePath = process.env.CAPY_VERIFY_SDK_RESPONSE
  ? path.resolve(process.env.CAPY_VERIFY_SDK_RESPONSE)
  : path.join(root, "fixtures/project/html-context/sdk-response/project-ai-html.json");

mkdirSync(assetsDir, { recursive: true });

const report = {
  ok: false,
  started_at: new Date().toISOString(),
  socket: process.env.CAPYBARA_SOCKET || null,
  sdk_response_path: sdkResponsePath,
  commands: [],
  checks: {}
};

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);

  report.checks.ready = waitFor("project workbench", () =>
    capyJson(["devtools", "--eval", readinessProbe()])
  , (data) => data?.cards === 6 && data?.visible === true && data?.projectPath);

  report.project_path = report.checks.ready.value.projectPath;
  report.checks.layout_before = capyJson(["devtools", "--eval", layoutProbe()]);
  assert(report.checks.layout_before.cards === 6, "workbench must render six cards");
  assert(report.checks.layout_before.canvasRect.width > 400, "canvas region must be wide enough");
  assert(report.checks.layout_before.workbenchRect.width > 300, "workbench region must be visible");
  assert(report.checks.layout_before.plannerRect.width > 240, "planner region must be visible");
  assert(report.checks.layout_before.pageErrors.length === 0, "page errors before generation must be empty");
  assert(report.checks.layout_before.consoleErrors.length === 0, "console errors before generation must be empty");

  report.checks.generation = capyJson(["devtools", "--eval", generationProbe(sdkResponsePath)]);
  assert(!report.checks.generation.error, `generation failed: ${report.checks.generation.error}`);
  assert(report.checks.generation.run?.status === "completed", "live generation must complete");
  assert(report.checks.generation.run?.output?.mode === "live", "generation output must record live mode");
  assert(report.checks.generation.run?.provider === "codex", "generation must use codex provider in verification");
  assert(report.checks.generation.previewIncludesLaunch === true, "preview must show AI-produced headline");
  assert(report.checks.generation.plannerMentionsSummary === true, "Planner must explain the AI output");
  assert(report.checks.generation.pageErrors.length === 0, "page errors after generation must be empty");
  assert(report.checks.generation.consoleErrors.length === 0, "console errors after generation must be empty");

  const projectRoot = path.resolve(root, report.project_path);
  const htmlPath = path.join(projectRoot, "web", "index.html");
  const html = readFileSync(htmlPath, "utf8");
  report.checks.disk = {
    html_path: htmlPath,
    contains_launch_headline: html.includes("Project Context Launch"),
    bytes: Buffer.byteLength(html),
  };
  assert(report.checks.disk.contains_launch_headline, "disk HTML must contain AI-produced headline");

  report.checks.layout_after = capyJson(["devtools", "--eval", layoutProbe()]);
  assert(report.checks.layout_after.canvasRect.width > 400, "canvas region must remain wide after generation");
  assert(report.checks.layout_after.pageErrors.length === 0, "page errors after layout probe must be empty");
  assert(report.checks.layout_after.consoleErrors.length === 0, "console errors after layout probe must be empty");

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
      browser: document.documentElement.dataset.capyBrowser,
      readyState: document.readyState,
      canvasRect: rect('[data-section="canvas-host"]'),
      workbenchRect: rect('#project-workbench'),
      packageRect: rect('#project-package-panel'),
      plannerRect: rect('[data-section="planner-chat"]'),
      previewRect: rect('#project-preview-frame'),
      cards: document.querySelectorAll('[data-project-card-kind]').length,
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function generationProbe(sdkResponsePath) {
  const sdkResponseJson = JSON.stringify(sdkResponsePath);
  return `new Promise((resolve) => {
    const provider = document.querySelector('#provider');
    const prompt = document.querySelector('#prompt');
    if (provider) {
      provider.value = 'codex';
      provider.dispatchEvent(new Event('change', { bubbles: true }));
    }
    if (prompt) {
      prompt.value = '把首屏改成发布会发布页，保留项目设计语言。';
      prompt.dispatchEvent(new Event('input', { bubbles: true }));
    }
    const card = Array.from(document.querySelectorAll('[data-project-card-kind="web"]'))
      .find((node) => node.dataset.projectCardId?.startsWith('art_'))
      || Array.from(document.querySelectorAll('[data-project-card-id]'))
        .find((node) => node.dataset.projectCardId?.startsWith('art_'));
    const action = card?.querySelector('.project-card-action');
    if (!card || !action) {
      resolve({ error: 'missing project card action', pageErrors: window.__capyPageErrors || [] });
      return;
    }
    card.click();
    setTimeout(async () => {
      try {
        await window.capyWorkbench.generateSelectedProjectArtifact({
          provider: 'codex',
          live: true,
          prompt: prompt?.value || '把首屏改成发布会发布页，保留项目设计语言。',
          sdkResponse: ${sdkResponseJson}
        });
      } catch (error) {
        resolve({ error: String(error), pageErrors: window.__capyPageErrors || [] });
        return;
      }
      setTimeout(() => {
        const state = window.capyWorkbench?.stateSnapshot?.();
        const generation = state?.projectPackage?.generation;
        const run = generation?.run;
        const previewSource = state?.projectPackage?.previewSource || '';
        const frameSource = document.querySelector('#project-preview-frame')?.srcdoc || '';
        const plannerText = document.querySelector('#message-list')?.textContent || '';
        const pageErrors = window.__capyPageErrors || [];
        const consoleErrors = (window.__capyConsoleEvents || []).filter((event) => event.level === 'error');
        resolve({
          status: state?.projectPackage?.status || null,
          selectedCardId: state?.projectPackage?.selectedCardId || null,
          run,
          previewIncludesLaunch: previewSource.includes('Project Context Launch') || frameSource.includes('Project Context Launch'),
          plannerMentionsSummary: plannerText.includes('首屏标题和说明') || plannerText.includes('Project Context Launch'),
          plannerText,
          pageErrors,
          consoleErrors
        });
      }, 350);
    }, 120);
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
