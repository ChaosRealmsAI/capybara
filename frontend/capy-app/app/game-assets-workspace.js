const DEFAULT_PACK = "/target/capy-game-assets-sample/pack.json";

export function createGameAssetsWorkspace({ state, dom, stringifyError }) {
  function installGameAssetsWorkspace() {
    dom.gameAssetsOpenEl?.addEventListener("click", () => {
      const path = dom.gameAssetsPathEl?.value?.trim() || DEFAULT_PACK;
      openPack(path);
    });
    dom.gameAssetsSampleEl?.addEventListener("click", () => {
      if (dom.gameAssetsPathEl) dom.gameAssetsPathEl.value = DEFAULT_PACK;
      openPack(DEFAULT_PACK);
    });
    dom.gameAssetsVerifyEl?.addEventListener("click", () => verifyView());
    renderGameAssetsWorkspace();
  }

  async function ensureDefaultPack() {
    if (state.gameAssets.pack || state.gameAssets.status === "loading") {
      renderGameAssetsWorkspace();
      return;
    }
    if (dom.gameAssetsPathEl && !dom.gameAssetsPathEl.value.trim()) {
      dom.gameAssetsPathEl.value = DEFAULT_PACK;
    }
    await openPack(dom.gameAssetsPathEl?.value?.trim() || DEFAULT_PACK);
  }

  async function openPack(path) {
    state.gameAssets.status = "loading";
    state.gameAssets.error = "";
    state.gameAssets.verifyStatus = "";
    state.gameAssets.path = path;
    renderGameAssetsWorkspace();
    try {
      const packUrl = normalizePackUrl(path);
      const response = await fetch(packUrl, { cache: "no-store" });
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
      const pack = await response.json();
      if (pack.schema !== "capy.game_assets.pack.v1") {
        throw new Error(`unsupported schema: ${pack.schema || "missing"}`);
      }
      state.gameAssets.status = "ready";
      state.gameAssets.pack = pack;
      state.gameAssets.rootUrl = packUrl.slice(0, packUrl.lastIndexOf("/") + 1);
      state.gameAssets.selectedAssetId = pack.assets?.[0]?.id || "";
      if (dom.gameAssetsPathEl) dom.gameAssetsPathEl.value = path;
      renderGameAssetsWorkspace();
    } catch (error) {
      state.gameAssets.status = "error";
      state.gameAssets.pack = null;
      state.gameAssets.error = stringifyError(error);
      renderGameAssetsWorkspace();
    }
  }

  async function verifyView() {
    const pack = state.gameAssets.pack;
    if (!pack) return;
    state.gameAssets.status = "verifying";
    state.gameAssets.verifyStatus = "";
    renderSummary();
    try {
      const imagePaths = collectImagePaths(pack);
      const loaded = await Promise.all(imagePaths.map((path) => loadImage(assetUrl(path))));
      const missing = loaded.filter((result) => !result.ok).map((result) => result.path);
      const frameCount = countFrames(pack);
      const passed = missing.length === 0 && (pack.assets?.length || 0) >= 5 && frameCount >= 16;
      state.gameAssets.status = "ready";
      state.gameAssets.verifyStatus = passed
        ? `通过 · ${pack.assets.length} assets · ${frameCount} frames`
        : `失败 · ${missing.length} missing`;
      state.gameAssets.error = passed ? "" : missing.join(", ");
      renderGameAssetsWorkspace();
    } catch (error) {
      state.gameAssets.status = "error";
      state.gameAssets.error = stringifyError(error);
      renderGameAssetsWorkspace();
    }
  }

  function renderGameAssetsWorkspace() {
    renderSummary();
    renderAssetList();
    renderPreview();
    renderFrames();
    renderInspector();
  }

  function renderSummary() {
    if (!dom.gameAssetsStatusEl) return;
    const pack = state.gameAssets.pack;
    if (state.gameAssets.status === "error") {
      dom.gameAssetsStatusEl.textContent = state.gameAssets.error || "加载失败";
      dom.gameAssetsStatusEl.dataset.status = "error";
      return;
    }
    if (state.gameAssets.status === "loading") {
      dom.gameAssetsStatusEl.textContent = "读取 pack.json";
      dom.gameAssetsStatusEl.dataset.status = "loading";
      return;
    }
    if (state.gameAssets.status === "verifying") {
      dom.gameAssetsStatusEl.textContent = "检查图片资源";
      dom.gameAssetsStatusEl.dataset.status = "loading";
      return;
    }
    if (!pack) {
      dom.gameAssetsStatusEl.textContent = "等待 pack.json";
      dom.gameAssetsStatusEl.dataset.status = "idle";
      return;
    }
    dom.gameAssetsStatusEl.textContent = state.gameAssets.verifyStatus ||
      `${pack.title || pack.id} · ${pack.assets?.length || 0} assets · ${countFrames(pack)} frames`;
    dom.gameAssetsStatusEl.dataset.status = "ready";
  }

  function renderAssetList() {
    if (!dom.gameAssetsListEl) return;
    const assets = state.gameAssets.pack?.assets || [];
    dom.gameAssetsListEl.replaceChildren(...assets.map((asset) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "game-assets-row";
      button.dataset.selected = asset.id === state.gameAssets.selectedAssetId ? "true" : "false";
      button.innerHTML = `<span>${escapeHtml(asset.name || asset.id)}</span><small>${escapeHtml(asset.kind || "asset")} · ${asset.actions?.length || 0} actions</small>`;
      button.addEventListener("click", () => {
        state.gameAssets.selectedAssetId = asset.id;
        renderGameAssetsWorkspace();
      });
      return button;
    }));
  }

  function renderPreview() {
    if (!dom.gameAssetsPreviewEl) return;
    const asset = selectedAsset();
    if (!asset) {
      dom.gameAssetsPreviewEl.innerHTML = `<div class="game-assets-empty">打开 pack 后预览素材</div>`;
      return;
    }
    dom.gameAssetsPreviewEl.innerHTML = `
      <div class="game-assets-hero-image"><img src="${assetUrl(asset.transparent_path)}" alt=""></div>
      <div class="game-assets-hero-meta">
        <span>${escapeHtml(asset.kind || "asset")}</span>
        <h2>${escapeHtml(asset.name || asset.id)}</h2>
        <p>${escapeHtml(asset.notes || "")}</p>
      </div>
    `;
  }

  function renderFrames() {
    if (!dom.gameAssetsFramesEl) return;
    const asset = selectedAsset();
    const actions = asset?.actions || [];
    if (!actions.length) {
      dom.gameAssetsFramesEl.innerHTML = `<div class="game-assets-empty">静态素材</div>`;
      return;
    }
    dom.gameAssetsFramesEl.replaceChildren(...actions.map((action) => {
      const section = document.createElement("section");
      section.className = "game-assets-action";
      const title = document.createElement("h3");
      title.textContent = action.name || action.id;
      const strip = document.createElement("div");
      strip.className = "game-assets-frame-strip";
      for (const path of action.frame_paths || []) {
        const img = document.createElement("img");
        img.src = assetUrl(path);
        img.alt = "";
        strip.appendChild(img);
      }
      const sheet = document.createElement("img");
      sheet.className = "game-assets-spritesheet";
      sheet.src = assetUrl(action.spritesheet_path);
      sheet.alt = "";
      section.append(title, strip, sheet);
      return section;
    }));
  }

  function renderInspector() {
    if (!dom.gameAssetsInspectorEl) return;
    const pack = state.gameAssets.pack;
    const asset = selectedAsset();
    if (!pack) {
      dom.gameAssetsInspectorEl.innerHTML = `<div class="game-assets-empty">无 pack</div>`;
      if (dom.gameAssetsContactSheetEl) dom.gameAssetsContactSheetEl.hidden = true;
      return;
    }
    dom.gameAssetsInspectorEl.innerHTML = `
      <div class="game-assets-kv"><span>Pack</span><strong>${escapeHtml(pack.id)}</strong></div>
      <div class="game-assets-kv"><span>Mode</span><strong>${escapeHtml(pack.mode || "fixture")}</strong></div>
      <div class="game-assets-kv"><span>Frames</span><strong>${countFrames(pack)}</strong></div>
      <div class="game-assets-kv"><span>Selected</span><strong>${escapeHtml(asset?.id || "none")}</strong></div>
      <div class="game-assets-kv"><span>Prompt</span><strong>${escapeHtml(asset?.prompt_path || "")}</strong></div>
    `;
    if (dom.gameAssetsContactSheetEl) {
      dom.gameAssetsContactSheetEl.hidden = false;
      dom.gameAssetsContactSheetEl.src = assetUrl(pack.outputs?.contact_sheet || "qa/contact-sheet.png");
    }
  }

  function selectedAsset() {
    const assets = state.gameAssets.pack?.assets || [];
    return assets.find((asset) => asset.id === state.gameAssets.selectedAssetId) || assets[0] || null;
  }

  function countFrames(pack) {
    return (pack.assets || []).reduce((total, asset) => total + (asset.actions || [])
      .reduce((sum, action) => sum + (action.frame_paths || []).length, 0), 0);
  }

  function collectImagePaths(pack) {
    const paths = [];
    if (pack.outputs?.contact_sheet) paths.push(pack.outputs.contact_sheet);
    for (const asset of pack.assets || []) {
      if (asset.transparent_path) paths.push(asset.transparent_path);
      for (const action of asset.actions || []) {
        if (action.spritesheet_path) paths.push(action.spritesheet_path);
        paths.push(...(action.frame_paths || []));
      }
    }
    return paths;
  }

  function normalizePackUrl(path) {
    const raw = String(path || DEFAULT_PACK).trim();
    if (/^https?:\/\//i.test(raw)) return raw;
    const cwd = window.CAPYBARA_SESSION?.cwd || "";
    if (cwd && raw.startsWith(`${cwd}/`)) return `/${raw.slice(cwd.length + 1)}`;
    if (raw.startsWith("/target/") || raw.startsWith("/fixtures/") || raw.startsWith("/spec/")) return raw;
    if (raw.startsWith("target/") || raw.startsWith("fixtures/") || raw.startsWith("spec/")) return `/${raw}`;
    return raw.startsWith("/") ? raw : `/${raw}`;
  }

  function assetUrl(path) {
    const raw = String(path || "");
    if (/^https?:\/\//i.test(raw) || raw.startsWith("/")) return raw;
    return `${state.gameAssets.rootUrl}${raw.split("/").map(encodeURIComponent).join("/")}`;
  }

  function loadImage(url) {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => resolve({ ok: true, path: url });
      img.onerror = () => resolve({ ok: false, path: url });
      img.src = url;
    });
  }

  return {
    installGameAssetsWorkspace,
    ensureDefaultPack,
    openPack,
    renderGameAssetsWorkspace,
    verifyView,
  };
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
