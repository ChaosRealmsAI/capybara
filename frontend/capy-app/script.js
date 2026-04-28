import initCanvas, {
  add_image_asset_at,
  ai_snapshot,
  ai_snapshot_text,
  create_content_card,
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

const topbar = document.querySelector(".topbar");
const listEl = document.querySelector("#conversation-list");
const messagesEl = document.querySelector("#message-list");
const titleEl = document.querySelector("#chat-title");
const subtitleEl = document.querySelector("#chat-subtitle");
const newChatEl = document.querySelector("#new-chat");
const stopEl = document.querySelector("#stop-run");
const formEl = document.querySelector("#composer");
const promptEl = document.querySelector("#prompt");
const providerEl = document.querySelector("#provider");
const cwdEl = document.querySelector("#cwd");
const modelEl = document.querySelector("#model");
const effortEl = document.querySelector("#effort");
const policyEl = document.querySelector("#policy");
const sandboxEl = document.querySelector("#sandbox");
const serviceTierEl = document.querySelector("#service-tier");
const runStatusEl = document.querySelector("#run-status");
const systemPromptEl = document.querySelector("#system-prompt");
const appendSystemPromptEl = document.querySelector("#append-system-prompt");
const developerInstructionsEl = document.querySelector("#developer-instructions");
const addDirsEl = document.querySelector("#add-dirs");
const allowedToolsEl = document.querySelector("#allowed-tools");
const disallowedToolsEl = document.querySelector("#disallowed-tools");
const mcpConfigEl = document.querySelector("#mcp-config");
const modelProviderEl = document.querySelector("#model-provider");
const approvalsReviewerEl = document.querySelector("#approvals-reviewer");
const reasoningSummaryEl = document.querySelector("#reasoning-summary");
const outputSchemaEl = document.querySelector("#output-schema");
const bareEl = document.querySelector("#bare");
const searchEl = document.querySelector("#search");
const writeCodeEl = document.querySelector("#write-code");
const runtimeFootEl = document.querySelector("#runtime-foot");
const canvasEl = document.querySelector("#capy-canvas");
const canvasPanelEl = document.querySelector('[data-section="canvas-host"]');
const canvasStatusEl = document.querySelector("#canvas-status");
const labelLayerEl = document.querySelector("#node-label-layer");
const regionLayerEl = document.querySelector("#context-region-layer");
const regionModeEl = document.querySelector("#region-mode");
const plannerContextEl = document.querySelector("#planner-context");
const contextTitleEl = document.querySelector("#context-title");
const contextMetaEl = document.querySelector("#context-meta");
const contextAttachmentsEl = document.querySelector("#context-attachments");
const imageToolPromptEl = document.querySelector("#image-tool-prompt");
const imageToolDryRunEl = document.querySelector("#image-tool-dry-run");
const imageToolLiveEl = document.querySelector("#image-tool-live");
const imageToolStatusEl = document.querySelector("#image-tool-status");
const imageToolMetaEl = document.querySelector("#image-tool-meta");

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
  }
};

window.CAPYBARA_STATE = state;
window.capy = {
  add_image_asset_at,
  ai_snapshot,
  ai_snapshot_text,
  create_content_card,
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
  moveNodeById,
  focusNode,
  selectNode,
  scheduleCanvasLabelRefresh,
  startLiveCanvasLabelRefresh,
  stateSnapshot,
  startCanvasImageTool,
  verifyCanvasImageTool,
  verifyLabelMoveSync
};

topbar?.addEventListener("mousedown", (event) => {
  if (event.button !== 0) return;
  const target = event.target;
  if (target instanceof HTMLElement && target.closest("button, input, a, select, [role=button]")) {
    return;
  }
  if (!window.ipc) return;
  window.ipc.postMessage(event.detail === 2 ? "maximize_toggle" : "drag_window");
});

window.__capyReceive = (response) => {
  const entry = pending.get(response.req_id);
  if (!entry) return;
  pending.delete(response.req_id);
  if (response.ok) {
    entry.resolve(response.data);
  } else {
    entry.reject(response.error || { error: "request failed" });
  }
};

window.addEventListener("capy:agent-event", (event) => {
  const detail = event.detail;
  if (!detail || detail.conversation_id !== state.activeId) return;
  if (detail.status) {
    setRunStatus(detail.status);
  }
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

newChatEl?.addEventListener("click", async () => {
  try {
    await createConversation();
  } catch (error) {
    renderError(error);
  }
});

stopEl?.addEventListener("click", async () => {
  if (!state.activeId) return;
  try {
    await rpc("conversation-stop", { id: state.activeId });
    await openConversation(state.activeId);
  } catch (error) {
    renderError(error);
  }
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
    if (!state.activeId) {
      await createConversation();
    }
    if (!state.activeId) return;
    promptEl.value = "";
    await updateConversationConfig();
    const outboundPrompt = composePromptWithContext(prompt);
    const canvasContext = activeCanvasContext();
    state.planner.lastOutboundPrompt = outboundPrompt;
    state.messages.push({
      id: `local-${Date.now()}`,
      role: "user",
      content: prompt
    });
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
});

writeCodeEl?.addEventListener("change", () => {
  applyWriteCodeDefaults();
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

init();

async function init() {
  cwdEl.value = window.CAPYBARA_SESSION?.cwd || "/Users/Zhuanz/workspace/capybara";
  syncPolicyOptions();
  setRunStatus("idle");
  renderCanvasToolStatus();
  renderMessages();
  await initCanvasWorkbench();
  try {
    const data = await rpc("conversation-list", {});
    state.dbPath = data.db_path || null;
    state.conversations = data.conversations || [];
    renderConversations();
    renderRuntimeFoot();
    if (state.conversations[0]) {
      await openConversation(state.conversations[0].id);
    }
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
  if (state.blocks.length > 0 || state.canvas.nodeCount > 0) {
    return state.blocks;
  }
  create_content_card("brand", "Brand Kit", 110, 105);
  create_content_card("image", "主视觉候选 A", 410, 96);
  create_content_card("web", "Landing Draft", 650, 322);
  create_content_card("video", "Storyboard", 222, 392);
  refreshPlannerContext();
  const preferred = state.blocks.find((node) => node.title === "主视觉候选 A") || state.blocks[0];
  if (preferred) {
    selectNode(preferred.id);
  }
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
  if (!Number.isFinite(numericId) || !Number.isFinite(nextX) || !Number.isFinite(nextY)) {
    return false;
  }
  const ok = move_node_by_id(numericId, nextX, nextY);
  refreshPlannerContext();
  return ok;
}

function createContentCard(kind, title, x, y) {
  const nextX = Number(x);
  const nextY = Number(y);
  if (!Number.isFinite(nextX) || !Number.isFinite(nextY)) {
    return { ok: false, error: "invalid position" };
  }
  const idx = create_content_card(kind, title, nextX, nextY);
  refreshPlannerContext();
  return {
    ok: true,
    index: Number(idx),
    selected_node: state.canvas.selectedNode,
    snapshot: stateSnapshot()
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
    nextX,
    nextY,
    bytes,
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
  if (imageToolPromptEl && !imageToolPromptEl.value.trim()) {
    imageToolPromptEl.value = prompt;
  }
  const placement = nextImagePlacement();
  state.canvasTool.status = "running";
  state.canvasTool.error = null;
  state.canvasTool.lastResult = null;
  renderCanvasToolStatus();
  const data = await rpc("canvas-generate-image", {
    prompt,
    provider: "apimart-gpt-image-2",
    size: "1:1",
    resolution: "1k",
    live,
    x: placement.x,
    y: placement.y,
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
    detail.image_base64,
    detail.title,
    detail.x,
    detail.y,
    {
      sourcePath: detail.source_path,
      provider: detail.provider,
      promptSummary: detail.prompt_summary
    }
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
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

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

function refreshPlannerContext() {
  const snapshot = normalizeValue(ai_snapshot()) || {};
  const context = normalizeValue(selected_context()) || { selected_count: 0, items: [] };
  const nodes = Array.isArray(snapshot.nodes) ? snapshot.nodes : [];
  const selectedItem = Array.isArray(context.items) ? context.items[0] || null : null;
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
  renderNodeLabels(nodes, state.selectedId, snapshot.viewport || null);
  renderRegionOverlay();
  renderPlannerContext(selectedItem);
  updateCanvasStatus(`${state.canvas.nodeCount} nodes · ${state.canvas.currentTool}`);
  return stateSnapshot();
}

function renderNodeLabels(nodes, selectedId, viewport) {
  if (!labelLayerEl) return;
  const existing = new Map(
    Array.from(labelLayerEl.querySelectorAll("[data-node-id]")).map((label) => [
      label.dataset.nodeId,
      label
    ])
  );
  for (const node of nodes) {
    if (!node || !node.bounds) continue;
    const nodeId = String(node.id);
    let label = existing.get(nodeId);
    if (!label) {
      label = document.createElement("button");
      label.type = "button";
      label.innerHTML = `<strong></strong><span></span>`;
      labelLayerEl.append(label);
    }
    existing.delete(nodeId);
    label.className = `node-label${String(node.id) === String(selectedId) ? " is-selected" : ""}`;
    label.dataset.nodeId = nodeId;
    const box = nodeLabelBox(node, viewport);
    label.style.left = "0";
    label.style.top = "0";
    label.style.transform = `translate3d(${box.x}px, ${box.y}px, 0)`;
    label.style.width = `${Math.max(150, Math.min(210, Math.round(node.bounds.w || 180)))}px`;
    label.querySelector("strong").textContent = node.title || `Node ${node.id}`;
    label.querySelector("span").textContent = `${contentKindLabel(node.content_kind)} · ${node.next_action || "ready"}`;
    label.onclick = () => selectNode(node.id);
  }
  for (const orphan of existing.values()) {
    orphan.remove();
  }
}

function nodeLabelBox(node, viewport) {
  const zoom = Number(viewport?.zoom) || 1;
  const offset = viewport?.camera_offset || { x: 0, y: 0 };
  return {
    x: Math.round(node.bounds.x * zoom + (Number(offset.x) || 0)),
    y: Math.round(node.bounds.y * zoom + (Number(offset.y) || 0))
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
    if (!liveLabelRefreshActive) {
      liveLabelRefreshFrame = 0;
      return;
    }
    refreshPlannerContext();
    liveLabelRefreshFrame = requestAnimationFrame(tick);
  };
  liveLabelRefreshFrame = requestAnimationFrame(tick);
}

function stopLiveCanvasLabelRefresh() {
  liveLabelRefreshActive = false;
  scheduleCanvasLabelRefresh();
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
          rect: {
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height
          },
          expected: {
            x: layerRect.left + box.x,
            y: layerRect.top + box.y
          },
          layerRect: {
            left: layerRect.left,
            top: layerRect.top
          },
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
              moved,
              nodeId: before.node.id,
              movedDistance: Number(movedDistance.toFixed(2)),
              duringAligned: aligned(during),
              afterAligned: aligned(after),
              before: { x: before.node.bounds.x, y: before.node.bounds.y },
              during: during ? {
                x: during.node.bounds.x,
                y: during.node.bounds.y,
                labelLeft: during.rect.left,
                expectedLeft: during.expected.x
              } : null,
              after: after ? {
                x: after.node.bounds.x,
                y: after.node.bounds.y,
                labelLeft: after.rect.left,
                expectedLeft: after.expected.x
              } : null,
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

function renderPlannerContext(item) {
  if (!contextTitleEl || !contextMetaEl) return;
  if (contextAttachmentsEl) contextAttachmentsEl.innerHTML = "";
  plannerContextEl?.classList.toggle("is-region", state.canvasContext.context?.kind === "image_region");
  if (!item) {
    contextTitleEl.textContent = "No selection";
    contextMetaEl.textContent = "选择左侧节点后，Planner 会围绕该对象工作。";
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
  titleEl.textContent = detail.conversation.title;
  subtitleEl.textContent = `${detail.conversation.provider} · ${detail.conversation.cwd}`;
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
    button.className = `conversation-item${item.id === state.activeId ? " active" : ""}`;
    button.type = "button";
    button.innerHTML = `
      <span class="title"></span>
      <span class="meta"></span>
    `;
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
    empty.textContent = "Select a canvas node, then ask Planner what to do next.";
    messagesEl.append(empty);
    return;
  }
  for (const message of state.messages) {
    messagesEl.append(messageNode(message.role, message.content));
  }
  for (const content of state.streaming.values()) {
    messagesEl.append(messageNode("assistant", content || "..."));
  }
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function messageNode(role, content) {
  const node = document.createElement("article");
  node.className = `message ${role}`;
  const label = document.createElement("div");
  label.className = "role";
  label.textContent = role;
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = content;
  node.append(label, bubble);
  return node;
}

function renderError(error) {
  state.messages = [{
    role: "system",
    content: stringifyError(error)
  }];
  renderMessages();
}

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
  if (providerEl.value === "codex") {
    policyEl.value = "never";
  } else {
    policyEl.value = "bypassPermissions";
  }
  sandboxEl.value = "danger-full-access";
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
  return String(value || "shape").replace(/_/g, " ");
}

function normalizeValue(value) {
  if (value === null || value === undefined) return value;
  if (typeof value === "bigint") return Number(value);
  if (Array.isArray(value)) return value.map(normalizeValue);
  if (typeof value === "object") {
    const normalized = {};
    for (const [key, inner] of Object.entries(value)) {
      normalized[key] = normalizeValue(inner);
    }
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
    canvasContext: state.canvasContext.context
  });
}

function nextFrame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

function stringifyError(error) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.stack || error.message;
  try {
    return JSON.stringify(error, null, 2);
  } catch (_err) {
    return String(error);
  }
}

function rpc(op, params) {
  return new Promise((resolve, reject) => {
    if (!window.ipc) {
      reject({ error: "Capybara shell IPC unavailable" });
      return;
    }
    const id = `ui-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    pending.set(id, { resolve, reject });
    window.ipc.postMessage(JSON.stringify({ kind: "rpc", id, op, params }));
  });
}
