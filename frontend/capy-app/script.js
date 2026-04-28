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

/* ─── DOM refs ─── */
const $ = (sel) => document.querySelector(sel);

const topbar = $(".topbar");
const cmdkTriggerEl = $("#cmdk-trigger");
const listEl = $("#conversation-list");
const messagesEl = $("#message-list");
const newChatEl = $("#new-chat");
const stopEl = $("#stop-run");
const runStatusEl = $("#run-status");
const formEl = $("#composer");
const promptEl = $("#prompt");
const configSummaryEl = $("#config-summary");
const configDialogEl = $("#config-dialog");
const configDialogCloseEl = $("#config-dialog-close");
const configDialogDoneEl = $("#config-dialog-done");
const providerEl = $("#provider");
const cwdEl = $("#cwd");
const modelEl = $("#model");
const effortEl = $("#effort");
const policyEl = $("#policy");
const sandboxEl = $("#sandbox");
const serviceTierEl = $("#service-tier");
const systemPromptEl = $("#system-prompt");
const appendSystemPromptEl = $("#append-system-prompt");
const developerInstructionsEl = $("#developer-instructions");
const addDirsEl = $("#add-dirs");
const allowedToolsEl = $("#allowed-tools");
const disallowedToolsEl = $("#disallowed-tools");
const mcpConfigEl = $("#mcp-config");
const modelProviderEl = $("#model-provider");
const approvalsReviewerEl = $("#approvals-reviewer");
const reasoningSummaryEl = $("#reasoning-summary");
const outputSchemaEl = $("#output-schema");
const bareEl = $("#bare");
const searchEl = $("#search");
const writeCodeEl = $("#write-code");
const runtimeFootEl = $("#runtime-foot");
const canvasEl = $("#capy-canvas");
const canvasPanelEl = $('[data-section="canvas-host"]');
const canvasStatusEl = $("#canvas-status");
const posterLayerEl = $("#poster-overlay-layer");
const labelLayerEl = $("#node-label-layer");
const regionLayerEl = $("#context-region-layer");
const regionModeEl = $("#region-mode");
const plannerContextEl = $("#planner-context");
const contextTitleEl = $("#context-title");
const contextMetaEl = $("#context-meta");
const contextAttachmentsEl = $("#context-attachments");
const nextFrameInspectorEl = $('[data-section="nextframe-inspector"]');
const nextFrameInspectorTitleEl = $("#nextframe-inspector-title");
const nextFrameInspectorStatusEl = $("#nextframe-inspector-status");
const nextFrameInspectorStagesEl = $("#nextframe-inspector-stages");
const cmdPaletteEl = $("#cmd-palette");
const cmdSearchEl = $("#cmd-search");
const cmdCloseEl = $("#cmd-close");
const cmdListEl = $("#cmd-list");
const cmdToolEl = $("#canvas-image-tool");
const cmdToolBackEl = $("#cmd-tool-back");
const imageToolPromptEl = $("#image-tool-prompt");
const imageToolDryRunEl = $("#image-tool-dry-run");
const imageToolLiveEl = $("#image-tool-live");
const imageToolStatusEl = $("#image-tool-status");
const imageToolMetaEl = $("#image-tool-meta");

let labelRefreshFrame = 0;
let liveLabelRefreshFrame = 0;
let liveLabelRefreshActive = false;
let canvasLabelSyncInstalled = false;

const pending = new Map();
const state = {
  conversations: [],
  activeId: null,
  messages: [],
  streaming: new Map(),
  dbPath: null,
  selectedId: null,
  blocks: [],
  canvas: {
    ready: false,
    nodeCount: 0,
    selectedNode: null,
    currentTool: "select",
    snapshotText: "",
    darkMode: false,
    error: null
  },
  planner: {
    context: null,
    contextText: "",
    lastOutboundPrompt: "",
    canvasContext: null
  },
  canvasContext: {
    regionMode: false,
    region: null,
    drag: null,
    context: null
  },
  canvasTool: {
    status: "idle",
    runId: null,
    lastResult: null,
    error: null
  },
  poster: {
    renderState: "idle",
    selectedLayerId: "headline",
    lastNodeId: null,
    lastError: null
  },
  nextframe: {
    attachments: new Map(),
    inspector: {
      nodeId: null,
      loading: false,
      detail: null,
      error: null
    }
  }
};

const posterDocuments = new Map();

window.CAPYBARA_STATE = state;
window.capy = {
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
};
window.capyWorkbench = {
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
  attachNextFrameComposition,
  openNextFrameComposition,
  openNextFrameInspector,
  startCanvasImageTool,
  verifyCanvasImageTool,
  verifyLabelMoveSync,
  verifyPosterRenderer,
  openCmdPalette,
  closeCmdPalette
};

/* ─── window drag (CEF native) ─── */
topbar?.addEventListener("mousedown", (event) => {
  if (event.button !== 0) return;
  const target = event.target;
  if (target instanceof HTMLElement && target.closest("button, input, a, select, [role=button]")) return;
  if (!window.ipc) return;
  window.ipc.postMessage(event.detail === 2 ? "maximize_toggle" : "drag_window");
});

/* ─── IPC bridge ─── */
window.__capyReceive = (response) => {
  const entry = pending.get(response.req_id);
  if (!entry) return;
  pending.delete(response.req_id);
  if (response.ok) entry.resolve(response.data);
  else entry.reject(response.error || { error: "request failed" });
};

window.addEventListener("capy:agent-event", (event) => {
  const detail = event.detail;
  if (!detail || detail.conversation_id !== state.activeId) return;
  if (detail.status) setRunStatus(detail.status);
  if (detail.kind === "assistant_delta") {
    const current = state.streaming.get(detail.run_id) || "";
    state.streaming.set(detail.run_id, current + (detail.delta || ""));
    renderMessages();
  } else if (detail.kind === "assistant_done" || detail.kind === "error") {
    state.streaming.delete(detail.run_id);
    openConversation(state.activeId).catch((error) => renderError(error));
  }
});

window.addEventListener("capy:canvas-tool-event", (event) => {
  handleCanvasToolEvent(event.detail).catch((error) => {
    state.canvasTool.status = "error";
    state.canvasTool.error = stringifyError(error);
    renderCanvasToolStatus();
  });
});

window.addEventListener("capy:canvas-node-attached", (event) => {
  handleCanvasNodeAttached(event.detail);
});

window.addEventListener("capy:nextframe-opened", (event) => {
  handleNextFrameOpened(event.detail);
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

function base64ToBytes(base64) {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
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

function nodeBounds(node) {
  return node?.bounds || node?.geometry || null;
}

function worldBoxToScreen(bounds, viewport) {
  const zoom = Number(viewport?.zoom) || 1;
  const offset = viewport?.camera_offset || { x: 0, y: 0 };
  return {
    x: Math.round(bounds.x * zoom + (Number(offset.x) || 0)),
    y: Math.round(bounds.y * zoom + (Number(offset.y) || 0)),
    w: Math.round(bounds.w * zoom),
    h: Math.round(bounds.h * zoom)
  };
}

function clampRectToBounds(rect, bounds) {
  if (!rect || !bounds) return null;
  const x1 = Math.max(bounds.x, rect.x);
  const y1 = Math.max(bounds.y, rect.y);
  const x2 = Math.min(bounds.x + bounds.w, rect.x + rect.w);
  const y2 = Math.min(bounds.y + bounds.h, rect.y + rect.h);
  if (x2 <= x1 || y2 <= y1) return null;
  return { x: x1, y: y1, w: x2 - x1, h: y2 - y1 };
}

function normalizeRect(x, y, w, h) {
  const nextX = Number(x) || 0;
  const nextY = Number(y) || 0;
  const nextW = Number(w) || 0;
  const nextH = Number(h) || 0;
  return {
    x: nextW < 0 ? nextX + nextW : nextX,
    y: nextH < 0 ? nextY + nextH : nextY,
    w: Math.abs(nextW),
    h: Math.abs(nextH)
  };
}

function roundGeometry(geometry) {
  return {
    x: round2(geometry.x),
    y: round2(geometry.y),
    w: round2(geometry.w),
    h: round2(geometry.h)
  };
}

function regionPercent(region, bounds) {
  if (!region || !bounds || !bounds.w || !bounds.h) return null;
  return {
    x: round4((region.x - bounds.x) / bounds.w),
    y: round4((region.y - bounds.y) / bounds.h),
    w: round4(region.w / bounds.w),
    h: round4(region.h / bounds.h)
  };
}

function compactGeometry(geometry) {
  return [geometry.x, geometry.y, geometry.w, geometry.h].map((value) => Math.round(Number(value) || 0)).join("-");
}

function contextSummary(node, region) {
  const title = node?.title || `Node ${node?.id || "unknown"}`;
  if (!region) {
    const label = node?.content_kind === "image" ? "selected image" : "selected node";
    return `${label} ${title} id=${node?.id}`;
  }
  return `region on ${title} id=${node?.id} bounds=${compactGeometry(region)}`;
}

function round2(value) {
  return Math.round((Number(value) || 0) * 100) / 100;
}

function round4(value) {
  return Math.round((Number(value) || 0) * 10000) / 10000;
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
  applyNextFrameAttachments(nodes);
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
  syncNextFrameInspector(selectedItem);
  updateCanvasStatus(`${state.canvas.nodeCount} nodes · ${state.canvas.currentTool}`);
  return stateSnapshot();
}

const TYPE_DOTS = {
  brand: "#fbbf24", image: "#f9a8d4", video: "#a78bfa", web: "#84cc16",
  "nextframe-composition": "#34d399",
  text: "#9ca3af", default: "#a78bfa"
};
const TYPE_ICONS = {
  brand: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="3.5"/><circle cx="12" cy="12" r="8.5" stroke-dasharray="2 3"/></svg>',
  image: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><circle cx="9" cy="10" r="1.6"/><path d="M4.5 17.5l4.5-4 4 3 3.5-2.5 3 2.5"/></svg>',
  video: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3.5" y="5.5" width="17" height="13" rx="2"/><path d="M10.5 9.5l4.5 2.5-4.5 2.5z" fill="currentColor"/></svg>',
  "nextframe-composition": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="4" y="5" width="16" height="14" rx="2"/><path d="M8 9h8M8 13h5M7 17h10"/></svg>',
  web: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.6 3 2.6 14 0 17M12 3.5c-2.6 3-2.6 14 0 17"/></svg>',
  default: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="8"/></svg>'
};

function inferType(node) {
  const componentKind = String(node?.capyComponentKind || node?.component_kind || "").toLowerCase();
  if (componentKind === "nextframe-composition") return "nextframe-composition";
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
    skin.dataset.capyComponentKind = type === "nextframe-composition" ? type : "";
    skin.dataset.capyNextframeState = node.nextframe?.state || "";
    skin.classList.toggle("is-selected", String(node.id) === String(selectedId));
    skin.querySelector(".node-dot").style.background = TYPE_DOTS[type] || TYPE_DOTS.default;
    skin.querySelector(".node-icon").innerHTML = TYPE_ICONS[type] || TYPE_ICONS.default;
    skin.querySelector(".node-type").textContent = type === "nextframe-composition"
      ? "nextframe"
      : String(node.content_kind || "node").toLowerCase();
    skin.querySelector(".node-title").textContent = node.title || `Node ${node.id}`;
    skin.querySelector(".node-meta").textContent = type === "nextframe-composition"
      ? (node.nextframe?.state || "preview-ready")
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

async function attachNextFrameComposition(canvasNodeId, compositionPath) {
  return rpc("nextframe-attach", {
    canvas_node_id: Number(canvasNodeId),
    composition_path: compositionPath
  });
}

async function openNextFrameComposition(canvasNodeId) {
  const report = await rpc("nextframe-open", {
    canvas_node_id: Number(canvasNodeId)
  });
  mountNextFramePreview(String(canvasNodeId), report.preview_url);
  return report;
}

async function openNextFrameInspector(canvasNodeId) {
  const nodeId = String(canvasNodeId);
  showNextFrameInspector(nodeId);
  if (state.nextframe.inspector.loading && state.nextframe.inspector.nodeId === nodeId) {
    return state.nextframe.inspector.detail;
  }
  state.nextframe.inspector.loading = true;
  state.nextframe.inspector.nodeId = nodeId;
  state.nextframe.inspector.error = null;
  renderNextFrameInspector();
  try {
    const detail = await rpc("nextframe-state-detail", {
      canvas_node_id: Number(canvasNodeId)
    });
    if (state.nextframe.inspector.nodeId !== nodeId) return detail;
    state.nextframe.inspector.detail = detail.attachment || null;
    state.nextframe.inspector.error = null;
    renderNextFrameInspector();
    return detail;
  } catch (error) {
    if (state.nextframe.inspector.nodeId !== nodeId) throw error;
    state.nextframe.inspector.error = stringifyError(error);
    renderNextFrameInspector();
    throw error;
  } finally {
    if (state.nextframe.inspector.nodeId === nodeId) {
      state.nextframe.inspector.loading = false;
      renderNextFrameInspector();
    }
  }
}

function syncNextFrameInspector(selectedItem) {
  const selectedBlock = state.blocks.find((node) => String(node.id) === String(selectedItem?.id));
  if (!selectedBlock || inferType(selectedBlock) !== "nextframe-composition") {
    hideNextFrameInspector();
    return;
  }
  const nodeId = String(selectedBlock.id);
  if (state.nextframe.inspector.nodeId === nodeId && state.nextframe.inspector.detail) {
    showNextFrameInspector(nodeId);
    return;
  }
  openNextFrameInspector(nodeId).catch(() => {});
}

function showNextFrameInspector(nodeId) {
  if (!nextFrameInspectorEl) return;
  nextFrameInspectorEl.hidden = false;
  state.nextframe.inspector.nodeId = String(nodeId);
}

function hideNextFrameInspector() {
  if (nextFrameInspectorEl) nextFrameInspectorEl.hidden = true;
  state.nextframe.inspector.nodeId = null;
  state.nextframe.inspector.detail = null;
  state.nextframe.inspector.error = null;
}

function renderNextFrameInspector() {
  if (!nextFrameInspectorEl || !nextFrameInspectorStagesEl) return;
  const inspector = state.nextframe.inspector;
  const detail = inspector.detail;
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
    nextFrameInspectorStagesEl.replaceChildren(inspectorMessage("Waiting for selection", "Select an attached NextFrame composition."));
    return;
  }
  nextFrameInspectorStagesEl.replaceChildren(
    stageCard("Source", "asset", sourceRows(detail.source)),
    stageCard("Composition", "json", compositionRows(detail.composition)),
    stageCard("Compile", detail.compile?.status || "missing", compileRows(detail.compile)),
    stageCard("Export", exportStatus(detail.export_jobs), exportRows(detail.export_jobs)),
    stageCard("Evidence", detail.evidence?.exists ? "linked" : "missing", evidenceRows(detail.evidence))
  );
}

function stageCard(title, status, rows) {
  const card = document.createElement("section");
  card.className = "inspector-stage";
  card.dataset.status = String(status || "idle");
  const head = document.createElement("header");
  head.innerHTML = `<span class="stage-icon"></span><h3></h3>`;
  head.querySelector("h3").textContent = title;
  const body = document.createElement("div");
  body.className = "stage-body";
  for (const row of rows) body.append(row);
  card.append(head, body);
  return card;
}

function sourceRows(source) {
  const posterRefs = Array.isArray(source?.poster_refs) ? source.poster_refs : [];
  const scrollRefs = Array.isArray(source?.scroll_media_refs) ? source.scroll_media_refs : [];
  return [
    kvRow("poster", refsText(posterRefs)),
    kvRow("scroll-media", refsText(scrollRefs)),
    kvRow("brand_tokens", source?.brand_tokens?.source_path || source?.brand_tokens?.tokens_ref || "none")
  ];
}

function compositionRows(composition) {
  return [
    linkRow("composition.json", composition?.path),
    codeRow(Array.isArray(composition?.preview_lines) ? composition.preview_lines.join("\n") : "")
  ];
}

function compileRows(compile) {
  return [
    kvRow("render_source.json", compile?.status || "missing"),
    linkRow("path", compile?.render_source_path),
    kvRow("compile_mode", compile?.compile_mode || "unknown"),
    kvRow("timestamp", compile?.timestamp || "not recorded")
  ];
}

function exportRows(jobs) {
  if (!Array.isArray(jobs) || jobs.length === 0) return [kvRow("jobs", "none")];
  return jobs.map((job) => {
    const row = document.createElement("div");
    row.className = "export-job-row";
    row.append(statusBadge(job.status), textSpan(job.output_path || job.job_id), textSpan(formatBytes(job.byte_size)));
    return row;
  });
}

function evidenceRows(evidence) {
  if (!evidence?.exists) return [kvRow("evidence/index.html", "not found")];
  return [linkRow("evidence/index.html", evidence.index_html)];
}

function kvRow(label, value) {
  const row = document.createElement("div");
  row.className = "stage-row";
  row.append(textSpan(label, "stage-key"), textSpan(value || "none", "stage-value"));
  return row;
}

function linkRow(label, path) {
  const row = kvRow(label, path || "missing");
  if (path) {
    const value = row.querySelector(".stage-value");
    const link = document.createElement("a");
    link.href = path;
    link.textContent = path;
    value.replaceChildren(link);
  }
  return row;
}

function codeRow(text) {
  const pre = document.createElement("pre");
  pre.className = "composition-preview";
  pre.textContent = text || "preview unavailable";
  return pre;
}

function statusBadge(status) {
  const badge = textSpan(stageLabel(status), "status-badge");
  badge.dataset.status = stageLabel(status);
  return badge;
}

function inspectorMessage(title, message) {
  const box = document.createElement("div");
  box.className = "inspector-message";
  box.append(textSpan(title, "message-title"), textSpan(message, "message-copy"));
  return box;
}

function textSpan(value, className = "") {
  const span = document.createElement("span");
  if (className) span.className = className;
  span.textContent = String(value || "");
  return span;
}

function refsText(refs) {
  if (!refs.length) return "none";
  return refs.map((item) => item.source_path || item.original_path || item.src || item.id).filter(Boolean).join(", ");
}

function exportStatus(jobs) {
  if (!Array.isArray(jobs) || jobs.length === 0) return "idle";
  if (jobs.some((job) => stageLabel(job.status) === "failed")) return "failed";
  if (jobs.some((job) => stageLabel(job.status) === "running")) return "running";
  if (jobs.some((job) => stageLabel(job.status) === "done")) return "done";
  return stageLabel(jobs[0].status);
}

function stageLabel(value) {
  if (!value) return "idle";
  if (typeof value === "string") return value;
  if (value.error) return "error";
  return String(value);
}

function formatBytes(value) {
  const bytes = Number(value);
  if (!Number.isFinite(bytes) || bytes <= 0) return "bytes unknown";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function handleCanvasNodeAttached(detail) {
  if (!detail) return;
  const nodeId = String(detail.canvas_node_id);
  state.nextframe.attachments.set(nodeId, {
    state: detail.state || "preview-ready",
    composition_ref: detail.composition_ref || null
  });
  const node = state.blocks.find((item) => String(item.id) === nodeId);
  if (node) {
    applyNextFrameAttachment(node, state.nextframe.attachments.get(nodeId));
  }
  const escapedNodeId = window.CSS?.escape ? CSS.escape(nodeId) : nodeId.replace(/"/g, '\\"');
  const label = labelLayerEl?.querySelector(`[data-node-id="${escapedNodeId}"]`);
  if (label) {
    label.dataset.capyComponentKind = "nextframe-composition";
    label.dataset.capyNextframeState = detail.state || "preview-ready";
    const type = label.querySelector(".node-type");
    const meta = label.querySelector(".node-meta");
    if (type) type.textContent = "nextframe";
    if (meta) meta.textContent = detail.state || "preview-ready";
  }
  scheduleCanvasLabelRefresh();
}

function handleNextFrameOpened(detail) {
  if (!detail) return;
  mountNextFramePreview(String(detail.canvas_node_id), detail.preview_url);
}

function mountNextFramePreview(nodeId, previewUrl) {
  if (!labelLayerEl || !previewUrl) return null;
  const escapedNodeId = window.CSS?.escape ? CSS.escape(nodeId) : nodeId.replace(/"/g, '\\"');
  const label = labelLayerEl.querySelector(`[data-node-id="${escapedNodeId}"]`);
  if (!label) return null;
  label.dataset.capyComponentKind = "nextframe-composition";
  if (!label.dataset.capyNextframeState) label.dataset.capyNextframeState = "preview-ready";
  let iframe = label.querySelector("iframe[data-capy-nextframe-preview]");
  if (!iframe) {
    iframe = document.createElement("iframe");
    iframe.dataset.capyNextframePreview = "";
    iframe.title = `NextFrame preview ${nodeId}`;
    iframe.loading = "lazy";
    iframe.sandbox = "allow-scripts allow-same-origin";
    label.append(iframe);
  }
  iframe.src = previewUrl;
  return iframe;
}

function applyNextFrameAttachments(nodes) {
  for (const node of nodes) {
    const attachment = state.nextframe.attachments.get(String(node?.id));
    if (attachment) applyNextFrameAttachment(node, attachment);
  }
}

function applyNextFrameAttachment(node, attachment) {
  if (!node || !attachment) return;
  node.capyComponentKind = "nextframe-composition";
  node.component_kind = "nextframe-composition";
  node.nextframe = attachment;
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
  if (canvasLabelSyncInstalled || !canvasEl) return;
  canvasLabelSyncInstalled = true;
  canvasEl.addEventListener("pointerdown", startLiveCanvasLabelRefresh);
  canvasEl.addEventListener("pointermove", scheduleCanvasLabelRefresh, { passive: true });
  canvasEl.addEventListener("wheel", scheduleCanvasLabelRefresh, { passive: true });
  canvasEl.addEventListener("keyup", scheduleCanvasLabelRefresh);
  window.addEventListener("pointerup", stopLiveCanvasLabelRefresh);
  window.addEventListener("pointercancel", stopLiveCanvasLabelRefresh);
  window.addEventListener("blur", stopLiveCanvasLabelRefresh);
}
function scheduleCanvasLabelRefresh() {
  if (labelRefreshFrame) return;
  labelRefreshFrame = requestAnimationFrame(() => {
    labelRefreshFrame = 0;
    refreshPlannerContext();
  });
}
function startLiveCanvasLabelRefresh() {
  liveLabelRefreshActive = true;
  if (liveLabelRefreshFrame) return;
  const tick = () => {
    if (!liveLabelRefreshActive) { liveLabelRefreshFrame = 0; return; }
    refreshPlannerContext();
    liveLabelRefreshFrame = requestAnimationFrame(tick);
  };
  liveLabelRefreshFrame = requestAnimationFrame(tick);
}
function stopLiveCanvasLabelRefresh() {
  liveLabelRefreshActive = false;
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

/* ─── config helpers ─── */
function currentConfig() {
  const config = { capyCanvasTools: true };
  if (effortEl.value) config.effort = effortEl.value;
  if (providerEl.value === "claude" && policyEl.value) config.permissionMode = policyEl.value;
  if (providerEl.value === "codex" && policyEl.value) config.approvalPolicy = policyEl.value;
  if (sandboxEl.value) config.sandbox = sandboxEl.value;
  if (serviceTierEl.value.trim()) config.serviceTier = serviceTierEl.value.trim();
  if (systemPromptEl.value.trim()) config.systemPrompt = systemPromptEl.value.trim();
  if (appendSystemPromptEl.value.trim()) config.appendSystemPrompt = appendSystemPromptEl.value.trim();
  if (developerInstructionsEl.value.trim()) config.developerInstructions = developerInstructionsEl.value.trim();
  const addDirs = addDirsEl.value.split(",").map((value) => value.trim()).filter(Boolean);
  if (addDirs.length) config.addDirs = addDirs;
  if (allowedToolsEl.value.trim()) config.allowedTools = allowedToolsEl.value.trim();
  if (disallowedToolsEl.value.trim()) config.disallowedTools = disallowedToolsEl.value.trim();
  if (mcpConfigEl.value.trim()) config.mcpConfig = mcpConfigEl.value.trim();
  if (modelProviderEl.value.trim()) config.modelProvider = modelProviderEl.value.trim();
  if (approvalsReviewerEl.value) config.approvalsReviewer = approvalsReviewerEl.value;
  if (reasoningSummaryEl.value) config.reasoningSummary = reasoningSummaryEl.value;
  if (outputSchemaEl.value.trim()) config.outputSchema = outputSchemaEl.value.trim();
  if (bareEl.checked) config.bare = true;
  if (searchEl.checked) config.search = true;
  if (writeCodeEl.checked) {
    config.writeCode = true;
    if (providerEl.value === "codex" && !config.approvalPolicy) config.approvalPolicy = "never";
    if (providerEl.value === "claude" && !config.permissionMode) config.permissionMode = "bypassPermissions";
    if (!config.sandbox) config.sandbox = "danger-full-access";
    config.allowDangerouslySkipPermissions = true;
    config.dangerouslySkipPermissions = true;
  }
  return config;
}

function syncPolicyOptions() {
  const provider = providerEl.value;
  const options = provider === "claude"
    ? [["", "policy"], ["default", "default"], ["acceptEdits", "accept edits"], ["plan", "plan"], ["dontAsk", "dont ask"], ["bypassPermissions", "bypass"]]
    : [["", "policy"], ["on-request", "on request"], ["never", "never"], ["untrusted", "untrusted"]];
  const current = policyEl.value;
  policyEl.innerHTML = "";
  for (const [value, label] of options) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    policyEl.append(option);
  }
  policyEl.value = options.some(([value]) => value === current) ? current : "";
}

function applyWriteCodeDefaults() {
  if (!writeCodeEl.checked) return;
  if (providerEl.value === "codex") policyEl.value = "never";
  else policyEl.value = "bypassPermissions";
  sandboxEl.value = "danger-full-access";
}

function updateConfigSummary() {
  if (!configSummaryEl) return;
  const provider = providerEl?.value || "claude";
  const effort = effortEl?.value || "default";
  const policy = policyEl?.value || "default";
  configSummaryEl.textContent = `${provider} · ${effort} · ${policy}`;
}

function setRunStatus(status) {
  runStatusEl.textContent = status || "idle";
  runStatusEl.dataset.status = status || "idle";
}

function renderRuntimeFoot() {
  const provider = providerEl.value === "claude" ? "Claude Code" : "Codex CLI";
  runtimeFootEl.textContent = `${provider} · Canvas CLI tools active · ${state.dbPath || "SQLite store pending"}`;
}

function updateCanvasStatus(text) {
  if (canvasStatusEl) canvasStatusEl.textContent = text;
}

function contentKindLabel(value) {
  if (value === "poster") return "poster";
  return String(value || "shape").replace(/_/g, " ");
}

/* ─── utils ─── */
function normalizeValue(value) {
  if (value === null || value === undefined) return value;
  if (typeof value === "bigint") return Number(value);
  if (Array.isArray(value)) return value.map(normalizeValue);
  if (typeof value === "object") {
    const normalized = {};
    for (const [key, inner] of Object.entries(value)) normalized[key] = normalizeValue(inner);
    return normalized;
  }
  return value;
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

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

function stringifyError(error) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.stack || error.message;
  try { return JSON.stringify(error, null, 2); } catch (_err) { return String(error); }
}

function rpc(op, params) {
  return new Promise((resolve, reject) => {
    if (!window.ipc) { reject({ error: "Capybara shell IPC unavailable" }); return; }
    const id = `ui-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    pending.set(id, { resolve, reject });
    window.ipc.postMessage(JSON.stringify({ kind: "rpc", id, op, params }));
  });
}
