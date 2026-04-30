#!/usr/bin/env node
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
