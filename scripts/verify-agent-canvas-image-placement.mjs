#!/usr/bin/env node
if (process.argv.includes("--help") || process.argv.includes("-h")) {
  const scriptName = process.argv[1] || "scripts/verify-*.mjs";
  console.log("Usage: node " + scriptName + " [script-specific args]\n\nUse when: AI runs a version-specific browser, DOM, state, or contract verification script listed by BDD, status.json, evidence notes, or capy help harness.\n\nRequired params: script-specific; inspect the owning version status/evidence entry that names this script. Many scripts default to their version directory.\n\nState effects: may start local browser work, interact with Capybara test hooks, and write screenshots, state, or logs under spec/versions/<version>/evidence/assets/ or target/.\n\nPitfalls: do not run by filename guessing; first read target/debug/capy help harness and the owning version status. This generic help describes the family, while the script body owns exact assertions.\n\nNext step: rerun without --help only after BDD/status names this script, then add outputs to evidence/index.html.\n");
  process.exit(0);
}
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const root = process.cwd();
const args = parseArgs(process.argv.slice(2));
const versionDir = path.resolve(args.versionDir || "spec/versions/v0.10-agent-canvas-image-placement");
const assetsDir = path.join(versionDir, "evidence", "assets");
fs.mkdirSync(assetsDir, { recursive: true });

const providerArg = args.provider || "all";
const modeArg = args.mode || "dry-run";
const providers = providerArg === "all" ? ["claude", "codex"] : [providerArg];
if (!["claude", "codex"].includes(providerArg) && providerArg !== "all") {
  throw new Error(`unsupported --provider=${providerArg}`);
}
if (!["dry-run", "live"].includes(modeArg)) {
  throw new Error(`unsupported --mode=${modeArg}`);
}
if (providerArg === "all" && modeArg === "live") {
  throw new Error("--mode live requires --provider=claude or --provider=codex");
}

const summary = {
  version: "v0.10-agent-canvas-image-placement",
  provider: providerArg,
  mode: modeArg,
  started_at: new Date().toISOString(),
  checks: []
};

for (const provider of providers) {
  summary.checks.push(runScenario(provider, modeArg));
}

summary.finished_at = new Date().toISOString();
summary.ok = summary.checks.every((check) => check.ok);
const summaryPath = path.join(assetsDir, `agent-canvas-${providerArg}-${modeArg}.json`);
fs.writeFileSync(summaryPath, JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));

function runScenario(provider, mode) {
  const slug = `agent-${provider}-${mode}`;
  const toolLog = path.join(assetsDir, `${slug}-tool-calls.jsonl`);
  const captureOut = path.join(assetsDir, `${slug}.png`);
  const report = {
    provider,
    mode,
    ok: false,
    tool_log: toolLog,
    capture: captureOut
  };
  fs.writeFileSync(toolLog, "");

  report.anchor_create = capyJson([
    "canvas",
    "create-card",
    "--kind",
    "image",
    "--title",
    `Agent anchor ${provider} ${mode}`,
    "--x",
    provider === "claude" ? "140" : "180",
    "--y",
    mode === "live" ? "360" : provider === "claude" ? "140" : "250"
  ]);
  report.before = capyJson(["canvas", "snapshot"]);
  const anchor = report.before.canvas?.selectedNode;
  assert(anchor, `${provider} ${mode}: anchor must be selected`);
  assert(anchor.geometry, `${provider} ${mode}: anchor geometry missing`);

  const expectedX = Math.round(anchor.geometry.x + anchor.geometry.w + 48);
  const expectedY = Math.round(anchor.geometry.y);
  const prompt = agentPrompt(provider, mode, expectedX, expectedY, slug);

  if (mode === "live") {
    report.doctor = capyJson(["image", "doctor"]);
    report.balance = capyJson(["image", "balance"]);
  }

  const conversation = capyJson([
    "chat",
    "new",
    "--provider",
    provider,
    "--cwd",
    root,
    "--write-code",
    "--capy-canvas-tools",
    "--capy-tool-log",
    toolLog,
    ...(provider === "codex" ? ["--effort", "low"] : [])
  ]);
  const conversationId = conversation.conversation?.id;
  assert(conversationId, `${provider} ${mode}: conversation id missing`);
  report.conversation_id = conversationId;

  const send = capyJson([
    "chat",
    "send",
    "--id",
    conversationId,
    "--write-code",
    "--capy-canvas-tools",
    "--capy-tool-log",
    toolLog,
    ...(provider === "codex" ? ["--effort", "low"] : []),
    prompt
  ]);
  const runId = send.run_id;
  assert(runId, `${provider} ${mode}: run id missing`);
  report.run_id = runId;

  report.conversation = waitForRun(conversationId, runId, mode === "live" ? 900_000 : 420_000);
  report.events = capyJson(["chat", "events", "--id", conversationId, "--run-id", runId]);
  const entries = readToolLog(toolLog);
  report.tool_entries = entries;
  assert(entries.some((entry) => entry.command === "snapshot" && entry.ok), `${provider} ${mode}: missing successful canvas snapshot tool call`);
  assert(entries.some((entry) => entry.command === "generate-image" && entry.ok), `${provider} ${mode}: missing successful canvas generate-image tool call`);
  const generate = entries.findLast?.((entry) => entry.command === "generate-image" && entry.ok)
    || entries.filter((entry) => entry.command === "generate-image" && entry.ok).at(-1);
  assert(generate, `${provider} ${mode}: missing generate-image entry`);
  if (mode === "dry-run") {
    assert(generate.argv.includes("--dry-run") || !generate.argv.includes("--live"), `${provider}: dry-run call must not use --live`);
  } else {
    assert(generate.argv.includes("--live"), `${provider}: live call must include --live`);
  }

  report.after = capyJson(["canvas", "snapshot"]);
  const selected = report.after.canvas?.selectedNode;
  assert(selected?.content_kind === "image", `${provider} ${mode}: selected node must be image`);
  assert(selected.source_path, `${provider} ${mode}: selected image source_path missing`);
  assert(
    Math.abs(selected.geometry.x - expectedX) <= 8 && Math.abs(selected.geometry.y - expectedY) <= 8,
    `${provider} ${mode}: image placement ${selected.geometry.x},${selected.geometry.y} did not match ${expectedX},${expectedY}`
  );
  if (mode === "live") {
    assert(fs.existsSync(selected.source_path), `${provider} live: live source path must exist`);
  }

  report.runtime = capyJson([
    "devtools",
    "--eval",
    "({pageErrors: window.__capyPageErrors || [], consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error'), canvasTool: document.querySelector('[data-section=\"canvas-image-tool\"]')?.getBoundingClientRect().toJSON?.() || null})"
  ]);
  assert((report.runtime.pageErrors || []).length === 0, `${provider} ${mode}: page errors must be empty`);
  assert((report.runtime.consoleErrors || []).length === 0, `${provider} ${mode}: console errors must be empty`);
  report.capture_result = capyJson(["capture", "--out", captureOut]);
  assert(report.capture_result.bytes > 100_000, `${provider} ${mode}: app-view capture too small`);
  report.ok = true;
  fs.writeFileSync(path.join(assetsDir, `${slug}.json`), JSON.stringify(report, null, 2));
  return report;
}

function agentPrompt(provider, mode, x, y, slug) {
  const liveFlag = mode === "live" ? "--live" : "--dry-run";
  const modeText = mode === "live" ? "real live provider call" : "dry-run fixture call";
  return `You must complete this task by executing shell commands, not by describing a plan.

Task: use Capybara internal CLI to create one generated image and place it beside the currently selected canvas node.

Rules:
- First run: target/debug/capy canvas snapshot
- Then run exactly one canvas generation command using ${liveFlag}.
- Use x=${x} and y=${y}.
- Use --out ${path.join(assetsDir, "agent-generated")}
- Use --name ${slug}
- Use --title "Generated image"
- Use this complete prompt as the final argument:
Scene: Warm product design board on a clean studio surface. Subject: One polished hero image for a Capybara AI design workspace. Important details: refined warm prism colors, premium composition, tactile creative tool mood, soft natural light. Use case: Canvas image node for ${provider} ${modeText} verification. Constraints: No text, no watermark, no UI chrome, no logos.

After the command succeeds, reply with a concise JSON object containing provider, mode, node_id, source_path, x, y.`;
}

function waitForRun(conversationId, runId, timeoutMs) {
  const started = Date.now();
  let detail = null;
  while (Date.now() - started < timeoutMs) {
    detail = capyJson(["chat", "open", "--id", conversationId]);
    const status = detail.conversation?.status;
    const events = capyJson(["chat", "events", "--id", conversationId, "--run-id", runId]);
    if (status === "error") {
      throw new Error(`agent run failed: ${JSON.stringify(events, null, 2)}`);
    }
    if (status === "idle" && hasCompletedEvent(events)) {
      return { status, detail, events };
    }
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1500);
  }
  throw new Error(`timed out waiting for run ${runId}`);
}

function hasCompletedEvent(events) {
  return (events.events || events || []).some?.((event) => {
    const kind = event.kind || event.event_json?.kind;
    const status = event.status || event.event_json?.status;
    return kind === "assistant_done" || status === "completed";
  });
}

function readToolLog(file) {
  return fs.readFileSync(file, "utf8")
    .split(/\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

function capyJson(args) {
  const output = run(["target/debug/capy", ...args]);
  return JSON.parse(output.stdout);
}

function run(argv) {
  const result = spawnSync(argv[0], argv.slice(1), {
    cwd: root,
    env: process.env,
    encoding: "utf8",
    timeout: 60_000,
    maxBuffer: 20 * 1024 * 1024
  });
  if (result.status !== 0) {
    throw new Error(`${argv.join(" ")} failed with ${result.status}\nSTDOUT:\n${result.stdout}\nSTDERR:\n${result.stderr}`);
  }
  return result;
}

function parseArgs(argv) {
  const parsed = {};
  for (let i = 0; i < argv.length; i += 1) {
    const value = argv[i];
    if (value === "--provider") {
      parsed.provider = argv[++i];
    } else if (value === "--mode") {
      parsed.mode = argv[++i];
    } else if (!parsed.versionDir) {
      parsed.versionDir = value;
    } else {
      throw new Error(`unknown argument: ${value}`);
    }
  }
  return parsed;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
