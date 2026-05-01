#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

import { initialQueue } from "./verify-ai-clip-suggestion-fixtures.mjs";
import { proposalAcceptCurrentEval, proposalAcceptEval, proposalGenerateEval, proposalHistoryReopenEval, proposalLoadEval, proposalRejectEval, proposalSaveFeedbackEval } from "./verify-video-clip-proposal-evals.mjs";
import { verifyProposalEvidencePage, writeProposalEvidencePage, writeProposalManifest } from "./verify-video-clip-proposal-report.mjs";
import {
  assertQueueIdsChangedTo,
  assertQueueIdsEqual,
  captureAttempt,
  copyFallbackImage,
  summarizeQueueMutation,
  summarizeVideoCapture,
  writeStateDerivedImage
} from "./video-verifier-shared.mjs";

if (process.argv.includes("--help") || process.argv.includes("-h")) {
  console.log(`Usage: scripts/verify-video-clip-proposal.mjs [spec/versions/<version>]

Use when:
  Verify the feedback-aware clip proposal diff loop, proposal revision/hash, and stale accept conflict guard end to end on a real CEF desktop.

Required params:
  Optional first arg is the version directory. Default: spec/versions/v0.52 for backward-compatible smoke runs.
  Requires target/debug/capy, local ffmpeg/ffprobe, macOS launchctl, CEF shell harness, ImageMagick magick, and Playwright for evidence-page browser check.

State effects:
  Writes evidence under <version>/evidence/assets/.
  Creates a disposable project at <version>/evidence/assets/video-clip-proposal-project.
  Imports two local WebM videos, seeds .capy/video-clip-queue.json, writes semantics and feedback through Project Core, launches an isolated debug shell id, generates proposal diff, rejects it, regenerates a stale candidate, changes the queue externally, proves stale accept returns conflicted without writing queue, regenerates a valid proposal, accepts it, closes and reopens the same project to prove persisted proposal history restores, and opens <version>/evidence/index.html.

Evidence outputs:
  video-clip-proposal-diff.json
  video-clip-proposal-queue-before-proposal.json
  video-clip-proposal-queue-after-proposal.json
  video-clip-proposal-queue-after-reject.json
  video-clip-proposal-queue-after-external-change.json
  video-clip-proposal-queue-after-conflict.json
  video-clip-proposal-queue-after-accept.json
  video-clip-proposal-summary.json
  video-clip-proposal-history.json
  video-clip-proposal-history-reopened-state.json
  video-clip-proposal-*-state.json
  video-clip-proposal-*-desktop.png
  video-clip-proposal-*-capture-verdict.json

Pitfalls:
  Proposal generation must not mutate .capy/video-clip-queue.json.
  Only explicit accept may update the queue; reject must preserve the original queue ids.
  Accepting a proposal whose base_queue_hash differs from the current queue returns conflicted and must not write .capy/video-clip-queue.json.
  This verifies deterministic local proposal logic, not paid model interpretation.
  The verifier attempts real CEF app-view capture for each key stage.
  If capture or screenshot times out, evidence records whether fallback is blocking or nonblocking.
  State-derived fallback PNGs are not treated as real screenshot success.

Next step:
  Review <version>/evidence/index.html and <version>/evidence/assets/video-clip-proposal-summary.json.`);
  process.exit(0);
}

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.52");
const versionId = path.basename(versionDir);
const evidenceDir = path.join(versionDir, "evidence"), assetsDir = path.join(evidenceDir, "assets");
const projectDir = path.join(assetsDir, "video-clip-proposal-project"), mediaDir = path.join(projectDir, "media");
const initialQueuePath = path.join(assetsDir, "video-clip-proposal-initial-queue.json"), projectProposalManifest = path.join(projectDir, ".capy", "video-clip-proposal.json");
const uiComposition = "__queue_only__";
const logs = [];
const openInstanceIds = [];
const stageCaptureVerdicts = [];
let currentSocket = "", shellBundleReady = false;

rmSync(assetsDir, { recursive: true, force: true });
mkdirSync(assetsDir, { recursive: true });

try {
  assert(existsSync(path.join(root, "target/debug/capy")), "missing target/debug/capy");
  command("ffmpeg", ["-version"], "video-clip-proposal-ffmpeg-version.log");
  command("ffprobe", ["-version"], "video-clip-proposal-ffprobe-version.log");
  rmSync(projectDir, { recursive: true, force: true });
  mkdirSync(mediaDir, { recursive: true });
  generateVideo(path.join(mediaDir, "camera-a-wide.webm"), "testsrc2=size=640x360:rate=30", 4, "video-clip-proposal-source-a-generate.log");
  generateVideo(path.join(mediaDir, "camera-b-close.webm"), "smptebars=size=480x270:rate=24", 5, "video-clip-proposal-source-b-generate.log");
  capyJson(["project", "init", "--project", projectDir, "--name", `${versionId} Video Clip Proposal Project`], "video-clip-proposal-project-init.json");
  rmSync(path.join(projectDir, ".capy", "evidence"), { recursive: true, force: true });
  const importA = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-a-wide.webm", "--title", "Camera A wide"], "video-clip-proposal-import-a.json");
  const importB = capyJson(["project", "import-video", "--project", projectDir, "--path", "media/camera-b-close.webm", "--title", "Camera B close"], "video-clip-proposal-import-b.json");
  writeJson("video-clip-proposal-initial-queue.json", initialQueue(importA, importB));
  capyJson(["project", "clip-queue", "write", "--project", projectDir, "--manifest", initialQueuePath], "video-clip-proposal-initial-queue-write.json");
  capyJson(["project", "clip-queue", "analyze", "--project", projectDir], "video-clip-proposal-cli-semantics.json");
  openShell("main", "video-clip-proposal-open-main.log");

  const loadedState = capyJson(["devtools", "--eval", proposalLoadEval(projectDir, uiComposition)], "video-clip-proposal-loaded-state.json", capyEnv());
  assertSemantics(loadedState.semantics, 2, "loaded");
  assertNoPageErrors(loadedState, "loaded");

  const savedState = capyJson(["devtools", "--eval", proposalSaveFeedbackEval("这段不适合开场")], "video-clip-proposal-feedback-saved-state.json", capyEnv());
  assertFeedback(savedState.feedback, "saved");
  const feedbackCli = capyJson(["project", "clip-queue", "feedbacks", "--project", projectDir], "video-clip-proposal-cli-feedbacks.json");
  assertFeedback(feedbackCli, "CLI feedbacks");

  const queueBeforeProposal = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-before-proposal.json");
  const proposedState = capyJson(["devtools", "--eval", proposalGenerateEval()], "video-clip-proposal-generated-state.json", capyEnv());
  assertProposal(proposedState.proposal, "generated");
  assertTextIncludes(proposedState.domProposalText, ["修改提案", "Before", "After", "Revision", "base_queue_hash", "可接受"], "proposal DOM");
  const historyAfterGenerated = capyJson(["project", "clip-queue", "proposal-history", "--project", projectDir], "video-clip-proposal-history-after-generated.json");
  assertHistory(historyAfterGenerated, ["proposed"], "history after generated");
  copyFileSync(projectProposalManifest, path.join(assetsDir, "video-clip-proposal-first-diff.json"));
  const queueAfterProposal = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-after-proposal.json");
  assertQueueIdsEqual(queueBeforeProposal, queueAfterProposal, "proposal generation mutated queue");

  const rejectedState = capyJson(["devtools", "--eval", proposalRejectEval()], "video-clip-proposal-rejected-state.json", capyEnv());
  assertDecision(rejectedState.proposal, "rejected");
  const historyAfterReject = capyJson(["project", "clip-queue", "proposal-history", "--project", projectDir], "video-clip-proposal-history-after-reject.json");
  assertHistory(historyAfterReject, ["rejected"], "history after reject");
  const queueAfterReject = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-after-reject.json");
  assertQueueIdsEqual(queueBeforeProposal, queueAfterReject, "reject mutated queue");

  const staleCandidateState = capyJson(["devtools", "--eval", proposalGenerateEval()], "video-clip-proposal-stale-candidate-state.json", capyEnv());
  assertProposal(staleCandidateState.proposal, "stale candidate");
  assert(staleCandidateState.proposal.revision > proposedState.proposal.revision, "stale candidate revision did not advance");
  copyFileSync(projectProposalManifest, path.join(assetsDir, "video-clip-proposal-stale-candidate-diff.json"));
  const externalQueueManifest = {
    ...queueAfterReject,
    items: (queueAfterReject.items || []).slice(0, 1)
  };
  writeJson("video-clip-proposal-external-queue-change.json", externalQueueManifest);
  capyJson(["project", "clip-queue", "write", "--project", projectDir, "--manifest", path.join(assetsDir, "video-clip-proposal-external-queue-change.json")], "video-clip-proposal-external-queue-write.json");
  const queueAfterExternalChange = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-after-external-change.json");
  assertQueueIdsChangedTo(queueAfterExternalChange, ["queue-initial-camera-a"], "external queue change did not apply");

  const conflictedState = capyJson(["devtools", "--eval", proposalAcceptCurrentEval("conflicted")], "video-clip-proposal-conflicted-state.json", capyEnv());
  assertDecision(conflictedState.proposal, "conflicted");
  assertTextIncludes(conflictedState.domProposalText, ["已过期", "冲突", "current_queue_hash"], "conflicted proposal DOM");
  const historyAfterConflict = capyJson(["project", "clip-queue", "proposal-history", "--project", projectDir], "video-clip-proposal-history-after-conflict.json");
  assertHistory(historyAfterConflict, ["rejected", "conflicted"], "history after conflict");
  const queueAfterConflict = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-after-conflict.json");
  assertQueueIdsEqual(queueAfterExternalChange, queueAfterConflict, "conflicted accept mutated queue");

  const acceptedState = capyJson(["devtools", "--eval", proposalAcceptEval()], "video-clip-proposal-accepted-state.json", capyEnv());
  assertDecision(acceptedState.proposal, "accepted");
  assert(acceptedState.proposal.revision > staleCandidateState.proposal.revision, "accepted proposal revision did not advance after conflict");
  const queueAfterAccept = capyJson(["project", "clip-queue", "inspect", "--project", projectDir], "video-clip-proposal-queue-after-accept.json");
  assertQueueIdsEqual({ items: acceptedState.proposal.after_queue || [] }, queueAfterAccept, "accept did not write proposal after_queue");
  assert(summarizeQueueMutation(queueAfterConflict, queueAfterAccept).changed, "valid accept did not change queue after conflict");
  copyFileSync(projectProposalManifest, path.join(assetsDir, "video-clip-proposal-diff.json"));
  capyJson(["project", "clip-queue", "proposal-current", "--project", projectDir], "video-clip-proposal-current-accepted.json");
  const proposalHistory = capyJson(["project", "clip-queue", "proposal-history", "--project", projectDir], "video-clip-proposal-history.json");
  assertHistory(proposalHistory, ["rejected", "conflicted", "accepted"], "final history");

  shutdown();
  openInstanceIds.length = 0;
  openShell("reopen", "video-clip-proposal-open-reopen.log");
  const reopenedState = capyJson(["devtools", "--eval", proposalHistoryReopenEval(projectDir, uiComposition)], "video-clip-proposal-history-reopened-state.json", capyEnv());
  assertReopenedHistory(reopenedState, proposalHistory);

  const summary = writeSummary({ loadedState, savedState, proposedState, rejectedState, staleCandidateState, conflictedState, acceptedState, reopenedState, feedbackCli, queueBeforeProposal, queueAfterProposal, queueAfterReject, queueAfterExternalChange, queueAfterConflict, queueAfterAccept, proposalHistory });
  writeProposalEvidencePage({ evidenceDir, logs, summary });
  writeProposalManifest({ evidenceDir, summary });
  await verifyProposalEvidencePage({ evidenceDir, assetsDir });
  command("open", [path.join(evidenceDir, "index.html")], "evidence-open.log");
  logs.push({ command: "verdict", ok: true });
  writeLogs();
  shutdown();
  console.log(JSON.stringify({ ok: true, assets: assetsDir, proposal: path.join(assetsDir, "video-clip-proposal-diff.json") }, null, 2));
} catch (error) {
  logs.push({ command: "verdict", ok: false, error: error instanceof Error ? error.message : String(error) });
  writeLogs();
  try { shutdown(); } catch {}
  console.error(JSON.stringify({ ok: false, error: error instanceof Error ? error.message : String(error), assets: assetsDir }, null, 2));
  process.exit(1);
}

function generateVideo(out, source, seconds, evidenceName) {
  command("ffmpeg", ["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", source, "-t", String(seconds), "-c:v", "libvpx", "-b:v", "1200k", "-pix_fmt", "yuv420p", out], evidenceName);
  assert(existsSync(out), `source video missing: ${out}`);
}

function openShell(phase, evidenceName) {
  const instanceId = `${versionId.replace(/[^A-Za-z0-9]+/g, "-")}-video-clip-proposal-${phase}`;
  currentSocket = `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock`;
  openInstanceIds.push(instanceId);
  const args = ["--id", instanceId, "--project", "demo", "--replace"];
  if (shellBundleReady) args.push("--skip-build");
  const env = { ...process.env, CAPYBARA_SOCKET: currentSocket, CAPY_VERIFY_VERSION_DIR: versionDir, CAPY_VERIFY_ASSETS: assetsDir, CAPY_VERIFY_OPEN_PROJECT: "demo", CAPY_LAUNCH_LABEL: launchLabel(instanceId) };
  launchShell(args, evidenceName, env);
  const ps = capyJson(["ps"], `video-clip-proposal-${phase}-ps.json`, capyEnv());
  assert(Number(ps.count || 0) > 0, `${phase} shell did not open a window`);
  shellBundleReady = true;
}

function command(cmd, args, evidenceName, options = {}) {
  const started = Date.now();
  const stdout = execFileSync(cmd, args, { cwd: root, env: options.env || process.env, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], maxBuffer: 128 * 1024 * 1024, timeout: options.timeout || 120_000 });
  if (evidenceName) writeFileSync(path.join(assetsDir, evidenceName), stdout);
  logs.push({ command: [cmd, ...args].join(" "), evidence: evidenceName || null, elapsed_ms: Date.now() - started, ok: true });
  return stdout;
}

function launchShell(args, evidenceName, env) {
  try { command("scripts/open-debug-shell.sh", args, evidenceName, { env }); } catch (error) {
    const message = `${String(error?.stdout || "")}${String(error?.stderr || "")}\n${error instanceof Error ? error.message : String(error)}\n`;
    writeFileSync(path.join(assetsDir, evidenceName), message);
    logs.push({ command: ["scripts/open-debug-shell.sh", ...args].join(" "), evidence: evidenceName, ok: false, error: message });
    throw error;
  }
}

function optionalCommandResult(cmd, args, evidenceName, options = {}) {
  const started = Date.now();
  try {
    command(cmd, args, evidenceName, options);
    return { ok: true, evidence: evidenceName, elapsed_ms: Date.now() - started };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    writeFileSync(path.join(assetsDir, evidenceName), evidenceName.endsWith(".json") ? `${JSON.stringify({ ok: false, error: message }, null, 2)}\n` : `${message}\n`);
    logs.push({ command: [cmd, ...args].join(" "), evidence: evidenceName, ok: false, error: message });
    return { ok: false, evidence: evidenceName, elapsed_ms: Date.now() - started, error: message };
  }
}

function capyJson(args, evidenceName, env = process.env) {
  const value = JSON.parse(command("target/debug/capy", args, evidenceName, { env }));
  writeJson(evidenceName, value);
  writeStageVisual(value, evidenceName);
  return value;
}

function writeJson(name, value) {
  writeFileSync(path.join(assetsDir, name), `${JSON.stringify(value, null, 2)}\n`);
}

function writeStageVisual(value, stateEvidenceName) {
  const imageName = {
    loaded: "video-clip-proposal-loaded-desktop.png",
    "feedback-saved": "video-clip-proposal-feedback-saved-desktop.png",
    "proposal-generated": "video-clip-proposal-generated-desktop.png",
    "proposal-rejected": "video-clip-proposal-rejected-desktop.png",
    "proposal-conflicted": "video-clip-proposal-conflicted-desktop.png",
    "proposal-accepted": "video-clip-proposal-accepted-desktop.png",
    "proposal-history-reopened": "video-clip-proposal-history-reopened-desktop.png"
  }[value?.stage];
  if (!imageName) return;
  const fallbackImage = imageName.replace(/-desktop\.png$/, "-state-derived.png");
  const appViewImage = imageName.replace(/-desktop\.png$/, "-app-view.png");
  const panels = [
    ["Clip queue", value.domQueueText || ""],
    ["修改提案", value.domProposalText || "尚未生成提案"],
    ["Proposal history", value.domProposalHistoryText || "尚未恢复历史"]
  ];
  writeStateDerivedImage({
    assetsDir,
    imageName: fallbackImage,
    title: "Capybara · 片段反馈修改提案",
    subtitle: "state-derived fallback · real CEF DOM/state returned",
    stage: value.stage,
    panels
  });
  const attempts = [];
  const captureEvidence = imageName.replace(/-desktop\.png$/, "-capture.json");
  const captureResult = optionalCommandResult(
    "target/debug/capy",
    ["capture", `--out=${path.join(assetsDir, appViewImage)}`],
    captureEvidence,
    { env: capyEnv(), timeout: 12_000 }
  );
  const captureOk = captureResult.ok && existsSync(path.join(assetsDir, appViewImage));
  attempts.push(captureAttempt({
    method: "capture",
    ok: captureOk,
    evidence: captureEvidence,
    image: appViewImage,
    elapsed_ms: captureResult.elapsed_ms,
    error: captureOk ? null : captureResult.error || "capture command returned without an image"
  }));

  let finalIsAppView = captureOk;
  if (!finalIsAppView) {
    const screenshotEvidence = imageName.replace(/-desktop\.png$/, "-screenshot.json");
    const screenshotResult = optionalCommandResult(
      "target/debug/capy",
      ["screenshot", `--out=${path.join(assetsDir, appViewImage)}`],
      screenshotEvidence,
      { env: capyEnv(), timeout: 12_000 }
    );
    const screenshotOk = screenshotResult.ok && existsSync(path.join(assetsDir, appViewImage));
    attempts.push(captureAttempt({
      method: "screenshot",
      ok: screenshotOk,
      evidence: screenshotEvidence,
      image: appViewImage,
      elapsed_ms: screenshotResult.elapsed_ms,
      error: screenshotOk ? null : screenshotResult.error || "screenshot command returned without an image"
    }));
    finalIsAppView = screenshotOk;
  }
  if (finalIsAppView) {
    copyFallbackImage({ assetsDir, fallbackImage: appViewImage, finalImage: imageName });
  } else {
    copyFallbackImage({ assetsDir, fallbackImage, finalImage: imageName });
  }
  const verdict = summarizeVideoCapture({
    version: versionId,
    stage: value.stage,
    attempts,
    state_evidence: stateEvidenceName,
    fallback_image: fallbackImage,
    final_image: imageName,
    real_dom_state: true,
    ui_errors: [...(value.consoleErrors || []), ...(value.pageErrors || [])],
    retry_command: `CAPYBARA_SOCKET=${currentSocket} target/debug/capy capture --out=${path.join(assetsDir, appViewImage)}`
  });
  const verdictName = imageName.replace(/-desktop\.png$/, "-capture-verdict.json");
  writeJson(verdictName, verdict);
  stageCaptureVerdicts.push(verdict);
  logs.push({
    command: `desktop capture verdict ${value.stage}`,
    evidence: verdictName,
    ok: verdict.verdict.status === "passed",
    status: verdict.capture.status
  });
  if (verdict.capture.blocking) throw new Error(`desktop capture blocked ${value.stage}: ${verdict.capture.status}`);
}

function writeSummary({ loadedState, savedState, proposedState, rejectedState, staleCandidateState, conflictedState, acceptedState, reopenedState, feedbackCli, queueBeforeProposal, queueAfterProposal, queueAfterReject, queueAfterExternalChange, queueAfterConflict, queueAfterAccept, proposalHistory }) {
  const summary = {
    version: versionId,
    verdict: stageCaptureVerdicts.some(item => item.capture.blocking) ? "failed" : "passed",
    project: projectDir,
    feedback: feedbackCli,
    proposal: acceptedState.proposal,
    first_proposal: proposedState.proposal,
    stale_candidate_proposal: staleCandidateState.proposal,
    conflict_decision: conflictedState.proposal,
    reject_decision: rejectedState.proposal?.decision || null,
    conflict_attempt_decision: conflictedState.proposal?.decision || null,
    accept_decision: acceptedState.proposal?.decision || null,
    proposal_history: proposalHistory,
    reopened_history_count: reopenedState.proposalHistory?.length || 0,
    reopened_history_statuses: (reopenedState.proposalHistory || []).map(item => item.status),
    reopened_history_readonly: reopenedState.historyDecisionButtonCount === 0,
    queue_before_proposal: queueBeforeProposal.items || [],
    queue_after_reject: queueAfterReject.items || [],
    queue_after_external_change: queueAfterExternalChange.items || [],
    queue_after_conflict: queueAfterConflict.items || [],
    queue_after_accept: queueAfterAccept.items || [],
    queue_mutation: {
      generate: summarizeQueueMutation(queueBeforeProposal, queueAfterProposal),
      reject: summarizeQueueMutation(queueBeforeProposal, queueAfterReject),
      external_change: summarizeQueueMutation(queueAfterReject, queueAfterExternalChange),
      conflicted_accept: summarizeQueueMutation(queueAfterExternalChange, queueAfterConflict),
      accept: summarizeQueueMutation(queueAfterConflict, queueAfterAccept)
    },
    capture_verdicts: stageCaptureVerdicts,
    states: { loaded: summarizeState(loadedState), saved: summarizeState(savedState), proposed: summarizeState(proposedState), rejected: summarizeState(rejectedState), stale_candidate: summarizeState(staleCandidateState), conflicted: summarizeState(conflictedState), accepted: summarizeState(acceptedState), reopened: summarizeState(reopenedState) }
  };
  writeJson("video-clip-proposal-summary.json", summary);
  return summary;
}

function summarizeState(state) {
  return { stage: state.stage, queue_count: state.queue?.length || 0, semantic_count: state.semantics?.items?.length || 0, feedback_count: state.feedback?.items?.length || 0, proposal_status: state.proposal?.status || "", proposal_revision: state.proposal?.revision || 0, base_queue_hash: state.proposal?.base_queue_hash || "", proposal_history_count: state.proposalHistory?.length || 0, proposal_history_statuses: (state.proposalHistory || []).map(item => item.status), history_readonly: state.historyDecisionButtonCount === 0, change_count: state.proposal?.changes?.length || 0, layout: state.layout, console_errors: state.consoleErrors || [], page_errors: state.pageErrors || [] };
}

function assertSemantics(manifest, minCount, label) {
  assert(manifest?.schema_version === "capy.project-video-clip-semantics.v1", `${label} semantics schema mismatch`);
  assert((manifest.items || []).length >= minCount, `${label} missing semantic items`);
}

function assertFeedback(manifest, label) {
  assert(manifest?.schema_version === "capy.project-video-clip-feedback.v1", `${label} feedback schema mismatch`);
  assert((manifest.items || []).some(item => item.feedback === "这段不适合开场" && item.queue_item_id === "queue-initial-camera-a"), `${label} missing segment feedback`);
}

function assertProposal(proposal, label) {
  assert(proposal?.schema_version === "capy.project-video-clip-proposal.v1", `${label} proposal schema mismatch`);
  assert(proposal.status === "proposed", `${label} proposal status mismatch`);
  assert(Number(proposal.revision || 0) >= 1, `${label} missing proposal revision`);
  assert(String(proposal.base_queue_hash || "").startsWith("queue-fnv1a64-"), `${label} missing base queue hash`);
  assert(proposal.current_queue_hash === proposal.base_queue_hash, `${label} current hash should match base hash at generation`);
  assert((proposal.changes || []).some(change => change.action === "deprioritize" && change.before_sequence === 1 && change.after_sequence === 2), `${label} missing deprioritize change`);
  assert((proposal.changes || []).some(change => change.feedback_reason && change.semantic_reason), `${label} missing feedback/semantic reasons`);
}

function assertDecision(proposal, status) {
  assert(proposal?.status === status, `proposal decision status should be ${status}`);
  const expectedDecision = status === "rejected" ? "reject" : "accept";
  assert(proposal?.decision?.decision === expectedDecision, `proposal decision payload should be ${status}`);
  if (status === "conflicted") {
    assert(proposal?.decision?.queue_updated === false, "conflicted proposal must not update queue");
    assert(proposal?.conflict?.conflict_type === "queue_changed_since_proposal", "conflicted proposal missing queue conflict");
    assert(proposal.conflict.current_queue_hash !== proposal.conflict.base_queue_hash, "conflict hashes should differ");
  }
}

function assertHistory(history, requiredStatuses, label) {
  assert(history?.schema_version === "capy.project-video-clip-proposal-history.v1", `${label} history schema mismatch`);
  const entries = history.entries || [];
  assert(entries.length >= requiredStatuses.length, `${label} history entry count too small`);
  for (const status of requiredStatuses) {
    assert(entries.some(entry => entry.status === status), `${label} missing ${status} entry`);
  }
  for (const entry of entries) {
    assert(Number(entry.revision || 0) >= 1, `${label} entry missing revision`);
    assert(String(entry.base_queue_hash || "").startsWith("queue-fnv1a64-"), `${label} entry missing base queue hash`);
    assert(Array.isArray(entry.changes) && entry.changes.length > 0, `${label} entry missing changes`);
  }
}

function assertReopenedHistory(state, history) {
  assertNoPageErrors(state, "reopened history");
  assert((state.proposalHistory || []).length === (history.entries || []).length, "reopened history count mismatch");
  assert(state.historyDecisionButtonCount === 0, "history rows must not contain decision buttons");
  assert(String(state.domProposalHistoryText || "").toLowerCase().includes("proposal history"), "reopened history DOM missing proposal history heading");
  assertTextIncludes(state.domProposalHistoryText, ["历史详情只读", "base_queue_hash", "已过期/冲突", "已接受"], "reopened history DOM");
  assert((state.proposalHistory || []).some(item => item.status === "conflicted" && item.conflict?.base_queue_hash && item.conflict?.current_queue_hash), "reopened history missing conflicted hashes");
}

function assertNoPageErrors(state, label) {
  const errors = [...(state.consoleErrors || []), ...(state.pageErrors || [])];
  assert(errors.length === 0, `${label} has page/console errors: ${JSON.stringify(errors)}`);
}

function assertTextIncludes(text, parts, label) {
  for (const part of parts) assert(String(text || "").includes(part), `${label} missing text: ${part}`);
}

function capyEnv() {
  return { ...process.env, CAPYBARA_SOCKET: currentSocket };
}

function launchLabel(instanceId) {
  return `com.capybara.debug.${instanceId}`;
}

function shutdown() {
  for (const instanceId of [...openInstanceIds].reverse()) {
    optionalCommandResult("target/debug/capy", ["quit"], `video-clip-proposal-${instanceId}-quit.json`, { timeout: 10_000, env: { ...process.env, CAPYBARA_SOCKET: `/tmp/capybara-${instanceId}-${process.getuid ? process.getuid() : "user"}.sock` } });
    optionalCommandResult("launchctl", ["remove", launchLabel(instanceId)], `video-clip-proposal-${instanceId}-launchctl-remove.log`);
  }
}

function writeLogs() {
  writeFileSync(path.join(assetsDir, "video-clip-proposal-command-log.json"), `${JSON.stringify(logs, null, 2)}\n`);
}

function assert(condition, message) { if (!condition) throw new Error(message); }
