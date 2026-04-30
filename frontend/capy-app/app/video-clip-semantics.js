export function createVideoClipSemanticsController(ctx) {
  const { state, dom, rpc, projectPath, stringifyError, renderVideoEditor, renderDelivery, formatTime, escapeHtml } = ctx;

  function render() {
    if (!dom.videoSemanticsEl) return;
    const status = state.video.clipSemanticsStatus || "idle";
    const manifest = state.video.clipSemantics;
    if (status === "analyzing") {
      dom.videoSemanticsEl.hidden = false;
      dom.videoSemanticsEl.innerHTML = "<p>正在分析片段语义...</p>";
      return;
    }
    if (status === "error") {
      dom.videoSemanticsEl.hidden = false;
      dom.videoSemanticsEl.innerHTML = `<p>片段语义分析失败：${escapeHtml(state.video.clipSemanticsError || "unknown")}</p>`;
      return;
    }
    const items = Array.isArray(manifest?.items) ? manifest.items : [];
    if (!items.length) {
      dom.videoSemanticsEl.hidden = true;
      dom.videoSemanticsEl.replaceChildren();
      return;
    }
    dom.videoSemanticsEl.hidden = false;
    const rows = items.map((item) => `
      <article class="video-semantic-card" data-video-semantic-id="${escapeHtml(item.id || "")}">
        <header>
          <strong>${String(item.sequence || "").padStart(2, "0")} · ${escapeHtml(item.source_video?.filename || item.scene || item.clip_id)}</strong>
          <span>${formatTime(item.start_ms)} - ${formatTime(item.end_ms)} · ${formatTime(item.duration_ms)}</span>
        </header>
        <p>${escapeHtml(item.summary_zh || "")}</p>
        <div class="video-semantic-tags"><b>标签</b>${(item.tags || []).map((tag) => `<span>${escapeHtml(tag)}</span>`).join("")}</div>
        <dl>
          <dt>节奏</dt><dd>${escapeHtml(item.rhythm || "")}</dd>
          <dt>用途</dt><dd>${escapeHtml(item.use_case || "")}</dd>
          <dt>理由</dt><dd>${escapeHtml(item.recommendation || "")}</dd>
        </dl>
      </article>
    `).join("");
    dom.videoSemanticsEl.innerHTML = `
      <header class="video-semantics-head">
        <div>
          <span>片段语义</span>
          <strong>${items.length} 段已保存到项目</strong>
        </div>
      </header>
      <div class="video-semantics-list">${rows}</div>
    `;
  }

  async function analyze() {
    const project = projectPath?.();
    if (!project || !rpc) {
      state.video.clipSemanticsStatus = "error";
      state.video.clipSemanticsError = "缺少项目路径";
      renderDelivery();
      return;
    }
    state.video.clipSemanticsStatus = "analyzing";
    state.video.clipSemanticsError = null;
    renderDelivery();
    try {
      const manifest = await rpc("project-video-clip-semantics-analyze", { project });
      applyManifest(manifest);
      state.video.clipSemanticsStatus = "saved";
      renderVideoEditor();
    } catch (error) {
      state.video.clipSemanticsStatus = "error";
      state.video.clipSemanticsError = stringifyError ? stringifyError(error) : String(error);
      renderVideoEditor();
    }
  }

  function applyManifest(manifest) {
    state.video.clipSemantics = manifest || null;
    state.video.clipSemanticsStatus = manifest?.items?.length ? "loaded" : "idle";
    state.video.clipSemanticsError = null;
  }

  function enrichQueueItem(item) {
    const semantic = findSemanticForItem(state.video.clipSemantics, item);
    if (!semantic) return item;
    return {
      ...item,
      semantic_ref: semantic.id || "",
      semantic_summary: semantic.summary_zh || "",
      semantic_tags: semantic.tags || [],
      semantic_reason: semantic.recommendation || ""
    };
  }

  return { render, analyze, applyManifest, enrichQueueItem };
}

export function findSemanticForItem(manifest, item) {
  const items = Array.isArray(manifest?.items) ? manifest.items : [];
  const key = semanticKey(item?.composition_path, item?.clip_id, item?.start_ms, item?.end_ms);
  return items.find((semantic) => semantic.clip_key === key)
    || items.find((semantic) =>
      String(semantic.composition_path || "") === String(item?.composition_path || "")
      && String(semantic.clip_id || "source") === String(item?.clip_id || "source")
      && Number(semantic.start_ms || 0) === Number(item?.start_ms || 0)
      && Number(semantic.end_ms || 0) === Number(item?.end_ms || 0));
}

export function semanticKey(compositionPath, clipId, startMs, endMs) {
  return `${String(compositionPath || "").trim()}|${String(clipId || "").trim()}|${Math.round(Number(startMs || 0))}|${Math.round(Number(endMs || 0))}`;
}
