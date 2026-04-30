export function createProjectPackage({ state, rpc, dom, stringifyError, appendPlannerMessage }) {
  const {
    projectPackagePanelEl,
    projectPackageTitleEl,
    projectPackageMetaEl,
    projectWorkbenchEl,
    projectWorkbenchCardsEl,
    projectSelectedSummaryEl,
    projectArtifactListEl,
    projectPreviewFrameEl,
    promptEl,
    providerEl,
    modelEl,
    effortEl,
  } = dom;

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
      state.projectPackage.inspection = inspection;
      state.projectPackage.workbench = workbench;
      const firstCard = firstSelectableCard(workbench);
      state.projectPackage.selectedCardId = firstCard?.id || null;
      state.projectPackage.selectedArtifactId = firstCard?.id?.startsWith("art_") ? firstCard.id : null;
      state.projectPackage.previewSource = "";
      await refreshSelectedPreview();
      state.projectPackage.status = "ready";
      renderProjectPackage();
      return { loaded: true, inspection };
    } catch (error) {
      state.projectPackage.status = "error";
      state.projectPackage.error = stringifyError(error);
      renderProjectPackage();
      return { loaded: false, error: state.projectPackage.error };
    }
  }

  async function buildSelectedContext(selector = null) {
    const artifact = selectedArtifact();
    if (!artifact || !state.projectPackage.path) {
      throw new Error("No selected project artifact");
    }
    return rpc("context-build", {
      project: state.projectPackage.path,
      artifact: artifact.id,
      selector
    });
  }

  async function generateSelectedArtifact(options = {}) {
    const artifact = selectedArtifact();
    if (!artifact || !state.projectPackage.path) {
      throw new Error("No selected project artifact");
    }
    const prompt = options.prompt || promptEl?.value.trim() || `Revise ${artifact.title || artifact.id} using project design language.`;
    const provider = options.provider || providerEl?.value || "codex";
    const live = options.live === undefined ? provider !== "fixture" : Boolean(options.live);
    state.projectPackage.status = "generating";
    renderProjectPackage();
    try {
      const result = await rpc("project-generate", {
        project: state.projectPackage.path,
        artifact: artifact.id,
        provider,
        prompt,
        dry_run: options.dryRun === true ? true : false,
        live,
        model: options.model || modelEl?.value || null,
        effort: options.effort || effortEl?.value || null,
        sdk_response: options.sdkResponse || null
      });
      state.projectPackage.generation = result;
      if (result.preview_source) state.projectPackage.previewSource = result.preview_source;
      state.projectPackage.workbench = await rpc("project-workbench", { project: state.projectPackage.path });
      state.projectPackage.status = "ready";
      renderProjectPackage();
      appendPlannerMessage?.(projectGenerateMessage(result, artifact, provider));
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
    renderArtifactList(artifacts);
    if (projectPreviewFrameEl) {
      projectPreviewFrameEl.srcdoc = previewFrameSource(selectedArtifact(), packageState.previewSource);
    }
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
    button.type = "button";
    button.className = "project-workbench-card";
    button.dataset.projectCardId = card.id;
    button.dataset.projectCardKind = card.kind;
    button.dataset.status = card.status;
    button.dataset.selected = card.id === state.projectPackage.selectedCardId ? "true" : "false";
    button.innerHTML = `
      <span class="project-card-kind">${escapeText(card.kind)}</span>
      <strong>${escapeText(card.title || card.kind)}</strong>
      <small>${escapeText(card.source_path || card.preview?.text || "项目汇总")}</small>
      <em>${escapeText(card.status || "ready")}</em>
    `;
    button.addEventListener("click", () => selectCard(card));
    if (card.id?.startsWith("art_")) {
      const action = document.createElement("span");
      action.className = "project-card-action";
      action.textContent = "AI 生成";
      action.addEventListener("click", (event) => {
        event.stopPropagation();
        generateSelectedAfter(card).catch(() => {});
      });
      button.append(action);
    }
    return button;
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
    refreshSelectedPreview().finally(() => renderProjectPackage());
  }

  async function refreshSelectedPreview() {
    const artifact = selectedArtifact();
    if (!artifact || !state.projectPackage.path) {
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
    selectedArtifact,
    renderProjectPackage,
  };
}

function firstSelectableCard(workbench) {
  const cards = workbench?.cards || [];
  return cards.find((card) => card.kind === "web" && card.id?.startsWith("art_"))
    || cards.find((card) => card.id?.startsWith("art_"))
    || cards[0]
    || null;
}

function escapeText(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function selectedCardSummary(rootState) {
  const packageState = rootState.projectPackage;
  const card = packageState?.workbench?.cards?.find((item) => item.id === packageState.selectedCardId);
  return card ? `${card.title} · ${card.status}` : "";
}

function selectedArtifactSummary(rootState) {
  const packageState = rootState.projectPackage;
  const artifact = packageState?.inspection?.artifacts?.artifacts?.find((item) => item.id === packageState.selectedArtifactId);
  return artifact ? `${artifact.kind} · ${artifact.source_path}` : "";
}

function previewFrameSource(artifact, source) {
  if (!source) return "<!doctype html><p>No artifact preview</p>";
  if (artifact?.kind === "html" || source.trimStart().startsWith("<svg")) return source;
  return `<!doctype html><pre style="white-space:pre-wrap;font:12px ui-monospace,monospace;padding:16px;color:#2f2437">${escapeText(source)}</pre>`;
}

function projectGenerateMessage(result, artifact, provider) {
  const summary = result?.run?.output?.summary_zh || "项目源文件已生成。";
  const runPath = result?.run_path || "";
  const changed = (result?.run?.changed_artifact_refs || []).join(", ") || artifact.id;
  const status = result?.run?.status || "completed";
  return {
    role: "assistant",
    content: `### ${summary}\n\n- Provider: ${provider}\n- Artifact: ${artifact.title || artifact.id}\n- Changed: ${changed}\n- Status: ${status}\n- Run: ${runPath}`
  };
}
