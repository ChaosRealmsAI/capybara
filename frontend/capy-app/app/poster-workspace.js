import { createPosterPreviewController } from "./poster-preview.js";

const SAMPLE_PATHS = {
  single: "/fixtures/poster/v1/single-poster.json",
  deck: "/fixtures/poster/v1/ppt-deck.json",
  shared: "/fixtures/poster/v1/shared-component.json",
};

export function createPosterWorkspace(ctx) {
  const { state, dom, stringifyError } = ctx;
  let resizeFrame = 0;
  const preview = createPosterPreviewController({ state, dom, stringifyError, currentPage });

  function installPosterWorkspace() {
    dom.posterOpenEl?.addEventListener("click", () => {
      const path = dom.posterPathEl?.value?.trim();
      if (path) openDocument(path);
    });
    dom.posterSampleSingleEl?.addEventListener("click", () => openDocument(SAMPLE_PATHS.single));
    dom.posterSampleDeckEl?.addEventListener("click", () => openDocument(SAMPLE_PATHS.deck));
    dom.posterSampleSharedEl?.addEventListener("click", () => openDocument(SAMPLE_PATHS.shared));
    dom.posterFieldSaveEl?.addEventListener("click", () => saveInspectorFields());
    dom.posterExportPngEl?.addEventListener("click", () => markExport("png"));
    dom.posterExportPdfEl?.addEventListener("click", () => markExport("pdf"));
    dom.posterVerifyEl?.addEventListener("click", () => verifyWorkspace());
    window.addEventListener("resize", () => {
      if (state.workspace?.activeTab !== "poster") return;
      window.cancelAnimationFrame(resizeFrame);
      resizeFrame = window.requestAnimationFrame(() => renderPosterWorkspace());
    });
    renderPosterWorkspace();
  }

  function ensureDefaultDocument() {
    if (state.posterWorkspace.document || state.posterWorkspace.status !== "idle") return null;
    return openDocument(dom.posterPathEl?.value?.trim() || SAMPLE_PATHS.single);
  }

  async function openDocument(path) {
    state.posterWorkspace.status = "loading";
    state.posterWorkspace.error = "";
    state.posterWorkspace.exportStatus = "";
    renderPosterWorkspace();
    try {
      const response = await fetch(path);
      if (!response.ok) throw new Error(`load ${path} failed: ${response.status}`);
      const document = await response.json();
      validateDocument(document);
      state.posterWorkspace.path = path;
      state.posterWorkspace.document = document;
      state.posterWorkspace.pageId = firstPage(document)?.id || "";
      state.posterWorkspace.layerPath = firstLayer(firstPage(document))?.id || "";
      state.posterWorkspace.status = "ready";
      state.posterWorkspace.error = "";
      state.posterWorkspace.exportStatus = "";
      if (dom.posterPathEl) dom.posterPathEl.value = path;
      preview.resetRuntime();
      renderPosterWorkspace();
    } catch (error) {
      state.posterWorkspace.status = "error";
      state.posterWorkspace.error = stringifyError(error);
      renderPosterWorkspace();
    }
  }

  function renderPosterWorkspace() {
    renderStatus();
    renderPages();
    renderLayers();
    renderInspector();
    renderSource();
    preview.renderStage();
  }

  function renderStatus() {
    if (!dom.posterStatusEl) return;
    const document = state.posterWorkspace.document;
    if (state.posterWorkspace.status === "error") {
      dom.posterStatusEl.textContent = state.posterWorkspace.error || "加载失败";
      dom.posterStatusEl.dataset.status = "error";
    } else if (!document) {
      dom.posterStatusEl.textContent = "等待 JSON 文档";
      dom.posterStatusEl.dataset.status = "idle";
    } else {
      const pageCount = document.pages?.length || 0;
      const layerCount = currentPage()?.layers?.length || 0;
      dom.posterStatusEl.textContent = `${document.title || document.id} · ${pageCount} pages · ${layerCount} layers`;
      dom.posterStatusEl.dataset.status = state.posterWorkspace.status;
    }
    if (dom.posterExportStatusEl) {
      dom.posterExportStatusEl.textContent = state.posterWorkspace.exportStatus || "未导出";
    }
  }

  function renderPages() {
    if (!dom.posterPagesEl) return;
    const pages = state.posterWorkspace.document?.pages || [];
    dom.posterPagesEl.replaceChildren(...pages.map((page, index) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "poster-page-row";
      button.dataset.selected = page.id === state.posterWorkspace.pageId ? "true" : "false";
      button.innerHTML = `<span>${escapeHtml(page.title || page.id)}</span><small>${index + 1}</small>`;
      button.addEventListener("click", () => {
        state.posterWorkspace.pageId = page.id;
        state.posterWorkspace.layerPath = firstLayer(page)?.id || "";
        preview.resetRuntime();
        renderPosterWorkspace();
      });
      return button;
    }));
  }

  function renderLayers() {
    if (!dom.posterLayersEl) return;
    const page = currentPage();
    const layers = Array.isArray(page?.layers) ? page.layers.slice() : [];
    layers.sort((left, right) => Number(right.z || 0) - Number(left.z || 0));
    dom.posterLayersEl.replaceChildren(...layers.map((layer) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "poster-layer-row";
      button.dataset.selected = layer.id === state.posterWorkspace.layerPath ? "true" : "false";
      button.innerHTML = `<span>${escapeHtml(layer.id)}</span><small>${escapeHtml(layer.kind || "layer")}</small>`;
      button.addEventListener("click", () => {
        state.posterWorkspace.layerPath = layer.id;
        renderPosterWorkspace();
      });
      return button;
    }));
  }

  function renderInspector() {
    if (!dom.posterInspectorEl) return;
    const layer = selectedLayer();
    if (!layer) {
      dom.posterInspectorEl.innerHTML = `<div class="poster-empty">选择一个 layer</div>`;
      return;
    }
    const b = bounds(layer);
    const style = layer.style || {};
    const params = layer.params || {};
    dom.posterInspectorEl.innerHTML = `
      <div class="poster-inspector-kv"><span>Layer</span><strong>${escapeHtml(layer.id)}</strong></div>
      <div class="poster-inspector-kv"><span>Kind</span><strong>${escapeHtml(layer.kind)}</strong></div>
      ${layer.kind === "text" ? field("text", "Text", layer.text || "", "textarea") : ""}
      ${layer.kind === "component" ? field("component", "Component", layer.component || "") : ""}
      <div class="poster-field-grid">
        ${field("bounds.x", "X", b.x)}
        ${field("bounds.y", "Y", b.y)}
        ${field("bounds.w", "W", b.w)}
        ${field("bounds.h", "H", b.h)}
        ${field("z", "Z", layer.z || 0)}
        ${field("style.fontSize", "Font", style.fontSize || "")}
        ${field("style.fontWeight", "Weight", style.fontWeight || "")}
        ${field("style.radius", "Radius", style.radius || "")}
      </div>
      ${field("style.fill", "Fill", style.fill || "")}
      ${field("style.color", "Color", style.color || "")}
      ${componentParamFields(params)}
    `;
  }

  function renderSource() {
    if (!dom.posterSourceEl) return;
    dom.posterSourceEl.textContent = state.posterWorkspace.document
      ? JSON.stringify(state.posterWorkspace.document, null, 2)
      : "";
  }

  function saveInspectorFields() {
    const layer = selectedLayer();
    if (!layer || !dom.posterInspectorEl) return;
    for (const input of dom.posterInspectorEl.querySelectorAll("[data-poster-field]")) {
      setLayerValue(layer, input.dataset.posterField, inputValue(input));
    }
    state.posterWorkspace.status = "patched";
    state.posterWorkspace.exportStatus = "JSON 已更新 · 等待验证/导出";
    preview.resetRuntime();
    renderPosterWorkspace();
  }

  function verifyWorkspace() {
    const document = state.posterWorkspace.document;
    const page = currentPage();
    try {
      validateDocument(document);
      if (!selectedLayer()) throw new Error("missing selected layer");
      if (!dom.posterPreviewEl?.querySelector("[data-layer-id]")) throw new Error("preview has no layers");
      state.posterWorkspace.status = "verified";
      state.posterWorkspace.exportStatus = `${page.title || page.id} · ${page.layers.length} layers · verified`;
    } catch (error) {
      state.posterWorkspace.status = "error";
      state.posterWorkspace.error = stringifyError(error);
    }
    renderPosterWorkspace();
  }

  function markExport(kind) {
    state.posterWorkspace.exportStatus = `${kind.toUpperCase()} 导出入口已连接 · 文件导出走后续 shell adapter`;
    renderStatus();
  }

  function currentPage() {
    const document = state.posterWorkspace.document;
    const pages = document?.pages || [];
    return pages.find((page) => page.id === state.posterWorkspace.pageId) || pages[0] || null;
  }

  function selectedLayer() {
    const page = currentPage();
    return (page?.layers || []).find((layer) => layer.id === state.posterWorkspace.layerPath) || null;
  }

  return {
    ensureDefaultDocument,
    installPosterWorkspace,
    renderPosterWorkspace,
    openDocument,
  };
}

function validateDocument(document) {
  if (!document || document.schema !== "capy.poster.document.v1") {
    throw new Error("document schema must be capy.poster.document.v1");
  }
  if (!document.viewport || !(Number(document.viewport.w || document.viewport.width) > 0) || !(Number(document.viewport.h || document.viewport.height) > 0)) {
    throw new Error("document viewport must include positive w/h");
  }
  if (!Array.isArray(document.pages) || document.pages.length === 0) {
    throw new Error("document requires pages[]");
  }
  for (const page of document.pages) {
    if (!page.id || !Array.isArray(page.layers)) throw new Error("every page requires id and layers[]");
    const ids = new Set();
    for (const layer of page.layers) {
      if (!layer.id || !layer.kind) throw new Error("every layer requires id and kind");
      if (ids.has(layer.id)) throw new Error(`duplicate layer id: ${layer.id}`);
      ids.add(layer.id);
      const b = bounds(layer);
      if (!(b.w > 0) || !(b.h > 0)) throw new Error(`layer ${layer.id} requires positive bounds`);
      if (layer.kind === "image" && !document.assets?.[layer.asset_ref || layer.assetId]) {
        throw new Error(`image layer ${layer.id} references missing asset`);
      }
      if (layer.kind === "component" && !document.components?.[layer.component]) {
        throw new Error(`component layer ${layer.id} references missing component`);
      }
    }
  }
}

function firstPage(document) {
  return Array.isArray(document?.pages) ? document.pages[0] || null : null;
}

function firstLayer(page) {
  return Array.isArray(page?.layers) ? page.layers[0] || null : null;
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

function field(name, label, value, kind = "input") {
  const escaped = escapeHtml(value);
  if (kind === "textarea") {
    return `<label class="poster-field poster-field-wide"><span>${label}</span><textarea data-poster-field="${name}">${escaped}</textarea></label>`;
  }
  return `<label class="poster-field"><span>${label}</span><input data-poster-field="${name}" value="${escaped}"></label>`;
}

function componentParamFields(params) {
  const fields = Object.entries(flatten(params, "params"));
  if (!fields.length) return "";
  return `<div class="poster-param-fields"><h3>Params</h3>${fields
    .map(([name, value]) => field(name, name.replace(/^params\./, ""), value))
    .join("")}</div>`;
}

function flatten(value, prefix, out = {}) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return out;
  for (const [key, item] of Object.entries(value)) {
    const field = `${prefix}.${key}`;
    if (item && typeof item === "object" && !Array.isArray(item)) {
      flatten(item, field, out);
    } else if (typeof item === "string" || typeof item === "number" || typeof item === "boolean") {
      out[field] = item;
    }
  }
  return out;
}

function setLayerValue(layer, field, value) {
  if (!field) return;
  const parts = field.split(".");
  let current = layer;
  if (parts[0] === "params") {
    layer.params = layer.params || {};
    current = layer.params;
    parts.shift();
  }
  for (const part of parts.slice(0, -1)) {
    current[part] = current[part] && typeof current[part] === "object" ? current[part] : {};
    current = current[part];
  }
  const last = parts.at(-1);
  if (!last) return;
  if (value === "" && (field === "component" || field.startsWith("style."))) {
    delete current[last];
    return;
  }
  current[last] = coerceValue(value);
}

function inputValue(input) {
  return input.tagName === "TEXTAREA" ? input.value : input.value.trim();
}

function coerceValue(value) {
  if (value === "true") return true;
  if (value === "false") return false;
  if (value !== "" && !Number.isNaN(Number(value))) return Number(value);
  return value;
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
