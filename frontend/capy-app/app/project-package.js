import { projectGenerateMessageContent } from "./planner-message-whitelist.js";
import { createProjectArtifactNodes } from "./project-artifact-nodes.js";
import { createAiDiffReviewPanel, reviewRunId } from "./ai-diff.js";
import { renderDesignLanguageSummary, renderSelectionContext } from "./project-context-panels.js";
import { projectCampaignMessage, renderProjectCampaignSummary } from "./project-campaign-summary.js";
import {
  absoluteProjectPath,
  assetFileUrl,
  escapeText,
  firstSelectableCard,
  previewFrameSource,
  projectCardMetaHtml,
  selectedArtifactSummary,
  selectedCardSummary
} from "./project-package-helpers.js";

export function createProjectPackage({ state, rpc, dom, stringifyError, appendPlannerMessage, canvasApi = {} }) {
  const {
    projectPackagePanelEl,
    projectPackageTitleEl,
    projectPackageMetaEl,
    projectWorkbenchEl,
    projectWorkbenchCardsEl,
    projectSelectedSummaryEl,
    projectDesignLanguageEl,
    projectSelectionContextEl,
    projectCampaignSummaryEl,
    projectArtifactListEl,
    projectPreviewFrameEl,
    promptEl,
    providerEl,
    modelEl,
    effortEl,
  } = dom;
  const artifactNodes = createProjectArtifactNodes({ state, rpc, canvasApi, stringifyError });
  const aiDiffPanel = createAiDiffReviewPanel(projectPackagePanelEl);

  async function loadProjectPackage(projectPath = window.CAPYBARA_SESSION?.project) {
    if (!projectPath || projectPath === "demo") {
      state.projectPackage.status = "idle";
      renderProjectPackage();
      return { loaded: false, reason: "no project package path" };
    }
    state.projectPackage.path = projectPath;
    state.projectPackage.status = "loading";
    state.projectPackage.error = null;
    renderProjectPackage();
    try {
      const inspection = await rpc("project-inspect", { project: projectPath });
      const workbench = await rpc("project-workbench", { project: projectPath });
      const surfaceNodes = await artifactNodes.loadSurfaceNodes(projectPath);
      state.projectPackage.inspection = inspection;
      state.projectPackage.workbench = workbench;
      state.projectPackage.surfaceNodes = surfaceNodes;
      const firstCard = firstSelectableCard(workbench);
      state.projectPackage.selectedCardId = firstCard?.id || null;
      state.projectPackage.selectedArtifactId = firstCard?.id?.startsWith("art_") ? firstCard.id : null;
      state.projectPackage.previewSource = "";
      state.projectPackage.selectionContext = null;
      state.projectPackage.campaign = null;
      state.projectPackage.review = null;
      await refreshSelectedPreview();
      state.projectPackage.status = "ready";
      renderProjectPackage();
      await artifactNodes.syncProjectArtifactNodes();
      if (state.projectPackage.selectedArtifactId) artifactNodes.selectArtifactNode(state.projectPackage.selectedArtifactId);
      return { loaded: true, inspection };
    } catch (error) {
      state.projectPackage.status = "error";
      state.projectPackage.error = stringifyError(error);
      renderProjectPackage();
      return { loaded: false, error: state.projectPackage.error };
    }
  }

  async function buildSelectedContext(selection = null) {
    const artifact = selectedArtifact();
    if (!artifact || !state.projectPackage.path) {
      throw new Error("No selected project artifact");
    }
    const options = typeof selection === "string" ? { selector: selection } : (selection || {});
    const context = await rpc("context-build", {
      project: state.projectPackage.path,
      artifact: artifact.id,
      selector: options.selector || null,
      json_pointer: options.jsonPointer || options.json_pointer || null,
      canvas_node: options.canvasNode || options.canvas_node || null
    });
    state.projectPackage.selectionContext = context.selection_context || null;
    renderProjectPackage();
    return context;
  }

  async function generateSelectedArtifact(options = {}) {
    const artifact = selectedArtifact();
    if (!artifact || !state.projectPackage.path) {
      throw new Error("No selected project artifact");
    }
    const prompt = options.prompt || promptEl?.value.trim() || `Revise ${artifact.title || artifact.id} using project design language.`;
    const provider = options.provider || providerEl?.value || "codex";
    const live = options.live === undefined ? provider !== "fixture" : Boolean(options.live);
    const review = options.review === undefined ? true : Boolean(options.review);
    state.projectPackage.status = "generating";
    renderProjectPackage();
    try {
      const result = await rpc("project-generate", {
        project: state.projectPackage.path,
        artifact: artifact.id,
        provider,
        prompt,
        dry_run: options.dryRun === true ? true : false,
        review,
        live,
        model: options.model || modelEl?.value || null,
        effort: options.effort || effortEl?.value || null,
        sdk_response: options.sdkResponse || null,
        selector: options.selector || state.projectPackage.selectionContext?.selector || null,
        json_pointer: options.jsonPointer || options.json_pointer || state.projectPackage.selectionContext?.json_pointer || null,
        canvas_node: options.canvasNode || options.canvas_node || state.projectPackage.selectionContext?.surface_node_id || null
      });
      state.projectPackage.generation = result;
      state.projectPackage.review = result?.run?.review ? result : null;
      if (result.preview_source) state.projectPackage.previewSource = result.preview_source;
      state.projectPackage.workbench = await rpc("project-workbench", { project: state.projectPackage.path });
      state.projectPackage.status = "ready";
      renderProjectPackage();
      appendPlannerMessage?.(projectGenerateMessage(result, artifact));
      return result;
    } catch (error) {
      state.projectPackage.status = "error";
      state.projectPackage.error = stringifyError(error);
      renderProjectPackage();
      throw error;
    }
  }

  async function generateCampaign(options = {}) {
    if (!state.projectPackage.path) throw new Error("No project package loaded");
    const input = typeof options === "string" ? { brief: options } : options;
    const brief = input.brief || promptEl?.value.trim() || "Create one coherent campaign across project artifacts.";
    state.projectPackage.status = "generating";
    renderProjectPackage();
    try {
      const result = await rpc("project-campaign-generate", {
        project: state.projectPackage.path,
        brief,
        artifacts: input.artifacts || input.artifact_ids || []
      });
      state.projectPackage.campaign = result;
      state.projectPackage.review = result?.proposals?.[0] || null;
      state.projectPackage.workbench = await rpc("project-workbench", { project: state.projectPackage.path });
      state.projectPackage.status = "ready";
      renderProjectPackage();
      appendPlannerMessage?.(projectCampaignMessage(result));
      return result;
    } catch (error) {
      state.projectPackage.status = "error";
      state.projectPackage.error = stringifyError(error);
      renderProjectPackage();
      throw error;
    }
  }

  function selectedArtifact() {
    const artifacts = state.projectPackage.inspection?.artifacts?.artifacts || [];
    return artifacts.find((artifact) => artifact.id === state.projectPackage.selectedArtifactId) || null;
  }

  function renderProjectPackage() {
    if (!projectPackagePanelEl) return;
    const packageState = state.projectPackage;
    const inspection = packageState.inspection;
    const artifacts = inspection?.artifacts?.artifacts || [];
    const isVisible = packageState.status !== "idle" && (inspection || packageState.status === "loading" || packageState.status === "error" || packageState.workbench);
    renderWorkbench();
    projectPackagePanelEl.hidden = !isVisible;
    if (!isVisible) return;
    projectPackagePanelEl.dataset.status = packageState.status;
    if (projectPackageTitleEl) {
      projectPackageTitleEl.textContent = inspection?.manifest?.name || "Project package";
    }
    if (projectPackageMetaEl) {
      if (packageState.status === "loading") projectPackageMetaEl.textContent = "loading";
      else if (packageState.status === "generating") projectPackageMetaEl.textContent = "CLI generating";
      else if (packageState.status === "error") projectPackageMetaEl.textContent = packageState.error || "error";
      else projectPackageMetaEl.textContent = selectedArtifactSummary(state) || `${artifacts.length} artifacts`;
    }
    renderDesignLanguageSummary(projectDesignLanguageEl, inspection?.design_language_summary || packageState.workbench?.design_language_summary);
    renderSelectionContext(projectSelectionContextEl, packageState.selectionContext);
    renderProjectCampaignSummary(projectCampaignSummaryEl, packageState.campaign);
    renderArtifactList(artifacts);
    if (projectPreviewFrameEl) {
      projectPreviewFrameEl.srcdoc = previewFrameSource(selectedArtifact(), packageState.previewSource, packageState);
    }
    aiDiffPanel.render(packageState.review, {
      accept: () => acceptSelectedReview().catch(() => {}),
      reject: () => rejectSelectedReview().catch(() => {}),
      retry: () => retrySelectedReview().catch(() => {}),
      undo: () => undoSelectedReview().catch(() => {}),
    });
  }

  async function acceptSelectedReview() {
    const result = await decideReview("project-run-accept");
    await refreshSelectedPreview();
    renderProjectPackage();
    appendPlannerMessage?.(projectReviewMessage(result, "接受"));
    return result;
  }

  async function rejectSelectedReview() {
    const result = await decideReview("project-run-reject");
    await refreshSelectedPreview();
    renderProjectPackage();
    appendPlannerMessage?.(projectReviewMessage(result, "拒绝"));
    return result;
  }

  async function retrySelectedReview() {
    const runId = currentReviewRunId();
    if (!runId || !state.projectPackage.path) throw new Error("No review run selected");
    state.projectPackage.status = "generating";
    renderProjectPackage();
    try {
      const result = await rpc("project-run-retry", {
        project: state.projectPackage.path,
        run_id: runId,
        actor: "desktop"
      });
      state.projectPackage.generation = result;
      state.projectPackage.review = result?.run?.review ? result : null;
      if (result.preview_source) state.projectPackage.previewSource = result.preview_source;
      state.projectPackage.status = "ready";
      renderProjectPackage();
      appendPlannerMessage?.(projectReviewMessage(result, "重试"));
      return result;
    } catch (error) {
      state.projectPackage.status = "error";
      state.projectPackage.error = stringifyError(error);
      renderProjectPackage();
      throw error;
    }
  }

  async function undoSelectedReview() {
    const result = await decideReview("project-run-undo");
    await refreshSelectedPreview();
    renderProjectPackage();
    appendPlannerMessage?.(projectReviewMessage(result, "撤销"));
    return result;
  }

  async function decideReview(op) {
    const runId = currentReviewRunId();
    if (!runId || !state.projectPackage.path) throw new Error("No review run selected");
    state.projectPackage.status = "generating";
    renderProjectPackage();
    try {
      const result = await rpc(op, {
        project: state.projectPackage.path,
        run_id: runId,
        actor: "desktop"
      });
      state.projectPackage.generation = result;
      state.projectPackage.review = result?.run?.review ? result : null;
      if (result.preview_source) state.projectPackage.previewSource = result.preview_source;
      state.projectPackage.workbench = await rpc("project-workbench", { project: state.projectPackage.path });
      state.projectPackage.status = "ready";
      renderProjectPackage();
      return result;
    } catch (error) {
      state.projectPackage.status = "error";
      state.projectPackage.error = stringifyError(error);
      renderProjectPackage();
      throw error;
    }
  }

  function currentReviewRunId() {
    return reviewRunId(state.projectPackage.review);
  }

  function renderWorkbench() {
    if (!projectWorkbenchEl || !projectWorkbenchCardsEl) return;
    const workbench = state.projectPackage.workbench;
    const cards = workbench?.cards || [];
    const visible = state.projectPackage.status !== "idle" && cards.length > 0;
    projectWorkbenchEl.hidden = !visible;
    if (!visible) return;
    projectWorkbenchEl.dataset.status = state.projectPackage.status;
    if (projectSelectedSummaryEl) {
      projectSelectedSummaryEl.textContent = selectedCardSummary(state) || `${cards.length} cards`;
    }
    projectWorkbenchCardsEl.replaceChildren(...cards.map((card) => cardButton(card)));
  }

  function cardButton(card) {
    const button = document.createElement("button");
    const isVideoCard = card.preview?.kind === "video" && card.preview?.composition_path;
    const thumb = isVideoCard && card.preview?.poster_frame_path
      ? `<img class="project-card-thumb" src="${escapeText(assetFileUrl(state.projectPackage.path, card.preview.poster_frame_path))}" alt="">`
      : "";
    button.type = "button";
    button.className = "project-workbench-card";
    button.dataset.projectCardId = card.id;
    button.dataset.projectCardKind = card.kind;
    button.dataset.status = card.status;
    button.dataset.selected = card.id === state.projectPackage.selectedCardId ? "true" : "false";
    if (isVideoCard) {
      button.dataset.videoCompositionPath = card.preview.composition_path || "";
      button.dataset.videoDurationMs = String(card.preview?.metadata?.duration_ms || 0);
      button.dataset.videoFilename = card.preview?.metadata?.filename || "";
    }
    button.innerHTML = `
      ${thumb}
      <span class="project-card-kind">${escapeText(card.kind)}</span>
      <strong>${escapeText(card.title || card.kind)}</strong>
      ${projectCardMetaHtml(card, isVideoCard)}
      <em>${escapeText(card.status || "ready")}</em>
    `;
    button.addEventListener("click", () => {
      if (isVideoCard) openVideoArtifact(card).catch(() => {});
      else selectCard(card);
    });
    if (card.id?.startsWith("art_")) {
      const action = document.createElement("span");
      action.className = "project-card-action";
      action.textContent = isVideoCard ? "打开视频" : "AI 生成";
      action.addEventListener("click", (event) => {
        event.stopPropagation();
        if (isVideoCard) openVideoArtifact(card).catch(() => {});
        else generateSelectedAfter(card).catch(() => {});
      });
      button.append(action);
    }
    return button;
  }

  async function openVideoArtifact(card) {
    selectCard(card);
    const composition = card.preview?.composition_path;
    if (!composition || !state.projectPackage.path) return;
    const path = absoluteProjectPath(state.projectPackage.path, composition);
    await window.capyWorkbench?.openVideoComposition?.(path);
  }

  async function generateSelectedAfter(card) {
    selectCard(card);
    await generateSelectedArtifact({
      prompt: promptEl?.value.trim() || `Update ${card.title || card.kind} from the selected project card.`
    });
  }

  function selectCard(card) {
    state.projectPackage.selectedCardId = card.id;
    state.projectPackage.selectedArtifactId = card.id?.startsWith("art_") ? card.id : null;
    if (state.projectPackage.selectedArtifactId) {
      artifactNodes.selectArtifactNode(state.projectPackage.selectedArtifactId);
    }
    refreshSelectedPreview().finally(() => renderProjectPackage());
  }

  async function refreshSelectedPreview() {
    const artifact = selectedArtifact();
    if (!artifact || !state.projectPackage.path) {
      state.projectPackage.previewSource = "";
      return;
    }
    if (artifact.kind === "video") {
      state.projectPackage.previewSource = "";
      return;
    }
    if (artifact.kind === "html" || artifact.kind === "image" || artifact.kind === "markdown" || artifact.kind?.endsWith("-json") || artifact.kind === "composition-json") {
      const read = await rpc("artifact-read", {
        project: state.projectPackage.path,
        artifact: artifact.id
      });
      state.projectPackage.previewSource = read.source || "";
    }
  }

  function renderArtifactList(artifacts) {
    if (!projectArtifactListEl) return;
    projectArtifactListEl.replaceChildren();
    for (const artifact of artifacts) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "project-artifact-row";
      button.dataset.artifactId = artifact.id;
      button.dataset.selected = artifact.id === state.projectPackage.selectedArtifactId ? "true" : "false";
      button.innerHTML = `
        <span>${escapeText(artifact.title || artifact.id)}</span>
        <small>${escapeText(artifact.kind || "artifact")} · ${escapeText(artifact.source_path || "")}</small>
      `;
      button.addEventListener("click", async () => {
        state.projectPackage.selectedCardId = artifact.id;
        state.projectPackage.selectedArtifactId = artifact.id;
        artifactNodes.selectArtifactNode(artifact.id);
        await refreshSelectedPreview();
        renderProjectPackage();
      });
      projectArtifactListEl.append(button);
    }
  }

  return {
    loadProjectPackage,
    buildSelectedContext,
    generateSelectedArtifact,
    generateCampaign,
    acceptSelectedReview,
    rejectSelectedReview,
    retrySelectedReview,
    undoSelectedReview,
    selectedArtifact,
    renderProjectPackage,
    syncProjectArtifactNodes: artifactNodes.syncProjectArtifactNodes,
    syncCanvasArtifactGeometry: artifactNodes.syncCanvasGeometry,
    syncCanvasArtifactSelection,
    selectArtifactNode: artifactNodes.selectArtifactNode,
  };

  function syncCanvasArtifactSelection() {
    const artifactRef = artifactNodes.selectedCanvasArtifact();
    if (!artifactRef || artifactRef.artifact_id === state.projectPackage.selectedArtifactId) return false;
    state.projectPackage.selectedArtifactId = artifactRef.artifact_id;
    state.projectPackage.selectedCardId = artifactRef.artifact_id;
    refreshSelectedPreview().finally(() => renderProjectPackage());
    return true;
  }
}

function projectGenerateMessage(result, artifact) {
  return {
    role: "assistant",
    content: projectGenerateMessageContent(result, artifact)
  };
}

function projectReviewMessage(result, label) {
  const run = result?.run || {};
  const status = run?.review?.status || run.status || "";
  const changed = (run.changed_artifact_refs || []).join(", ") || run.artifact_id || "";
  return {
    role: "assistant",
    content: `### AI Diff ${label}\n\n- Artifact: ${changed}\n- Status: ${status}\n- Run: ${run.id || ""}`
  };
}
