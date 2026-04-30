import { createPosterPreviewController } from "./poster-preview.js";

const SAMPLE_PATHS = {
  single: "/fixtures/poster/v1/single-poster.json",
  deck: "/fixtures/poster/v1/ppt-deck.json",
  shared: "/fixtures/poster/v1/shared-component.json",
};

export function createPosterWorkspace(ctx) {
  const { state, dom, rpc, stringifyError } = ctx;
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
    dom.posterSaveJsonEl?.addEventListener("click", () => saveDocument());
    dom.posterExportPngEl?.addEventListener("click", () => exportDocument(["png"]));
    dom.posterExportPdfEl?.addEventListener("click", () => exportDocument(["pdf"]));
    dom.posterExportPptxEl?.addEventListener("click", () => exportDocument(["pptx"]));
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
      const document = await loadDocument(path);
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

  async function saveDocument() {
    const document = state.posterWorkspace.document;
    if (!document) return;
    try {
      validateDocument(document);
      const result = await rpc("poster-document-save", {
        document,
        path: state.posterWorkspace.path,
      });
      state.posterWorkspace.status = "saved";
      state.posterWorkspace.path = result.path || state.posterWorkspace.path;
      state.posterWorkspace.exportStatus = `JSON 已保存 · ${result.path || ""}`;
      if (dom.posterPathEl && result.path) dom.posterPathEl.value = result.path;
    } catch (error) {
      state.posterWorkspace.status = "error";
      state.posterWorkspace.error = stringifyError(error);
      state.posterWorkspace.exportStatus = `保存失败 · ${stringifyError(error)}`;
    }
    renderStatus();
  }

  async function exportDocument(formats) {
    const document = state.posterWorkspace.document;
    if (!document) return;
    try {
      validateDocument(document);
      state.posterWorkspace.exportStatus = `${formats.join("/").toUpperCase()} 导出中...`;
      renderStatus();
      const result = await rpc("poster-document-export", {
        document,
        path: state.posterWorkspace.path,
        formats,
        page: "all",
      });
      state.posterWorkspace.status = "exported";
      state.posterWorkspace.exportManifest = result.manifest_path || "";
      const path = result.pdf_path || result.pptx_path || result.pages?.[0]?.png_path || result.manifest_path || "";
      state.posterWorkspace.exportStatus = `${formats.join("/").toUpperCase()} 已导出 · ${path}`;
    } catch (error) {
      state.posterWorkspace.status = "error";
      state.posterWorkspace.error = stringifyError(error);
      state.posterWorkspace.exportStatus = `导出失败 · ${stringifyError(error)}`;
    }
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

async function loadDocument(path) {
  try {
    const response = await fetch(path);
    if (!response.ok) throw new Error(`load ${path} failed: ${response.status}`);
    return await response.json();
  } catch (error) {
    const fallback = sampleDocumentForPath(path);
    if (fallback) return fallback;
    throw error;
  }
}

function sampleDocumentForPath(path) {
  const key = String(path || "").replace(/^https?:\/\/[^/]+/, "");
  if (!key.endsWith("/fixtures/poster/v1/single-poster.json")) return null;
  return cloneJson({
    schema: "capy.poster.document.v1",
    id: "single-poster-demo",
    title: "AI Design Poster",
    viewport: { w: 1920, h: 1080, ratio: "16:9" },
    theme: { background: "#fffaf0", accent: "#a78bfa" },
    assets: {
      hero: {
        type: "image",
        src: "data:image/svg+xml,%3Csvg%20width%3D%22640%22%20height%3D%22640%22%20viewBox%3D%220%200%20640%20640%22%20xmlns%3D%22http%3A//www.w3.org/2000/svg%22%3E%3Cdefs%3E%3CradialGradient%20id%3D%22g%22%20cx%3D%2230%25%22%20cy%3D%2220%25%22%20r%3D%2280%25%22%3E%3Cstop%20stop-color%3D%22%23fff7ed%22/%3E%3Cstop%20offset%3D%22.55%22%20stop-color%3D%22%23c4b5fd%22/%3E%3Cstop%20offset%3D%221%22%20stop-color%3D%22%23f59e0b%22/%3E%3C/radialGradient%3E%3C/defs%3E%3Crect%20width%3D%22640%22%20height%3D%22640%22%20rx%3D%22120%22%20fill%3D%22url(%23g)%22/%3E%3Ccircle%20cx%3D%22324%22%20cy%3D%22272%22%20r%3D%22126%22%20fill%3D%22%231c1917%22%20opacity%3D%22.9%22/%3E%3Cpath%20d%3D%22M210%20412h228c-18%2066-72%20104-138%20104-42%200-76-12-106-36z%22%20fill%3D%22%23fffaf0%22%20opacity%3D%22.92%22/%3E%3C/svg%3E",
        provenance: { kind: "fallback", source: "app-bundled" }
      }
    },
    pages: [
      {
        id: "cover",
        title: "Cover",
        background: "#fffaf0",
        layers: [
          { id: "wash", kind: "shape", shape: "rect", bounds: { x: 0, y: 0, w: 1920, h: 1080 }, z: 0, style: { fill: "linear-gradient(135deg, #fffaf0 0%, #fef3c7 48%, #ede9fe 100%)" } },
          { id: "eyebrow", kind: "text", text: "CAPYBARA JSON POSTER", bounds: { x: 150, y: 145, w: 760, h: 64 }, z: 2, style: { fontSize: 32, fontWeight: 800, color: "#78716c" } },
          { id: "headline", kind: "text", text: "每层都可选中\n每层都可编辑", bounds: { x: 150, y: 250, w: 860, h: 250 }, z: 3, style: { fontSize: 94, fontWeight: 900, color: "#1c1917", lineHeight: 1.05 } },
          { id: "subhead", kind: "text", text: "JSON 是源文件，Preview 是调试面，Inspector 是局部 patch 入口。", bounds: { x: 160, y: 545, w: 680, h: 130 }, z: 4, style: { fontSize: 34, fontWeight: 760, color: "#57534e", lineHeight: 1.25 } },
          { id: "hero-card", kind: "image", asset_ref: "hero", bounds: { x: 1220, y: 250, w: 430, h: 430 }, z: 4, style: { radius: 72, objectFit: "cover" } },
          { id: "cta-bg", kind: "shape", shape: "pill", bounds: { x: 160, y: 720, w: 360, h: 86 }, z: 5, style: { fill: "#1c1917", radius: 999 } },
          { id: "cta-text", kind: "text", text: "打开 Inspector", bounds: { x: 210, y: 742, w: 260, h: 44 }, z: 6, style: { fontSize: 30, fontWeight: 880, color: "#fffaf0" } }
        ]
      }
    ]
  });
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
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
