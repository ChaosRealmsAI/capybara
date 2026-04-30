#!/usr/bin/env node
if (process.argv.includes("--help") || process.argv.includes("-h")) {
  const scriptName = process.argv[1] || "scripts/verify-*.mjs";
  console.log("Usage: node " + scriptName + " [script-specific args]\n\nUse when: AI runs a version-specific browser, DOM, state, or contract verification script listed by BDD, status.json, evidence notes, or capy help harness.\n\nRequired params: script-specific; inspect the owning version status/evidence entry that names this script. Many scripts default to their version directory.\n\nState effects: may start local browser work, interact with Capybara test hooks, and write screenshots, state, or logs under spec/versions/<version>/evidence/assets/ or target/.\n\nPitfalls: do not run by filename guessing; first read target/debug/capy help harness and the owning version status. This generic help describes the family, while the script body owns exact assertions.\n\nNext step: rerun without --help only after BDD/status names this script, then add outputs to evidence/index.html.\n");
  process.exit(0);
}
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.37-ai-diff-acceptance");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const resultPath = path.join(assetsDir, "diff-review-state.json");
const desktopShotPath = path.join(assetsDir, "diff-review-desktop.png");
const acceptedShotPath = path.join(assetsDir, "diff-review-accepted.png");
const sdkResponsePath = process.env.CAPY_VERIFY_SDK_RESPONSE
  ? path.resolve(process.env.CAPY_VERIFY_SDK_RESPONSE)
  : path.join(root, "fixtures/project/html-context/sdk-response/project-ai-html.json");
const projectRoot = path.join(root, "target", "capy-v37-ai-diff-project");

mkdirSync(assetsDir, { recursive: true });

const report = {
  ok: false,
  started_at: new Date().toISOString(),
  sdk_response_path: sdkResponsePath,
  project_path: projectRoot,
  socket: process.env.CAPYBARA_SOCKET || null,
  commands: [],
  checks: {}
};

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  prepareProject();
  const htmlPath = path.join(projectRoot, "web", "index.html");
  const beforeSource = readFileSync(htmlPath, "utf8");
  report.checks.before_hash = hashText(beforeSource);

  if (process.env.CAPY_VERIFY_SKIP_OPEN !== "1") {
    execFileSync(path.join(root, "scripts/build-canvas-for-app.sh"), {
      cwd: root,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"]
    });
    const instance = openDebugShell(projectRoot);
    report.socket = instance.socket;
  }

  report.checks.ready = waitFor("project workbench", () =>
    capyJson(["devtools", "--eval", readinessProbe()])
  , (data) => data?.cards === 6 && data?.visible === true && data?.projectPath);

  report.checks.layout_before = capyJson(["devtools", "--eval", layoutProbe()]);
  assert(report.checks.layout_before.pageErrors.length === 0, "page errors before review must be empty");
  assert(report.checks.layout_before.consoleErrors.length === 0, "console errors before review must be empty");
  assert(report.checks.layout_before.workbenchRect.width > 300, "workbench must be visible");
  assert(report.checks.layout_before.packageRect.width > 260, "project package panel must be visible");

  report.checks.proposal = capyJson(["devtools", "--eval", proposalProbe(sdkResponsePath)]);
  assert(report.checks.proposal.run?.status === "proposed", "generation must create proposed run");
  assert(report.checks.proposal.reviewStatus === "proposed", "review state must be proposed");
  assert(report.checks.proposal.reviewPanelVisible === true, "AI diff review panel must be visible");
  assert(report.checks.proposal.previewIncludesLaunch === true, "preview must show proposed source");
  assert(report.checks.proposal.pageErrors.length === 0, "page errors after proposal must be empty");
  assert(report.checks.proposal.consoleErrors.length === 0, "console errors after proposal must be empty");
  const afterProposalSource = readFileSync(htmlPath, "utf8");
  report.checks.proposal_disk_hash = hashText(afterProposalSource);
  assert(report.checks.before_hash === report.checks.proposal_disk_hash, "proposal must not mutate disk source");

  report.checks.desktop_capture = capyJson(["capture", "--out", desktopShotPath]);
  assert(statSync(desktopShotPath).size > 100000, "desktop capture must be non-empty");

  report.checks.accept_click = capyJson(["click", "--query", "[data-ai-diff-action='accept']"]);
  report.checks.accepted = waitFor("accepted review", () =>
    capyJson(["devtools", "--eval", reviewStateProbe()])
  , (data) => data?.reviewStatus === "accepted");
  const acceptedSource = readFileSync(htmlPath, "utf8");
  assert(acceptedSource.includes("Project Context Launch"), "accept must mutate disk source");
  report.checks.accepted_hash = hashText(acceptedSource);
  report.checks.accepted_capture = capyJson(["capture", "--out", acceptedShotPath]);
  assert(statSync(acceptedShotPath).size > 100000, "accepted desktop capture must be non-empty");

  report.checks.undo_click = capyJson(["click", "--query", "[data-ai-diff-action='undo']"]);
  report.checks.undone = waitFor("undone review", () =>
    capyJson(["devtools", "--eval", reviewStateProbe()])
  , (data) => data?.reviewStatus === "reverted" && data?.previewIncludesLaunch === false);
  const undoneSource = readFileSync(htmlPath, "utf8");
  assert(hashText(undoneSource) === report.checks.before_hash, "undo must restore previous source");

  report.checks.reject = capyJson(["devtools", "--eval", rejectProbe(sdkResponsePath)]);
  assert(report.checks.reject.reviewStatus === "rejected", "reject must update review state");
  assert(hashText(readFileSync(htmlPath, "utf8")) === report.checks.before_hash, "reject must leave disk source unchanged");
  assert(report.checks.reject.pageErrors.length === 0, "page errors after reject must be empty");
  assert(report.checks.reject.consoleErrors.length === 0, "console errors after reject must be empty");

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

function prepareProject() {
  rmSync(projectRoot, { recursive: true, force: true });
  execFileSync("cp", ["-R", path.join(root, "fixtures/project/html-context"), projectRoot]);
}

function openDebugShell(project) {
  execFileSync(path.join(root, "scripts/open-debug-shell.sh"), [
    "--id", "v37-ai-diff",
    "--project", project,
    "--replace"
  ], { cwd: root, env: process.env, stdio: ["ignore", "pipe", "pipe"] });
  const manifestPath = path.join(root, "tmp/capy-debug-shells/v37-ai-diff/instance.json");
  const instance = JSON.parse(readFileSync(manifestPath, "utf8"));
  process.env.CAPYBARA_SOCKET = instance.socket;
  return instance;
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
    return {
      visible: !document.querySelector('#project-workbench')?.hidden,
      cards: document.querySelectorAll('[data-project-card-kind]').length,
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
      canvasRect: rect('[data-section="canvas-host"]'),
      workbenchRect: rect('#project-workbench'),
      packageRect: rect('#project-package-panel'),
      plannerRect: rect('[data-section="planner-chat"]'),
      reviewRect: rect('[data-component="ai-diff-review"]'),
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function proposalProbe(sdkResponsePath) {
  const sdkResponseJson = JSON.stringify(sdkResponsePath);
  const reviewState = reviewStateExpression();
  return `new Promise(async (resolve) => {
    try {
      const provider = document.querySelector('#provider');
      const prompt = document.querySelector('#prompt');
      if (provider) {
        provider.value = 'codex';
        provider.dispatchEvent(new Event('change', { bubbles: true }));
      }
      if (prompt) {
        prompt.value = '把首屏改成发布会发布页，先进入审阅。';
        prompt.dispatchEvent(new Event('input', { bubbles: true }));
      }
      const card = Array.from(document.querySelectorAll('[data-project-card-kind="web"]'))
        .find((node) => node.dataset.projectCardId?.startsWith('art_'))
        || Array.from(document.querySelectorAll('[data-project-card-id]'))
          .find((node) => node.dataset.projectCardId?.startsWith('art_'));
      card?.click();
      await window.capyWorkbench.generateSelectedProjectArtifact({
        provider: 'codex',
        live: true,
        review: true,
        prompt: prompt?.value || '把首屏改成发布会发布页，先进入审阅。',
        sdkResponse: ${sdkResponseJson}
      });
      setTimeout(() => resolve(${reviewState}), 350);
    } catch (error) {
      resolve({ error: String(error), pageErrors: window.__capyPageErrors || [] });
    }
  })`;
}

function rejectProbe(sdkResponsePath) {
  const sdkResponseJson = JSON.stringify(sdkResponsePath);
  const reviewState = reviewStateExpression();
  return `new Promise(async (resolve) => {
    try {
      await window.capyWorkbench.generateSelectedProjectArtifact({
        provider: 'codex',
        live: true,
        review: true,
        prompt: '再次生成一个待拒绝的发布页修改。',
        sdkResponse: ${sdkResponseJson}
      });
      await window.capyWorkbench.rejectSelectedProjectReview();
      setTimeout(() => resolve(${reviewState}), 350);
    } catch (error) {
      resolve({ error: String(error), pageErrors: window.__capyPageErrors || [] });
    }
  })`;
}

function reviewStateProbe() {
  return reviewStateExpression();
}

function reviewStateExpression() {
  return `(() => {
    const state = window.capyWorkbench?.stateSnapshot?.();
    const review = state?.projectPackage?.review;
    const run = review?.run;
    const frameSource = document.querySelector('#project-preview-frame')?.srcdoc || '';
    const panel = document.querySelector('[data-component="ai-diff-review"]');
    const box = panel?.getBoundingClientRect();
    return {
      status: state?.projectPackage?.status || null,
      run,
      reviewStatus: run?.review?.status || null,
      selectedArtifactId: state?.projectPackage?.selectedArtifactId || null,
      reviewPanelVisible: Boolean(panel && !panel.hidden && box && box.width > 100 && box.height > 80),
      previewIncludesLaunch: frameSource.includes('Project Context Launch') || (state?.projectPackage?.previewSource || '').includes('Project Context Launch'),
      pageErrors: window.__capyPageErrors || [],
      consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error')
    };
  })()`;
}

function summarize(value) {
  if (value && typeof value === "object") {
    const keys = Object.keys(value).slice(0, 8);
    return Object.fromEntries(keys.map((key) => [key, value[key]]));
  }
  return value;
}

function hashText(text) {
  let hash = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  for (const byte of Buffer.from(text)) {
    hash ^= BigInt(byte);
    hash = (hash * prime) & 0xffffffffffffffffn;
  }
  return `fnv1a64-${hash.toString(16).padStart(16, "0")}`;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function writeReport() {
  writeFileSync(resultPath, `${JSON.stringify(report, null, 2)}\n`);
}
