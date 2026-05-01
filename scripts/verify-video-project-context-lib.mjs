import { existsSync } from "node:fs";

export function buildVideoProjectContextEvidence({ capyJson, projectDir, artifactId, contextOut }) {
  const context = capyJson(
    ["context", "build", "--project", projectDir, "--artifact", artifactId, "--out", contextOut],
    "video-project-context-package-cli.json"
  );
  assert(existsSync(contextOut), `context package file missing: ${contextOut}`);
  assert(context.schema_version === "capy.context.v1", "context package schema mismatch");
  assert(context.artifact_id === artifactId, "context package artifact id mismatch");
  const video = context.video_project_context;
  assert(video?.schema_version === "capy.video-project-context.v1", "missing video_project_context");
  assert(String(video.package_id || "").startsWith("vpctx-fnv1a64-"), "missing stable video context package id");
  assert(video.anchor_artifact?.artifact_id === artifactId, "missing stable anchor artifact id");
  assert((video.source_media || []).length >= 2, "video context missing source media");
  assert((video.clip_queue?.item_count || 0) >= 1, "video context missing current queue");
  assert(String(video.clip_queue?.current_queue_hash || "").startsWith("queue-fnv1a64-"), "video context missing current queue hash");
  const counts = video.proposal_history?.status_counts || {};
  for (const status of ["accepted", "rejected", "conflicted"]) {
    assert(Number(counts[status] || 0) >= 1, `video context missing ${status} history`);
  }
  const conflict = (video.proposal_history?.conflicts || [])[0]?.conflict;
  assert(conflict?.message_zh, "video context missing conflict reason");
  assert(conflict?.base_queue_hash && conflict?.current_queue_hash, "video context missing conflict hashes");
  assert(video.safety?.safe_for_next_ai_input === true, "video context not marked safe for next AI input");
  assert(video.safety?.no_queue_write === true, "context build must be read-only");
  assert(video.safety?.proposal_history_read_only === true, "proposal history must be read-only");
  return context;
}

export function summarizeVideoProjectContext(context) {
  const video = context.video_project_context || {};
  return {
    context_id: context.context_id,
    context_path: "assets/video-project-context-package.json",
    package_id: video.package_id || "",
    anchor_artifact_id: video.anchor_artifact?.artifact_id || context.artifact_id || "",
    source_media_count: video.project_summary?.source_media_count || 0,
    queue_item_count: video.clip_queue?.item_count || 0,
    current_queue_hash: video.clip_queue?.current_queue_hash || "",
    proposal_history_count: video.proposal_history?.entry_count || 0,
    status_counts: video.proposal_history?.status_counts || {},
    conflicts: (video.proposal_history?.conflicts || []).map(item => ({
      proposal_id: item.proposal_id,
      revision: item.revision,
      conflict_type: item.conflict?.conflict_type || "",
      message_zh: item.conflict?.message_zh || "",
      base_queue_hash: item.conflict?.base_queue_hash || "",
      current_queue_hash: item.conflict?.current_queue_hash || ""
    })),
    design_language_ref: video.design_constraints?.design_language_ref || context.design_language_ref || "",
    design_asset_count: video.design_constraints?.assets?.length || 0,
    safe_for_next_ai_input: video.safety?.safe_for_next_ai_input === true,
    no_queue_write: video.safety?.no_queue_write === true,
    proposal_history_read_only: video.safety?.proposal_history_read_only === true,
    safety_note_zh: video.safety?.safe_next_input_note_zh || ""
  };
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
