#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.39-multi-artifact-campaign");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const projectRoot = path.join(root, "target", "capy-v39-campaign-project");
const fixtureRoot = path.join(root, "fixtures", "project", "html-context");
const htmlPath = path.join(projectRoot, "web", "index.html");
const resultPath = path.join(assetsDir, "multi-artifact-campaign-desktop-state.json");
const canvasShotPath = path.join(assetsDir, "multi-artifact-campaign-canvas.png");
const desktopShotPath = path.join(assetsDir, "multi-artifact-campaign-desktop.png");

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
  const beforeHtml = readFileSync(htmlPath, "utf8");

  report.checks.open = capyJson(["open", `--project=${projectRoot}`]);
  report.checks.ready = waitFor(
    "campaign project",
    () => capyJson(["devtools", "--eval", readinessProbe(projectRoot)]),
    (data) => data?.visible === true && data?.projectPath === projectRoot
  );

  report.checks.campaign = capyJson(["devtools", "--eval", campaignProbe()]);
  assert(report.checks.campaign.run?.status === "proposed", "campaign run must be proposed");
  assert(report.checks.campaign.proposals === 4, "campaign must create four proposals");
  assert(report.checks.campaign.artifactRuns === 4, "campaign run must track four artifact runs");
  assert(report.checks.campaign.summaryVisible === true, "campaign summary panel must be visible");
  assert(report.checks.campaign.firstReviewStatus === "proposed", "first proposal must be selected for review");
  assert(report.checks.campaign.plannerHasInternalLeak === false, "planner message must hide internal ids");
  assert(report.checks.campaign.pageErrors.length === 0, "page errors after campaign must be empty");
  assert(report.checks.campaign.consoleErrors.length === 0, "console errors after campaign must be empty");

  const afterHtml = readFileSync(htmlPath, "utf8");
  report.checks.disk = {
    html_unchanged: beforeHtml === afterHtml,
    before_bytes: Buffer.byteLength(beforeHtml),
    after_bytes: Buffer.byteLength(afterHtml)
  };
  assert(report.checks.disk.html_unchanged, "campaign generate must not mutate HTML before review accept");

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

function campaignProbe() {
  return `new Promise(async (resolve) => {
    try {
      const result = await window.capyWorkbench.generateProjectCampaign({
        brief: '把 Web、海报、PPT 和视频故事板收成同一场发布 campaign。'
      });
      setTimeout(() => {
        const state = window.capyWorkbench.stateSnapshot();
        const summary = document.querySelector('#project-campaign-summary');
        const plannerText = document.querySelector('#message-list')?.textContent || '';
        const internalLeakPattern = /Provider:|Artifact:|Changed:|Status:|Run:|\\.capy\\/runs|\\b(?:art|surf_art|proj|gen|run|camp)_[a-z0-9_]{16,}\\b/i;
        resolve({
          run: result.run,
          proposals: result.proposals?.length || 0,
          artifactRuns: result.run?.artifact_runs?.length || 0,
          stateRunId: state.projectPackage.campaign?.run?.id || null,
          firstReviewStatus: state.projectPackage.review?.run?.status || null,
          summaryVisible: summary ? !summary.hidden : false,
          summaryText: summary?.textContent || '',
          plannerHasInternalLeak: internalLeakPattern.test(plannerText),
          plannerText,
          pageErrors: window.__capyPageErrors || [],
          consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
        });
      }, 300);
    } catch (error) {
      resolve({ error: String(error), pageErrors: window.__capyPageErrors || [] });
    }
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
