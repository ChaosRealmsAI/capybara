export function createVideoClipDeliveryController(ctx) {
  const { state, dom, exportComposition, seek, renderVideoEditor, selectedTrack, firstTrackForClip } = ctx;

  function install() {
    dom.videoProposalGenerateEl?.addEventListener("click", () => generateClipProposal());
  }

  function applyOpenResult(clips) {
    const selectedClipId = state.video.selectedRange?.clip_id;
    const clip = clips.find((item) => item.id === selectedClipId) || clips[0] || null;
    state.video.selectedRange = clip ? rangeFromClip(clip) : null;
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    state.video.lastExport = null;
  }

  function render() {
    const range = state.video.selectedRange;
    if (dom.videoRangeSummaryEl) {
      dom.videoRangeSummaryEl.textContent = range
        ? `${range.scene || range.clip_id} · ${formatTime(range.start_ms)} - ${formatTime(range.end_ms)}`
        : "未选择片段";
    }
    if (!dom.videoProposalEl) return;
    const proposal = state.video.clipProposal;
    if (!state.video.editor) {
      dom.videoProposalEl.textContent = "打开 composition.json 后选择 scene";
      return;
    }
    if (!proposal) {
      dom.videoProposalEl.innerHTML = range
        ? `<p>已选择 ${escapeHtml(range.scene || range.clip_id)}，可生成片段 proposal。</p>`
        : "<p>选择左侧 scene 后生成可交付片段 proposal。</p>";
      return;
    }
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
    state.video.selectedRange = rangeFromClip(clip);
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    const track = firstTrackForClip(clip.id);
    if (track) state.video.selectedTrackId = track.id;
    seek(Number(clip.start_ms || 0));
    renderVideoEditor();
  }

  function selectRangeFromTrack(track) {
    state.video.selectedRange = rangeFromTrack(track);
    state.video.clipProposal = null;
    state.video.proposalStatus = "idle";
    seek(Number(track.start_ms || 0));
    renderVideoEditor();
  }

  function generateClipProposal() {
    const range = state.video.selectedRange || rangeFromTrack(selectedTrack());
    if (!range || !state.video.compositionPath) return;
    const outputFilename = outputFilenameForRange(range);
    state.video.selectedRange = range;
    state.video.clipProposal = {
      id: `proposal-${safeSlug(range.clip_id)}-${range.start_ms}-${range.end_ms}`,
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
        render_source_path: state.video.renderSourcePath
      }
    };
    state.video.proposalStatus = "ready";
    render();
  }

  function confirmClipProposal() {
    if (!state.video.clipProposal) generateClipProposal();
    const proposal = state.video.clipProposal;
    if (!proposal) return;
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

  return { install, applyOpenResult, render, selectClipRange, selectRangeFromTrack };
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
