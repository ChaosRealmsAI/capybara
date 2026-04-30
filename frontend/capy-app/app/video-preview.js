import { createComponentRuntime, destroyMounted } from "./component-runtime.js";

export function createVideoPreviewController({ state, dom, stringifyError }) {
  const componentRuntime = createComponentRuntime();
  const previewMounted = new Map();
  let previewRenderToken = 0;

  async function renderPreviewFrame() {
    const host = dom.videoPreviewEl;
    const source = state.video.renderSource;
    const token = ++previewRenderToken;
    if (!host) return;
    if (!source) {
      resetPreviewRuntime();
      host.innerHTML = `<div class="video-preview-placeholder">打开 composition.json 后预览</div>`;
      host.dataset.previewReady = "false";
      return;
    }
    try {
      if (host.querySelector(".video-preview-placeholder") && previewMounted.size === 0) {
        host.replaceChildren();
      }
      host.style.background = source.theme?.background || "#080706";
      const stage = ensurePreviewStage(host, source);
      const active = new Set();
      const tracks = Array.isArray(source.tracks) ? source.tracks.slice() : [];
      tracks.sort((left, right) => Number(left.z || 0) - Number(right.z || 0));
      for (const track of tracks) {
        const clips = Array.isArray(track.clips) ? track.clips : [];
        for (const clip of clips) {
          const begin = clipStart(clip);
          const end = clipEnd(clip);
          if (state.video.playheadMs < begin || state.video.playheadMs > end) continue;
          const params = clip.params || {};
          const componentId = params.component;
          if (!componentId) continue;
          const key = `${state.video.renderSourcePath}::${track.id}::${clip.id}`;
          active.add(key);
          let entry = previewMounted.get(key);
          if (!entry) {
            const el = document.createElement("div");
            el.className = "video-preview-layer";
            el.dataset.trackId = String(track.id || "");
            el.dataset.clipId = String(clip.id || "");
            el.style.zIndex = String(Number(track.z || 0));
            stage.appendChild(el);
            const module = await previewComponentModule(source, componentId);
            if (token !== previewRenderToken) return;
            entry = { el, module };
            previewMounted.set(key, entry);
            entry.module.mount && entry.module.mount(entry.el, clipContext(source, track, clip));
          }
          entry.module.update && entry.module.update(entry.el, clipContext(source, track, clip));
        }
      }
      for (const [key, entry] of previewMounted) {
        if (active.has(key)) continue;
        entry.module.destroy && entry.module.destroy(entry.el);
        entry.el.remove();
        previewMounted.delete(key);
      }
      host.dataset.previewReady = "true";
      host.dataset.previewError = "";
      host.dataset.currentTimeMs = String(state.video.playheadMs || 0);
    } catch (error) {
      resetPreviewRuntime();
      host.dataset.previewReady = "false";
      host.dataset.previewError = stringifyError(error);
      host.innerHTML = `<div class="video-preview-placeholder">${escapeHtml(stringifyError(error))}</div>`;
    }
  }

  function previewComponentModule(source, id) {
    const sourceText = source.components && source.components[id];
    return componentRuntime.loadModule(`${state.video.renderSourcePath}::${id}`, sourceText);
  }

  function ensurePreviewStage(host, source) {
    const viewport = source.viewport || {};
    const width = Number(viewport.w || viewport.width || 1920);
    const height = Number(viewport.h || viewport.height || 1080);
    let stage = host.querySelector(".video-preview-stage");
    if (!stage) {
      stage = document.createElement("div");
      stage.className = "video-preview-stage";
      host.appendChild(stage);
    }
    const scale = Math.min(
      Math.max(1, host.clientWidth) / width,
      Math.max(1, host.clientHeight) / height
    );
    stage.style.width = `${width}px`;
    stage.style.height = `${height}px`;
    stage.style.transform = `translate(-50%, -50%) scale(${scale})`;
    return stage;
  }

  function clipContext(source, track, clip) {
    const begin = clipStart(clip);
    const end = clipEnd(clip);
    const duration = Math.max(1, end - begin);
    const localTime = Math.max(0, Math.min(duration, (state.video.playheadMs || 0) - begin));
    const params = clip.params || {};
    return {
      timeMs: state.video.playheadMs || 0,
      localTimeMs: localTime,
      progress: localTime / duration,
      durationMs: duration,
      params: params.params || params,
      style: params.style || {},
      track: params.track || { id: track.id, kind: track.kind },
      theme: source.theme || {},
      viewport: source.viewport || {},
      mode: "preview"
    };
  }

  function clipStart(clip) {
    return Number(clip.begin_ms ?? clip.begin ?? 0);
  }

  function clipEnd(clip) {
    return Number(clip.end_ms ?? clip.end ?? clipStart(clip));
  }

  function resetPreviewRuntime() {
    destroyMounted(previewMounted);
    componentRuntime.clear();
    dom.videoPreviewEl?.replaceChildren();
  }

  return { renderPreviewFrame, resetPreviewRuntime };
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
