#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { normalize, parseArgs, SDK_ROOT } from "./normalize.mjs";
import { runClaude, runClaudeStream } from "./providers/claude.mjs";
import { runCodex, runCodexStream } from "./providers/codex.mjs";

const [command = "help", ...rest] = process.argv.slice(2);

try {
  const args = parseArgs(rest);
  if (command === "doctor") printJson(doctor());
  else if (command === "normalize") printJson(normalize(args));
  else if (command === "run") await run(args);
  else if (command === "run-stream") await runStream(args);
  else if (command === "help" || command === "--help" || command === "-h") printHelp();
  else throw new Error(`unknown command: ${command}`);
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  if (process.argv.includes("--json")) printJson({ ok: false, error: message });
  else console.error(message);
  process.exitCode = 1;
}

async function run(args) {
  const prompt = args.prompt ?? args._.join(" ");
  if (!prompt || String(prompt).trim().length === 0) {
    throw new Error("run requires --prompt or positional prompt text");
  }
  const normalized = normalize({ ...args, prompt });
  const output = normalized.provider === "codex" ? await runCodex(normalized) : await runClaude(normalized);
  if (!args.raw) {
    delete output.items;
    delete output.messages;
  }
  printJson(output);
}

async function runStream(args) {
  const prompt = args.prompt ?? args._.join(" ");
  if (!prompt || String(prompt).trim().length === 0) {
    throw new Error("run-stream requires --prompt or positional prompt text");
  }
  const normalized = normalize({ ...args, prompt });
  const emit = (event) => printJsonLine(event);
  try {
    const output = normalized.provider === "codex"
      ? await runCodexStream(normalized, emit)
      : await runClaudeStream(normalized, emit);
    if (!args.raw) {
      delete output.items;
      delete output.messages;
    }
    printJsonLine({ ok: true, type: "run_completed", ...output });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    printJsonLine({ ok: false, type: "run_failed", provider: normalized.provider, error: message, normalized });
    process.exitCode = 1;
  }
}

function doctor() {
  const codexSdk = readPackage("@openai/codex-sdk");
  const claudeSdk = readPackage("@anthropic-ai/claude-agent-sdk");
  const codexCli = runVersion("codex", ["--version"]);
  const claudeCli = runVersion("claude", ["--version"]);
  return {
    ok: Boolean(codexSdk.version && claudeSdk.version && codexCli.ok && claudeCli.ok),
    kind: "capy-agent-sdk-doctor",
    node: process.version,
    sdk_root: SDK_ROOT,
    sdk: { openai_codex: codexSdk, anthropic_claude: claudeSdk },
    runtime: { codex: codexCli, claude: claudeCli },
  };
}

function readPackage(name) {
  const packagePath = path.join(SDK_ROOT, "node_modules", ...name.split("/"), "package.json");
  try {
    const pkg = JSON.parse(fs.readFileSync(packagePath, "utf8"));
    return { ok: true, name: pkg.name, version: pkg.version, path: packagePath };
  } catch (error) {
    return { ok: false, name, error: error instanceof Error ? error.message : String(error) };
  }
}

function runVersion(program, args) {
  const output = spawnSync(program, args, { encoding: "utf8", env: process.env });
  return {
    ok: output.status === 0,
    command: [program, ...args].join(" "),
    status: output.status,
    stdout: output.stdout.trim(),
    stderr: output.stderr.trim(),
    error: output.error ? output.error.message : undefined,
  };
}

function printJson(value) {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
}

function printJsonLine(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
}

function printHelp() {
  process.stdout.write(`Capybara Agent SDK Runtime

Usage:
  node tools/capy-agent-sdk/src/cli.mjs help
  node tools/capy-agent-sdk/src/cli.mjs doctor
  node tools/capy-agent-sdk/src/cli.mjs normalize --provider <claude|codex> [options]
  node tools/capy-agent-sdk/src/cli.mjs run --provider <claude|codex> --prompt <text> [options]
  node tools/capy-agent-sdk/src/cli.mjs run-stream --provider <claude|codex> --prompt <text> [options]

Use when:
  Maintaining the internal SDK process boundary used by Capybara shell/chat.
  Normal AI operators should discover this through target/debug/capy agent help sdk.

Required params:
  run and run-stream need --provider <claude|codex> and --prompt <text> or
  positional prompt text. normalize needs --provider.

Pitfalls:
  Direct run/run-stream can call real Claude/Codex SDKs and may write code when
  --write-code is passed. Prefer capy chat/agent commands for product workflows.

Next step:
  Run target/debug/capy agent sdk doctor, then use capy chat or capy agent unless
  you are changing this adapter.

Commands:
  doctor
  normalize --provider <claude|codex> [options]
  run --provider <claude|codex> --prompt <text> [options]
  run-stream --provider <claude|codex> --prompt <text> [options]

Shared options:
  --cwd <path>
  --model <name>
  --effort <minimal|low|medium|high|xhigh|max>
  --write-code
  --add-dir <path>
  --allowed-tools <tool,tool>
  --disallowed-tools <tool,tool>
  --tools <claude_code|tool,tool>
  --mcp-config <json-or-path>
  --output-schema <json-or-path>
  --max-turns <n>
  --raw

Stream output:
  run-stream emits JSONL. Each line is one of:
    {"type":"segment","op":"upsert","segment":{...}}
    {"type":"run_completed","segments":[...],"content":"..."}
    {"type":"run_failed","error":"..."}

Normalized segment kinds:
  text, frontend_artifact, thinking, todo, command, file_change,
  tool_call, web_search, progress, usage, error

Codex options:
  --approval-policy <never|on-request|on-failure|untrusted>
  --sandbox <read-only|workspace-write|danger-full-access>
  --thread-id <id>
  --search
  --skip-git-repo-check
  --codex-config <key=value>
  --codex-path <path>

Claude options:
  --permission-mode <default|acceptEdits|bypassPermissions|plan|dontAsk|auto>
  --max-budget-usd <usd>
  --setting-source <user|project|local>
  --session-id <uuid>
  --resume <uuid>
  --no-session-persistence
  --claude-path <path>

Full-auto mapping:
  --write-code + codex  -> approvalPolicy=never, sandbox=danger-full-access
  --write-code + claude -> permissionMode=bypassPermissions, allowDangerouslySkipPermissions=true, tools=claude_code

Known provider boundary:
  Codex SDK rejects reasoning effort "minimal" when image_gen/web_search tools are present. Use "low" for smoke runs.
`);
}
