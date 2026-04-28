#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const args = parseArgs(process.argv.slice(2));
const versionDir = path.resolve(args.versionDir || "spec/versions/v0.14-canvas-context-interface");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const capy = path.join(root, "target", "debug", "capy");
const mode = args.mode || "all";
const providerArg = args.provider || "all";
const providers = providerArg === "all" ? ["claude", "codex"] : providerArg === "none" ? [] : providerArg.split(",");
const resultPath = path.join(assetsDir, "canvas-context-interface-verifier.json");
const chatEventsPath = path.join(assetsDir, "chat-context-events.json");
const agentRunsPath = path.join(assetsDir, "agent-context-runs.json");
const capturePath = path.join(assetsDir, "canvas-context-workbench.png");
const plannerShotPath = path.join(assetsDir, "canvas-context-planner.png");

if (!["all", "selected-image", "region", "stale-context"].includes(mode)) {
  throw new Error(`unsupported --mode=${mode}`);
}
for (const provider of providers) {
  if (!["claude", "codex"].includes(provider)) throw new Error(`unsupported --provider=${provider}`);
}

mkdirSync(assetsDir, { recursive: true });

const report = {
  ok: false,
  version: "v0.14-canvas-context-interface",
  mode,
  providers,
  socket: process.env.CAPYBARA_SOCKET || null,
  started_at: new Date().toISOString(),
  commands: [],
  checks: {},
  agent_runs: []
};

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  report.checks.seeded = waitFor("canvas seed", () =>
    capyJson(["devtools", "--eval", ensureSelectionProbe()])
  , (data) => data?.canvas?.ready === true && data?.canvas?.nodeCount >= 4 && data?.selectedId);

  const anchorPrompt = [
    "Scene: Warm product design board with soft natural light.",
    "Subject: A selected context image for Capybara canvas understanding.",
    "Important details: refined color blocks, premium composition, visible shape contrast.",
    "Use case: Canvas context packet verification.",
    "Constraints: No text, no watermark, no UI chrome."
  ].join(" ");
  report.checks.anchor = capyJson([
    "canvas",
    "generate-image",
    "--dry-run",
    "--out",
    path.join(assetsDir, "context-anchor"),
    "--name",
    "context-anchor",
    "--title",
    "Context anchor image",
    anchorPrompt
  ]);
  const selectedNode = report.checks.anchor.inserted?.inserted_node;
  assert(selectedNode?.content_kind === "image", "anchor selected node must be an image");
  assert(selectedNode?.source_path && existsSync(selectedNode.source_path), "anchor image source_path must exist");

  if (mode === "all" || mode === "selected-image") {
    report.checks.selected_packet = exportPacket("selected-context-packet", ["--selected"]);
    assertPacket(report.checks.selected_packet, "selected_image", ["viewport.png", "selected-node.png"]);
  }

  const region = capyJson(["devtools", "--eval", regionProbe()]);
  report.checks.region_ui = region;
  assert(region.ok === true, "region probe must create active context");
  assert(region.context?.kind === "image_region", "region probe must activate image_region context");

  if (mode === "all" || mode === "region") {
    report.checks.region_packet = exportPacket("region-context-packet", []);
    assertPacket(report.checks.region_packet, "image_region", ["viewport.png", "selected-node.png", "region.png"]);
  }

  report.checks.planner_state = capyJson(["state", "--key", "planner.canvasContext"]);
  assert(report.checks.planner_state.value?.context_id, "planner.canvasContext must expose context_id");
  if (mode === "all" || mode === "stale-context") {
    report.checks.stale_guard = capyJson(["devtools", "--eval", staleContextProbe()]);
    assert(report.checks.stale_guard.ok === true, "stale context guard must clear old region context");
  }
  report.checks.planner_shot = capyJson(["screenshot", "--region", "planner", "--out", plannerShotPath]);
  report.checks.capture = capyJson(["capture", "--out", capturePath]);
  assert(report.checks.capture.bytes > 100000, "native capture must be non-empty");

  const runtime = capyJson([
    "devtools",
    "--eval",
    "({pageErrors: window.__capyPageErrors || [], consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === 'error'), context: window.capyWorkbench.activeCanvasContext()})"
  ]);
  report.checks.runtime = runtime;
  assert(runtime.pageErrors.length === 0, "page errors must be empty");
  assert(runtime.consoleErrors.length === 0, "console errors must be empty");

  const contextForChat = (mode === "selected-image" ? report.checks.selected_packet : report.checks.region_packet || report.checks.selected_packet)
    ?.context_json;
  if (providers.length > 0) {
    for (const provider of providers) {
      report.agent_runs.push(runAgent(provider, contextForChat));
    }
  }
  if (providers.length > 0 || !existsSync(agentRunsPath)) {
    writeFileSync(chatEventsPath, JSON.stringify(report.agent_runs.map((run) => run.chat), null, 2));
    writeFileSync(agentRunsPath, JSON.stringify(report.agent_runs, null, 2));
  }

  report.ok = providers.length === 0 || report.agent_runs.every((run) => run.ok);
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

function exportPacket(name, extraArgs) {
  const out = path.join(assetsDir, name);
  const result = capyJson(["canvas", "context", "export", ...extraArgs, "--out", out]);
  assert(result.ok === true, `${name}: export must report ok`);
  assert(existsSync(result.context_json), `${name}: context.json must exist`);
  return {
    ...result,
    context: JSON.parse(readFileSync(result.context_json, "utf8"))
  };
}

function assertPacket(packet, expectedKind, requiredNames) {
  assert(packet.context_kind === expectedKind, `expected ${expectedKind}, got ${packet.context_kind}`);
  assert(packet.context.context_id, "context packet must include context_id");
  assert(packet.context.source?.node?.id, "context packet must include source node id");
  assert(packet.context.geometry?.node_world, "context packet must include node geometry");
  const paths = packet.attachment_paths || [];
  for (const name of requiredNames) {
    const found = paths.find((value) => value.endsWith(name));
    assert(found, `missing attachment ${name}`);
    assert(existsSync(found), `attachment does not exist: ${found}`);
    assert(statSync(found).size > 1000, `attachment too small: ${found}`);
  }
}

function runAgent(provider, contextJsonPath) {
  const slug = `context-${provider}`;
  const toolLog = path.join(assetsDir, `${slug}-tool-calls.jsonl`);
  writeFileSync(toolLog, "");
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
  const id = conversation.conversation?.id;
  assert(id, `${provider}: conversation id missing`);
  const prompt = `Use the attached Canvas Context Packet. You must run target/debug/capy canvas context export --selected --out ${path.join(assetsDir, `${slug}-agent-export`)} and read its context.json. Reply as concise JSON with provider, context_id, source_node_id, and one sentence about what the selected image/region contains.`;
  const send = capyJson([
    "chat",
    "send",
    "--id",
    id,
    "--canvas-context",
    contextJsonPath,
    "--write-code",
    "--capy-canvas-tools",
    "--capy-tool-log",
    toolLog,
    ...(provider === "codex" ? ["--effort", "low"] : []),
    prompt
  ], 90_000);
  const runId = send.run_id;
  const chat = waitForRun(id, runId, provider === "codex" ? 480_000 : 420_000);
  const entries = readToolLog(toolLog);
  const opened = capyJson(["chat", "open", "--id", id]);
  const userMessage = (opened.messages || []).findLast?.((message) => message.role === "user")
    || [...(opened.messages || [])].reverse().find((message) => message.role === "user");
  assert(userMessage?.event_json?.canvas_context?.context_id, `${provider}: user message must persist canvas_context`);
  assert(entries.some((entry) => entry.command === "context-export" && entry.ok), `${provider}: missing context-export tool call`);
  return {
    ok: true,
    provider,
    conversation_id: id,
    run_id: runId,
    tool_log: toolLog,
    tool_entries: entries,
    chat
  };
}

function waitForRun(conversationId, runId, timeoutMs) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const detail = capyJson(["chat", "open", "--id", conversationId], 60_000);
    const events = capyJson(["chat", "events", "--id", conversationId, "--run-id", runId], 60_000);
    const status = detail.conversation?.status;
    if (status === "error") {
      throw new Error(`agent run failed: ${JSON.stringify(events, null, 2)}`);
    }
    if (status === "idle" && hasCompletedEvent(events)) {
      return { status, detail, events };
    }
    sleep(1500);
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
  return readFileSync(file, "utf8")
    .split(/\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

function regionProbe() {
  return `(() => {
    const state = window.capyWorkbench.refreshPlannerContext();
    const node = state.canvas?.selectedNode;
    const b = node?.bounds || node?.geometry;
    if (!b || node.content_kind !== 'image') return { ok: false, reason: 'selected image missing', node };
    return window.capyWorkbench.setCanvasContextRegion({
      x: b.x + b.w * 0.2,
      y: b.y + b.h * 0.18,
      w: b.w * 0.42,
      h: b.h * 0.36
    });
  })()`;
}

function staleContextProbe() {
  return `(() => {
    const workbench = window.capyWorkbench;
    const before = workbench.activeCanvasContext();
    workbench.clearCanvasContextRegion();
    let state = workbench.refreshPlannerContext();
    const next = (state.blocks || []).find((node) => String(node.id) !== String(before?.source_node_id)) || null;
    if (next) {
      workbench.selectNode(next.id);
      state = workbench.refreshPlannerContext();
    }
    const after = workbench.activeCanvasContext();
    return {
      ok: Boolean(before?.context_id && after?.context_id && before.context_id !== after.context_id && !after.region_bounds_world),
      before,
      after
    };
  })()`;
}

function ensureSelectionProbe() {
  return `(() => {
    const workbench = window.capyWorkbench;
    if (!workbench?.refreshPlannerContext) return null;
    let state = workbench.refreshPlannerContext();
    if (!state?.selectedId && Array.isArray(state?.blocks)) {
      const node = state.blocks.find((item) => item.content_kind === 'image') || state.blocks[0];
      if (node) {
        workbench.selectNode(node.id);
        state = workbench.refreshPlannerContext();
      }
    }
    return state;
  })()`;
}

function capyJson(args, timeout = 60_000) {
  const started = Date.now();
  const stdout = execFileSync(capy, args, {
    cwd: root,
    env: process.env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    timeout,
    maxBuffer: 30 * 1024 * 1024
  });
  const parsed = JSON.parse(stdout);
  report.commands.push({
    cmd: ["target/debug/capy", ...args.map(maskLongArg)].join(" "),
    elapsed_ms: Date.now() - started,
    output_summary: summarize(parsed)
  });
  return parsed;
}

function waitFor(label, producer, predicate) {
  let lastError = null;
  for (let attempt = 1; attempt <= 50; attempt += 1) {
    try {
      const value = producer();
      if (predicate(value)) return { label, attempt, value: summarize(value) };
      lastError = new Error(`${label} not ready on attempt ${attempt}`);
    } catch (error) {
      lastError = error;
    }
    sleep(250);
  }
  throw lastError || new Error(`${label} did not become ready`);
}

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

function summarize(value) {
  if (!value || typeof value !== "object") return value;
  if (value.context?.context_id) {
    return {
      ok: value.ok,
      context_id: value.context.context_id,
      context_kind: value.context_kind,
      context_json: value.context_json,
      attachment_paths: value.attachment_paths
    };
  }
  const keys = Object.keys(value).slice(0, 8);
  return Object.fromEntries(keys.map((key) => [key, key === "context" ? "<context>" : value[key]]));
}

function maskLongArg(value) {
  return typeof value === "string" && value.length > 150 ? `${value.slice(0, 130)}...` : value;
}

function writeReport() {
  writeFileSync(resultPath, JSON.stringify(report, null, 2));
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--mode") parsed.mode = argv[++index];
    else if (arg.startsWith("--mode=")) parsed.mode = arg.slice("--mode=".length);
    else if (arg === "--provider") parsed.provider = argv[++index];
    else if (arg.startsWith("--provider=")) parsed.provider = arg.slice("--provider=".length);
    else if (!parsed.versionDir) parsed.versionDir = arg;
    else throw new Error(`unknown argument: ${arg}`);
  }
  return parsed;
}
