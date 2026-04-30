import initCanvas, {
  add_image_asset_at,
  ai_snapshot,
  ai_snapshot_text,
  create_content_card,
  create_poster_document_card,
  center_view_on,
  current_tool,
  dark_mode,
  fit_view_to_content,
  focus_node,
  list_shapes,
  move_node_by_id,
  pan_view_by,
  reset_view,
  select_node,
  selected_context,
  selected_context_text,
  set_vector_style,
  set_tool,
  shape_count,
  zoom_view_at,
  start as startCanvas
} from "./canvas-pkg/capy_canvas_web.js";
import { buildPosterState, cloneDefaultPosterDocument, cloneDocument, parsePosterDocument, renderPosterStage, validatePosterDocument } from "./poster-renderer.js";
import { clampRectToBounds, compactGeometry, nodeBounds, normalizeRect, regionPercent, roundGeometry, worldBoxToScreen } from "./workbench/geometry.js";
import { compileRows, compositionRows, evidenceRows, exportRows, exportStatus, inspectorMessage, sourceRows, stageCard, stageLabel } from "./timeline/inspector-render.js";
import { createCanvasContext } from "./app/canvas-context.js";
import { createCanvasControls } from "./app/canvas-controls.js";
import { createCanvasRenderer } from "./app/canvas-renderer.js";
import { createCanvasWorkbench } from "./app/canvas-workbench.js";
import { createConversations } from "./app/conversations.js";
import { dom } from "./app/dom.js";
import { installIpcReceiver, installNativeWindowDrag, installShellEventListeners, createRpc } from "./app/ipc.js";
import { createProjectPackageWiring } from "./app/project-package-wiring.js";
import { createRuntimeControls } from "./app/runtime-controls.js";
import { installShellUi } from "./app/shell-ui.js";
import { labelSync, nodeRegistry, pending, posterDocuments, state } from "./app/state.js";
import { createStateSnapshot } from "./app/state-snapshot.js";
import { base64ToBytes, contentKindLabel, nextFrame, normalizeValue, stringifyError } from "./app/utils.js";
import { createGameAssetsWorkspace } from "./app/game-assets-workspace.js";
import { createPosterWorkspace } from "./app/poster-workspace.js";
import { createTimelineWorkbench } from "./app/timeline-workbench.js";
import { createVideoEditor } from "./app/video-editor.js";
import { installWindowFacade } from "./app/window-facade.js";

const {
  topbar, cmdkTriggerEl, listEl, messagesEl, newChatEl, stopEl, runStatusEl,
  formEl, promptEl, configSummaryEl, configDialogEl, configDialogCloseEl,
  configDialogDoneEl, providerEl, cwdEl, modelEl, effortEl, policyEl, sandboxEl,
  serviceTierEl, systemPromptEl, appendSystemPromptEl, developerInstructionsEl,
  addDirsEl, allowedToolsEl, disallowedToolsEl, mcpConfigEl, modelProviderEl,
  approvalsReviewerEl, reasoningSummaryEl, outputSchemaEl, bareEl, searchEl,
  writeCodeEl, runtimeFootEl, canvasEl, canvasPanelEl, canvasStatusEl,
  canvasToolButtonsEl, canvasColorButtonsEl, canvasZoomButtonsEl, canvasZoomValueEl,
  miniMapEl, miniMapNodesEl, miniMapViewportEl,
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
let controlsApi;
let workbenchApi;
let timelineApi;
let videoEditorApi;
let posterWorkspaceApi;
let gameAssetsWorkspaceApi;
let conversationsApi;
let projectPackageApi;

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

const stateSnapshot = createStateSnapshot({
  state,
  normalizeValue,
  posterDocumentsState: () => rendererApi.posterDocumentsState()
});

timelineApi = createTimelineWorkbench({
  state, labelLayerEl, nextFrameInspectorEl, nextFrameInspectorTitleEl,
  nextFrameInspectorStatusEl, nextFrameInspectorStagesEl, rpc, stringifyError,
  inspectorMessage, sourceRows, compositionRows, compileRows, exportRows,
  exportStatus, evidenceRows, stageCard, stageLabel,
  scheduleCanvasLabelRefresh: (...args) => rendererApi.scheduleCanvasLabelRefresh(...args),
  inferType: (...args) => rendererApi.inferType(...args),
});

posterWorkspaceApi = createPosterWorkspace({
  state, dom, rpc, stringifyError,
});

gameAssetsWorkspaceApi = createGameAssetsWorkspace({
  state, dom, stringifyError,
});

videoEditorApi = createVideoEditor({
  state, dom, rpc, stringifyError, setRunStatus,
  renderPosterWorkspace: (...args) => posterWorkspaceApi.renderPosterWorkspace(...args),
  ensurePosterDocument: (...args) => posterWorkspaceApi.ensureDefaultDocument(...args),
  renderGameAssetsWorkspace: (...args) => gameAssetsWorkspaceApi.renderGameAssetsWorkspace(...args),
  ensureGameAssetsPack: (...args) => gameAssetsWorkspaceApi.ensureDefaultPack(...args),
});

projectPackageApi = createProjectPackageWiring({
  state,
  rpc,
  dom,
  stringifyError,
  appendPlannerMessage: (message) => {
    state.messages.push({ id: `project-${Date.now()}`, ...message });
    conversationsApi.renderMessages();
  },
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

controlsApi = createCanvasControls({
  state,
  canvasToolButtonsEl,
  canvasColorButtonsEl,
  canvasZoomButtonsEl,
  canvasZoomValueEl,
  miniMapEl,
  miniMapNodesEl,
  miniMapViewportEl,
  set_tool,
  set_vector_style,
  center_view_on,
  zoom_view_at,
  pan_view_by,
  reset_view,
  fit_view_to_content,
  refreshPlannerContext: () => refreshPlannerContext(),
  stringifyError,
});
controlsApi.installCanvasControls();
posterWorkspaceApi.installPosterWorkspace();
gameAssetsWorkspaceApi.installGameAssetsWorkspace();
videoEditorApi.installVideoEditor();

const shellUi = installShellUi({
  state, configDialogEl, configSummaryEl, configDialogCloseEl, configDialogDoneEl,
  cmdkTriggerEl, cmdPaletteEl, cmdSearchEl, cmdCloseEl, cmdListEl, cmdToolEl,
  cmdToolBackEl, imageToolPromptEl, updateConfigSummary,
  updateConversationConfig: (...args) => conversationsApi.updateConversationConfig(...args),
  renderRuntimeFoot, renderError: (...args) => conversationsApi.renderError(...args),
  defaultImagePrompt: (...args) => workbenchApi.defaultImagePrompt(...args),
});

installWindowFacade({
  state,
  capyApi: {
    add_image_asset_at, ai_snapshot, ai_snapshot_text, create_content_card,
    create_poster_document_card, center_view_on, current_tool, dark_mode, fit_view_to_content,
    focus_node, list_shapes, move_node_by_id, pan_view_by, reset_view, select_node,
    selected_context, selected_context_text, set_vector_style, set_tool, shape_count, zoom_view_at
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
    loadProjectPackage: (...args) => projectPackageApi.loadProjectPackage(...args),
    buildSelectedProjectContext: (...args) => projectPackageApi.buildSelectedContext(...args),
    generateSelectedProjectArtifact: (...args) => projectPackageApi.generateSelectedArtifact(...args),
    switchWorkspaceTab: (...args) => videoEditorApi.switchWorkspace(...args),
    openVideoComposition: (...args) => videoEditorApi.openComposition(...args),
    renderVideoEditor: (...args) => videoEditorApi.renderVideoEditor(...args),
    openPosterDocument: (...args) => posterWorkspaceApi.openDocument(...args),
    renderPosterWorkspace: (...args) => posterWorkspaceApi.renderPosterWorkspace(...args),
    openGameAssetsPack: (...args) => gameAssetsWorkspaceApi.openPack(...args),
    renderGameAssetsWorkspace: (...args) => gameAssetsWorkspaceApi.renderGameAssetsWorkspace(...args),
    startCanvasImageTool: (...args) => workbenchApi.startCanvasImageTool(...args),
    verifyCanvasImageTool: (...args) => workbenchApi.verifyCanvasImageTool(...args),
    verifyLabelMoveSync: (...args) => contextApi.verifyLabelMoveSync(...args),
    verifyPosterRenderer: (...args) => rendererApi.verifyPosterRenderer(...args),
    setCanvasTool: (...args) => controlsApi.setCanvasTool(...args),
    setCanvasZoom: (...args) => controlsApi.setCanvasZoom(...args),
    centerFromMiniMapPoint: (...args) => controlsApi.centerFromMiniMapPoint(...args),
    openCmdPalette: (...args) => shellUi.openCmdPalette(...args),
    closeCmdPalette: (...args) => shellUi.closeCmdPalette(...args),
    setPlannerMessages: (messages = []) => {
      state.messages = Array.isArray(messages) ? messages : [];
      state.streaming.clear();
      conversationsApi.renderMessages();
      return { messages: state.messages.length, streaming: state.streaming.size };
    },
    setPlannerStreaming: (content = "") => {
      state.streaming.clear();
      state.streaming.set("verify", { content: String(content || ""), segments: [] });
      setRunStatus("running");
      conversationsApi.renderMessages();
      return { runStatus: state.planner.runStatus, streaming: state.streaming.size };
    },
    setPlannerRunStatus: (status = "idle") => {
      setRunStatus(status);
      if (status !== "running") state.streaming.clear();
      conversationsApi.renderMessages();
      return { runStatus: state.planner.runStatus, streaming: state.streaming.size };
    },
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

[effortEl, modelEl, cwdEl, policyEl].forEach((el) => {
  const eventName = el === modelEl || el === cwdEl ? "input" : "change";
  el?.addEventListener(eventName, () => updateConfigSummary());
});

imageToolDryRunEl?.addEventListener("click", () => runImageTool(false));
imageToolLiveEl?.addEventListener("click", () => runImageTool(true));

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
  const semanticNodes = nodes.filter((node) => !isVectorGraphic(node));
  const vectorNodes = nodes.filter(isVectorGraphic);
  const selectedItem = Array.isArray(context.items) ? context.items[0] || null : null;
  const selectedNode = selectedItem && !isVectorGraphic(selectedItem) ? selectedItem : null;
  const selectedVector = selectedItem && isVectorGraphic(selectedItem) ? selectedItem : null;
  registerCanvasNodes(semanticNodes);
  timelineApi.applyTimelineAttachments(semanticNodes);
  state.blocks = semanticNodes;
  state.selectedId = selectedNode?.id || null;
  state.canvas.ready = true;
  state.canvas.nodeCount = semanticNodes.length;
  state.canvas.vectorCount = vectorNodes.length;
  state.canvas.objectCount = Number(shape_count()) || nodes.length;
  state.canvas.objects = nodes;
  state.canvas.selectedNode = selectedNode;
  state.canvas.selectedVector = selectedVector;
  state.canvas.currentTool = current_tool();
  state.canvas.darkMode = Boolean(dark_mode());
  state.canvas.viewport = snapshot.viewport || null;
  state.canvas.snapshotText = ai_snapshot_text();
  state.planner.context = selectedNode ? context : { selected_count: 0, items: [] };
  state.planner.contextText = selectedNode ? selected_context_text() : "";
  contextApi.syncCanvasContext(selectedNode, snapshot.viewport || null);
  rendererApi.renderPosterOverlays(semanticNodes, state.selectedId, snapshot.viewport || null);
  rendererApi.renderNodeLabels(semanticNodes, state.selectedId, snapshot.viewport || null);
  controlsApi?.renderCanvasControls(snapshot);
  contextApi.renderRegionOverlay();
  contextApi.renderPlannerContext(selectedNode);
  timelineApi.syncTimelineInspector(selectedNode);
  updateCanvasStatus(canvasStatusLabel(state.canvas));
  return stateSnapshot();
}

function isVectorGraphic(node) {
  return String(node?.content_kind || "").toLowerCase() === "shape";
}

function canvasStatusLabel(canvas) {
  const parts = [];
  if (canvas.nodeCount) parts.push(`${canvas.nodeCount} ${canvas.nodeCount === 1 ? "node" : "nodes"}`);
  if (canvas.vectorCount) parts.push(`${canvas.vectorCount} ${canvas.vectorCount === 1 ? "vector" : "vectors"}`);
  if (!parts.length) parts.push("empty");
  return `${parts.join(" · ")} · ${canvas.currentTool || "select"}`;
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

async function init() {
  cwdEl.value = window.CAPYBARA_SESSION?.cwd || "/Users/Zhuanz/workspace/capybara";
  syncPolicyOptions();
  applyWriteCodeDefaults();
  setRunStatus("idle");
  workbenchApi.renderCanvasToolStatus();
  conversationsApi.renderMessages();
  updateConfigSummary();
  await workbenchApi.initCanvasWorkbench();
  await projectPackageApi.loadProjectPackage();
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
