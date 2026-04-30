export function createCanvasContext(ctx) {
  const {
    state,
    canvasEl,
    canvasPanelEl,
    regionLayerEl,
    regionModeEl,
    plannerContextEl,
    contextTitleEl,
    contextMetaEl,
    contextAttachmentsEl,
    clampRectToBounds,
    compactGeometry,
    nodeBounds,
    normalizeRect,
    regionPercent,
    roundGeometry,
    worldBoxToScreen,
    normalizeValue,
    contentKindLabel,
    selected_context_text,
    refreshPlannerContext,
    renderRegionOverlayHook,
    selectNode,
    moveNodeById,
    nodeLabelBox,
    startLiveCanvasLabelRefresh,
    scheduleCanvasLabelRefresh,
    stopLiveCanvasLabelRefresh,
  } = ctx;

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
  const isArtifact = selectedItem.content_kind === "project_artifact";
  const bounds = selectedItem.bounds || selectedItem.geometry || {};
  const kind = isRegion ? "image_region" : isImage ? "selected_image" : isArtifact ? "project_artifact" : "selected_node";
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
    artifact_ref: selectedItem.artifact_ref || null,
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
        const node = nodes.find((item) => state.canvas.selectedNode?.id
          && String(item.id) === String(state.canvas.selectedNode.id))
          || nodes.find((item) => item.content_kind !== "shape");
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
        done({ passed: false, reason: "missing semantic node or label", pageErrors: window.__capyPageErrors || [] });
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
      item.artifact_ref?.artifact_id ? `artifact=${item.artifact_ref.artifact_id}` : null,
      item.artifact_ref?.source_path || item.source_path ? `source=${item.artifact_ref?.source_path || item.source_path}` : null,
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
    packet.artifact_ref?.artifact_id ? `artifact_id=${packet.artifact_ref.artifact_id}` : null,
    packet.artifact_ref?.surface_node_id ? `surface_node_id=${packet.artifact_ref.surface_node_id}` : null,
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


  return {
    installCanvasRegionSelection,
    renderRegionMode,
    setCanvasContextRegion,
    clearCanvasContextRegion,
    syncCanvasContext,
    activeCanvasContext,
    renderRegionOverlay,
    renderPlannerContext,
    composePromptWithContext,
    verifyLabelMoveSync,
  };
}
