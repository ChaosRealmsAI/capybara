export function createProjectPackage({ state, rpc, dom, stringifyError }) {
  const {
    projectPackagePanelEl,
    projectPackageTitleEl,
    projectPackageMetaEl,
    projectArtifactListEl,
    projectPreviewFrameEl,
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
      state.projectPackage.inspection = inspection;
      const firstHtml = firstHtmlArtifact(inspection);
      state.projectPackage.selectedArtifactId = firstHtml?.id || null;
      state.projectPackage.previewSource = "";
      if (firstHtml) {
        const read = await rpc("artifact-read", {
          project: projectPath,
          artifact: firstHtml.id
        });
        state.projectPackage.previewSource = read.source || "";
      }
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

  function selectedArtifact() {
    const artifacts = state.projectPackage.inspection?.artifacts?.artifacts || [];
    return artifacts.find((artifact) => artifact.id === state.projectPackage.selectedArtifactId) || null;
  }

  function renderProjectPackage() {
    if (!projectPackagePanelEl) return;
    const packageState = state.projectPackage;
    const inspection = packageState.inspection;
    const artifacts = inspection?.artifacts?.artifacts || [];
    const isVisible = packageState.status !== "idle" && (inspection || packageState.status === "loading" || packageState.status === "error");
    projectPackagePanelEl.hidden = !isVisible;
    if (!isVisible) return;
    projectPackagePanelEl.dataset.status = packageState.status;
    if (projectPackageTitleEl) {
      projectPackageTitleEl.textContent = inspection?.manifest?.name || "Project package";
    }
    if (projectPackageMetaEl) {
      if (packageState.status === "loading") projectPackageMetaEl.textContent = "loading";
      else if (packageState.status === "error") projectPackageMetaEl.textContent = packageState.error || "error";
      else projectPackageMetaEl.textContent = `${artifacts.length} artifacts`;
    }
    renderArtifactList(artifacts);
    if (projectPreviewFrameEl) {
      projectPreviewFrameEl.srcdoc = packageState.previewSource || "<!doctype html><p>No HTML artifact</p>";
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
        state.projectPackage.selectedArtifactId = artifact.id;
        if (artifact.kind === "html" && state.projectPackage.path) {
          const read = await rpc("artifact-read", {
            project: state.projectPackage.path,
            artifact: artifact.id
          });
          state.projectPackage.previewSource = read.source || "";
        }
        renderProjectPackage();
      });
      projectArtifactListEl.append(button);
    }
  }

  return {
    loadProjectPackage,
    buildSelectedContext,
    selectedArtifact,
    renderProjectPackage,
  };
}

function firstHtmlArtifact(inspection) {
  const artifacts = inspection?.artifacts?.artifacts || [];
  return artifacts.find((artifact) => artifact.kind === "html") || artifacts[0] || null;
}

function escapeText(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
