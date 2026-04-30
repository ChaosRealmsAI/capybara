import { queueFromManifest } from "./video-clip-queue.js";

export function createVideoClipSuggestionController(ctx) {
  const {
    state,
    dom,
    rpc,
    projectPath,
    stringifyError,
    renderVideoEditor,
    renderDelivery,
    formatTime,
    escapeHtml
  } = ctx;

  function render() {
    if (!dom.videoSuggestionEl) return;
    const status = state.video.clipSuggestionStatus || "idle";
    const suggestion = state.video.clipSuggestion;
    if (status === "planning") {
      dom.videoSuggestionEl.hidden = false;
      dom.videoSuggestionEl.innerHTML = "<p>正在生成本地 AI 剪辑建议...</p>";
      return;
    }
    if (status === "error") {
      dom.videoSuggestionEl.hidden = false;
      dom.videoSuggestionEl.innerHTML = `<p>AI 建议生成失败：${escapeHtml(state.video.clipSuggestionError || "unknown")}</p>`;
      return;
    }
    if (!suggestion) {
      dom.videoSuggestionEl.hidden = true;
      dom.videoSuggestionEl.replaceChildren();
      return;
    }
    dom.videoSuggestionEl.hidden = false;
    const adopted = status === "adopted";
    const rows = (suggestion.items || []).map((item) => `
      <li>
        <strong>${String(item.sequence).padStart(2, "0")} · ${escapeHtml(item.source_video?.filename || item.scene || item.clip_id)}</strong>
        <span>${formatTime(item.start_ms)} - ${formatTime(item.end_ms)} · ${formatTime(item.duration_ms)}</span>
        ${item.semantic_summary ? `<small>摘要：${escapeHtml(item.semantic_summary)}</small>` : ""}
        ${item.semantic_tags?.length ? `<small>标签：${item.semantic_tags.map(escapeHtml).join(" · ")}</small>` : ""}
        ${item.feedback_text ? `<small>用户反馈：${escapeHtml(item.feedback_text)}</small>` : ""}
        <em>${escapeHtml(item.reason || "本地 planner 建议")}</em>
        ${item.semantic_reason ? `<em>语义理由：${escapeHtml(item.semantic_reason)}</em>` : ""}
        ${item.feedback_reason ? `<em>反馈调整：${escapeHtml(item.feedback_reason)}</em>` : ""}
      </li>
    `).join("");
    dom.videoSuggestionEl.innerHTML = `
      <header class="video-suggestion-head">
        <div>
          <span>AI 剪辑建议</span>
          <strong>${escapeHtml(suggestion.suggestion_id || "suggestion")}</strong>
        </div>
        <button class="tool-button primary" type="button" data-video-adopt-suggestion>${adopted ? "已采用" : "采用方案"}</button>
      </header>
      <p>${escapeHtml(suggestion.rationale || "本地 deterministic planner 基于项目素材和队列生成。")}</p>
      <ol class="video-suggestion-list">${rows}</ol>
    `;
    const adopt = dom.videoSuggestionEl.querySelector("[data-video-adopt-suggestion]");
    if (adopt) {
      adopt.disabled = adopted;
      adopt.addEventListener("click", () => adoptSuggestion());
    }
  }

  async function generate() {
    const project = projectPath?.();
    if (!project || !rpc) {
      state.video.clipSuggestionStatus = "error";
      state.video.clipSuggestionError = "缺少项目路径";
      renderDelivery();
      return;
    }
    state.video.clipSuggestionStatus = "planning";
    state.video.clipSuggestionError = null;
    renderDelivery();
    try {
      const suggestion = await rpc("project-video-clip-queue-suggest", { project });
      state.video.clipSuggestion = suggestion;
      state.video.clipSuggestionStatus = "ready";
      state.video.clipSuggestionError = null;
      renderDelivery();
    } catch (error) {
      state.video.clipSuggestionStatus = "error";
      state.video.clipSuggestionError = stringifyError ? stringifyError(error) : String(error);
      renderDelivery();
    }
  }

  async function adoptSuggestion() {
    const project = projectPath?.();
    const suggestion = state.video.clipSuggestion;
    if (!project || !rpc || !suggestion) return;
    const items = (suggestion.items || []).map((item) => suggestionQueueItem(item, suggestion));
    state.video.clipQueuePersistStatus = "saving";
    state.video.clipQueuePersistError = null;
    renderDelivery();
    try {
      const manifest = await rpc("project-video-clip-queue-set", { project, items, reason: "adopt-ai-suggestion" });
      state.video.clipQueueManifest = manifest;
      state.video.clipQueue = queueFromManifest(manifest, project);
      state.video.clipQueuePersistStatus = "saved";
      state.video.clipQueuePersistError = null;
      state.video.clipSuggestion = { ...suggestion, status: "adopted", adopted_at: Date.now() };
      state.video.clipSuggestionStatus = "adopted";
      state.video.clipProposal = null;
      state.video.proposalStatus = "idle";
      renderVideoEditor();
    } catch (error) {
      state.video.clipQueuePersistStatus = "error";
      state.video.clipQueuePersistError = stringifyError ? stringifyError(error) : String(error);
      state.video.clipSuggestionStatus = "error";
      state.video.clipSuggestionError = state.video.clipQueuePersistError;
      renderVideoEditor();
    }
  }

  return { render, generate };
}

function suggestionQueueItem(item, suggestion) {
  const start = Math.round(Number(item.start_ms || 0));
  const end = Math.round(Number(item.end_ms || 0));
  return {
    id: item.id || `queue-${suggestion.suggestion_id}-${item.sequence}`,
    sequence: item.sequence,
    composition_path: item.composition_path,
    render_source_path: item.render_source_path || "",
    clip_id: item.clip_id || "source",
    track_id: item.track_id || "",
    scene: item.scene || item.clip_id || "AI 建议片段",
    start_ms: start,
    end_ms: end,
    duration_ms: Math.max(1, Math.round(Number(item.duration_ms || end - start))),
    source_video: item.source_video || null,
    suggestion_id: suggestion.suggestion_id || item.suggestion_id || "",
    suggestion_reason: item.reason || "",
    semantic_ref: item.semantic_ref || "",
    semantic_summary: item.semantic_summary || "",
    semantic_tags: item.semantic_tags || [],
    semantic_reason: item.semantic_reason || "",
    updated_at: Date.now()
  };
}
