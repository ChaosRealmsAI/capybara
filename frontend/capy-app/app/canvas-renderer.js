export function createCanvasRenderer(ctx) {
  const {
    state,
    posterDocuments,
    posterLayerEl,
    labelLayerEl,
    canvasEl,
    labelSync,
    renderPosterStage,
    buildPosterState,
    cloneDefaultPosterDocument,
    cloneDocument,
    normalizeValue,
    stringifyError,
    refreshPlannerContext,
    selectNode,
    moveNodeById,
    loadPosterDocument,
    updatePosterDocument,
  } = ctx;

const TYPE_DOTS = {
  brand: "#fbbf24", image: "#f9a8d4", video: "#a78bfa", web: "#84cc16",
  "timeline-composition": "#34d399",
  text: "#9ca3af", default: "#a78bfa"
};
const TYPE_ICONS = {
  brand: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="3.5"/><circle cx="12" cy="12" r="8.5" stroke-dasharray="2 3"/></svg>',
  image: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><circle cx="9" cy="10" r="1.6"/><path d="M4.5 17.5l4.5-4 4 3 3.5-2.5 3 2.5"/></svg>',
  video: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="3.5" y="5.5" width="17" height="13" rx="2"/><path d="M10.5 9.5l4.5 2.5-4.5 2.5z" fill="currentColor"/></svg>',
  "timeline-composition": '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="4" y="5" width="16" height="14" rx="2"/><path d="M8 9h8M8 13h5M7 17h10"/></svg>',
  web: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.6 3 2.6 14 0 17M12 3.5c-2.6 3-2.6 14 0 17"/></svg>',
  default: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><circle cx="12" cy="12" r="8"/></svg>'
};

function inferType(node) {
  const componentKind = String(node?.capyComponentKind || node?.component_kind || "").toLowerCase();
  if (componentKind === "timeline-composition") return "timeline-composition";
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
    skin.dataset.capyComponentKind = type === "timeline-composition" ? type : "";
    skin.dataset.capyTimelineState = node.timeline?.state || "";
    skin.classList.toggle("is-selected", String(node.id) === String(selectedId));
    skin.querySelector(".node-dot").style.background = TYPE_DOTS[type] || TYPE_DOTS.default;
    skin.querySelector(".node-icon").innerHTML = TYPE_ICONS[type] || TYPE_ICONS.default;
    skin.querySelector(".node-type").textContent = type === "timeline-composition"
      ? "timeline"
      : String(node.content_kind || "node").toLowerCase();
    skin.querySelector(".node-title").textContent = node.title || `Node ${node.id}`;
    skin.querySelector(".node-meta").textContent = type === "timeline-composition"
      ? (node.timeline?.state || "preview-ready")
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
  if (labelSync.installed || !canvasEl) return;
  labelSync.installed = true;
  canvasEl.addEventListener("pointerdown", startLiveCanvasLabelRefresh);
  canvasEl.addEventListener("pointermove", scheduleCanvasLabelRefresh, { passive: true });
  canvasEl.addEventListener("wheel", scheduleCanvasLabelRefresh, { passive: true });
  canvasEl.addEventListener("keyup", scheduleCanvasLabelRefresh);
  window.addEventListener("pointerup", stopLiveCanvasLabelRefresh);
  window.addEventListener("pointercancel", stopLiveCanvasLabelRefresh);
  window.addEventListener("blur", stopLiveCanvasLabelRefresh);
}
function scheduleCanvasLabelRefresh() {
  if (labelSync.refreshFrame) return;
  labelSync.refreshFrame = requestAnimationFrame(() => {
    labelSync.refreshFrame = 0;
    refreshPlannerContext();
  });
}
function startLiveCanvasLabelRefresh() {
  labelSync.liveRefreshActive = true;
  if (labelSync.liveRefreshFrame) return;
  const tick = () => {
    if (!labelSync.liveRefreshActive) {
      labelSync.liveRefreshFrame = 0;
      return;
    }
    refreshPlannerContext();
    labelSync.liveRefreshFrame = requestAnimationFrame(tick);
  };
  labelSync.liveRefreshFrame = requestAnimationFrame(tick);
}
function stopLiveCanvasLabelRefresh() {
  labelSync.liveRefreshActive = false;
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


  return {
    inferType,
    renderPosterOverlays,
    renderNodeLabels,
    nodeLabelBox,
    nodeOverlayBox,
    installCanvasLabelSync,
    scheduleCanvasLabelRefresh,
    startLiveCanvasLabelRefresh,
    stopLiveCanvasLabelRefresh,
    verifyPosterRenderer,
    posterStateForNode,
    posterDocumentsState,
  };
}
