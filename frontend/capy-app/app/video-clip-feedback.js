import { semanticKey } from "./video-clip-semantics.js";

export function createVideoClipFeedbackController(ctx) {
  const { state, rpc, projectPath, stringifyError, renderVideoEditor } = ctx;

  function applyManifest(manifest) {
    state.video.clipFeedback = manifest || null;
    state.video.clipFeedbackStatus = manifest?.items?.length ? "loaded" : "idle";
    state.video.clipFeedbackError = null;
  }

  function feedbackForItem(item) {
    const items = Array.isArray(state.video.clipFeedback?.items) ? state.video.clipFeedback.items : [];
    const key = semanticKey(item?.composition_path, item?.clip_id, item?.start_ms, item?.end_ms);
    return items.find((feedback) => feedback.clip_key === key)
      || items.find((feedback) => feedback.queue_item_id === item?.id);
  }

  async function saveFeedback(item, feedbackText) {
    const project = projectPath?.();
    if (!project || !rpc || !item?.id) {
      state.video.clipFeedbackStatus = "error";
      state.video.clipFeedbackError = "缺少项目路径或片段";
      renderVideoEditor();
      return;
    }
    state.video.clipFeedbackStatus = "saving";
    state.video.clipFeedbackError = null;
    renderVideoEditor();
    try {
      const manifest = await rpc("project-video-clip-feedback-set", {
        project,
        queue_item_id: item.id,
        feedback: feedbackText || ""
      });
      applyManifest(manifest);
      state.video.clipFeedbackStatus = "saved";
    } catch (error) {
      state.video.clipFeedbackStatus = "error";
      state.video.clipFeedbackError = stringifyError ? stringifyError(error) : String(error);
    }
    renderVideoEditor();
  }

  return { applyManifest, feedbackForItem, saveFeedback };
}
