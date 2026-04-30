import { execFileSync } from "node:child_process";
import { copyFileSync, writeFileSync } from "node:fs";
import path from "node:path";

import { buildCaptureAttempt } from "./post-export-capture-verdict.mjs";

export const VIDEO_VERIFIER_CAPTURE_SCHEMA = "capy.video-verifier.capture-verdict.v1";

export function queueIds(queueOrItems) {
  const items = Array.isArray(queueOrItems) ? queueOrItems : queueOrItems?.items || [];
  return items.map(item => String(item.id || ""));
}

export function assertQueueIdsEqual(before, after, message = "queue ids changed unexpectedly") {
  const beforeIds = queueIds(before);
  const afterIds = queueIds(after);
  if (JSON.stringify(beforeIds) !== JSON.stringify(afterIds)) {
    throw new Error(`${message}: before=${beforeIds.join(",")} after=${afterIds.join(",")}`);
  }
}

export function assertQueueIdsChangedTo(queue, expectedIds, message = "queue ids did not match expected order") {
  const actualIds = queueIds(queue);
  const expected = expectedIds.map(String);
  if (JSON.stringify(actualIds) !== JSON.stringify(expected)) {
    throw new Error(`${message}: expected=${expected.join(",")} actual=${actualIds.join(",")}`);
  }
}

export function summarizeQueueMutation(before, after) {
  const before_ids = queueIds(before);
  const after_ids = queueIds(after);
  return {
    before_ids,
    after_ids,
    changed: JSON.stringify(before_ids) !== JSON.stringify(after_ids)
  };
}

export function captureAttempt({ method, ok, evidence = null, image = null, elapsed_ms = null, error = null }) {
  return buildCaptureAttempt({ method, ok, evidence, image, elapsed_ms, error });
}

export function summarizeVideoCapture({
  version,
  stage,
  generated_at = new Date().toISOString(),
  attempts = [],
  state_evidence = null,
  fallback_image = null,
  final_image = null,
  real_dom_state = false,
  ui_errors = [],
  retry_command = null
}) {
  const normalizedAttempts = attempts.map(captureAttempt);
  const successfulAttempt = normalizedAttempts.find(attempt => attempt.ok);
  const hasTimeout = normalizedAttempts.some(attempt => attempt.failure_kind === "timeout");
  const hasStateFallback = Boolean(real_dom_state && state_evidence && fallback_image);
  const hasUiErrors = Array.isArray(ui_errors) && ui_errors.length > 0;

  let status = "missing";
  let blocking = true;
  let final_image_source = null;
  let rationale = "";

  if (successfulAttempt) {
    status = "captured";
    blocking = false;
    final_image_source = "app-view-capture";
    rationale = "CEF app-view capture succeeded; the visible PNG is real desktop evidence.";
  } else if (hasUiErrors) {
    status = hasTimeout ? "timeout_blocking_ui_errors" : "failed_blocking_ui_errors";
    blocking = true;
    final_image_source = hasStateFallback ? "state-derived-fallback" : null;
    rationale = "capture failed and the CEF state reported page or console errors.";
  } else if (hasTimeout && hasStateFallback) {
    status = "timeout_nonblocking_with_real_cef_state_fallback";
    blocking = false;
    final_image_source = "state-derived-fallback";
    rationale = "capture timed out, but real CEF DOM/state returned with no page or console errors; fallback image is marked as state-derived, not as capture success.";
  } else if (hasStateFallback) {
    status = "failed_nonblocking_with_real_cef_state_fallback";
    blocking = false;
    final_image_source = "state-derived-fallback";
    rationale = "capture failed, but real CEF DOM/state returned with no page or console errors; fallback image is marked as state-derived, not as capture success.";
  } else {
    status = hasTimeout ? "timeout_blocking_no_visible_state" : "failed_blocking_no_visible_state";
    blocking = true;
    final_image_source = null;
    rationale = "capture failed and no trusted real CEF DOM/state fallback was available.";
  }

  const blockers = blocking ? [status] : [];
  const warnings = !blocking && status !== "captured" ? [status] : [];
  return {
    schema: VIDEO_VERIFIER_CAPTURE_SCHEMA,
    version,
    generated_at,
    stage,
    capture: {
      status,
      blocking,
      attempts: normalizedAttempts,
      final_image,
      final_image_source,
      fallback_image,
      state_evidence,
      real_dom_state: Boolean(real_dom_state),
      ui_errors,
      retry_command,
      rationale
    },
    verdict: {
      status: blocking ? "failed" : "passed",
      blockers,
      warnings
    }
  };
}

export function writeStateDerivedImage({ assetsDir, imageName, title, subtitle, stage, panels }) {
  const svgName = imageName.replace(/\.png$/, ".svg");
  const svgPath = path.join(assetsDir, svgName);
  const pngPath = path.join(assetsDir, imageName);
  writeFileSync(svgPath, stateSvg({ title, subtitle, stage, panels }));
  execFileSync("magick", [svgPath, pngPath], { stdio: ["ignore", "pipe", "pipe"] });
  return { svg: svgName, png: imageName };
}

export function copyFallbackImage({ assetsDir, fallbackImage, finalImage }) {
  copyFileSync(path.join(assetsDir, fallbackImage), path.join(assetsDir, finalImage));
}

function stateSvg({ title, subtitle, stage, panels }) {
  const panelSvg = panels.map(([panelTitle, text], index) => {
    const x = 24 + index * 312;
    const width = index === 2 ? 288 : 286;
    return `<g><rect x="${x}" y="104" width="${width}" height="452" rx="14" fill="#fff" stroke="#d8dee8"/><text x="${x + 20}" y="140" font-size="20" font-weight="700" fill="#0f172a">${xml(panelTitle)}</text>${textLines(text, x + 20, 172, width - 40)}</g>`;
  }).join("");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="600" viewBox="0 0 960 600"><rect width="960" height="600" fill="#f6f8fb"/><text x="36" y="54" font-size="28" font-weight="700" fill="#101827">${xml(title)}</text><text x="38" y="82" font-size="14" fill="#64748b">${xml(subtitle)} · stage=${xml(stage || "")}</text>${panelSvg}</svg>`;
}

function textLines(text, x, y, width) {
  const approx = Math.max(10, Math.floor(width / 8));
  const words = String(text || "").replace(/\s+/g, " ").slice(0, 1400).split(" ");
  const lines = [];
  let line = "";
  for (const word of words) {
    const next = line ? `${line} ${word}` : word;
    if (next.length > approx && line) {
      lines.push(line);
      line = word;
    } else {
      line = next;
    }
    if (lines.length >= 17) break;
  }
  if (line && lines.length < 18) lines.push(line);
  return lines.map((lineText, index) => `<text x="${x}" y="${y + index * 22}" font-size="14" fill="#334155">${xml(lineText)}</text>`).join("");
}

function xml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
