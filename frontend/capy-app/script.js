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
import { createCanvasContext } from "./app/canvas-context.js";
import { createCanvasRenderer } from "./app/canvas-renderer.js";
import { createCanvasWorkbench } from "./app/canvas-workbench.js";
import { createConversations } from "./app/conversations.js";
import { dom } from "./app/dom.js";
import { installIpcReceiver, installNativeWindowDrag, installShellEventListeners, createRpc } from "./app/ipc.js";
import { createRuntimeControls } from "./app/runtime-controls.js";
import { installShellUi } from "./app/shell-ui.js";
import { labelSync, nodeRegistry, pending, posterDocuments, state } from "./app/state.js";
import { base64ToBytes, contentKindLabel, nextFrame, normalizeValue, stringifyError } from "./app/utils.js";
import { createTimelineWorkbench } from "./app/timeline-workbench.js";
import { installWindowFacade } from "./app/window-facade.js";

const {
  topbar, cmdkTriggerEl, listEl, messagesEl, newChatEl, stopEl, runStatusEl,
  formEl, promptEl, configSummaryEl, configDialogEl, configDialogCloseEl,
  configDialogDoneEl, providerEl, cwdEl, modelEl, effortEl, policyEl, sandboxEl,
  serviceTierEl, systemPromptEl, appendSystemPromptEl, developerInstructionsEl,
  addDirsEl, allowedToolsEl, disallowedToolsEl, mcpConfigEl, modelProviderEl,
  approvalsReviewerEl, reasoningSummaryEl, outputSchemaEl, bareEl, searchEl,
  writeCodeEl, runtimeFootEl, canvasEl, canvasPanelEl, canvasStatusEl,
  posterLayerEl, labelLayerEl, regionLayerEl, regionModeEl, plannerContextEl,
  contextTitleEl, contextMetaEl, contextAttachmentsEl,
  timelineInspectorEl: nextFrameInspectorEl,
  timelineInspectorTitleEl: nextFrameInspectorTitleEl,
  timelineInspectorStatusEl: nextFrameInspectorStatusEl,
  timelineInspectorStagesEl: nextFrameInspectorStagesEl,
  cmdPaletteEl, cmdSearchEl, cmdCloseEl, cmdListEl, cmdToolEl, cmdToolBackEl,
  imageToolPromptEl, imageToolDryRunEl, imageToolLiveEl, imageToolStatusEl,
  imageToolMetaEl,
} = dom;

const rpc = createRpc(pending);
const runtimeApi = createRuntimeControls({ state, dom });
const {
  currentConfig, syncPolicyOptions, applyWriteCodeDefaults, updateConfigSummary,
  setRunStatus, renderRuntimeFoot, updateCanvasStatus,
} = runtimeApi;

let rendererApi;
let contextApi;
let workbenchApi;
let timelineApi;
let conversationsApi;

conversationsApi = createConversations({
  state, rpc, currentConfig, syncPolicyOptions, applyWriteCodeDefaults,
  updateConfigSummary, setRunStatus, renderRuntimeFoot, stringifyError,
  listEl, messagesEl, providerEl, cwdEl, modelEl, effortEl, policyEl, sandboxEl,
  serviceTierEl, systemPromptEl, appendSystemPromptEl, developerInstructionsEl,
  addDirsEl, allowedToolsEl, disallowedToolsEl, mcpConfigEl, modelProviderEl,
  approvalsReviewerEl, reasoningSummaryEl, outputSchemaEl, bareEl, searchEl, writeCodeEl,
});

rendererApi = createCanvasRenderer({
  state, posterDocuments, posterLayerEl, labelLayerEl, canvasEl, labelSync,
  renderPosterStage, buildPosterState, cloneDefaultPosterDocument, cloneDocument,
  normalizeValue, stringifyError,
  refreshPlannerContext: () => refreshPlannerContext(),
  selectNode: (...args) => workbenchApi.selectNode(...args),
  moveNodeById: (...args) => workbenchApi.moveNodeById(...args),
  loadPosterDocument: (...args) => workbenchApi.loadPosterDocument(...args),
  updatePosterDocument: (...args) => workbenchApi.updatePosterDocument(...args),
});

timelineApi = createTimelineWorkbench({
  state, labelLayerEl, nextFrameInspectorEl, nextFrameInspectorTitleEl,
  nextFrameInspectorStatusEl, nextFrameInspectorStagesEl, rpc, stringifyError,
  inspectorMessage, sourceRows, compositionRows, compileRows, exportRows,
  exportStatus, evidenceRows, stageCard, stageLabel,
  scheduleCanvasLabelRefresh: (...args) => rendererApi.scheduleCanvasLabelRefresh(...args),
  inferType: (...args) => rendererApi.inferType(...args),
});

contextApi = createCanvasContext({
  state, canvasEl, canvasPanelEl, regionLayerEl, regionModeEl, plannerContextEl,
  contextTitleEl, contextMetaEl, contextAttachmentsEl, clampRectToBounds,
  compactGeometry, nodeBounds, normalizeRect, regionPercent, roundGeometry,
  worldBoxToScreen, normalizeValue, contentKindLabel, selected_context_text,
  refreshPlannerContext: () => refreshPlannerContext(),
  selectNode: (...args) => workbenchApi.selectNode(...args),
  moveNodeById: (...args) => workbenchApi.moveNodeById(...args),
  nodeLabelBox: (...args) => rendererApi.nodeLabelBox(...args),
  startLiveCanvasLabelRefresh: (...args) => rendererApi.startLiveCanvasLabelRefresh(...args),
  scheduleCanvasLabelRefresh: (...args) => rendererApi.scheduleCanvasLabelRefresh(...args),
  stopLiveCanvasLabelRefresh: (...args) => rendererApi.stopLiveCanvasLabelRefresh(...args),
});

workbenchApi = createCanvasWorkbench({
  initCanvas, startCanvas, state, updateCanvasStatus,
  installCanvasLabelSync: (...args) => rendererApi.installCanvasLabelSync(...args),
  installCanvasRegionSelection: (...args) => contextApi.installCanvasRegionSelection(...args),
  nextFrame, stringifyError, renderError: (...args) => conversationsApi.renderError(...args),
  refreshPlannerContext: () => refreshPlannerContext(), create_content_card,
  create_poster_document_card, select_node, focus_node, move_node_by_id,
  add_image_asset_at, base64ToBytes, cloneDefaultPosterDocument, cloneDocument,
  parsePosterDocument, validatePosterDocument, posterDocuments,
  posterStateForNode: (...args) => rendererApi.posterStateForNode(...args),
  stateSnapshot: () => stateSnapshot(), rpc, imageToolPromptEl, imageToolStatusEl, imageToolMetaEl,
});

const shellUi = installShellUi({
  state, configDialogEl, configSummaryEl, configDialogCloseEl, configDialogDoneEl,
  cmdkTriggerEl, cmdPaletteEl, cmdSearchEl, cmdCloseEl, cmdListEl, cmdToolEl,
  cmdToolBackEl, imageToolPromptEl, updateConfigSummary,
  updateConversationConfig: (...args) => conversationsApi.updateConversationConfig(...args),
  renderRuntimeFoot, renderError: (...args) => conversationsApi.renderError(...args),
  defaultImagePrompt: (...args) => workbenchApi.defaultImagePrompt(...args),
  seedDemoCanvas: (...args) => workbenchApi.seedDemoCanvas(...args),
});

installWindowFacade({
  state,
  capyApi: {
    add_image_asset_at, ai_snapshot, ai_snapshot_text, create_content_card,
    create_poster_document_card, current_tool, dark_mode, focus_node, list_shapes,
    move_node_by_id, select_node, selected_context, selected_context_text, shape_count
  },
  workbenchApi: {
    composePromptWithContext: (...args) => contextApi.composePromptWithContext(...args),
    activeCanvasContext: (...args) => contextApi.activeCanvasContext(...args),
    setCanvasContextRegion: (...args) => contextApi.setCanvasContextRegion(...args),
    clearCanvasContextRegion: (...args) => contextApi.clearCanvasContextRegion(...args),
    refreshPlannerContext, seedDemoCanvas: (...args) => workbenchApi.seedDemoCanvas(...args),
    createContentCard: (...args) => workbenchApi.createContentCard(...args),
    insertImageFromBase64: (...args) => workbenchApi.insertImageFromBase64(...args),
    loadPosterDocument: (...args) => workbenchApi.loadPosterDocument(...args),
    updatePosterDocument: (...args) => workbenchApi.updatePosterDocument(...args),
    moveNodeById: (...args) => workbenchApi.moveNodeById(...args),
    focusNode: (...args) => workbenchApi.focusNode(...args),
    selectNode: (...args) => workbenchApi.selectNode(...args),
    scheduleCanvasLabelRefresh: (...args) => rendererApi.scheduleCanvasLabelRefresh(...args),
    startLiveCanvasLabelRefresh: (...args) => rendererApi.startLiveCanvasLabelRefresh(...args),
    stateSnapshot, attachTimelineComposition: (...args) => timelineApi.attachTimelineComposition(...args),
    openTimelineComposition: (...args) => timelineApi.openTimelineComposition(...args),
    openTimelineInspector: (...args) => timelineApi.openTimelineInspector(...args),
    startCanvasImageTool: (...args) => workbenchApi.startCanvasImageTool(...args),
    verifyCanvasImageTool: (...args) => workbenchApi.verifyCanvasImageTool(...args),
    verifyLabelMoveSync: (...args) => contextApi.verifyLabelMoveSync(...args),
    verifyPosterRenderer: (...args) => rendererApi.verifyPosterRenderer(...args),
    openCmdPalette: (...args) => shellUi.openCmdPalette(...args),
    closeCmdPalette: (...args) => shellUi.closeCmdPalette(...args),
  }
});

installNativeWindowDrag(topbar);
installIpcReceiver(pending);
installShellEventListeners({
  state,
  setRunStatus,
  renderMessages: (...args) => conversationsApi.renderMessages(...args),
  openConversation: (...args) => conversationsApi.openConversation(...args),
  renderError: (...args) => conversationsApi.renderError(...args),
  handleCanvasToolEvent: (...args) => workbenchApi.handleCanvasToolEvent(...args),
  renderCanvasToolStatus: (...args) => workbenchApi.renderCanvasToolStatus(...args),
  stringifyError,
  handleCanvasNodeAttached: (...args) => timelineApi.handleCanvasNodeAttached(...args),
  handleTimelineOpened: (...args) => timelineApi.handleTimelineOpened(...args),
});

newChatEl?.addEventListener("click", async () => {
  try { await conversationsApi.createConversation(); } catch (error) { conversationsApi.renderError(error); }
});

stopEl?.addEventListener("click", async () => {
  if (!state.activeId) return;
  try {
    await rpc("conversation-stop", { id: state.activeId });
    await conversationsApi.openConversation(state.activeId);
  } catch (error) { conversationsApi.renderError(error); }
});

regionModeEl?.addEventListener("click", () => {
  state.canvasContext.regionMode = !state.canvasContext.regionMode;
  contextApi.renderRegionMode();
});

formEl?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const prompt = promptEl.value.trim();
  if (!prompt) return;
  try {
    if (!state.activeId) await conversationsApi.createConversation();
    if (!state.activeId) return;
    promptEl.value = "";
    await conversationsApi.updateConversationConfig();
    const outboundPrompt = contextApi.composePromptWithContext(prompt);
    const canvasContext = contextApi.activeCanvasContext();
    state.planner.lastOutboundPrompt = outboundPrompt;
    state.messages.push({ id: `local-${Date.now()}`, role: "user", content: prompt });
    conversationsApi.renderMessages();
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
    conversationsApi.renderError(error);
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

imageToolDryRunEl?.addEventListener("click", () => runImageTool(false));
imageToolLiveEl?.addEventListener("click", () => runImageTool(true));
document.querySelectorAll(".view-tab").forEach((btn) => {
  btn.addEventListener("click", () => {
    document.querySelectorAll(".view-tab").forEach((b) => b.classList.toggle("active", b === btn));
  });
});

function runImageTool(live) {
  workbenchApi.startCanvasImageTool({ live }).catch((error) => {
    state.canvasTool.status = "error";
    state.canvasTool.error = stringifyError(error);
    workbenchApi.renderCanvasToolStatus();
  });
}

function refreshPlannerContext() {
  const snapshot = normalizeValue(ai_snapshot()) || {};
  const context = normalizeValue(selected_context()) || { selected_count: 0, items: [] };
  const nodes = Array.isArray(snapshot.nodes) ? snapshot.nodes : [];
  const selectedItem = Array.isArray(context.items) ? context.items[0] || null : null;
  registerCanvasNodes(nodes);
  timelineApi.applyTimelineAttachments(nodes);
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
  contextApi.syncCanvasContext(selectedItem, snapshot.viewport || null);
  rendererApi.renderPosterOverlays(nodes, state.selectedId, snapshot.viewport || null);
  rendererApi.renderNodeLabels(nodes, state.selectedId, snapshot.viewport || null);
  contextApi.renderRegionOverlay();
  contextApi.renderPlannerContext(selectedItem);
  timelineApi.syncTimelineInspector(selectedItem);
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

function stateSnapshot() {
  return normalizeValue({
    canvas: state.canvas,
    selectedId: state.selectedId,
    blocks: state.blocks,
    planner: state.planner,
    poster: {
      ...state.poster,
      documents: rendererApi.posterDocumentsState()
    },
    canvasContext: state.canvasContext.context
  });
}

async function init() {
  cwdEl.value = window.CAPYBARA_SESSION?.cwd || "/Users/Zhuanz/workspace/capybara";
  syncPolicyOptions();
  setRunStatus("idle");
  workbenchApi.renderCanvasToolStatus();
  conversationsApi.renderMessages();
  updateConfigSummary();
  await workbenchApi.initCanvasWorkbench();
  try {
    const data = await rpc("conversation-list", {});
    state.dbPath = data.db_path || null;
    state.conversations = data.conversations || [];
    conversationsApi.renderConversations();
    renderRuntimeFoot();
    if (state.conversations[0]) await conversationsApi.openConversation(state.conversations[0].id);
  } catch (error) {
    conversationsApi.renderError(error);
  }
}

init();
