export function createCanvasWorkbench(ctx) {
  const {
    initCanvas,
    startCanvas,
    state,
    updateCanvasStatus,
    installCanvasLabelSync,
    installCanvasRegionSelection,
    nextFrame,
    stringifyError,
    renderError,
    refreshPlannerContext,
    create_content_card,
    create_poster_document_card,
    select_node,
    focus_node,
    move_node_by_id,
    add_image_asset_at,
    base64ToBytes,
    cloneDefaultPosterDocument,
    cloneDocument,
    parsePosterDocument,
    validatePosterDocument,
    posterDocuments,
    posterStateForNode,
    stateSnapshot,
    rpc,
    imageToolPromptEl,
    imageToolStatusEl,
    imageToolMetaEl,
  } = ctx;

async function initCanvasWorkbench() {
  try {
    await initCanvas();
    startCanvas("capy-canvas");
    state.canvas.ready = true;
    updateCanvasStatus("Canvas ready");
    installCanvasLabelSync();
    installCanvasRegionSelection();
    await nextFrame();
    seedDemoCanvas();
    refreshPlannerContext();
    window.setInterval(refreshPlannerContext, 450);
  } catch (error) {
    state.canvas.ready = false;
    state.canvas.error = stringifyError(error);
    updateCanvasStatus("Canvas unavailable");
    renderError(error);
  }
}

function seedDemoCanvas() {
  if (state.blocks.length > 0 || state.canvas.nodeCount > 0) return state.blocks;
  create_content_card("brand", "Brand Kit", 110, 105);
  create_content_card("image", "主视觉候选 A", 410, 96);
  create_content_card("web", "Landing Draft", 650, 322);
  create_content_card("video", "Storyboard", 222, 392);
  loadPosterDocument(cloneDefaultPosterDocument(), {
    title: "Poster JSON preview",
    x: 360,
    y: 118,
    sourcePath: "fixture://poster/default"
  });
  refreshPlannerContext();
  const preferred = state.blocks.find((node) => node.title === "主视觉候选 A") || state.blocks[0];
  if (preferred) selectNode(preferred.id);
  return state.blocks;
}

function selectNode(id) {
  const numericId = Number(id);
  if (!Number.isFinite(numericId)) return false;
  const ok = select_node(numericId);
  refreshPlannerContext();
  return ok;
}

function focusNode(id) {
  const numericId = Number(id);
  if (!Number.isFinite(numericId)) return false;
  const ok = focus_node(numericId);
  refreshPlannerContext();
  return ok;
}

function moveNodeById(id, x, y) {
  const numericId = Number(id);
  const nextX = Number(x);
  const nextY = Number(y);
  if (!Number.isFinite(numericId) || !Number.isFinite(nextX) || !Number.isFinite(nextY)) return false;
  const ok = move_node_by_id(numericId, nextX, nextY);
  refreshPlannerContext();
  return ok;
}

function createContentCard(kind, title, x, y) {
  const nextX = Number(x);
  const nextY = Number(y);
  if (!Number.isFinite(nextX) || !Number.isFinite(nextY)) return { ok: false, error: "invalid position" };
  const idx = create_content_card(kind, title, nextX, nextY);
  refreshPlannerContext();
  return { ok: true, index: Number(idx), selected_node: state.canvas.selectedNode, snapshot: stateSnapshot() };
}

function loadPosterDocument(rawDocument, options = {}) {
  const document = parsePosterDocument(rawDocument || cloneDefaultPosterDocument());
  validatePosterDocument(document);
  const placement = posterPlacement(options);
  const title = String(options.title || document.title || "Poster document");
  const sourcePath = String(options.sourcePath || options.source_path || "");
  const idx = create_poster_document_card(title, placement.x, placement.y, sourcePath);
  refreshPlannerContext();
  const selectedNode = state.canvas.selectedNode;
  if (!selectedNode?.id) {
    throw new Error("Poster document node was not selected after creation.");
  }
  const nodeId = String(selectedNode.id);
  posterDocuments.set(nodeId, {
    document: cloneDocument(document),
    renderState: "rendered",
    error: null,
    selectedLayerId: options.selectedLayerId || "headline",
    sourcePath
  });
  state.poster.renderState = "rendered";
  state.poster.lastNodeId = Number(selectedNode.id);
  state.poster.lastError = null;
  state.poster.selectedLayerId = options.selectedLayerId || "headline";
  refreshPlannerContext();
  return {
    ok: true,
    kind: "poster-document",
    index: Number(idx),
    node_id: Number(selectedNode.id),
    content_kind: selectedNode.content_kind,
    document_title: document.title || title,
    source_path: sourcePath || null,
    render_state: "rendered",
    poster_state: posterStateForNode(selectedNode.id),
    selected_node: state.canvas.selectedNode
  };
}

function updatePosterDocument(nodeId, rawDocument) {
  const key = String(nodeId || state.poster.lastNodeId || "");
  const entry = posterDocuments.get(key);
  if (!entry) {
    throw new Error(`Poster document node ${key || "<missing>"} is not loaded.`);
  }
  try {
    const document = parsePosterDocument(rawDocument);
    validatePosterDocument(document);
    entry.document = cloneDocument(document);
    entry.renderState = "rendered";
    entry.error = null;
    state.poster.renderState = "rendered";
    state.poster.lastError = null;
  } catch (error) {
    entry.renderState = "error-preserved";
    entry.error = error instanceof Error ? error.message : String(error);
    state.poster.renderState = "error-preserved";
    state.poster.lastError = entry.error;
  }
  state.poster.lastNodeId = Number(key);
  refreshPlannerContext();
  return {
    ok: entry.renderState === "rendered",
    node_id: Number(key),
    render_state: entry.renderState,
    error: entry.error,
    poster_state: posterStateForNode(key)
  };
}

function posterPlacement(options) {
  const x = Number(options.x);
  const y = Number(options.y);
  if (Number.isFinite(x) && Number.isFinite(y)) {
    return { x, y };
  }
  const current = refreshPlannerContext();
  const selected = current.canvas?.selectedNode;
  const bounds = selected?.bounds || selected?.geometry || null;
  if (bounds) {
    return {
      x: Number(bounds.x || 0) + Number(bounds.w || 320) + 56,
      y: Number(bounds.y || 0)
    };
  }
  const viewport = current.canvas?.viewport;
  return {
    x: Math.round((viewport?.visible_world?.x || 80) + 640),
    y: Math.round((viewport?.visible_world?.y || 80) + 100)
  };
}

async function insertImageFromBase64(base64, title, x, y, meta = {}) {
  const bytes = base64ToBytes(base64);
  const nextX = Number(x);
  const nextY = Number(y);
  if (!Number.isFinite(nextX) || !Number.isFinite(nextY)) {
    throw new Error("insertImageFromBase64 requires numeric x/y");
  }
  const idx = add_image_asset_at(
    nextX, nextY, bytes,
    title || "Generated image",
    meta.sourcePath || "",
    meta.provider || "",
    meta.promptSummary || ""
  );
  refreshPlannerContext();
  return {
    ok: true,
    index: Number(idx),
    inserted_node: state.canvas.selectedNode,
    node_count: state.canvas.nodeCount,
    source_path: meta.sourcePath || null,
    provider: meta.provider || null,
    prompt_summary: meta.promptSummary || null
  };
}

async function startCanvasImageTool({ live = false } = {}) {
  const prompt = imageToolPromptEl?.value.trim() || defaultImagePrompt();
  if (imageToolPromptEl && !imageToolPromptEl.value.trim()) imageToolPromptEl.value = prompt;
  const placement = nextImagePlacement();
  state.canvasTool.status = "running";
  state.canvasTool.error = null;
  state.canvasTool.lastResult = null;
  renderCanvasToolStatus();
  const data = await rpc("canvas-generate-image", {
    prompt, provider: "apimart-gpt-image-2", size: "1:1", resolution: "1k", live,
    x: placement.x, y: placement.y,
    title: live ? "Live generated image" : "Dry-run generated image",
    name: live ? "desktop-live-image" : "desktop-dry-run-image"
  });
  state.canvasTool.runId = data.run_id || null;
  state.canvasTool.lastResult = data;
  renderCanvasToolStatus();
  return data;
}

async function handleCanvasToolEvent(detail) {
  if (!detail) return null;
  const safeDetail = { ...detail };
  delete safeDetail.image_base64;
  state.canvasTool.runId = detail.run_id || state.canvasTool.runId;
  state.canvasTool.lastResult = safeDetail;
  if (detail.ok === false) {
    state.canvasTool.status = "error";
    state.canvasTool.error = detail.error?.message || "canvas image tool failed";
    renderCanvasToolStatus();
    return safeDetail;
  }
  const inserted = await insertImageFromBase64(
    detail.image_base64, detail.title, detail.x, detail.y,
    { sourcePath: detail.source_path, provider: detail.provider, promptSummary: detail.prompt_summary }
  );
  state.canvasTool.status = "inserted";
  state.canvasTool.error = null;
  state.canvasTool.lastResult = { ...safeDetail, inserted };
  renderCanvasToolStatus();
  return state.canvasTool.lastResult;
}

function renderCanvasToolStatus() {
  if (!imageToolStatusEl || !imageToolMetaEl) return;
  imageToolStatusEl.textContent = state.canvasTool.status || "idle";
  imageToolStatusEl.dataset.status = state.canvasTool.status || "idle";
  const result = state.canvasTool.lastResult;
  if (state.canvasTool.error) {
    imageToolMetaEl.textContent = state.canvasTool.error;
  } else if (result?.inserted?.inserted_node) {
    const node = result.inserted.inserted_node;
    imageToolMetaEl.textContent = `Inserted #${node.id} · ${node.source_path || "canvas image"}`;
  } else if (state.canvasTool.runId) {
    imageToolMetaEl.textContent = state.canvasTool.runId;
  } else {
    imageToolMetaEl.textContent = "Dry run does not spend credits. Live performs one provider call.";
  }
}

function nextImagePlacement() {
  const current = refreshPlannerContext();
  const selected = current.canvas?.selectedNode;
  const bounds = selected?.bounds || selected?.geometry || null;
  if (bounds) {
    return {
      x: Number(bounds.x || 0) + Number(bounds.w || 220) + 48,
      y: Number(bounds.y || 0)
    };
  }
  const viewport = current.canvas?.viewport;
  return {
    x: Math.round((viewport?.visible_world?.x || 80) + 360),
    y: Math.round((viewport?.visible_world?.y || 80) + 140)
  };
}

function defaultImagePrompt() {
  const selected = state.canvas.selectedNode;
  const title = selected?.title || "Capybara design direction";
  return [
    "Scene: Warm studio product design board with soft natural light.",
    `Subject: A polished hero image inspired by ${title}.`,
    "Important details: premium visual direction, refined colors, clean composition.",
    "Use case: Canvas image node for design exploration.",
    "Constraints: No text, no watermark, no UI chrome."
  ].join(" ");
}

/* ─── verify hooks ─── */
function verifyCanvasImageTool() {
  return new Promise((resolve) => {
    const before = refreshPlannerContext();
    if (imageToolPromptEl) imageToolPromptEl.value = defaultImagePrompt();
    startCanvasImageTool({ live: false }).catch((error) => {
      resolve({ passed: false, reason: stringifyError(error), before });
    });
    const started = Date.now();
    const timer = setInterval(() => {
      const current = refreshPlannerContext();
      if (state.canvasTool.status === "inserted") {
        clearInterval(timer);
        const node = current.canvas?.selectedNode;
        resolve({
          passed: Boolean(node && node.content_kind === "image" && current.canvas.nodeCount > before.canvas.nodeCount),
          before_count: before.canvas.nodeCount,
          after_count: current.canvas.nodeCount,
          selected_node: node,
          tool: state.canvasTool,
          pageErrors: window.__capyPageErrors || [],
          consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === "error")
        });
      } else if (state.canvasTool.status === "error" || Date.now() - started > 20000) {
        clearInterval(timer);
        resolve({
          passed: false,
          before_count: before.canvas.nodeCount,
          after_count: current.canvas.nodeCount,
          status: state.canvasTool.status,
          error: state.canvasTool.error,
          pageErrors: window.__capyPageErrors || []
        });
      }
    }, 250);
  });
}


  return {
    initCanvasWorkbench,
    seedDemoCanvas,
    selectNode,
    focusNode,
    moveNodeById,
    createContentCard,
    loadPosterDocument,
    updatePosterDocument,
    insertImageFromBase64,
    startCanvasImageTool,
    handleCanvasToolEvent,
    renderCanvasToolStatus,
    defaultImagePrompt,
    verifyCanvasImageTool,
  };
}
