import assert from "node:assert/strict";
import test from "node:test";

import {
  SegmentNormalizer,
  primaryContentFromSegments,
  segmentsFromClaudeMessages,
  segmentsFromCodexItems,
} from "../src/segments.mjs";

test("Codex items normalize common response shapes", () => {
  const segments = segmentsFromCodexItems([
    { id: "m1", type: "agent_message", text: "先说明。\n\n```html\n<section><h2>报告</h2><p>通过</p></section>\n```" },
    { id: "c1", type: "command_execution", command: "pwd", aggregated_output: "/tmp/repo\n", exit_code: 0, status: "completed" },
    { id: "f1", type: "file_change", changes: [{ path: "probe.md", kind: "add" }], status: "completed" },
    { id: "t1", type: "todo_list", items: [{ text: "验证", completed: true }] },
    { id: "w1", type: "web_search", query: "Codex SDK", action: { type: "search" } },
  ], { input_tokens: 10, output_tokens: 2 });

  assert.deepEqual(segments.map((segment) => segment.kind), [
    "text",
    "frontend_artifact",
    "command",
    "file_change",
    "todo",
    "web_search",
    "usage",
  ]);
  assert.equal(segments.find((segment) => segment.kind === "frontend_artifact").title, "报告");
  assert.equal(segments.find((segment) => segment.kind === "command").metadata.exit_code, 0);
  assert.match(primaryContentFromSegments(segments), /```html/);
});

test("Codex stream events emit upsert segments", () => {
  const emitted = [];
  const normalizer = new SegmentNormalizer("codex", (event) => emitted.push(event));
  normalizer.acceptCodexEvent({ type: "turn.started" });
  normalizer.acceptCodexEvent({
    type: "item.completed",
    item: { id: "cmd", type: "command_execution", command: "missing", aggregated_output: "not found", exit_code: 1, status: "failed" },
  });

  assert.equal(emitted.some((event) => event.type === "segment"), true);
  const command = normalizer.finalSegments().find((segment) => segment.kind === "command");
  assert.equal(command.status, "failed");
  assert.equal(command.text, "not found");
});

test("Claude messages merge tool_use and tool_result by id", () => {
  const segments = segmentsFromClaudeMessages([
    {
      type: "assistant",
      uuid: "a1",
      message: {
        id: "msg1",
        content: [
          { type: "tool_use", id: "tool1", name: "Bash", input: { command: "cat missing.txt" } },
        ],
      },
    },
    {
      type: "user",
      uuid: "u1",
      message: {
        content: [
          { type: "tool_result", tool_use_id: "tool1", content: "Exit code 1\nmissing", is_error: true },
        ],
      },
      tool_use_result: "Error: Exit code 1\nmissing",
    },
    {
      type: "assistant",
      uuid: "a2",
      message: {
        id: "msg2",
        content: [{ type: "text", text: "失败原因：文件不存在。" }],
      },
    },
    { type: "result", subtype: "success", usage: { input_tokens: 3, output_tokens: 4 }, total_cost_usd: 0.01 },
  ]);

  const command = segments.find((segment) => segment.kind === "command");
  assert.equal(command.status, "failed");
  assert.match(command.text, /missing/);
  assert.equal(segments.some((segment) => segment.kind === "text"), true);
  assert.equal(segments.at(-1).kind, "usage");
});

test("Claude partial stream text uses stable ids and final assistant overwrites it", () => {
  const normalizer = new SegmentNormalizer("claude");
  normalizer.acceptClaudeMessage({
    type: "stream_event",
    uuid: "s1",
    event: { type: "message_start", message: { id: "msg-stream" } },
  });
  normalizer.acceptClaudeMessage({
    type: "stream_event",
    uuid: "s2",
    event: { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } },
  });
  normalizer.acceptClaudeMessage({
    type: "stream_event",
    uuid: "s3",
    event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "正在" } },
  });
  normalizer.acceptClaudeMessage({
    type: "assistant",
    uuid: "a1",
    message: { id: "msg-stream", content: [{ type: "text", text: "正在生成报告。" }] },
  });

  const text = normalizer.finalSegments().find((segment) => segment.kind === "text");
  assert.equal(text.text, "正在生成报告。");
  assert.equal(text.status, "completed");
});
