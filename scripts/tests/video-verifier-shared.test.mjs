import test from "node:test";
import assert from "node:assert/strict";

import {
  assertQueueIdsChangedTo,
  assertQueueIdsEqual,
  captureAttempt,
  queueIds,
  summarizeQueueMutation,
  summarizeVideoCapture
} from "../video-verifier-shared.mjs";

test("queue helpers prove unchanged and changed proposal decision states", () => {
  const before = { items: [{ id: "queue-a" }, { id: "queue-b" }] };
  const same = { items: [{ id: "queue-a" }, { id: "queue-b" }] };
  const accepted = { items: [{ id: "queue-b" }, { id: "queue-a" }] };

  assert.deepEqual(queueIds(before), ["queue-a", "queue-b"]);
  assertQueueIdsEqual(before, same);
  assertQueueIdsChangedTo(accepted, ["queue-b", "queue-a"]);
  assert.deepEqual(summarizeQueueMutation(before, same), {
    before_ids: ["queue-a", "queue-b"],
    after_ids: ["queue-a", "queue-b"],
    changed: false
  });
  assert.equal(summarizeQueueMutation(before, accepted).changed, true);
  assert.throws(() => assertQueueIdsEqual(before, accepted), /queue ids changed/);
});

test("capture verdict passes when app-view capture succeeds", () => {
  const verdict = summarizeVideoCapture({
    version: "v-test",
    stage: "proposal-generated",
    attempts: [
      captureAttempt({
        method: "capture",
        ok: true,
        evidence: "capture.json",
        image: "proposal-app-view.png",
        elapsed_ms: 900
      })
    ],
    final_image: "proposal-desktop.png",
    state_evidence: "proposal-state.json",
    real_dom_state: true
  });

  assert.equal(verdict.verdict.status, "passed");
  assert.equal(verdict.capture.status, "captured");
  assert.equal(verdict.capture.final_image_source, "app-view-capture");
  assert.equal(verdict.capture.blocking, false);
});

test("capture timeout is nonblocking only with real CEF state fallback and no UI errors", () => {
  const verdict = summarizeVideoCapture({
    version: "v-test",
    stage: "proposal-generated",
    attempts: [
      {
        method: "capture",
        ok: false,
        evidence: "capture.json",
        error: "event ack timed out after 12s"
      }
    ],
    fallback_image: "proposal-state-derived.png",
    final_image: "proposal-desktop.png",
    state_evidence: "proposal-state.json",
    real_dom_state: true,
    ui_errors: []
  });

  assert.equal(verdict.verdict.status, "passed");
  assert.equal(verdict.capture.status, "timeout_nonblocking_with_real_cef_state_fallback");
  assert.equal(verdict.capture.final_image_source, "state-derived-fallback");
  assert.deepEqual(verdict.verdict.warnings, ["timeout_nonblocking_with_real_cef_state_fallback"]);
});

test("capture timeout blocks when there is no trusted visible state fallback", () => {
  const verdict = summarizeVideoCapture({
    version: "v-test",
    stage: "proposal-generated",
    attempts: [
      {
        method: "capture",
        ok: false,
        evidence: "capture.json",
        error: "event ack timed out after 12s"
      }
    ],
    state_evidence: null,
    real_dom_state: false
  });

  assert.equal(verdict.verdict.status, "failed");
  assert.equal(verdict.capture.status, "timeout_blocking_no_visible_state");
  assert.equal(verdict.capture.blocking, true);
});

test("capture fallback blocks when CEF state reports UI errors", () => {
  const verdict = summarizeVideoCapture({
    version: "v-test",
    stage: "proposal-generated",
    attempts: [
      {
        method: "screenshot",
        ok: false,
        evidence: "screenshot.json",
        error: "capture failed"
      }
    ],
    fallback_image: "proposal-state-derived.png",
    final_image: "proposal-desktop.png",
    state_evidence: "proposal-state.json",
    real_dom_state: true,
    ui_errors: ["Uncaught TypeError"]
  });

  assert.equal(verdict.verdict.status, "failed");
  assert.equal(verdict.capture.status, "failed_blocking_ui_errors");
  assert.equal(verdict.capture.blocking, true);
});
