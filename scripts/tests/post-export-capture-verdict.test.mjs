import test from "node:test";
import assert from "node:assert/strict";

import {
  buildCaptureAttempt,
  buildTimeoutSample,
  classifyCaptureFailure,
  summarizePostExportCapture
} from "../post-export-capture-verdict.mjs";

test("classifies timeout wording from capture failures", () => {
  assert.equal(classifyCaptureFailure("event ack timed out after 60s"), "timeout");
  assert.equal(classifyCaptureFailure("IPC timeout while waiting for capture"), "timeout");
  assert.equal(classifyCaptureFailure("socket connection refused"), "connection");
  assert.equal(classifyCaptureFailure("unexpected empty png"), "failed");
});

test("passes when the post-export capture succeeds", () => {
  const verdict = summarizePostExportCapture({
    version: "v-test",
    export_ok: true,
    export_status: "done",
    attempts: [
      buildCaptureAttempt({
        method: "capture",
        ok: true,
        evidence: "capture.json",
        image: "export-desktop.png",
        elapsed_ms: 1200
      })
    ],
    final_image: "export-desktop.png",
    state_evidence: "export-state.json"
  });

  assert.equal(verdict.verdict.status, "passed");
  assert.equal(verdict.capture.status, "captured");
  assert.equal(verdict.capture.blocking, false);
  assert.deepEqual(verdict.verdict.blockers, []);
  assert.deepEqual(verdict.verdict.warnings, []);
});

test("treats export-passed capture timeout as nonblocking only with prior visible evidence", () => {
  const verdict = buildTimeoutSample("v0.49-q");

  assert.equal(verdict.verdict.status, "passed");
  assert.equal(verdict.capture.status, "timeout_nonblocking_with_prior_visible_capture");
  assert.equal(verdict.capture.blocking, false);
  assert.deepEqual(verdict.verdict.blockers, []);
  assert.deepEqual(verdict.verdict.warnings, ["timeout_nonblocking_with_prior_visible_capture"]);
});

test("blocks when capture times out and no real visible evidence exists", () => {
  const verdict = summarizePostExportCapture({
    version: "v-test",
    export_ok: true,
    export_status: "done",
    attempts: [
      {
        method: "capture",
        ok: false,
        evidence: "capture.json",
        error: "event ack timed out after 60s"
      }
    ],
    state_evidence: "export-state.json"
  });

  assert.equal(verdict.verdict.status, "failed");
  assert.equal(verdict.capture.status, "timeout_blocking_no_visible_capture");
  assert.equal(verdict.capture.blocking, true);
  assert.deepEqual(verdict.verdict.blockers, ["timeout_blocking_no_visible_capture"]);
});

test("does not allow capture success to hide export failure", () => {
  const verdict = summarizePostExportCapture({
    version: "v-test",
    export_ok: false,
    export_status: "failed",
    attempts: [
      {
        method: "capture",
        ok: true,
        evidence: "capture.json",
        image: "export-desktop.png"
      }
    ],
    final_image: "export-desktop.png",
    state_evidence: "export-state.json"
  });

  assert.equal(verdict.verdict.status, "failed");
  assert.deepEqual(verdict.verdict.blockers, ["export_failed"]);
});
