import { createComponentRuntime, destroyMounted } from "./component-runtime.js";

export function createPosterPreviewController({ state, dom, stringifyError, currentPage }) {
  const componentRuntime = createComponentRuntime();
  const mounted = new Map();
  let renderToken = 0;

  async function renderStage() {
    const host = dom.posterPreviewEl;
    const document = state.posterWorkspace.document;
    const page = currentPage();
    const token = ++renderToken;
    if (!host) return;
    if (!document || !page) {
      resetRuntime();
      host.innerHTML = `<div class="poster-workspace-placeholder">打开 JSON 后预览</div>`;
      host.dataset.previewReady = "false";
      return;
    }
    try {
      destroyMounted(mounted);
      host.replaceChildren();
      host.style.background = page.background || document.theme?.background || "#fffaf0";
      const stage = globalThis.document.createElement("div");
      stage.className = "poster-workspace-stage";
      stage.dataset.pageId = page.id;
      const viewport = document.viewport || {};
      const width = Number(viewport.w || viewport.width || 1920);
      const height = Number(viewport.h || viewport.height || 1080);
      const scale = Math.min(Math.max(1, host.clientWidth) / width, Math.max(1, host.clientHeight) / height);
      stage.style.width = `${width}px`;
      stage.style.height = `${height}px`;
      stage.style.transform = `translate(-50%, -50%) scale(${scale})`;
      stage.style.background = page.background || document.theme?.background || "#fffaf0";
      host.appendChild(stage);

      const active = new Set();
      const layers = (page.layers || []).filter((layer) => layer.visible !== false);
      layers.sort((left, right) => Number(left.z || 0) - Number(right.z || 0));
      for (const layer of layers) {
        const key = `${page.id}::${layer.id}`;
        active.add(key);
        const el = globalThis.document.createElement("div");
        el.className = "poster-workspace-layer";
        el.dataset.layerId = layer.id;
        el.dataset.kind = layer.kind || "";
        el.dataset.selected = layer.id === state.posterWorkspace.layerPath ? "true" : "false";
        applyLayerBox(el, layer);
        if (layer.kind === "component") {
          stage.appendChild(el);
          const component = document.components?.[layer.component];
          const module = await componentRuntime.loadComponent(
            `${state.posterWorkspace.path || document.id}::${layer.component}`,
            component,
            state.posterWorkspace.path || globalThis.location?.href || "",
          );
          if (token !== renderToken) return;
          mounted.set(key, { el, module });
          module.mount && module.mount(el, componentContext(document, page, layer));
          module.update && module.update(el, componentContext(document, page, layer));
        } else {
          renderStaticLayer(el, document, layer);
          stage.appendChild(el);
        }
      }
      for (const [key, entry] of mounted) {
        if (active.has(key)) continue;
        entry.module?.destroy && entry.module.destroy(entry.el);
        entry.el.remove();
        mounted.delete(key);
      }
      host.dataset.previewReady = "true";
      host.dataset.previewError = "";
    } catch (error) {
      resetRuntime();
      host.dataset.previewReady = "false";
      host.dataset.previewError = stringifyError(error);
      host.innerHTML = `<div class="poster-workspace-placeholder">${escapeHtml(stringifyError(error))}</div>`;
    }
  }

  function resetRuntime() {
    destroyMounted(mounted);
    componentRuntime.clear();
    dom.posterPreviewEl?.replaceChildren();
  }

  return { renderStage, resetRuntime };
}

function renderStaticLayer(el, document, layer) {
  const style = layer.style || {};
  if (layer.kind === "text") {
    el.textContent = layer.text || "";
    el.style.display = "flex";
    el.style.alignItems = style.alignItems || "flex-start";
    el.style.justifyContent = style.justifyContent || "flex-start";
    el.style.whiteSpace = "pre-line";
    el.style.lineHeight = String(style.lineHeight || 1.08);
  } else if (layer.kind === "image") {
    const asset = document.assets?.[layer.asset_ref || layer.assetId];
    const image = globalThis.document.createElement("img");
    image.src = asset?.src || "";
    image.alt = layer.id;
    image.style.width = "100%";
    image.style.height = "100%";
    image.style.objectFit = style.objectFit || "contain";
    el.appendChild(image);
  }
}

function applyLayerBox(el, layer) {
  const b = bounds(layer);
  const style = layer.style || {};
  el.style.left = `${b.x}px`;
  el.style.top = `${b.y}px`;
  el.style.width = `${b.w}px`;
  el.style.height = `${b.h}px`;
  el.style.zIndex = String(Number(layer.z || 0));
  el.style.background = style.fill || "transparent";
  el.style.color = style.color || "#1c1917";
  el.style.fontSize = style.fontSize ? `${Number(style.fontSize)}px` : "";
  el.style.fontWeight = style.fontWeight ? String(style.fontWeight) : "";
  el.style.fontFamily = style.fontFamily || "'PingFang SC', 'Source Han Sans CN', sans-serif";
  el.style.borderRadius = style.radius !== undefined ? `${Number(style.radius)}px` : "";
  if (style.borderColor) {
    el.style.border = `${Number(style.borderWidth || 2)}px solid ${style.borderColor}`;
  }
}

function componentContext(document, page, layer) {
  return {
    mode: "poster-preview",
    progress: 0,
    params: layer.params || {},
    style: layer.style || {},
    theme: document.theme || {},
    viewport: document.viewport || {},
    surface: { kind: "poster", page: { id: page.id }, layer: { id: layer.id } },
    page: { id: page.id, title: page.title || page.id },
    layer: { id: layer.id, kind: layer.kind },
  };
}

function bounds(layer) {
  const raw = layer.bounds || layer;
  return {
    x: Number(raw.x || 0),
    y: Number(raw.y || 0),
    w: Number(raw.w ?? raw.width ?? 0),
    h: Number(raw.h ?? raw.height ?? 0),
  };
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
