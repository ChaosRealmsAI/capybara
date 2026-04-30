import { normalizedQueue, queueFromManifest, queueExportRange, queueItemFromRange, queueManifestItem, queueTotalDuration, renumberQueue, renderQueue } from "./video-clip-queue.js";
import { createVideoClipSemanticsController } from "./video-clip-semantics.js";
import { createVideoClipSuggestionController } from "./video-clip-suggestion.js";
export function createVideoClipDeliveryController(ctx) {
  const { state, dom, rpc, projectPath, stringifyError, exportComposition, seek, renderVideoEditor, selectedTrack, firstTrackForClip } = ctx;
  let persistSerial = 0;
  const clipSemantics = createVideoClipSemanticsController({
    state,
    dom,
    rpc,
    projectPath,
    stringifyError,
    renderVideoEditor,
    renderDelivery: render,
    formatTime,
    escapeHtml
  });
  const clipSuggestion = createVideoClipSuggestionController({
    state,
    dom,
    rpc,
    projectPath,
    stringifyError,
    renderVideoEditor,
    renderDelivery: render,
    formatTime,
    escapeHtml
  });
  function install() {
    dom.videoProposalGenerateEl?.addEventListener("click", () => generateClipProposal());
    dom.videoSemanticsAnalyzeEl?.addEventListener("click", () => clipSemantics.analyze());
    dom.videoSuggestionGenerateEl?.addEventListener("click", () => clipSuggestion.generate());
    dom.videoQueueAddEl?.addEventListener("click", () => addCurrentRangeToQueue());
    dom.videoRangeStartEl?.addEventListener("input", () => updateRangeFromInputs());
    dom.videoRangeEndEl?.addEventListener("input", () => updateRangeFromInputs());
  }
  function applyOpenResult(clips) {
    const selectedClipId = state.video.selectedRange?.clip_id;
    const clip = clips.find((item) => item.id === selectedClipId) || clips[0] || null;
    state.video.selectedRange = clip ? rangeFromClip(clip) : null;
    state.video.clipQueue = Array.isArray(state.video.clipQueue) ? state.video.clipQueue : [];
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    state.video.lastExport = null;
  }
  function applyProjectQueueManifest(manifest, loadedProjectPath = projectPath?.()) {
    state.video.clipQueueManifest = manifest || null;
    state.video.clipQueue = queueFromManifest(manifest, loadedProjectPath);
    state.video.clipQueuePersistStatus = "loaded";
    state.video.clipQueuePersistError = null;
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    renderVideoEditor();
  }
  function applyProjectSemanticsManifest(manifest) {
    clipSemantics.applyManifest(manifest);
    renderVideoEditor();
  }
  function render() {
    const range = state.video.selectedRange;
    syncRangeInputs(range);
    renderRangeSummary(range);
    renderQueue({ state, dom, moveQueueItem, removeQueueItem, formatTime, escapeHtml });
    clipSemantics.render();
    clipSuggestion.render();
    renderProposal();
  }
  function renderRangeSummary(range) {
    if (!dom.videoRangeSummaryEl) return;
    dom.videoRangeSummaryEl.textContent = range
      ? `${range.scene || range.clip_id} · ${formatTime(range.start_ms)} - ${formatTime(range.end_ms)}`
      : "未选择片段";
  }
  function renderProposal() {
    if (!dom.videoProposalEl) return;
    const proposal = state.video.clipProposal;
    if (!state.video.editor && !state.video.clipQueue?.length) {
      dom.videoProposalEl.textContent = "打开 composition.json 后选择 scene";
      return;
    }
    if (!proposal) {
      const queue = normalizedQueue(state);
      dom.videoProposalEl.innerHTML = queue.length
        ? `<p>剪辑队列已有 ${queue.length} 个片段，可生成多片段 proposal。</p>`
        : state.video.selectedRange
          ? `<p>已选择 ${escapeHtml(state.video.selectedRange.scene || state.video.selectedRange.clip_id)}，可生成单片段 proposal，或先加入队列。</p>`
          : "<p>选择左侧 scene 后生成可交付片段 proposal。</p>";
      return;
    }
    if (proposal.kind === "video-clip-queue-proposal") {
      renderQueueProposal(proposal);
      return;
    }
    renderSingleProposal(proposal);
  }
  function renderQueueProposal(proposal) {
    const exported = proposal.status === "exported";
    const rows = (proposal.clips || []).map((item) => `
      <li>
        <strong>${String(item.sequence).padStart(2, "0")} · ${escapeHtml(item.source_video?.filename || item.scene || item.clip_id)}</strong>
        <span>${formatTime(item.start_ms)} - ${formatTime(item.end_ms)} · ${formatTime(item.duration_ms)}</span>
      </li>
    `).join("");
    dom.videoProposalEl.innerHTML = `
      <dl class="video-proposal-grid">
        <dt>片段数量</dt><dd>${proposal.clip_count} 个</dd>
        <dt>总时长</dt><dd>${formatTime(proposal.total_duration_ms)}</dd>
        <dt>输出文件</dt><dd>${escapeHtml(proposal.output_filename)}</dd>
        <dt>proposal</dt><dd>${escapeHtml(proposal.id)}</dd>
      </dl>
      <ol class="video-proposal-list">${rows}</ol>
      <button class="tool-button primary" type="button" data-video-confirm-proposal>${exported ? "已导出" : "确认导出队列"}</button>
    `;
    const confirm = dom.videoProposalEl.querySelector("[data-video-confirm-proposal]");
    if (confirm) {
      confirm.disabled = exported;
      confirm.addEventListener("click", () => confirmClipProposal());
    }
  }
  function renderSingleProposal(proposal) {
    const exported = proposal.status === "exported";
    dom.videoProposalEl.innerHTML = `
      <dl class="video-proposal-grid">
        <dt>起止时间</dt><dd>${formatTime(proposal.start_ms)} - ${formatTime(proposal.end_ms)}</dd>
        <dt>场景说明</dt><dd>${escapeHtml(proposal.scene)}</dd>
        <dt>输出文件</dt><dd>${escapeHtml(proposal.output_filename)}</dd>
        <dt>来源</dt><dd>${escapeHtml(proposal.source.composition_path)}</dd>
      </dl>
      <button class="tool-button primary" type="button" data-video-confirm-proposal>${exported ? "已导出" : "确认导出片段"}</button>
    `;
    const confirm = dom.videoProposalEl.querySelector("[data-video-confirm-proposal]");
    if (confirm) {
      confirm.disabled = exported;
      confirm.addEventListener("click", () => confirmClipProposal());
    }
  }
  function selectClipRange(clip) {
    setSelectedRange(rangeFromClip(clip), false);
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    const track = firstTrackForClip(clip.id);
    if (track) state.video.selectedTrackId = track.id;
    seek(Number(clip.start_ms || 0));
    renderVideoEditor();
  }
  function selectRangeFromTrack(track) {
    setSelectedRange(rangeFromTrack(track), false);
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    seek(Number(track.start_ms || 0));
    renderVideoEditor();
  }
  function addCurrentRangeToQueue() {
    const range = state.video.selectedRange || rangeFromTrack(selectedTrack());
    if (!range || !state.video.compositionPath) return;
    const selected = setSelectedRange(range, false);
    if (!selected) return;
    const queue = normalizedQueue(state);
    queue.push(queueItemFromRange({
      state,
      range: selected,
      sequence: queue.length + 1,
      currentVideoSourceSummary
    }));
    state.video.clipQueue = renumberQueue(queue);
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    persistQueue("add");
    renderVideoEditor();
  }
  function moveQueueItem(id, delta) {
    const queue = normalizedQueue(state);
    const index = queue.findIndex((item) => item.id === id);
    if (index < 0) return;
    const next = Math.max(0, Math.min(queue.length - 1, index + delta));
    if (next === index) return;
    const [item] = queue.splice(index, 1);
    queue.splice(next, 0, item);
    state.video.clipQueue = renumberQueue(queue);
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    persistQueue("move");
    renderVideoEditor();
  }
  function removeQueueItem(id) {
    state.video.clipQueue = renumberQueue(normalizedQueue(state).filter((item) => item.id !== id));
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    persistQueue("remove");
    renderVideoEditor();
  }
  function generateClipProposal() {
    const queue = normalizedQueue(state);
    if (queue.length) {
      generateQueueProposal(queue);
      render();
      return;
    }
    generateSingleClipProposal();
  }
  function generateSingleClipProposal() {
    const range = state.video.selectedRange || rangeFromTrack(selectedTrack());
    if (!range || !state.video.compositionPath) return;
    const outputFilename = outputFilenameForRange(range);
    state.video.selectedRange = range;
    state.video.clipProposal = {
      id: `proposal-${safeSlug(range.clip_id)}-${range.start_ms}-${range.end_ms}`,
      kind: "video-clip-proposal",
      status: "ready",
      clip_id: range.clip_id,
      track_id: range.track_id || "",
      start_ms: range.start_ms,
      end_ms: range.end_ms,
      duration_ms: Math.max(1, Number(range.end_ms || 0) - Number(range.start_ms || 0)),
      scene: range.scene || range.clip_id || "selected scene",
      output_filename: outputFilename,
      output_path: outputPathForProposal(state.video.compositionPath, outputFilename),
      source: {
        composition_path: state.video.compositionPath,
        render_source_path: state.video.renderSourcePath,
        video: currentVideoSourceSummary()
      }
    };
    state.video.proposalStatus = "ready";
    render();
  }
  function generateQueueProposal(queue) {
    const total = queueTotalDuration(queue);
    const outputFilename = `clip-queue-${queue.length}-${Math.round(total)}.mp4`;
    state.video.clipProposal = {
      id: `proposal-clip-queue-${queue.length}-${Math.round(total)}`,
      kind: "video-clip-queue-proposal",
      status: "ready",
      clip_count: queue.length,
      total_duration_ms: total,
      output_filename: outputFilename,
      output_path: outputPathForProposal(state.video.compositionPath || queue[0]?.composition_path, outputFilename),
      clips: queue.map((item) => ({ ...item }))
    };
    state.video.proposalStatus = "ready";
  }
  function confirmClipProposal() {
    if (!state.video.clipProposal) generateClipProposal();
      const proposal = state.video.clipProposal;
    if (!proposal) return;
    if (proposal.kind === "video-clip-queue-proposal") {
      return exportComposition({
        mode: "clip-queue-proposal",
        out: proposal.output_path,
        queue: proposal.clips.map(queueExportRange),
        proposal
      });
    }
    return exportComposition({
      mode: "clip-proposal",
      out: proposal.output_path,
      range: {
        clip_id: proposal.clip_id,
        track_id: proposal.track_id,
        start_ms: proposal.start_ms,
        end_ms: proposal.end_ms
      },
      proposal
    });
  }
  function persistQueue(reason) {
    const project = projectPath?.();
    if (!project || !rpc) return;
    const serial = ++persistSerial;
    const items = normalizedQueue(state).map(clipSemantics.enrichQueueItem).map(queueManifestItem);
    state.video.clipQueuePersistStatus = "saving";
    state.video.clipQueuePersistError = null;
    rpc("project-video-clip-queue-set", { project, items, reason })
      .then((manifest) => {
        if (serial !== persistSerial) return;
        state.video.clipQueueManifest = manifest;
        state.video.clipQueuePersistStatus = "saved";
        state.video.clipQueuePersistError = null;
        state.video.clipQueue = queueFromManifest(manifest, project);
        renderVideoEditor();
      })
      .catch((error) => {
        if (serial !== persistSerial) return;
        state.video.clipQueuePersistStatus = "error";
        state.video.clipQueuePersistError = stringifyError ? stringifyError(error) : String(error);
        renderVideoEditor();
      });
  }
  function setSelectedRange(range, shouldRender = true) {
    if (!range) return null;
    const duration = Math.max(1, Number(state.video.durationMs || range.end_ms || 0));
    const start = Math.max(0, Math.min(duration, Number(range.start_ms || 0)));
    const end = Math.max(start + 1, Math.min(duration, Number(range.end_ms || start + 1)));
    state.video.selectedRange = {
      ...range,
      start_ms: Math.round(start),
      end_ms: Math.round(end),
      duration_ms: Math.round(end - start)
    };
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    seek(state.video.selectedRange.start_ms);
    if (shouldRender) renderVideoEditor();
    return state.video.selectedRange;
  }
  function updateRangeFromInputs() {
    const current = state.video.selectedRange || rangeFromTrack(selectedTrack());
    if (!current) return;
    const start = secondsInput(dom.videoRangeStartEl, Number(current.start_ms || 0) / 1000);
    const end = secondsInput(dom.videoRangeEndEl, Number(current.end_ms || 0) / 1000);
    setSelectedRange({
      ...current,
      start_ms: Math.round(start * 1000),
      end_ms: Math.round(end * 1000)
    });
  }
  function syncRangeInputs(range) {
    if (!dom.videoRangeStartEl || !dom.videoRangeEndEl) return;
    if (domInputLocked(dom.videoRangeStartEl) || domInputLocked(dom.videoRangeEndEl)) return;
    dom.videoRangeStartEl.value = range ? secondsValue(range.start_ms) : "0";
    dom.videoRangeEndEl.value = range ? secondsValue(range.end_ms) : "0";
    const maxSeconds = Math.max(0, Number(state.video.durationMs || 0) / 1000);
    dom.videoRangeStartEl.max = maxSeconds ? String(maxSeconds) : "";
    dom.videoRangeEndEl.max = maxSeconds ? String(maxSeconds) : "";
  }
  function currentVideoSourceSummary() {
    const editorSource = state.video.editor?.source_video;
    const renderSource = videoSourceSummary(state.video.renderSource);
    const source = editorSource || renderSource || {};
    return {
      src: source.src || "",
      filename: source.filename || source.name || "",
      duration_ms: Number(source.duration_ms || 0),
      width: Number(source.width || 0),
      height: Number(source.height || 0),
      source_start_ms: Number(source.source_start_ms || 0),
      source_end_ms: Number(source.source_end_ms || 0)
    };
  }
  return {
    install,
    applyOpenResult,
    applyProjectQueueManifest,
    applyProjectSemanticsManifest,
    render,
    selectClipRange,
    selectRangeFromTrack,
    setSelectedRange
  };
}
export function rangeFromClip(clip) {
  if (!clip) return null;
  return {
    clip_id: String(clip.id || "clip"),
    scene: String(clip.name || clip.id || "scene"),
    start_ms: Number(clip.start_ms || 0),
    end_ms: Number(clip.end_ms || Number(clip.start_ms || 0) + Number(clip.duration_ms || 0)),
    duration_ms: Number(clip.duration_ms || 0)
  };
}
function rangeFromTrack(track) {
  if (!track) return null;
  return {
    clip_id: String(track.clip_id || "composition"),
    track_id: String(track.id || ""),
    scene: String(track.label || track.clip_id || track.id || "selected track"),
    start_ms: Number(track.start_ms || 0),
    end_ms: Number(track.end_ms || Number(track.start_ms || 0) + Number(track.duration_ms || 0)),
    duration_ms: Number(track.duration_ms || 0)
  };
}
function outputFilenameForRange(range) {
  return `clip-${safeSlug(range.clip_id)}-${Math.round(Number(range.start_ms || 0))}-${Math.round(Number(range.end_ms || 0))}.mp4`;
}
function outputPathForProposal(compositionPath, filename) {
  if (globalThis.CAPY_VIDEO_EXPORT_DIR) {
    return `${String(globalThis.CAPY_VIDEO_EXPORT_DIR).replace(/\/+$/, "")}/${filename}`;
  }
  const normalized = String(compositionPath || "").replaceAll("\\", "/");
  const slash = normalized.lastIndexOf("/");
  const directory = slash >= 0 ? normalized.slice(0, slash) : ".";
  return `${directory}/exports/${filename}`;
}
function videoSourceSummary(source) {
  const tracks = Array.isArray(source?.tracks) ? source.tracks : [];
  for (const track of tracks) {
    const clips = Array.isArray(track.clips) ? track.clips : [];
    for (const clip of clips) {
      const params = clip.params || {};
      const kind = track.kind || params.track?.kind || "";
      if (kind !== "video" && !params.src) continue;
      return {
        src: params.src || "",
        filename: params.filename || "",
        duration_ms: Number(params.duration_ms || 0),
        width: Number(params.width || 0),
        height: Number(params.height || 0),
        source_start_ms: Number(params.source_start_ms || 0),
        source_end_ms: Number(params.source_end_ms || 0)
      };
    }
  }
  return null;
}
function secondsInput(input, fallback) {
  const value = Number(input?.value || fallback || 0);
  return Number.isFinite(value) ? Math.max(0, value) : fallback;
}
function secondsValue(ms) {
  const seconds = Number(ms || 0) / 1000;
  return seconds.toFixed(seconds >= 10 ? 1 : 2).replace(/\.?0+$/, "");
}
function domInputLocked(input) {
  return document.activeElement === input;
}
function safeSlug(value) {
  return String(value || "clip")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    || "clip";
}
function formatTime(ms) {
  const seconds = Number(ms || 0) / 1000;
  return `${seconds.toFixed(seconds >= 10 ? 1 : 2)}s`;
}
function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
