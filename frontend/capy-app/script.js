import initCanvas, {
  add_image_asset_at,
  ai_snapshot,
  ai_snapshot_text,
  create_content_card,
  create_poster_document_card,
  current_tool,
  dark_mode,
  focus_node,
  list_shapes,
  move_node_by_id,
  select_node,
  selected_context,
  selected_context_text,
  shape_count,
  start as startCanvas
} from "./canvas-pkg/capy_canvas_web.js";
import {
  buildPosterState,
  cloneDefaultPosterDocument,
  cloneDocument,
  parsePosterDocument,
  renderPosterStage,
  validatePosterDocument
} from "./poster-renderer.js";
import {
  clampRectToBounds,
  compactGeometry,
  nodeBounds,
  normalizeRect,
  regionPercent,
  roundGeometry,
  worldBoxToScreen
} from "./workbench/geometry.js";
import {
  compileRows,
  compositionRows,
  evidenceRows,
  exportRows,
  exportStatus,
  inspectorMessage,
  sourceRows,
  stageCard,
  stageLabel,
} from "./timeline/inspector-render.js";
import { dom } from "./app/dom.js";
import { installIpcReceiver, installNativeWindowDrag, installShellEventListeners, createRpc } from "./app/ipc.js";
import { createRuntimeControls } from "./app/runtime-controls.js";
import { labelSync, nodeRegistry, pending, posterDocuments, state } from "./app/state.js";
import { base64ToBytes, contentKindLabel, nextFrame, normalizeValue, stringifyError } from "./app/utils.js";
import { installWindowFacade } from "./app/window-facade.js";

/* ─── DOM refs ─── */
const {
  topbar,
  cmdkTriggerEl,
  listEl,
  messagesEl,
  newChatEl,
  stopEl,
  runStatusEl,
  formEl,
  promptEl,
  configSummaryEl,
  configDialogEl,
  configDialogCloseEl,
  configDialogDoneEl,
  providerEl,
  cwdEl,
  modelEl,
  effortEl,
  policyEl,
  sandboxEl,
  serviceTierEl,
  systemPromptEl,
  appendSystemPromptEl,
  developerInstructionsEl,
  addDirsEl,
  allowedToolsEl,
  disallowedToolsEl,
  mcpConfigEl,
  modelProviderEl,
  approvalsReviewerEl,
  reasoningSummaryEl,
  outputSchemaEl,
  bareEl,
  searchEl,
  writeCodeEl,
  runtimeFootEl,
  canvasEl,
  canvasPanelEl,
  canvasStatusEl,
  posterLayerEl,
  labelLayerEl,
  regionLayerEl,
  regionModeEl,
  plannerContextEl,
  contextTitleEl,
  contextMetaEl,
  contextAttachmentsEl,
  timelineInspectorEl: nextFrameInspectorEl,
  timelineInspectorTitleEl: nextFrameInspectorTitleEl,
  timelineInspectorStatusEl: nextFrameInspectorStatusEl,
  timelineInspectorStagesEl: nextFrameInspectorStagesEl,
  cmdPaletteEl,
  cmdSearchEl,
  cmdCloseEl,
  cmdListEl,
  cmdToolEl,
  cmdToolBackEl,
  imageToolPromptEl,
  imageToolDryRunEl,
  imageToolLiveEl,
  imageToolStatusEl,
  imageToolMetaEl,
} = dom;

const rpc = createRpc(pending);
const {
  currentConfig,
  syncPolicyOptions,
  applyWriteCodeDefaults,
  updateConfigSummary,
  setRunStatus,
  renderRuntimeFoot,
  updateCanvasStatus,
} = createRuntimeControls({ state, dom });

installWindowFacade({
  state,
  capyApi: {
  add_image_asset_at,
  ai_snapshot,
  ai_snapshot_text,
  create_content_card,
  create_poster_document_card,
  current_tool,
  dark_mode,
  focus_node,
  list_shapes,
  move_node_by_id,
  select_node,
  selected_context,
  selected_context_text,
  shape_count
  },
  workbenchApi: {
    composePromptWithContext,
    activeCanvasContext,
    setCanvasContextRegion,
    clearCanvasContextRegion,
    refreshPlannerContext,
    seedDemoCanvas,
    createContentCard,
    insertImageFromBase64,
    loadPosterDocument,
    updatePosterDocument,
    moveNodeById,
    focusNode,
    selectNode,
    scheduleCanvasLabelRefresh,
    startLiveCanvasLabelRefresh,
    stateSnapshot,
    attachTimelineComposition,
    openTimelineComposition,
    openTimelineInspector,
    startCanvasImageTool,
    verifyCanvasImageTool,
    verifyLabelMoveSync,
    verifyPosterRenderer,
    openCmdPalette,
    closeCmdPalette
  }
});

installNativeWindowDrag(topbar);
installIpcReceiver(pending);
installShellEventListeners({
  state,
  setRunStatus,
  renderMessages,
  openConversation,
  renderError,
  handleCanvasToolEvent,
  renderCanvasToolStatus,
  stringifyError,
  handleCanvasNodeAttached,
  handleTimelineOpened,
});
/* ─── form / button listeners ─── */
newChatEl?.addEventListener("click", async () => {
  try { await createConversation(); } catch (error) { renderError(error); }
});

stopEl?.addEventListener("click", async () => {
  if (!state.activeId) return;
  try {
    await rpc("conversation-stop", { id: state.activeId });
    await openConversation(state.activeId);
  } catch (error) { renderError(error); }
});

regionModeEl?.addEventListener("click", () => {
  state.canvasContext.regionMode = !state.canvasContext.regionMode;
  renderRegionMode();
});

formEl?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const prompt = promptEl.value.trim();
  if (!prompt) return;
  try {
    if (!state.activeId) await createConversation();
    if (!state.activeId) return;
    promptEl.value = "";
    await updateConversationConfig();
    const outboundPrompt = composePromptWithContext(prompt);
    const canvasContext = activeCanvasContext();
    state.planner.lastOutboundPrompt = outboundPrompt;
    state.messages.push({ id: `local-${Date.now()}`, role: "user", content: prompt });
    renderMessages();
    setRunStatus("running");
    await rpc("conversation-send", {
      id: state.activeId,
      prompt: outboundPrompt,
      config: currentConfig(),
      model: modelEl.value.trim() || null,
      canvas_context: canvasContext
    });
  } catch (error) {
    setRunStatus("error");
    renderError(error);
  }
});

providerEl?.addEventListener("change", () => {
  syncPolicyOptions();
  applyWriteCodeDefaults();
  renderRuntimeFoot();
  updateConfigSummary();
});

[effortEl, policyEl, sandboxEl, writeCodeEl].forEach((el) => {
  el?.addEventListener("change", () => {
    if (el === writeCodeEl) applyWriteCodeDefaults();
    updateConfigSummary();
  });
});

imageToolDryRunEl?.addEventListener("click", () => {
  startCanvasImageTool({ live: false }).catch((error) => {
    state.canvasTool.status = "error";
    state.canvasTool.error = stringifyError(error);
    renderCanvasToolStatus();
  });
});

imageToolLiveEl?.addEventListener("click", () => {
  startCanvasImageTool({ live: true }).catch((error) => {
    state.canvasTool.status = "error";
    state.canvasTool.error = stringifyError(error);
    renderCanvasToolStatus();
  });
});

/* ─── view tabs (stub · 切换 active class) ─── */
document.querySelectorAll(".view-tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".view-tab").forEach((b) => b.classList.toggle("active", b === btn));
  });
});

/* ─── config dialog · 弹窗 ─── */
function openConfigDialog() {
  if (!configDialogEl) return;
  configDialogEl.classList.add("is-open");
  configDialogEl.style.display = "grid";
}
async function closeConfigDialog() {
  if (!configDialogEl) return;
  configDialogEl.classList.remove("is-open");
  configDialogEl.style.display = "none";
  updateConfigSummary();
  if (state.activeId) {
    try { await updateConversationConfig(); } catch (e) { renderError(e); }
  }
}
configSummaryEl?.addEventListener("click", openConfigDialog);
configDialogCloseEl?.addEventListener("click", () => closeConfigDialog());
configDialogDoneEl?.addEventListener("click", () => closeConfigDialog());
configDialogEl?.addEventListener("click", (e) => {
  if (e.target === configDialogEl) closeConfigDialog();
});

/* ─── cmd-K palette ─── */
function openCmdPalette() {
  if (!cmdPaletteEl) return;
  switchCmdView("list");
  cmdPaletteEl.classList.add("is-open");
  cmdPaletteEl.style.display = "grid";
  setTimeout(() => cmdSearchEl?.focus(), 30);
}
function closeCmdPalette() {
  if (!cmdPaletteEl) return;
  cmdPaletteEl.classList.remove("is-open");
  cmdPaletteEl.style.display = "none";
  if (cmdSearchEl) cmdSearchEl.value = "";
  switchCmdView("list");
}
function switchCmdView(view) {
  if (!cmdToolEl || !cmdListEl) return;
  if (view === "tool") {
    cmdListEl.style.display = "none";
    cmdToolEl.hidden = false;
    setTimeout(() => imageToolPromptEl?.focus(), 30);
  } else {
    cmdListEl.style.display = "";
    cmdToolEl.hidden = true;
  }
}
cmdkTriggerEl?.addEventListener("click", openCmdPalette);
cmdCloseEl?.addEventListener("click", closeCmdPalette);
cmdToolBackEl?.addEventListener("click", () => switchCmdView("list"));
cmdPaletteEl?.addEventListener("click", (e) => {
  if (e.target === cmdPaletteEl) closeCmdPalette();
});
cmdPaletteEl?.addEventListener("close", () => switchCmdView("list"));

cmdListEl?.addEventListener("click", (e) => {
  const row = e.target instanceof HTMLElement ? e.target.closest(".cmd-row") : null;
  if (!row) return;
  const cmd = row.dataset.cmd;
  runCmd(cmd);
});

cmdSearchEl?.addEventListener("input", () => {
  const q = cmdSearchEl.value.trim().toLowerCase();
  cmdListEl?.querySelectorAll(".cmd-row").forEach((row) => {
    const text = row.textContent.toLowerCase();
    row.style.display = !q || text.includes(q) ? "" : "none";
  });
});

cmdSearchEl?.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    const visible = cmdListEl?.querySelector('.cmd-row:not([style*="display: none"])');
    if (visible) runCmd(visible.dataset.cmd);
  }
});

window.addEventListener("keydown", (e) => {
  const isCmdK = (e.metaKey || e.ctrlKey) && (e.key === "k" || e.key === "K");
  if (isCmdK) {
    e.preventDefault();
    if (cmdPaletteEl?.classList.contains("is-open")) closeCmdPalette(); else openCmdPalette();
  }
  if (e.key === "Escape") {
    if (cmdPaletteEl?.classList.contains("is-open")) closeCmdPalette();
    if (configDialogEl?.classList.contains("is-open")) closeConfigDialog();
  }
});

function runCmd(cmd) {
  if (cmd === "open-image-tool") {
    if (imageToolPromptEl && !imageToolPromptEl.value.trim()) {
      imageToolPromptEl.value = defaultImagePrompt();
    }
    switchCmdView("tool");
    return;
  }
  if (cmd === "dark-mode") {
    state.canvas.darkMode = !state.canvas.darkMode;
    document.documentElement.dataset.canvasDark = state.canvas.darkMode ? "true" : "false";
    closeCmdPalette();
    return;
  }
  if (cmd === "seed-demo") {
    seedDemoCanvas();
    closeCmdPalette();
    return;
  }
}

/* ─── boot ─── */
init();

async function init() {
  cwdEl.value = window.CAPYBARA_SESSION?.cwd || "/Users/Zhuanz/workspace/capybara";
  syncPolicyOptions();
  setRunStatus("idle");
  renderCanvasToolStatus();
  renderMessages();
  updateConfigSummary();
  await initCanvasWorkbench();
  try {
    const data = await rpc("conversation-list", {});
    state.dbPath = data.db_path || null;
    state.conversations = data.conversations || [];
    renderConversations();
    renderRuntimeFoot();
    if (state.conversations[0]) await openConversation(state.conversations[0].id);
  } catch (error) {
    renderError(error);
  }
}

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

function installCanvasRegionSelection() {
  if (!regionLayerEl) return;
  regionLayerEl.addEventListener("pointerdown", (event) => {
    if (!state.canvasContext.regionMode || !isRegionCapableSelection()) return;
    const start = clampPointToSelectedNode(screenPointToWorld(event.clientX, event.clientY));
    if (!start) return;
    event.preventDefault();
    regionLayerEl.setPointerCapture?.(event.pointerId);
    state.canvasContext.drag = { pointerId: event.pointerId, start };
    setCanvasContextRegion({ x: start.x, y: start.y, w: 1, h: 1 });
  });
  regionLayerEl.addEventListener("pointermove", (event) => {
    const drag = state.canvasContext.drag;
    if (!drag || drag.pointerId !== event.pointerId) return;
    const current = clampPointToSelectedNode(screenPointToWorld(event.clientX, event.clientY));
    if (!current) return;
    event.preventDefault();
    setCanvasContextRegion(normalizeRect(
      drag.start.x,
      drag.start.y,
      current.x - drag.start.x,
      current.y - drag.start.y
    ));
  });
  regionLayerEl.addEventListener("pointerup", finishRegionDrag);
  regionLayerEl.addEventListener("pointercancel", finishRegionDrag);
  renderRegionMode();
}

function finishRegionDrag(event) {
  const drag = state.canvasContext.drag;
  if (!drag || drag.pointerId !== event.pointerId) return;
  regionLayerEl?.releasePointerCapture?.(event.pointerId);
  state.canvasContext.drag = null;
  const region = state.canvasContext.region;
  if (!region || region.bounds.w < 4 || region.bounds.h < 4) {
    clearCanvasContextRegion();
  }
}

function renderRegionMode() {
  if (regionModeEl) {
    regionModeEl.setAttribute("aria-pressed", state.canvasContext.regionMode ? "true" : "false");
  }
  if (regionLayerEl) {
    regionLayerEl.classList.toggle("is-active", Boolean(state.canvasContext.regionMode && isRegionCapableSelection()));
  }
}

function setCanvasContextRegion(bounds, options = {}) {
  const selected = state.canvas.selectedNode;
  const selectedBounds = nodeBounds(selected);
  if (!selectedBounds) return { ok: false, error: "no selected canvas node" };
  if (selected.content_kind !== "image") {
    return { ok: false, error: "selected node is not an image" };
  }
  const coordinateSpace = options.coordinateSpace || bounds.coordinateSpace || "world";
  const world = coordinateSpace === "node-relative"
    ? normalizeRect(
      selectedBounds.x + Number(bounds.x || 0),
      selectedBounds.y + Number(bounds.y || 0),
      Number(bounds.w || bounds.width || 0),
      Number(bounds.h || bounds.height || 0)
    )
    : normalizeRect(
      Number(bounds.x || 0),
      Number(bounds.y || 0),
      Number(bounds.w || bounds.width || 0),
      Number(bounds.h || bounds.height || 0)
    );
  const clamped = clampRectToBounds(world, selectedBounds);
  if (!clamped || clamped.w <= 0 || clamped.h <= 0) {
    return { ok: false, error: "region is outside selected image" };
  }
  state.canvasContext.region = {
    node_id: selected.id,
    bounds: roundGeometry(clamped),
    coordinate_space: "canvas_world"
  };
  syncCanvasContext(selected, state.canvas.viewport);
  renderRegionOverlay();
  renderPlannerContext(selected);
  return { ok: true, context: activeCanvasContext() };
}

function clearCanvasContextRegion() {
  state.canvasContext.region = null;
  syncCanvasContext(state.canvas.selectedNode, state.canvas.viewport);
  renderRegionOverlay();
  renderPlannerContext(state.canvas.selectedNode);
  return { ok: true, context: activeCanvasContext() };
}

function syncCanvasContext(selectedItem, viewport) {
  const region = state.canvasContext.region;
  if (!selectedItem || (region && String(region.node_id) !== String(selectedItem.id))) {
    state.canvasContext.region = null;
  }
  const context = buildCanvasContextPreview(selectedItem, viewport);
  state.canvasContext.context = context;
  state.planner.canvasContext = context;
  renderRegionMode();
}

function buildCanvasContextPreview(selectedItem, viewport) {
  if (!selectedItem) return null;
  const region = state.canvasContext.region;
  const isRegion = Boolean(region && String(region.node_id) === String(selectedItem.id));
  const isImage = selectedItem.content_kind === "image";
  const bounds = selectedItem.bounds || selectedItem.geometry || {};
  const kind = isRegion ? "image_region" : isImage ? "selected_image" : "selected_node";
  const contextId = isRegion
    ? `ctx-live-region-${selectedItem.id}-${compactGeometry(region.bounds)}`
    : `ctx-live-selected-${selectedItem.id}`;
  return normalizeValue({
    schema_version: 1,
    context_id: contextId,
    kind,
    source_node_id: selectedItem.id,
    source_node_title: selectedItem.title || `Node ${selectedItem.id}`,
    content_kind: selectedItem.content_kind,
    source_path: selectedItem.source_path || null,
    node_bounds_world: bounds,
    region_bounds_world: isRegion ? region.bounds : null,
    region_bounds_node_percent: isRegion ? regionPercent(region.bounds, bounds) : null,
    viewport,
    attachment_paths: [],
    expected_attachments: isRegion
      ? ["viewport.png", "selected-node.png", "region.png", "context.json"]
      : ["viewport.png", "selected-node.png", "context.json"],
    summary: contextSummary(selectedItem, isRegion ? region.bounds : null)
  });
}

function activeCanvasContext() {
  refreshPlannerContext();
  return normalizeValue(state.canvasContext.context);
}

function renderRegionOverlay() {
  if (!regionLayerEl) return;
  regionLayerEl.querySelectorAll(".context-region-box").forEach((node) => node.remove());
  const region = state.canvasContext.region;
  const selected = state.canvas.selectedNode;
  if (!region || !selected || String(region.node_id) !== String(selected.id)) return;
  const box = worldBoxToScreen(region.bounds, state.canvas.viewport);
  const node = document.createElement("div");
  node.className = "context-region-box";
  node.dataset.label = "Region context";
  node.style.left = `${box.x}px`;
  node.style.top = `${box.y}px`;
  node.style.width = `${Math.max(8, box.w)}px`;
  node.style.height = `${Math.max(8, box.h)}px`;
  regionLayerEl.append(node);
}

function isRegionCapableSelection() {
  return Boolean(state.canvas.selectedNode?.content_kind === "image" && nodeBounds(state.canvas.selectedNode));
}

function screenPointToWorld(clientX, clientY) {
  const rect = (regionLayerEl || canvasPanelEl || canvasEl)?.getBoundingClientRect();
  const viewport = state.canvas.viewport || { zoom: 1, camera_offset: { x: 0, y: 0 } };
  const zoom = Number(viewport.zoom) || 1;
  const offset = viewport.camera_offset || { x: 0, y: 0 };
  return {
    x: (clientX - (rect?.left || 0) - (Number(offset.x) || 0)) / zoom,
    y: (clientY - (rect?.top || 0) - (Number(offset.y) || 0)) / zoom
  };
}

function clampPointToSelectedNode(point) {
  const bounds = nodeBounds(state.canvas.selectedNode);
  if (!point || !bounds) return null;
  return {
    x: Math.min(bounds.x + bounds.w, Math.max(bounds.x, point.x)),
    y: Math.min(bounds.y + bounds.h, Math.max(bounds.y, point.y))
  };
}

function contextSummary(node, region) {
  const title = node?.title || `Node ${node?.id || "unknown"}`;
  if (!region) {
    const label = node?.content_kind === "image" ? "selected image" : "selected node";
    return `${label} ${title} id=${node?.id}`;
  }
  return `region on ${title} id=${node?.id} bounds=${compactGeometry(region)}`;
}

function verifyLabelMoveSync() {
  return new Promise((resolve) => {
    const done = (value) => resolve(normalizeValue(value));
    try {
      if (!canvasEl || !labelLayerEl) {
        done({ passed: false, reason: "missing canvas or label layer", pageErrors: window.__capyPageErrors || [] });
        return;
      }
      const snapshotTarget = () => {
        const current = refreshPlannerContext();
        const nodes = Array.isArray(current?.blocks) ? current.blocks : [];
        const node = nodes.find((item) => item.title === "Storyboard")
          || nodes.find((item) => item.content_kind === "video");
        if (!node?.bounds) return null;
        selectNode(node.id);
        const selected = refreshPlannerContext();
        const selectedNode = selected.blocks.find((item) => String(item.id) === String(node.id)) || node;
        const label = labelLayerEl.querySelector(`[data-node-id="${selectedNode.id}"]`);
        if (!label) return null;
        const viewport = selected.canvas?.viewport || { zoom: 1, camera_offset: { x: 0, y: 0 } };
        const box = nodeLabelBox(selectedNode, viewport);
        const layerRect = labelLayerEl.getBoundingClientRect();
        const rect = label.getBoundingClientRect();
        return {
          node: selectedNode,
          rect: { left: rect.left, top: rect.top, width: rect.width, height: rect.height },
          expected: { x: layerRect.left + box.x, y: layerRect.top + box.y },
          layerRect: { left: layerRect.left, top: layerRect.top },
          viewport
        };
      };
      const aligned = (sample) => Boolean(sample
        && Math.abs(sample.rect.left - sample.expected.x) <= 10
        && Math.abs(sample.rect.top - sample.expected.y) <= 10);
      const before = snapshotTarget();
      if (!before) {
        done({ passed: false, reason: "missing Storyboard node or label", pageErrors: window.__capyPageErrors || [] });
        return;
      }
      canvasEl.focus({ preventScroll: true });
      startLiveCanvasLabelRefresh();
      const nextX = before.node.bounds.x + 84;
      const nextY = before.node.bounds.y + 48;
      const moved = moveNodeById(before.node.id, nextX, nextY);
      setTimeout(() => {
        scheduleCanvasLabelRefresh();
        setTimeout(() => {
          const during = snapshotTarget();
          setTimeout(() => {
            stopLiveCanvasLabelRefresh();
            const after = snapshotTarget();
            const dx = (after?.node?.bounds?.x || 0) - (before.node.bounds.x || 0);
            const dy = (after?.node?.bounds?.y || 0) - (before.node.bounds.y || 0);
            const movedDistance = Math.hypot(dx, dy);
            done({
              passed: Boolean(aligned(during) && aligned(after) && movedDistance >= 20),
              moved, nodeId: before.node.id,
              movedDistance: Number(movedDistance.toFixed(2)),
              duringAligned: aligned(during),
              afterAligned: aligned(after),
              before: { x: before.node.bounds.x, y: before.node.bounds.y },
              during: during ? { x: during.node.bounds.x, y: during.node.bounds.y, labelLeft: during.rect.left, expectedLeft: during.expected.x } : null,
              after: after ? { x: after.node.bounds.x, y: after.node.bounds.y, labelLeft: after.rect.left, expectedLeft: after.expected.x } : null,
              pageErrors: window.__capyPageErrors || [],
              consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === "error")
            });
          }, 120);
        }, 120);
      }, 80);
    } catch (error) {
      done({ passed: false, reason: String(error), pageErrors: window.__capyPageErrors || [] });
    }
  });
}

/* ─── canvas state sync ─── */
function refreshPlannerContext() {
  const snapshot = normalizeValue(ai_snapshot()) || {};
  const context = normalizeValue(selected_context()) || { selected_count: 0, items: [] };
  const nodes = Array.isArray(snapshot.nodes) ? snapshot.nodes : [];
  const selectedItem = Array.isArray(context.items) ? context.items[0] || null : null;
  registerCanvasNodes(nodes);
  applyTimelineAttachments(nodes);
  state.blocks = nodes;
  state.selectedId = selectedItem?.id || null;
  state.canvas.ready = true;
  state.canvas.nodeCount = Number(shape_count()) || nodes.length;
  state.canvas.selectedNode = selectedItem;
  state.canvas.currentTool = current_tool();
  state.canvas.darkMode = Boolean(dark_mode());
  state.canvas.viewport = snapshot.viewport || null;
  state.canvas.snapshotText = ai_snapshot_text();
  state.planner.context = context;
  state.planner.contextText = selected_context_text();
  syncCanvasContext(selectedItem, snapshot.viewport || null);
  renderPosterOverlays(nodes, state.selectedId, snapshot.viewport || null);
  renderNodeLabels(nodes, state.selectedId, snapshot.viewport || null);
  renderRegionOverlay();
  renderPlannerContext(selectedItem);
  syncTimelineInspector(selectedItem);
  updateCanvasStatus(`${state.canvas.nodeCount} nodes · ${state.canvas.currentTool}`);
  return stateSnapshot();
}

function registerCanvasNodes(nodes) {
  const ids = nodes
    .map((node) => Number(node?.id))
    .filter((id) => Number.isFinite(id) && id >= 0)
    .sort((a, b) => a - b);
  const key = ids.join(",");
  if (!ids.length || key === nodeRegistry.key) return;
  nodeRegistry.key = key;
  rpc("canvas-nodes-register", { ids }).catch(() => {
    nodeRegistry.key = "";
  });
}

const TYPE_DOTS = {
  brand: "#fbbf24", image: "#f9a8d4", video: "#a78bfa", web: "#84cc16",
  "timeline-composition": "#34d399",
  text: "#9ca3af", default: "#a78bfa"
};
const TYPE_ICONS = {
  brand: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="3.5"/><circle cx="12" cy="12" r="8.5" stroke-dasharray="2 3"/></svg>',
  image: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><circle cx="9" cy="10" r="1.6"/><path d="M4.5 17.5l4.5-4 4 3 3.5-2.5 3 2.5"/></svg>',
  video: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3.5" y="5.5" width="17" height="13" rx="2"/><path d="M10.5 9.5l4.5 2.5-4.5 2.5z" fill="currentColor"/></svg>',
  "timeline-composition": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="4" y="5" width="16" height="14" rx="2"/><path d="M8 9h8M8 13h5M7 17h10"/></svg>',
  web: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.6 3 2.6 14 0 17M12 3.5c-2.6 3-2.6 14 0 17"/></svg>',
  default: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="8"/></svg>'
};

function inferType(node) {
  const componentKind = String(node?.capyComponentKind || node?.component_kind || "").toLowerCase();
  if (componentKind === "timeline-composition") return "timeline-composition";
  const k = String(node?.content_kind || "").toLowerCase();
  if (k === "brand") return "brand";
  if (k === "image") return "image";
  if (k === "video") return "video";
  if (k === "web") return "web";
  if (k === "text") return "text";
  return "default";
}

function renderPosterOverlays(nodes, selectedId, viewport) {
  if (!posterLayerEl) return;
  const existing = new Map(
    Array.from(posterLayerEl.querySelectorAll("[data-poster-node-id]")).map((node) => [
      node.dataset.posterNodeId,
      node
    ])
  );
  for (const node of nodes) {
    if (!node || node.content_kind !== "poster" || !node.bounds) continue;
    const nodeId = String(node.id);
    const entry = posterDocuments.get(nodeId);
    if (!entry) continue;
    let preview = existing.get(nodeId);
    if (!preview) {
      preview = document.createElement("div");
      preview.className = "poster-preview";
      preview.tabIndex = 0;
      preview.setAttribute("role", "button");
      preview.innerHTML = `
        <div class="poster-preview-head">
          <strong></strong>
          <span></span>
        </div>
        <div class="poster-preview-frame"></div>
        <div class="poster-preview-error"></div>
      `;
      posterLayerEl.append(preview);
      preview.addEventListener("click", () => selectNode(node.id));
      preview.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          selectNode(node.id);
        }
      });
    }
    existing.delete(nodeId);
    const box = nodeOverlayBox(node, viewport);
    preview.dataset.posterNodeId = nodeId;
    preview.dataset.renderState = entry.renderState;
    preview.className = `poster-preview${String(node.id) === String(selectedId) ? " is-selected" : ""}`;
    preview.style.left = "0";
    preview.style.top = "0";
    preview.style.transform = `translate3d(${box.x}px, ${box.y}px, 0)`;
    preview.style.width = `${box.w}px`;
    preview.style.height = `${box.h}px`;
    preview.querySelector("strong").textContent = node.title || "Poster document";
    preview.querySelector("span").textContent = entry.renderState === "error-preserved"
      ? "error preserved"
      : "JSON -> HTML";
    const frame = preview.querySelector(".poster-preview-frame");
    const scale = box.w / Number(entry.document.canvas.width || 1920);
    frame.replaceChildren(renderPosterStage(entry.document, {
      scale,
      selectedLayerId: entry.selectedLayerId || state.poster.selectedLayerId
    }));
    const errorNode = preview.querySelector(".poster-preview-error");
    errorNode.textContent = entry.error || "";
  }
  for (const orphan of existing.values()) {
    orphan.remove();
  }
}

function renderNodeLabels(nodes, selectedId, viewport) {
  if (!labelLayerEl) return;
  const existing = new Map(
    Array.from(labelLayerEl.querySelectorAll("[data-node-id]")).map((label) => [label.dataset.nodeId, label])
  );
  for (const node of nodes) {
    if (!node || !node.bounds) continue;
    if (node.content_kind === "poster") continue;
    const nodeId = String(node.id);
    let skin = existing.get(nodeId);
    if (!skin) {
      skin = document.createElement("div");
      skin.className = "node-label";
      skin.setAttribute("aria-hidden", "true");
      skin.innerHTML = `
        <div class="node-head">
          <span class="node-dot"></span>
          <span class="node-icon"></span>
          <span class="node-type"></span>
        </div>
        <strong class="node-title"></strong>
        <span class="node-meta"></span>
      `;
      labelLayerEl.append(skin);
    }
    existing.delete(nodeId);
    skin.dataset.nodeId = nodeId;
    const type = inferType(node);
    skin.dataset.capyComponentKind = type === "timeline-composition" ? type : "";
    skin.dataset.capyTimelineState = node.timeline?.state || "";
    skin.classList.toggle("is-selected", String(node.id) === String(selectedId));
    skin.querySelector(".node-dot").style.background = TYPE_DOTS[type] || TYPE_DOTS.default;
    skin.querySelector(".node-icon").innerHTML = TYPE_ICONS[type] || TYPE_ICONS.default;
    skin.querySelector(".node-type").textContent = type === "timeline-composition"
      ? "timeline"
      : String(node.content_kind || "node").toLowerCase();
    skin.querySelector(".node-title").textContent = node.title || `Node ${node.id}`;
    skin.querySelector(".node-meta").textContent = type === "timeline-composition"
      ? (node.timeline?.state || "preview-ready")
      : (node.next_action || "ready");
    const box = nodeLabelBox(node, viewport);
    const zoom = Number(viewport?.zoom) || 1;
    const w = Math.max(160, Math.round((node.bounds.w || 200) * zoom));
    const h = Math.max(86, Math.round((node.bounds.h || 120) * zoom));
    skin.style.left = "0";
    skin.style.top = "0";
    skin.style.transform = `translate3d(${box.x}px, ${box.y}px, 0)`;
    skin.style.width = `${w}px`;
    skin.style.height = `${h}px`;
  }
  for (const orphan of existing.values()) orphan.remove();
}

async function attachTimelineComposition(canvasNodeId, compositionPath) {
  return rpc("timeline-attach", {
    canvas_node_id: Number(canvasNodeId),
    composition_path: compositionPath
  });
}

async function openTimelineComposition(canvasNodeId) {
  const report = await rpc("timeline-open", {
    canvas_node_id: Number(canvasNodeId)
  });
  mountTimelinePreview(String(canvasNodeId), report.preview_url);
  return report;
}

async function openTimelineInspector(canvasNodeId) {
  const nodeId = String(canvasNodeId);
  showTimelineInspector(nodeId);
  if (state.timeline.inspector.loading && state.timeline.inspector.nodeId === nodeId) {
    return state.timeline.inspector.detail;
  }
  state.timeline.inspector.loading = true;
  state.timeline.inspector.nodeId = nodeId;
  state.timeline.inspector.error = null;
  renderTimelineInspector();
  try {
    const detail = await rpc("timeline-state-detail", {
      canvas_node_id: Number(canvasNodeId)
    });
    if (state.timeline.inspector.nodeId !== nodeId) return detail;
    state.timeline.inspector.detail = detail.attachment || null;
    state.timeline.inspector.error = null;
    renderTimelineInspector();
    return detail;
  } catch (error) {
    if (state.timeline.inspector.nodeId !== nodeId) throw error;
    state.timeline.inspector.error = stringifyError(error);
    renderTimelineInspector();
    throw error;
  } finally {
    if (state.timeline.inspector.nodeId === nodeId) {
      state.timeline.inspector.loading = false;
      renderTimelineInspector();
    }
  }
}

function syncTimelineInspector(selectedItem) {
  const selectedBlock = state.blocks.find((node) => String(node.id) === String(selectedItem?.id));
  if (!selectedBlock || inferType(selectedBlock) !== "timeline-composition") {
    hideTimelineInspector();
    return;
  }
  const nodeId = String(selectedBlock.id);
  if (state.timeline.inspector.nodeId === nodeId && state.timeline.inspector.detail) {
    showTimelineInspector(nodeId);
    return;
  }
  openTimelineInspector(nodeId).catch(() => {});
}

function showTimelineInspector(nodeId) {
  if (!nextFrameInspectorEl) return;
  nextFrameInspectorEl.hidden = false;
  state.timeline.inspector.nodeId = String(nodeId);
}

function hideTimelineInspector() {
  if (nextFrameInspectorEl) nextFrameInspectorEl.hidden = true;
  state.timeline.inspector.nodeId = null;
  state.timeline.inspector.detail = null;
  state.timeline.inspector.error = null;
}

function renderTimelineInspector() {
  if (!nextFrameInspectorEl || !nextFrameInspectorStagesEl) return;
  const inspector = state.timeline.inspector;
  const detail = inspector.detail;
  if (inspector.nodeId || detail || inspector.loading || inspector.error) {
    nextFrameInspectorEl.hidden = false;
  }
  nextFrameInspectorTitleEl.textContent = detail
    ? `Node ${detail.canvas_node_id}`
    : `Node ${inspector.nodeId || ""}`;
  nextFrameInspectorStatusEl.textContent = inspector.loading
    ? "loading"
    : inspector.error
      ? "error"
      : stageLabel(detail?.state || "idle");
  nextFrameInspectorStatusEl.dataset.status = inspector.error ? "error" : stageLabel(detail?.state || "idle");
  if (inspector.error) {
    nextFrameInspectorStagesEl.replaceChildren(inspectorMessage("State detail unavailable", inspector.error));
    return;
  }
  if (!detail) {
    nextFrameInspectorStagesEl.replaceChildren(inspectorMessage("Waiting for selection", "Select an attached Timeline composition."));
    return;
  }
  nextFrameInspectorStagesEl.replaceChildren(
    stageCard("Source", "asset", sourceRows(detail.source)),
    stageCard("Composition", "json", compositionRows(detail.composition)),
    stageCard("Compile", detail.compile?.status || "missing", compileRows(detail.compile)),
    stageCard("Export", exportStatus(detail.export_jobs), exportRows(detail.export_jobs)),
    stageCard("Evidence", detail.evidence?.exists ? "ready" : "missing", evidenceRows(detail.evidence))
  );
}

function handleCanvasNodeAttached(detail) {
  if (!detail) return;
  const nodeId = String(detail.canvas_node_id);
  state.timeline.attachments.set(nodeId, {
    state: detail.state || "preview-ready",
    composition_ref: detail.composition_ref || null
  });
  const node = state.blocks.find((item) => String(item.id) === nodeId);
  if (node) {
    applyTimelineAttachment(node, state.timeline.attachments.get(nodeId));
  }
  const escapedNodeId = window.CSS?.escape ? CSS.escape(nodeId) : nodeId.replace(/"/g, '\\"');
  const label = labelLayerEl?.querySelector(`[data-node-id="${escapedNodeId}"]`);
  if (label) {
    label.dataset.capyComponentKind = "timeline-composition";
    label.dataset.capyTimelineState = detail.state || "preview-ready";
    const type = label.querySelector(".node-type");
    const meta = label.querySelector(".node-meta");
    if (type) type.textContent = "timeline";
    if (meta) meta.textContent = detail.state || "preview-ready";
  }
  scheduleCanvasLabelRefresh();
}

function handleTimelineOpened(detail) {
  if (!detail) return;
  mountTimelinePreview(String(detail.canvas_node_id), detail.preview_url);
}

function mountTimelinePreview(nodeId, previewUrl) {
  if (!labelLayerEl || !previewUrl) return null;
  const escapedNodeId = window.CSS?.escape ? CSS.escape(nodeId) : nodeId.replace(/"/g, '\\"');
  const label = labelLayerEl.querySelector(`[data-node-id="${escapedNodeId}"]`);
  if (!label) return null;
  label.dataset.capyComponentKind = "timeline-composition";
  if (!label.dataset.capyTimelineState) label.dataset.capyTimelineState = "preview-ready";
  let iframe = label.querySelector("iframe[data-capy-timeline-preview]");
  if (!iframe) {
    iframe = document.createElement("iframe");
    iframe.dataset.capyTimelinePreview = "";
    iframe.title = `Timeline preview ${nodeId}`;
    iframe.loading = "lazy";
    iframe.sandbox = "allow-scripts allow-same-origin";
    label.append(iframe);
  }
  iframe.src = previewUrl;
  return iframe;
}

function applyTimelineAttachments(nodes) {
  for (const node of nodes) {
    const attachment = state.timeline.attachments.get(String(node?.id));
    if (attachment) applyTimelineAttachment(node, attachment);
  }
}

function applyTimelineAttachment(node, attachment) {
  if (!node || !attachment) return;
  node.capyComponentKind = "timeline-composition";
  node.component_kind = "timeline-composition";
  node.timeline = attachment;
  if (!node.content_kind || node.content_kind === "video") node.content_kind = "video";
}

function nodeLabelBox(node, viewport) {
  const zoom = Number(viewport?.zoom) || 1;
  const offset = viewport?.camera_offset || { x: 0, y: 0 };
  return {
    x: Math.round(node.bounds.x * zoom + (Number(offset.x) || 0)),
    y: Math.round(node.bounds.y * zoom + (Number(offset.y) || 0))
  };
}

function nodeOverlayBox(node, viewport) {
  const base = nodeLabelBox(node, viewport);
  const zoom = Number(viewport?.zoom) || 1;
  return {
    ...base,
    w: Math.max(220, Math.round(Number(node.bounds.w || 360) * zoom)),
    h: Math.max(124, Math.round(Number(node.bounds.h || 202.5) * zoom))
  };
}

function installCanvasLabelSync() {
  if (labelSync.installed || !canvasEl) return;
  labelSync.installed = true;
  canvasEl.addEventListener("pointerdown", startLiveCanvasLabelRefresh);
  canvasEl.addEventListener("pointermove", scheduleCanvasLabelRefresh, { passive: true });
  canvasEl.addEventListener("wheel", scheduleCanvasLabelRefresh, { passive: true });
  canvasEl.addEventListener("keyup", scheduleCanvasLabelRefresh);
  window.addEventListener("pointerup", stopLiveCanvasLabelRefresh);
  window.addEventListener("pointercancel", stopLiveCanvasLabelRefresh);
  window.addEventListener("blur", stopLiveCanvasLabelRefresh);
}
function scheduleCanvasLabelRefresh() {
  if (labelSync.refreshFrame) return;
  labelSync.refreshFrame = requestAnimationFrame(() => {
    labelSync.refreshFrame = 0;
    refreshPlannerContext();
  });
}
function startLiveCanvasLabelRefresh() {
  labelSync.liveRefreshActive = true;
  if (labelSync.liveRefreshFrame) return;
  const tick = () => {
    if (!labelSync.liveRefreshActive) {
      labelSync.liveRefreshFrame = 0;
      return;
    }
    refreshPlannerContext();
    labelSync.liveRefreshFrame = requestAnimationFrame(tick);
  };
  labelSync.liveRefreshFrame = requestAnimationFrame(tick);
}
function stopLiveCanvasLabelRefresh() {
  labelSync.liveRefreshActive = false;
  scheduleCanvasLabelRefresh();
}

function verifyPosterRenderer() {
  return new Promise((resolve) => {
    const done = (value) => resolve(normalizeValue(value));
    try {
      let current = refreshPlannerContext();
      let posterNode = current.blocks.find((node) => node.content_kind === "poster");
      if (!posterNode) {
        loadPosterDocument(cloneDefaultPosterDocument(), {
          title: "Verification poster",
          x: 360,
          y: 118,
          sourcePath: "fixture://poster/verification"
        });
        current = refreshPlannerContext();
        posterNode = current.blocks.find((node) => node.content_kind === "poster");
      }
      if (!posterNode) {
        done({ passed: false, reason: "poster node not found" });
        return;
      }

      selectNode(posterNode.id);
      const initial = posterOverlaySample(posterNode.id);
      const entry = posterDocuments.get(String(posterNode.id));
      const edited = cloneDocument(entry.document);
      const headline = edited.layers.find((layer) => layer.id === "headline");
      if (headline) {
        headline.text = "LOCAL\nPOSTER";
        headline.x = 132;
      }
      const editResult = updatePosterDocument(posterNode.id, edited);
      const afterEdit = posterOverlaySample(posterNode.id);
      const invalidResult = updatePosterDocument(posterNode.id, "{ invalid poster json");
      const afterInvalid = posterOverlaySample(posterNode.id);

      const beforeMoveNode = refreshPlannerContext().blocks.find((node) => String(node.id) === String(posterNode.id));
      const beforeMove = posterOverlaySample(posterNode.id);
      moveNodeById(posterNode.id, beforeMoveNode.bounds.x + 72, beforeMoveNode.bounds.y + 44);
      setTimeout(() => {
        const afterMove = posterOverlaySample(posterNode.id);
        const movedDistance = Math.hypot(
          (afterMove?.node?.bounds?.x || 0) - (beforeMove?.node?.bounds?.x || 0),
          (afterMove?.node?.bounds?.y || 0) - (beforeMove?.node?.bounds?.y || 0)
        );
        const pageErrors = window.__capyPageErrors || [];
        const consoleErrors = (window.__capyConsoleEvents || []).filter((event) => event.level === "error");
        done({
          passed: Boolean(
            initial?.layerCount >= 3
            && afterEdit?.headline === "LOCAL\nPOSTER"
            && invalidResult.render_state === "error-preserved"
            && afterInvalid?.headline === "LOCAL\nPOSTER"
            && movedDistance > 20
            && afterMove?.aligned
            && pageErrors.length === 0
            && consoleErrors.length === 0
          ),
          node_id: Number(posterNode.id),
          initial,
          editResult,
          afterEdit,
          invalidResult,
          afterInvalid,
          beforeMove,
          afterMove,
          movedDistance: Number(movedDistance.toFixed(2)),
          pageErrors,
          consoleErrors,
          poster_state: posterStateForNode(posterNode.id)
        });
      }, 120);
    } catch (error) {
      done({
        passed: false,
        reason: stringifyError(error),
        pageErrors: window.__capyPageErrors || [],
        consoleErrors: (window.__capyConsoleEvents || []).filter((event) => event.level === "error")
      });
    }
  });
}

function posterOverlaySample(nodeId) {
  const current = refreshPlannerContext();
  const node = current.blocks.find((item) => String(item.id) === String(nodeId));
  const overlay = posterLayerEl?.querySelector(`[data-poster-node-id="${nodeId}"]`);
  const stage = overlay?.querySelector(".poster-stage");
  const headline = stage?.querySelector('[data-layer-id="headline"]');
  if (!node || !overlay || !stage) return null;
  const viewport = current.canvas?.viewport || { zoom: 1, camera_offset: { x: 0, y: 0 } };
  const box = nodeOverlayBox(node, viewport);
  const layerRect = posterLayerEl.getBoundingClientRect();
  const rect = overlay.getBoundingClientRect();
  return {
    node,
    renderState: overlay.dataset.renderState,
    headline: headline?.textContent || null,
    layerCount: stage.querySelectorAll("[data-layer-id]").length,
    rect: {
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height
    },
    expected: {
      left: layerRect.left + box.x,
      top: layerRect.top + box.y
    },
    aligned: Math.abs(rect.left - (layerRect.left + box.x)) <= 10
      && Math.abs(rect.top - (layerRect.top + box.y)) <= 10
  };
}

function posterStateForNode(nodeId) {
  const entry = posterDocuments.get(String(nodeId));
  if (!entry) return null;
  return {
    node_id: Number(nodeId),
    source_path: entry.sourcePath || null,
    ...buildPosterState(entry.document, entry.renderState, entry.error)
  };
}

function posterDocumentsState() {
  return Array.from(posterDocuments.keys()).map((nodeId) => posterStateForNode(nodeId));
}
function renderPlannerContext(item) {
  if (!contextTitleEl || !contextMetaEl) return;
  if (contextAttachmentsEl) contextAttachmentsEl.innerHTML = "";
  plannerContextEl?.classList.toggle("is-region", state.canvasContext.context?.kind === "image_region");
  if (!item) {
    contextTitleEl.textContent = "No selection";
    contextMetaEl.textContent = "选择左侧节点 · Planner 围绕该对象工作";
    return;
  }
  const active = state.canvasContext.context;
  contextTitleEl.textContent = active?.kind === "image_region"
    ? `Region · ${item.title || `Node ${item.id}`}`
    : item.title || `Node ${item.id}`;
  const region = active?.region_bounds_world;
  const detail = region
    ? [
      contentKindLabel(item.content_kind),
      `id=${item.id}`,
      `x=${Math.round(region.x)} y=${Math.round(region.y)} w=${Math.round(region.w)} h=${Math.round(region.h)}`
    ].join(" · ")
    : [
      contentKindLabel(item.content_kind),
      `id=${item.id}`,
      item.source_path ? "source ready" : null,
      item.next_action,
      item.editor_route
    ].filter(Boolean).join(" · ");
  contextMetaEl.textContent = detail || "Planner context is ready.";
  renderContextChips(active);
}

function composePromptWithContext(prompt) {
  const context = state.planner.contextText || selected_context_text();
  const packet = state.canvasContext.context || activeCanvasContext();
  const trimmed = prompt.trim();
  if (!packet && !context.trim()) return trimmed;
  const packetLines = packet ? [
    `context_id=${packet.context_id}`,
    `kind=${packet.kind}`,
    `source_node_id=${packet.source_node_id}`,
    `source_node_title=${packet.source_node_title}`,
    `source_path=${packet.source_path || "none"}`,
    packet.region_bounds_world
      ? `region_world=${JSON.stringify(packet.region_bounds_world)}`
      : null,
    `expected_attachments=${(packet.expected_attachments || []).join(",")}`
  ].filter(Boolean).join("\n") : "";
  return `${trimmed}\n\n[Canvas context packet]\n${packetLines}\n\n[Canvas selection]\n${context}`.trim();
}

function renderContextChips(active) {
  if (!contextAttachmentsEl || !active) return;
  const chips = [
    active.context_id,
    active.kind === "image_region" ? "region.png" : "selected-node.png",
    "viewport.png"
  ];
  for (const chip of chips.filter(Boolean)) {
    const node = document.createElement("span");
    node.className = "context-chip";
    node.textContent = chip;
    contextAttachmentsEl.append(node);
  }
}

/* ─── conversations / messages ─── */
async function createConversation() {
  const data = await rpc("conversation-create", {
    provider: providerEl.value,
    cwd: cwdEl.value.trim() || "/Users/Zhuanz/workspace/capybara",
    model: modelEl.value.trim() || null,
    config: currentConfig()
  });
  await refreshList();
  await openConversation(data.conversation.id);
}

async function refreshList() {
  const data = await rpc("conversation-list", {});
  state.dbPath = data.db_path || state.dbPath;
  state.conversations = data.conversations || [];
  renderConversations();
  renderRuntimeFoot();
}

async function openConversation(id) {
  if (!id) return;
  const detail = await rpc("conversation-open", { id });
  state.activeId = detail.conversation.id;
  state.messages = detail.messages || [];
  providerEl.value = detail.conversation.provider;
  cwdEl.value = detail.conversation.cwd;
  modelEl.value = detail.conversation.model || "";
  effortEl.value = detail.conversation.config?.effort || "";
  const policy = detail.conversation.provider === "claude"
    ? detail.conversation.config?.permissionMode
    : detail.conversation.config?.approvalPolicy;
  policyEl.value = policy || "";
  sandboxEl.value = detail.conversation.config?.sandbox || "";
  serviceTierEl.value = detail.conversation.config?.serviceTier || "";
  systemPromptEl.value = detail.conversation.config?.systemPrompt || "";
  appendSystemPromptEl.value = detail.conversation.config?.appendSystemPrompt || "";
  developerInstructionsEl.value = detail.conversation.config?.developerInstructions || "";
  addDirsEl.value = (detail.conversation.config?.addDirs || []).join(", ");
  allowedToolsEl.value = detail.conversation.config?.allowedTools || "";
  disallowedToolsEl.value = detail.conversation.config?.disallowedTools || "";
  mcpConfigEl.value = detail.conversation.config?.mcpConfig || "";
  modelProviderEl.value = detail.conversation.config?.modelProvider || "";
  approvalsReviewerEl.value = detail.conversation.config?.approvalsReviewer || "";
  reasoningSummaryEl.value = detail.conversation.config?.reasoningSummary || "";
  outputSchemaEl.value = detail.conversation.config?.outputSchema || "";
  bareEl.checked = Boolean(detail.conversation.config?.bare);
  searchEl.checked = Boolean(detail.conversation.config?.search);
  writeCodeEl.checked = Boolean(detail.conversation.config?.writeCode);
  setRunStatus(detail.conversation.status || "idle");
  syncPolicyOptions();
  applyWriteCodeDefaults();
  renderConversations();
  renderMessages();
  renderRuntimeFoot();
  updateConfigSummary();
}

async function updateConversationConfig() {
  if (!state.activeId) return;
  await rpc("conversation-update-config", {
    id: state.activeId,
    model: modelEl.value.trim() || null,
    config: currentConfig()
  });
  await refreshList();
}

function renderConversations() {
  listEl.innerHTML = "";
  for (const item of state.conversations) {
    const button = document.createElement("button");
    button.type = "button";
    const isRunning = (item.status || "").toLowerCase() === "running";
    button.className = `conversation-item${item.id === state.activeId ? " active" : ""}${isRunning ? " is-running" : ""}`;
    button.innerHTML = `<span class="dot"></span><span class="title"></span><span class="meta"></span>`;
    button.querySelector(".title").textContent = item.title;
    button.querySelector(".meta").textContent = `${item.provider} · ${item.status}`;
    button.addEventListener("click", () => openConversation(item.id).catch((error) => renderError(error)));
    listEl.append(button);
  }
}

function renderMessages() {
  messagesEl.innerHTML = "";
  if (state.messages.length === 0 && state.streaming.size === 0) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.textContent = "选中画布上的节点 · Planner 会围绕该对象工作。试试 ⌘K 打开生图工具。";
    messagesEl.append(empty);
    return;
  }
  for (const message of state.messages) messagesEl.append(messageNode(message.role, message.content));
  for (const content of state.streaming.values()) messagesEl.append(messageNode("assistant", content || "..."));
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function messageNode(role, content) {
  const node = document.createElement("article");
  node.className = `message ${role}`;
  if (role !== "user") {
    const label = document.createElement("div");
    label.className = "role";
    label.textContent = role;
    node.append(label);
  }
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = content;
  node.append(bubble);
  return node;
}

function renderError(error) {
  state.messages = [{ role: "system", content: stringifyError(error) }];
  renderMessages();
}

function stateSnapshot() {
  return normalizeValue({
    canvas: state.canvas,
    selectedId: state.selectedId,
    blocks: state.blocks,
    planner: state.planner,
    poster: {
      ...state.poster,
      documents: posterDocumentsState()
    },
    canvasContext: state.canvasContext.context
  });
}
