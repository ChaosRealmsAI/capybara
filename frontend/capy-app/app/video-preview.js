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
          const trackKind = track.kind || params.track?.kind || "";
          if (trackKind === "video" || params.src) {
            const key = `${state.video.renderSourcePath}::${track.id}::${clip.id}`;
            active.add(key);
            let entry = previewMounted.get(key);
            if (!entry) {
              entry = mountVideoLayer(stage, source, track, clip);
              previewMounted.set(key, entry);
            }
            updateVideoLayer(entry, source, track, clip);
            continue;
          }
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
    const definition = source.components && source.components[id];
    return componentRuntime.loadComponent(
      `${state.video.renderSourcePath}::${id}`,
      definition,
      state.video.renderSourcePath || globalThis.location?.href || "",
    );
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
      surface: { kind: "video", track: { id: track.id }, clip: { id: clip.id } },
      mode: "preview"
    };
  }

  function mountVideoLayer(stage, source, track, clip) {
    const el = document.createElement("div");
    el.className = "video-preview-layer";
    el.dataset.trackId = String(track.id || "");
    el.dataset.clipId = String(clip.id || "");
    el.style.zIndex = String(Number(track.z || 0));
    const video = document.createElement("video");
    video.className = "video-preview-video";
    video.muted = true;
    video.playsInline = true;
    video.preload = "auto";
    video.controls = false;
    video.addEventListener("loadeddata", () => {
      dom.videoPreviewEl?.setAttribute("data-video-ready", "true");
    });
    video.addEventListener("error", () => {
      dom.videoPreviewEl?.setAttribute("data-video-error", video.error?.message || "video load failed");
    });
    el.append(video);
    stage.appendChild(el);
    const entry = {
      el,
      video,
      sourceKey: "",
      module: {
        destroy() {
          video.pause();
          video.removeAttribute("src");
          video.load();
        }
      }
    };
    updateVideoLayer(entry, source, track, clip);
    return entry;
  }

  function updateVideoLayer(entry, source, track, clip) {
    const params = clip.params || {};
    const src = normalizeVideoSrc(params.src || "");
    if (entry.sourceKey !== src) {
      entry.sourceKey = src;
      entry.video.src = src;
      entry.video.load();
    }
    const begin = clipStart(clip);
    const end = clipEnd(clip);
    const duration = Math.max(1, end - begin);
    const localTime = Math.max(0, Math.min(duration, (state.video.playheadMs || 0) - begin));
    const sourceStart = Number(params.source_start_ms || 0);
    const nextTime = (sourceStart + localTime) / 1000;
    if (Number.isFinite(nextTime) && Math.abs((entry.video.currentTime || 0) - nextTime) > 0.08) {
      try {
        entry.video.currentTime = nextTime;
      } catch {
        entry.video.dataset.pendingTime = String(nextTime);
      }
    }
    entry.video.pause();
    entry.el.style.zIndex = String(Number(track.z || 0));
    entry.video.style.objectFit = source?.meta?.video_fit || "contain";
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

function normalizeVideoSrc(src) {
  const value = String(src || "");
  if (!value) return "";
  if (/^file:/i.test(value)) {
    const localPath = decodeFileUrl(value);
    return workspaceRelativeUrl(localPath) || value;
  }
  if (/^(https?|data|blob):/i.test(value)) return value;
  const workspace = workspaceRelativeUrl(value);
  if (workspace) return workspace;
  if (value.startsWith("/")) return `file://${encodePath(value)}`;
  return value;
}

function workspaceRelativeUrl(path) {
  const cwd = String(window.CAPYBARA_SESSION?.cwd || "").replace(/\/+$/, "");
  if (!cwd || !path.startsWith(`${cwd}/`)) return "";
  return `/${path.slice(cwd.length + 1).split("/").map(encodeURIComponent).join("/")}`;
}

function decodeFileUrl(value) {
  try {
    return decodeURIComponent(value.replace(/^file:\/\//i, ""));
  } catch {
    return value.replace(/^file:\/\//i, "");
  }
}

function encodePath(path) {
  return path
    .split("/")
    .map((part, index) => index === 0 ? "" : encodeURIComponent(part))
    .join("/");
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
