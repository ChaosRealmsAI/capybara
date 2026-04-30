export function createCanvasControls(ctx) {
  const {
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
    refreshPlannerContext,
    stringifyError,
  } = ctx;

  function installCanvasControls() {
    for (const button of canvasToolButtonsEl || []) {
      button.addEventListener("click", () => setCanvasTool(button.dataset.canvasTool));
    }
    for (const button of canvasColorButtonsEl || []) {
      button.addEventListener("click", () => setCanvasColor(button));
    }
    for (const button of canvasZoomButtonsEl || []) {
      button.addEventListener("click", () => setCanvasZoom(button.dataset.canvasZoom));
    }
    miniMapEl?.addEventListener("pointerdown", (event) => {
      if (event.button !== 0) return;
      event.preventDefault();
      try {
        miniMapEl.setPointerCapture?.(event.pointerId);
      } catch {
        // Synthetic verifier events do not always have an active browser pointer.
      }
      miniMapEl.dataset.dragging = "true";
      centerFromMiniMapPoint(event.clientX, event.clientY);
    });
    miniMapEl?.addEventListener("pointermove", (event) => {
      if (miniMapEl.dataset.dragging !== "true") return;
      event.preventDefault();
      centerFromMiniMapPoint(event.clientX, event.clientY);
    });
    miniMapEl?.addEventListener("pointerup", (event) => {
      miniMapEl.dataset.dragging = "false";
      try {
        miniMapEl.releasePointerCapture?.(event.pointerId);
      } catch {}
    });
    miniMapEl?.addEventListener("pointercancel", (event) => {
      miniMapEl.dataset.dragging = "false";
      try {
        miniMapEl.releasePointerCapture?.(event.pointerId);
      } catch {}
    });
    miniMapEl?.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      const rect = miniMapEl.getBoundingClientRect();
      centerFromMiniMapPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
    });
  }

  function setCanvasColor(button) {
    const stroke = button?.dataset.stroke || "#8a6fae";
    const fill = button?.dataset.fill || "#fef3c7";
    const fillStyle = button?.dataset.fillStyle || "hachure";
    try {
      set_vector_style(stroke, fill, fillStyle);
      state.canvas.currentStyle = { stroke, fill, fillStyle };
      renderColorButtons(button);
      refreshPlannerContext();
      return { ok: true, stroke, fill, fillStyle };
    } catch (error) {
      return { ok: false, error: stringifyError(error) };
    }
  }

  function setCanvasZoom(action) {
    const viewport = state.canvas.viewport || {};
    const centerX = Number(viewport.width) / 2 || 600;
    const centerY = Number(viewport.height) / 2 || 400;
    try {
      if (action === "in") {
        zoom_view_at(centerX, centerY, 1.25);
      } else if (action === "out") {
        zoom_view_at(centerX, centerY, 0.8);
      } else if (action === "fit") {
        fit_view_to_content();
      } else if (action === "reset") {
        reset_view();
      } else if (action === "pan-left") {
        pan_view_by(96, 0);
      } else if (action === "pan-right") {
        pan_view_by(-96, 0);
      } else if (action === "pan-up") {
        pan_view_by(0, 96);
      } else if (action === "pan-down") {
        pan_view_by(0, -96);
      } else {
        return { ok: false, error: `unknown zoom action: ${action}` };
      }
      const snapshot = refreshPlannerContext();
      return { ok: true, action, viewport: snapshot?.canvas?.viewport || null };
    } catch (error) {
      return { ok: false, action, error: stringifyError(error) };
    }
  }

  function setCanvasTool(tool) {
    if (!tool) return { ok: false, error: "missing tool" };
    try {
      const normalized = set_tool(tool);
      state.canvas.currentTool = normalized;
      renderToolButtons(normalized);
      refreshPlannerContext();
      return { ok: true, tool: normalized };
    } catch (error) {
      return { ok: false, error: stringifyError(error) };
    }
  }

  function renderCanvasControls(snapshot = {}) {
    renderToolButtons(state.canvas.currentTool || snapshot.current_tool || "select");
    renderColorButtons();
    renderZoomControls(snapshot.viewport || state.canvas.viewport || null);
    renderMiniMap(Array.isArray(snapshot.nodes) ? snapshot.nodes : [], snapshot.viewport || null);
  }

  function renderToolButtons(currentTool) {
    const active = normalizeTool(currentTool);
    for (const button of canvasToolButtonsEl || []) {
      const isActive = normalizeTool(button.dataset.canvasTool) === active;
      button.classList.toggle("active", isActive);
      button.setAttribute("aria-pressed", isActive ? "true" : "false");
    }
  }

  function renderColorButtons(activeButton = null) {
    for (const button of canvasColorButtonsEl || []) {
      const isActive = activeButton
        ? button === activeButton
        : button.dataset.stroke === state.canvas.currentStyle?.stroke;
      button.classList.toggle("active", isActive);
      button.setAttribute("aria-pressed", isActive ? "true" : "false");
    }
  }

  function renderZoomControls(viewport) {
    const zoom = Number(viewport?.zoom) || 1;
    if (canvasZoomValueEl) {
      canvasZoomValueEl.textContent = `${Math.round(zoom * 100)}%`;
      canvasZoomValueEl.dataset.zoom = String(zoom);
    }
    for (const button of canvasZoomButtonsEl || []) {
      button.dataset.zoom = String(zoom);
    }
  }

  function renderMiniMap(nodes, viewport) {
    if (!miniMapEl || !miniMapNodesEl || !miniMapViewportEl || !viewport?.visible_world) return;
    const model = miniMapModel(nodes, viewport);
    miniMapEl.dataset.nodeCount = String(nodes.length);
    miniMapEl.dataset.zoom = String(viewport.zoom || 1);
    miniMapEl.dataset.ready = "true";
    miniMapNodesEl.replaceChildren(...nodes.map((node) => miniNode(node, model)));
    applyBox(miniMapViewportEl, worldToMiniBox(viewport.visible_world, model));
  }

  function miniNode(node, model) {
    const item = document.createElement("span");
    item.className = "mini-map-node";
    if (node.selected) item.classList.add("is-selected");
    item.dataset.nodeId = String(node.id || "");
    applyBox(item, worldToMiniBox(node.bounds || node.geometry || null, model));
    return item;
  }

  function centerFromMiniMapPoint(clientX, clientY) {
    if (!miniMapEl || !state.canvas.viewport?.visible_world) return false;
    const rect = miniMapEl.getBoundingClientRect();
    const model = miniMapModel(state.canvas.objects || state.blocks, state.canvas.viewport);
    const world = miniToWorld(clientX - rect.left, clientY - rect.top, rect, model);
    if (!world) return false;
    center_view_on(world.x, world.y);
    return refreshPlannerContext();
  }

  function miniMapModel(nodes, viewport) {
    const viewportBox = viewport?.visible_world || { x: 0, y: 0, w: 1000, h: 700 };
    let minX = Number(viewportBox.x) || 0;
    let minY = Number(viewportBox.y) || 0;
    let maxX = minX + (Number(viewportBox.w) || 1000);
    let maxY = minY + (Number(viewportBox.h) || 700);
    for (const node of nodes || []) {
      const box = node.bounds || node.geometry;
      if (!box) continue;
      minX = Math.min(minX, Number(box.x) || 0);
      minY = Math.min(minY, Number(box.y) || 0);
      maxX = Math.max(maxX, (Number(box.x) || 0) + Math.max(1, Number(box.w) || 1));
      maxY = Math.max(maxY, (Number(box.y) || 0) + Math.max(1, Number(box.h) || 1));
    }
    const w = Math.max(1, maxX - minX);
    const h = Math.max(1, maxY - minY);
    const padX = Math.max(80, w * 0.1);
    const padY = Math.max(60, h * 0.1);
    return { x: minX - padX, y: minY - padY, w: w + padX * 2, h: h + padY * 2 };
  }

  function worldToMiniBox(box, model) {
    if (!box) return { left: "0%", top: "0%", width: "0%", height: "0%" };
    const x = Number(box.x) || 0;
    const y = Number(box.y) || 0;
    const w = Math.max(1, Number(box.w) || 1);
    const h = Math.max(1, Number(box.h) || 1);
    return {
      left: percent((x - model.x) / model.w),
      top: percent((y - model.y) / model.h),
      width: percent(w / model.w),
      height: percent(h / model.h),
    };
  }

  function miniToWorld(localX, localY, rect, model) {
    if (!rect.width || !rect.height) return null;
    return {
      x: model.x + (localX / rect.width) * model.w,
      y: model.y + (localY / rect.height) * model.h,
    };
  }

  function applyBox(element, box) {
    element.style.left = box.left;
    element.style.top = box.top;
    element.style.width = box.width;
    element.style.height = box.height;
  }

  function percent(value) {
    return `${Math.max(0, Math.min(100, value * 100)).toFixed(3)}%`;
  }

  function normalizeTool(tool) {
    const value = String(tool || "select").trim().toLowerCase();
    if (value === "cursor") return "select";
    if (value === "circle") return "ellipse";
    if (value === "tri") return "triangle";
    if (value === "pen") return "freehand";
    if (value === "sticky") return "sticky_note";
    if (value === "node") return "sticky_note";
    if (value === "link") return "arrow";
    return value;
  }

  return {
    installCanvasControls,
    renderCanvasControls,
    setCanvasTool,
    setCanvasColor,
    setCanvasZoom,
    centerFromMiniMapPoint,
  };
}
