#!/usr/bin/env node
if (process.argv.includes("--help") || process.argv.includes("-h")) {
  const scriptName = process.argv[1] || "scripts/verify-*.mjs";
  console.log("Usage: node " + scriptName + " [script-specific args]\n\nUse when: AI runs a version-specific browser, DOM, state, or contract verification script listed by BDD, status.json, evidence notes, or capy help harness.\n\nRequired params: script-specific; inspect the owning version status/evidence entry that names this script. Many scripts default to their version directory.\n\nState effects: may start local browser work, interact with Capybara test hooks, and write screenshots, state, or logs under spec/versions/<version>/evidence/assets/ or target/.\n\nPitfalls: do not run by filename guessing; first read target/debug/capy help harness and the owning version status. This generic help describes the family, while the script body owns exact assertions.\n\nNext step: rerun without --help only after BDD/status names this script, then add outputs to evidence/index.html.\n");
  process.exit(0);
}
import assert from "node:assert/strict";

import {
  providerDefaultModel,
  safeProviderModelValue,
} from "../frontend/capy-app/app/runtime-controls.js";

assert.equal(providerDefaultModel("codex"), "gpt-5.5");
assert.equal(providerDefaultModel("claude"), "sonnet");

assert.equal(safeProviderModelValue("codex", "sonnet"), "gpt-5.5");
assert.equal(safeProviderModelValue("codex", "opus"), "gpt-5.5");
assert.equal(safeProviderModelValue("codex", "gpt-5.4"), "gpt-5.4");

assert.equal(safeProviderModelValue("claude", "gpt-5.5"), "sonnet");
assert.equal(safeProviderModelValue("claude", "opus"), "opus");

console.log("runtime model provider guard passed");
