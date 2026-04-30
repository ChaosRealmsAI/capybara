#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/check-sdk-only-agent-runtime.sh

Use when: AI changes agent runtime code and must ensure shell chat execution
stays behind tools/capy-agent-sdk instead of direct Claude/Codex CLI backends.

Required params: none.
State effects: read-only.
Pitfalls: this is an architecture guard, not an SDK availability doctor.
Next step: run target/debug/capy agent sdk doctor for runtime availability.
USAGE
  exit 0
fi

fail_guardrail() {
  local message="$1"
  local next_step="$2"
  echo "architecture check failed: $message" >&2
  echo "next step · $next_step" >&2
  exit 2
}

rg -q 'mod sdk;' crates/capy-shell/src/agent.rs ||
  fail_guardrail \
    "shell agent runtime must keep the SDK module wired" \
    "restore crates/capy-shell/src/agent/sdk.rs as the only provider process boundary"

rg -q 'let result = run_sdk' crates/capy-shell/src/agent.rs ||
  fail_guardrail \
    "shell agent turns must dispatch through SDK only" \
    "make spawn_turn call run_sdk without a direct provider CLI fallback branch"

matches="$(
  rg -n 'mod (claude|codex|jsonrpc)|run_claude|run_codex|codex app-server|thread/start|thread/resume|turn/start|tool_launch\("claude"\)' \
    crates/capy-shell/src/agent.rs crates/capy-shell/src/agent \
    | rg -v 'agent/sdk.rs|agent/sdk_tests.rs|agent/tests.rs' || true
)"
if [[ -n "$matches" ]]; then
  echo "$matches" >&2
  fail_guardrail \
    "Shell chat runtime must be SDK-only; direct Claude/Codex CLI backend code is forbidden" \
    "route provider execution through tools/capy-agent-sdk via crates/capy-shell/src/agent/sdk.rs"
fi
