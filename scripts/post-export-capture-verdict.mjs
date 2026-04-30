export const POST_EXPORT_CAPTURE_VERDICT_SCHEMA = "capy.post_export_capture.verdict.v1";

export function buildCaptureAttempt({ method, ok, evidence = null, image = null, elapsed_ms = null, error = null }) {
  return {
    method,
    ok: Boolean(ok),
    evidence,
    image,
    elapsed_ms,
    failure_kind: ok ? "none" : classifyCaptureFailure(error),
    error: ok ? null : String(error || "unknown capture failure")
  };
}

export function classifyCaptureFailure(error) {
  const text = String(error || "").toLowerCase();
  if (text.includes("timed out") || text.includes("timeout") || text.includes("etimedout")) {
    return "timeout";
  }
  if (text.includes("no such file") || text.includes("socket") || text.includes("connection refused")) {
    return "connection";
  }
  return "failed";
}

export function summarizePostExportCapture({
  version,
  stage = "post-export",
  generated_at = new Date().toISOString(),
  export_ok,
  export_status = null,
  export_evidence = null,
  export_output_path = null,
  state_evidence = null,
  prior_visible_evidence = null,
  fallback_image = null,
  final_image = null,
  attempts = [],
  ui_errors = [],
  retry_command = null
}) {
  const normalizedAttempts = attempts.map(buildCaptureAttempt);
  const successfulAttempt = normalizedAttempts.find(attempt => attempt.ok);
  const hasTimeout = normalizedAttempts.some(attempt => attempt.failure_kind === "timeout");
  const hasPriorVisibleEvidence = Boolean(prior_visible_evidence || fallback_image);
  const hasBlockingUiErrors = Array.isArray(ui_errors) && ui_errors.length > 0;
  const exportPassed = Boolean(export_ok);

  let captureStatus = "missing";
  let captureBlocking = true;
  let finalImageSource = null;
  let rationale = "";

  if (successfulAttempt) {
    captureStatus = "captured";
    captureBlocking = false;
    finalImageSource = "post-export-capture";
    rationale = "post-export desktop capture succeeded after export state was collected.";
  } else if (!exportPassed) {
    captureStatus = "blocked_by_export_failure";
    captureBlocking = true;
    finalImageSource = fallback_image ? "prior-visible-fallback" : null;
    rationale = "export did not pass, so capture classification cannot make the export acceptable.";
  } else if (hasBlockingUiErrors) {
    captureStatus = hasTimeout ? "timeout_blocking_ui_errors" : "failed_blocking_ui_errors";
    captureBlocking = true;
    finalImageSource = fallback_image ? "prior-visible-fallback" : null;
    rationale = "capture failed and the export state contains blocking UI/page errors.";
  } else if (hasTimeout && hasPriorVisibleEvidence) {
    captureStatus = "timeout_nonblocking_with_prior_visible_capture";
    captureBlocking = false;
    finalImageSource = fallback_image ? "prior-visible-fallback" : null;
    rationale = "export passed and earlier real desktop evidence exists; the timeout is recorded as screenshot evidence degradation, not as export failure.";
  } else if (hasPriorVisibleEvidence) {
    captureStatus = "failed_nonblocking_with_prior_visible_capture";
    captureBlocking = false;
    finalImageSource = fallback_image ? "prior-visible-fallback" : null;
    rationale = "export passed and earlier real desktop evidence exists, but post-export capture did not succeed. This remains a warning.";
  } else {
    captureStatus = hasTimeout ? "timeout_blocking_no_visible_capture" : "failed_blocking_no_visible_capture";
    captureBlocking = true;
    finalImageSource = null;
    rationale = "post-export capture failed and no earlier real desktop evidence is available.";
  }

  const blockers = [];
  const warnings = [];
  if (!exportPassed) blockers.push("export_failed");
  if (captureBlocking) blockers.push(captureStatus);
  if (!captureBlocking && captureStatus !== "captured") warnings.push(captureStatus);

  return {
    schema: POST_EXPORT_CAPTURE_VERDICT_SCHEMA,
    version,
    generated_at,
    stage,
    export: {
      ok: exportPassed,
      status: export_status || (exportPassed ? "passed" : "failed"),
      evidence: export_evidence,
      output_path: export_output_path
    },
    capture: {
      status: captureStatus,
      blocking: captureBlocking,
      attempts: normalizedAttempts,
      final_image,
      final_image_source: finalImageSource,
      fallback_image,
      prior_visible_evidence,
      state_evidence,
      ui_errors,
      retry_command,
      rationale
    },
    verdict: {
      status: exportPassed && !captureBlocking ? "passed" : "failed",
      blockers,
      warnings
    }
  };
}

export function buildTimeoutSample(version = "v0.49-q") {
  return summarizePostExportCapture({
    version,
    generated_at: "2026-05-01T00:00:00.000Z",
    export_ok: true,
    export_status: "done",
    export_evidence: "spec/versions/v0.49-q/evidence/assets/ai-clip-suggestion-export-state.json",
    export_output_path: "spec/versions/v0.49-q/evidence/assets/ai-clip-suggestion-delivery.mp4",
    state_evidence: "spec/versions/v0.49-q/evidence/assets/ai-clip-suggestion-export-state.json",
    prior_visible_evidence: "spec/versions/v0.49-q/evidence/assets/ai-clip-suggestion-restored-desktop.png",
    fallback_image: "spec/versions/v0.49-q/evidence/assets/ai-clip-suggestion-restored-desktop.png",
    final_image: "spec/versions/v0.49-q/evidence/assets/ai-clip-suggestion-export-desktop.png",
    retry_command: "CAPYBARA_SOCKET=<same-socket> target/debug/capy capture --out spec/versions/v0.49-q/evidence/assets/ai-clip-suggestion-export-desktop-retry.png",
    attempts: [
      {
        method: "capture",
        ok: false,
        evidence: "ai-clip-suggestion-export-capture.json",
        image: "spec/versions/v0.49-q/evidence/assets/ai-clip-suggestion-export-desktop.png",
        elapsed_ms: 60000,
        error: "event ack timed out after 60s"
      }
    ],
    ui_errors: []
  });
}
