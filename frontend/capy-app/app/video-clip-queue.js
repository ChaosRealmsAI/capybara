export function renderQueue({ state, dom, moveQueueItem, removeQueueItem, formatTime, escapeHtml }) {
  const queue = normalizedQueue(state);
  const total = queueTotalDuration(queue);
  if (dom.videoQueueSummaryEl) {
    dom.videoQueueSummaryEl.textContent = queue.length
      ? `${queue.length} 个待导出片段 · 总时长 ${formatTime(total)}`
      : "剪辑队列为空";
  }
  if (!dom.videoQueueEl) return;
  if (!queue.length) {
    dom.videoQueueEl.innerHTML = `<p class="video-queue-empty">选择时间范围后点击加入队列。</p>`;
    return;
  }
  dom.videoQueueEl.replaceChildren(...queue.map((item) => queueCard({
    item,
    total: queue.length,
    moveQueueItem,
    removeQueueItem,
    formatTime,
    escapeHtml
  })));
}

export function queueItemFromRange({ state, range, sequence, currentVideoSourceSummary }) {
  const sourceVideo = currentVideoSourceSummary();
  return {
    id: `queue-${Date.now()}-${sequence}-${safeSlug(range.clip_id)}-${Math.round(range.start_ms)}-${Math.round(range.end_ms)}`,
    sequence,
    clip_id: range.clip_id,
    track_id: range.track_id || "",
    scene: range.scene || range.clip_id || `片段 ${sequence}`,
    start_ms: Math.round(Number(range.start_ms || 0)),
    end_ms: Math.round(Number(range.end_ms || 0)),
    duration_ms: Math.max(1, Math.round(Number(range.end_ms || 0) - Number(range.start_ms || 0))),
    composition_path: state.video.compositionPath,
    render_source_path: state.video.renderSourcePath,
    source_video: sourceVideo
  };
}

export function queueExportRange(item) {
  return {
    sequence: item.sequence,
    composition_path: item.composition_path,
    clip_id: item.clip_id,
    track_id: item.track_id || "",
    start_ms: item.start_ms,
    end_ms: item.end_ms,
    duration_ms: item.duration_ms,
    scene: item.scene || item.clip_id,
    source_video: item.source_video || null
  };
}

export function normalizedQueue(state) {
  const queue = Array.isArray(state.video.clipQueue) ? state.video.clipQueue : [];
  state.video.clipQueue = renumberQueue(queue);
  return state.video.clipQueue;
}

export function renumberQueue(queue) {
  return queue.map((item, index) => ({
    ...item,
    sequence: index + 1,
    duration_ms: Math.max(1, Number(item.duration_ms || Number(item.end_ms || 0) - Number(item.start_ms || 0)))
  }));
}

export function queueTotalDuration(queue) {
  return queue.reduce((total, item) => total + Math.max(1, Number(item.duration_ms || 0)), 0);
}

function queueCard({ item, total, moveQueueItem, removeQueueItem, formatTime, escapeHtml }) {
  const card = document.createElement("article");
  card.className = "video-queue-card";
  card.dataset.queueItemId = item.id;
  card.dataset.sequence = String(item.sequence);
  card.innerHTML = `
    <div class="video-queue-index">${String(item.sequence).padStart(2, "0")}</div>
    <div class="video-queue-copy">
      <strong>${escapeHtml(item.source_video?.filename || item.scene || item.clip_id)}</strong>
      <span>${escapeHtml(item.scene || item.clip_id)} · ${formatTime(item.start_ms)} - ${formatTime(item.end_ms)} · ${formatTime(item.duration_ms)}</span>
    </div>
    <div class="video-queue-actions">
      <button class="tool-button secondary" type="button" data-video-queue-move="-1" ${item.sequence <= 1 ? "disabled" : ""}>上移</button>
      <button class="tool-button secondary" type="button" data-video-queue-move="1" ${item.sequence >= total ? "disabled" : ""}>下移</button>
      <button class="tool-button secondary" type="button" data-video-queue-remove>移除</button>
    </div>
  `;
  card.querySelectorAll("[data-video-queue-move]").forEach((button) => {
    button.addEventListener("click", () => moveQueueItem(item.id, Number(button.dataset.videoQueueMove || 0)));
  });
  card.querySelector("[data-video-queue-remove]")?.addEventListener("click", () => removeQueueItem(item.id));
  return card;
}

function safeSlug(value) {
  return String(value || "clip")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    || "clip";
}
