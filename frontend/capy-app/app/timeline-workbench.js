export function createTimelineWorkbench(ctx) {
  const {
    state,
    labelLayerEl,
    nextFrameInspectorEl,
    nextFrameInspectorTitleEl,
    nextFrameInspectorStatusEl,
    nextFrameInspectorStagesEl,
    rpc,
    stringifyError,
    inspectorMessage,
    sourceRows,
    compositionRows,
    compileRows,
    exportRows,
    exportStatus,
    evidenceRows,
    stageCard,
    stageLabel,
    scheduleCanvasLabelRefresh,
    inferType,
  } = ctx;

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


  return {
    attachTimelineComposition,
    openTimelineComposition,
    openTimelineInspector,
    syncTimelineInspector,
    handleCanvasNodeAttached,
    handleTimelineOpened,
    applyTimelineAttachments,
  };
}
