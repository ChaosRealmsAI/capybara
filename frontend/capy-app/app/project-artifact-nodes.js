export function createProjectArtifactNodes({ state, rpc, canvasApi, stringifyError }) {
  const geometryTimers = new Map();

  async function loadSurfaceNodes(projectPath) {
    const surfaceNodes = await rpc("project-surface-nodes", { project: projectPath });
    state.projectPackage.surfaceNodes = surfaceNodes;
    state.projectPackage.artifactNodes = {};
    return surfaceNodes;
  }

  async function syncProjectArtifactNodes() {
    const surfaceNodes = state.projectPackage.surfaceNodes?.nodes || [];
    const projectId = state.projectPackage.surfaceNodes?.project_id || "";
    const artifacts = state.projectPackage.inspection?.artifacts?.artifacts || [];
    const byId = new Map(artifacts.map((artifact) => [artifact.id, artifact]));
    for (const surfaceNode of surfaceNodes) {
      const artifact = byId.get(surfaceNode.artifact_id);
      if (!artifact) continue;
      const existing = runtimeNodeFor(surfaceNode.id);
      if (existing) {
        state.projectPackage.artifactNodes[surfaceNode.id] = runtimeEntry(existing, artifact);
        continue;
      }
      const created = canvasApi.createProjectArtifactNode?.({
        title: artifact.title || artifact.id,
        projectId,
        surfaceNodeId: surfaceNode.id,
        artifactId: artifact.id,
        artifactKind: artifact.kind,
        sourcePath: artifact.source_path || "",
        geometry: surfaceNode.geometry
      });
      const node = created?.selected_node || canvasApi.refreshPlannerContext?.().canvas?.selectedNode;
      if (node?.id) state.projectPackage.artifactNodes[surfaceNode.id] = runtimeEntry(node, artifact);
    }
    canvasApi.scheduleCanvasLabelRefresh?.();
    canvasApi.refreshPlannerContext?.();
  }

  function selectArtifactNode(artifactId) {
    const surfaceNode = (state.projectPackage.surfaceNodes?.nodes || [])
      .find((node) => node.artifact_id === artifactId);
    if (!surfaceNode) return false;
    const runtime = runtimeNodeFor(surfaceNode.id);
    if (!runtime?.id) return false;
    canvasApi.selectNode?.(runtime.id);
    return true;
  }

  function syncCanvasGeometry() {
    if (state.projectPackage.status !== "ready" || !state.projectPackage.path) return;
    const surfaceNodes = state.projectPackage.surfaceNodes?.nodes || [];
    const byId = new Map(surfaceNodes.map((node) => [node.id, node]));
    for (const runtime of projectArtifactRuntimeNodes()) {
      const surfaceId = runtime.artifact_ref?.surface_node_id;
      const surfaceNode = byId.get(surfaceId);
      const bounds = runtime.bounds || runtime.geometry;
      if (!surfaceId || !surfaceNode || !bounds) continue;
      const next = roundedGeometry(bounds);
      if (!geometryChanged(surfaceNode.geometry, next)) continue;
      surfaceNode.geometry = next;
      queueGeometryPersist(surfaceId, next);
    }
  }

  function selectedCanvasArtifact() {
    const selected = state.canvas.selectedNode;
    if (selected?.content_kind !== "project_artifact" || !selected.artifact_ref) return null;
    return selected.artifact_ref;
  }

  function runtimeNodeFor(surfaceNodeId) {
    return projectArtifactRuntimeNodes()
      .find((node) => node.artifact_ref?.surface_node_id === surfaceNodeId) || null;
  }

  function projectArtifactRuntimeNodes() {
    return (state.blocks || []).filter((node) => node?.content_kind === "project_artifact");
  }

  function queueGeometryPersist(surfaceNodeId, geometry) {
    clearTimeout(geometryTimers.get(surfaceNodeId));
    geometryTimers.set(surfaceNodeId, setTimeout(async () => {
      try {
        const updated = await rpc("project-surface-node-update", {
          project: state.projectPackage.path,
          node_id: surfaceNodeId,
          geometry
        });
        state.projectPackage.surfaceNodes = updated;
      } catch (error) {
        state.projectPackage.error = stringifyError(error);
      }
    }, 320));
  }

  return {
    loadSurfaceNodes,
    syncProjectArtifactNodes,
    selectArtifactNode,
    syncCanvasGeometry,
    selectedCanvasArtifact,
  };
}

function runtimeEntry(node, artifact) {
  return {
    nodeId: Number(node.id),
    artifactId: artifact.id,
    sourcePath: artifact.source_path || ""
  };
}

function roundedGeometry(bounds) {
  return {
    x: Math.round(Number(bounds.x || 0) * 100) / 100,
    y: Math.round(Number(bounds.y || 0) * 100) / 100,
    w: Math.round(Number(bounds.w || 0) * 100) / 100,
    h: Math.round(Number(bounds.h || 0) * 100) / 100
  };
}

function geometryChanged(a = {}, b = {}) {
  return ["x", "y", "w", "h"].some((key) => Math.abs(Number(a[key] || 0) - Number(b[key] || 0)) > 0.5);
}
