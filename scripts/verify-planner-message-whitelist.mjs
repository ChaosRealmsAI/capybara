#!/usr/bin/env node
import assert from "node:assert/strict";
import {
  hasPlannerInternalLeak,
  projectGenerateMessageContent,
  sanitizePlannerMessageText
} from "../frontend/capy-app/app/planner-message-whitelist.js";

const fixtureResult = {
  run_path: ".capy/runs/gen_1cd89fe7a06d483ea38011562e0d2bad.json",
  run: {
    status: "completed",
    provider: "codex",
    changed_artifact_refs: ["art_00000000000000000000000000000001"],
    output: {
      summary_zh: "首屏标题和说明已经改成更像发布会发布页的表达。"
    }
  }
};

const generated = projectGenerateMessageContent(fixtureResult, {
  id: "art_00000000000000000000000000000001",
  kind: "html",
  title: "Landing HTML"
});

assert.equal(hasPlannerInternalLeak(generated), false);
assert.match(generated, /首屏标题和说明/);
assert.match(generated, /对象：Landing HTML/);
assertNoInternalCopy(generated);

const legacyProposed = `### 首屏标题和说明已经改成更像发布会发布页的表达。

- Provider: codex
- Artifact: Landing HTML
- Changed: art_00000000000000000000000000000001
- Status: proposed
- Run: .capy/runs/gen_1cd89fe7a06d483ea38011562e0d2bad.json`;

const proposed = sanitizePlannerMessageText(legacyProposed);
assert.equal(hasPlannerInternalLeak(proposed), false);
assert.match(proposed, /等待你审核|等待你确认|修改已应用/);
assert.match(proposed, /对象：Landing HTML/);
assertNoInternalCopy(proposed);

const legacyReverted = `### AI Diff 撤销

- Artifact:
  art_00000000000000000000000000000001
- Status: reverted
- Run:
  gen_722e473db9ff48b7afc873dfa1b81a65`;

const reverted = sanitizePlannerMessageText(legacyReverted);
assert.equal(reverted, "### AI Diff 已撤销\n\n这次 AI 变更已撤销。");
assert.equal(hasPlannerInternalLeak(reverted), false);
assertNoInternalCopy(reverted);

const legacyRejected = `### AI Diff 拒绝

- Artifact:
  art_00000000000000000000000000000001
- Status: rejected
- Run:
  gen_1cd89fe7a06d483ea38011562e0d2bad`;

const rejected = sanitizePlannerMessageText(legacyRejected);
assert.equal(rejected, "### AI Diff 已拒绝\n\n这次 AI 变更已拒绝，未应用到项目。");
assert.equal(hasPlannerInternalLeak(rejected), false);
assertNoInternalCopy(rejected);

const ordinary = "### 普通回复\n\nProvider 可以作为产品概念被解释，但没有 run 或 artifact 泄漏。";
assert.equal(sanitizePlannerMessageText(ordinary), ordinary);

console.log("planner message whitelist check passed");

function assertNoInternalCopy(text) {
  assert.doesNotMatch(text, /Provider:|Artifact:|Changed:|Status:|Run:/i);
  assert.doesNotMatch(text, /\b(?:art|surf_art|proj)_[a-z0-9_]{16,}\b/i);
  assert.doesNotMatch(text, /\b(?:gen|run)_[a-f0-9]{16,}\b/i);
  assert.doesNotMatch(text, /\.capy\/runs\//i);
  assert.doesNotMatch(text, /\b(?:codex|claude)\b/i);
}
