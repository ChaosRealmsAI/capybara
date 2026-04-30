import { createVideoPreviewController } from "./video-preview.js";

export function createVideoEditor(ctx) {
  const {
    state,
    dom,
    rpc,
    stringifyError,
    setRunStatus,
    renderPosterWorkspace,
    ensurePosterDocument,
    renderGameAssetsWorkspace,
    ensureGameAssetsPack,
  } = ctx;
  const preview = createVideoPreviewController({ state, dom, stringifyError });

  function installVideoEditor() {
    dom.workspaceTabs.forEach((button) => {
      button.addEventListener("click", () => switchWorkspace(button.dataset.workspaceTab || "canvas"));
    });
    dom.videoOpenEl?.addEventListener("click", () => {
      const path = dom.videoPathEl?.value?.trim();
      if (path) openComposition(path);
    });
    dom.videoSampleEl?.addEventListener("click", () => {
      const root = window.CAPYBARA_SESSION?.cwd || "/Users/Zhuanz/workspace/capybara";
      if (dom.videoPathEl) {
        dom.videoPathEl.value = `${root}/fixtures/timeline/video-editing/compositions/main.json`;
      }
      openComposition(dom.videoPathEl.value);
    });
    dom.videoPlayheadEl?.addEventListener("input", () => {
      const value = Number(dom.videoPlayheadEl.value || 0);
      seek(value);
    });
    dom.videoFieldSaveEl?.addEventListener("click", () => saveSelectedField());
    dom.videoExportEl?.addEventListener("click", () => exportComposition());
    dom.videoRecordEl?.addEventListener("click", () => exportComposition({ mode: "record" }));
    window.addEventListener("capy:timeline-composition-opened", (event) => {
      applyOpenResult(event.detail);
      switchWorkspace("video");
    });
    switchWorkspace(state.workspace.activeTab);
    renderVideoEditor();
  }

  function switchWorkspace(tab) {
    state.workspace.activeTab = ["video", "poster", "game-assets"].includes(tab) ? tab : "canvas";
    const videoActive = state.workspace.activeTab === "video";
    const posterActive = state.workspace.activeTab === "poster";
    const gameAssetsActive = state.workspace.activeTab === "game-assets";
    dom.workspaceTabs.forEach((button) => {
      const active = button.dataset.workspaceTab === state.workspace.activeTab;
      button.classList.toggle("active", active);
      button.setAttribute("aria-selected", active ? "true" : "false");
    });
    if (dom.brandSubtitleEl) {
      dom.brandSubtitleEl.textContent = videoActive
        ? "视频剪辑"
        : posterActive
          ? "海报"
          : gameAssetsActive
            ? "游戏素材"
            : "Canvas";
    }
    if (videoActive && dom.timelineInspectorEl) {
      state.workspace.timelineInspectorWasOpen = !dom.timelineInspectorEl.hidden;
    }
    dom.canvasPanelEl.hidden = videoActive || posterActive || gameAssetsActive;
    dom.plannerEl.hidden = videoActive || posterActive || gameAssetsActive;
    if (dom.timelineInspectorEl) {
      dom.timelineInspectorEl.hidden = videoActive || posterActive || gameAssetsActive
        ? true
        : !state.workspace.timelineInspectorWasOpen;
    }
    if (dom.videoEditorEl) {
      dom.videoEditorEl.hidden = !videoActive;
    }
    if (dom.posterWorkspaceEl) {
      dom.posterWorkspaceEl.hidden = !posterActive;
    }
    if (dom.gameAssetsWorkspaceEl) {
      dom.gameAssetsWorkspaceEl.hidden = !gameAssetsActive;
    }
    if (videoActive) {
      window.requestAnimationFrame(() => renderVideoEditor());
    }
    if (posterActive) {
      window.requestAnimationFrame(() => {
        if (state.posterWorkspace?.document) {
          renderPosterWorkspace && renderPosterWorkspace();
        } else {
          ensurePosterDocument && ensurePosterDocument();
        }
      });
    }
    if (gameAssetsActive) {
      window.requestAnimationFrame(() => {
        if (state.gameAssets?.pack) {
          renderGameAssetsWorkspace && renderGameAssetsWorkspace();
        } else {
          ensureGameAssetsPack && ensureGameAssetsPack();
        }
      });
    }
  }

  async function openComposition(path) {
    state.video.status = "loading";
    state.video.error = null;
    renderVideoEditor();
    try {
      const result = await rpc("timeline-composition-open", { composition_path: path });
      applyOpenResult(result);
      switchWorkspace("video");
    } catch (error) {
      state.video.status = "error";
      state.video.error = stringifyError(error);
      renderVideoEditor();
    }
  }

  function applyOpenResult(result) {
    state.video.status = "ready";
    state.video.error = null;
    state.video.compositionPath = result.composition_path || "";
    state.video.renderSourcePath = result.render_source_path || "";
    if (state.video.previewSourceKey !== state.video.renderSourcePath) {
      preview.resetPreviewRuntime();
      state.video.previewSourceKey = state.video.renderSourcePath;
    }
    state.video.renderSource = result.render_source || null;
    state.video.previewUrl = result.preview_url || "";
    state.video.editor = result.editor || null;
    state.video.durationMs = Number(result.editor?.duration_ms || 0);
    state.video.playheadMs = Math.min(state.video.playheadMs || 0, state.video.durationMs || 0);
    const tracks = Array.isArray(result.editor?.tracks) ? result.editor.tracks : [];
    if (!state.video.selectedTrackId && tracks[0]) {
      state.video.selectedTrackId = tracks[0].id;
    }
    if (dom.videoPathEl && state.video.compositionPath) {
      dom.videoPathEl.value = state.video.compositionPath;
    }
    renderVideoEditor();
  }

  function renderVideoEditor() {
    renderSummary();
    renderClips();
    renderTimeline();
    renderInspector();
    renderExportState();
    preview.renderPreviewFrame();
  }

  function renderSummary() {
    if (!dom.videoStatusEl) return;
    const editor = state.video.editor;
    if (state.video.status === "error") {
      dom.videoStatusEl.textContent = state.video.error || "加载失败";
      dom.videoStatusEl.dataset.status = "error";
      return;
    }
    if (!editor) {
      dom.videoStatusEl.textContent = "等待 composition.json";
      dom.videoStatusEl.dataset.status = "idle";
      return;
    }
    dom.videoStatusEl.textContent = `${editor.name || "Composition"} · ${formatTime(editor.duration_ms || 0)} · ${editor.tracks?.length || 0} tracks`;
    dom.videoStatusEl.dataset.status = state.video.status;
  }

  function renderClips() {
    if (!dom.videoClipsEl) return;
    const clips = Array.isArray(state.video.editor?.clips) ? state.video.editor.clips : [];
    dom.videoClipsEl.replaceChildren(...clips.map((clip) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "video-clip-row";
      button.innerHTML = `<span>${escapeHtml(clip.name || clip.id)}</span><small>${formatTime(clip.start_ms)} - ${formatTime(clip.end_ms)}</small>`;
      button.addEventListener("click", () => seek(Number(clip.start_ms || 0)));
      return button;
    }));
  }

  function renderTimeline() {
    if (!dom.videoTimelineEl) return;
    const tracks = Array.isArray(state.video.editor?.tracks) ? state.video.editor.tracks : [];
    const duration = Math.max(1, Number(state.video.editor?.duration_ms || 1));
    dom.videoTimelineEl.replaceChildren(...tracks.map((track) => {
      const row = document.createElement("div");
      row.className = "video-track-row";
      row.dataset.selected = track.id === state.video.selectedTrackId ? "true" : "false";
      const label = document.createElement("button");
      label.type = "button";
      label.className = "video-track-label";
      label.textContent = track.local_id || track.id;
      label.addEventListener("click", () => selectTrack(track.id));
      const lane = document.createElement("div");
      lane.className = "video-track-lane";
      const clip = document.createElement("button");
      clip.type = "button";
      clip.className = "video-track-clip";
      clip.textContent = track.label || track.kind || "track";
      clip.style.left = `${(Number(track.start_ms || 0) / duration) * 100}%`;
      clip.style.width = `${Math.max(3, (Number(track.duration_ms || 0) / duration) * 100)}%`;
      clip.addEventListener("click", () => selectTrack(track.id));
      lane.appendChild(clip);
      row.append(label, lane);
      return row;
    }));
    const playhead = Math.max(0, Math.min(100, (Number(state.video.playheadMs || 0) / duration) * 100));
    dom.videoTimelineEl.style.setProperty("--video-playhead", `${playhead}%`);
    if (dom.videoPlayheadEl) {
      dom.videoPlayheadEl.max = String(duration);
      dom.videoPlayheadEl.value = String(state.video.playheadMs || 0);
    }
    if (dom.videoTimeEl) {
      dom.videoTimeEl.textContent = `${formatTime(state.video.playheadMs || 0)} / ${formatTime(duration)}`;
    }
  }

  function renderInspector() {
    if (!dom.videoInspectorEl) return;
    const track = selectedTrack();
    if (!track) {
      dom.videoInspectorEl.innerHTML = `<div class="video-empty">选择一个轨道</div>`;
      return;
    }
    const fields = Array.isArray(track.fields) ? track.fields : [];
    const first = selectedField(track) || fields[0] || null;
    dom.videoInspectorEl.innerHTML = `
      <div class="video-inspector-kv"><span>Track</span><strong>${escapeHtml(track.id)}</strong></div>
      <div class="video-inspector-kv"><span>Kind</span><strong>${escapeHtml(track.kind || "component")}</strong></div>
      <label class="video-field-label">
        <span>Field</span>
        <select id="video-field-select"></select>
      </label>
      <label class="video-field-label">
        <span>Value</span>
        <input id="video-field-value">
      </label>
    `;
    const select = dom.videoInspectorEl.querySelector("#video-field-select");
    const input = dom.videoInspectorEl.querySelector("#video-field-value");
    if (select) {
      select.replaceChildren(...fields.map((field) => {
        const option = document.createElement("option");
        option.value = field.field;
        option.textContent = field.field;
        option.selected = first && field.field === first.field;
        return option;
      }));
      select.addEventListener("change", () => {
        state.video.selectedField = select.value;
        renderInspector();
      });
    }
    if (input && first) input.value = primitiveToInput(first.value);
  }

  function renderExportState() {
    if (!dom.videoExportStatusEl) return;
    const job = state.video.exportJob;
    if (!job) {
      dom.videoExportStatusEl.textContent = "未导出";
      return;
    }
    dom.videoExportStatusEl.textContent = `${job.status || "unknown"} · ${job.output_path || ""}`;
  }

  function selectTrack(id) {
    state.video.selectedTrackId = id;
    state.video.selectedField = "";
    renderVideoEditor();
  }

  function selectedTrack() {
    const tracks = Array.isArray(state.video.editor?.tracks) ? state.video.editor.tracks : [];
    return tracks.find((track) => track.id === state.video.selectedTrackId) || tracks[0] || null;
  }

  function selectedField(track) {
    const fields = Array.isArray(track?.fields) ? track.fields : [];
    return fields.find((field) => field.field === state.video.selectedField) || null;
  }

  async function saveSelectedField() {
    const track = selectedTrack();
    if (!track || !state.video.compositionPath) return;
    const select = dom.videoInspectorEl?.querySelector("#video-field-select");
    const input = dom.videoInspectorEl?.querySelector("#video-field-value");
    const field = select?.value;
    if (!field || !input) return;
    state.video.status = "saving";
    renderSummary();
    try {
      const result = await rpc("timeline-composition-patch", {
        composition_path: state.video.compositionPath,
        track_id: track.id,
        field,
        value: coerceInputValue(input.value)
      });
      applyOpenResult(result);
    } catch (error) {
      state.video.status = "error";
      state.video.error = stringifyError(error);
      renderVideoEditor();
    }
  }

  async function exportComposition(options = {}) {
    if (!state.video.compositionPath) return;
    state.video.exportJob = { status: options.mode === "record" ? "recording" : "exporting" };
    renderExportState();
    try {
      const exportResolution = state.video.renderSource?.meta?.export?.resolution || "";
      const is4k = exportResolution === "4k" || Number(state.video.renderSource?.viewport?.w || 0) >= 3840;
      const result = await rpc("timeline-export-start", {
        composition_path: state.video.compositionPath,
        fps: 30,
        profile: is4k ? "final" : "draft",
        resolution: is4k ? "4k" : exportResolution || undefined,
        parallel: is4k ? 2 : undefined,
        strict_recorder: is4k || options.mode === "record"
      });
      state.video.exportJob = result.job || null;
      renderExportState();
      setRunStatus("idle");
    } catch (error) {
      state.video.exportJob = { status: "failed", output_path: stringifyError(error) };
      renderExportState();
      setRunStatus("error");
    }
  }

  function seek(value) {
    state.video.playheadMs = Math.max(0, Math.min(Number(value || 0), state.video.durationMs || Number.MAX_SAFE_INTEGER));
    renderTimeline();
    preview.renderPreviewFrame();
  }

  return {
    installVideoEditor,
    switchWorkspace,
    openComposition,
    renderVideoEditor,
  };
}

function primitiveToInput(value) {
  if (value === null || value === undefined) return "";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

function coerceInputValue(value) {
  const trimmed = String(value || "").trim();
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (trimmed && !Number.isNaN(Number(trimmed))) return Number(trimmed);
  return value;
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
