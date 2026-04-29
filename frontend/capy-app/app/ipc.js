export function createRpc(pending) {
  return function rpc(op, params) {
    return new Promise((resolve, reject) => {
      if (!window.ipc) {
        reject({ error: "Capybara shell IPC unavailable" });
        return;
      }
      const id = `ui-${Date.now()}-${Math.random().toString(16).slice(2)}`;
      pending.set(id, { resolve, reject });
      window.ipc.postMessage(JSON.stringify({ kind: "rpc", id, op, params }));
    });
  };
}

export function installIpcReceiver(pending) {
  window.__capyReceive = (response) => {
    const entry = pending.get(response.req_id);
    if (!entry) return;
    pending.delete(response.req_id);
    if (response.ok) entry.resolve(response.data);
    else entry.reject(response.error || { error: "request failed" });
  };
}

export function installShellEventListeners(handlers) {
  const {
    state,
    setRunStatus,
    renderMessages,
    openConversation,
    renderError,
    handleCanvasToolEvent,
    renderCanvasToolStatus,
    stringifyError,
    handleCanvasNodeAttached,
    handleTimelineOpened,
  } = handlers;

  window.addEventListener("capy:agent-event", (event) => {
    const detail = event.detail;
    if (!detail || detail.conversation_id !== state.activeId) return;
    if (detail.status) setRunStatus(detail.status);
    if (detail.kind === "assistant_delta") {
      const current = state.streaming.get(detail.run_id) || "";
      state.streaming.set(detail.run_id, current + (detail.delta || ""));
      renderMessages();
    } else if (detail.kind === "assistant_done" || detail.kind === "error") {
      state.streaming.delete(detail.run_id);
      openConversation(state.activeId).catch((error) => renderError(error));
    }
  });

  window.addEventListener("capy:canvas-tool-event", (event) => {
    handleCanvasToolEvent(event.detail).catch((error) => {
      state.canvasTool.status = "error";
      state.canvasTool.error = stringifyError(error);
      renderCanvasToolStatus();
    });
  });

  window.addEventListener("capy:canvas-node-attached", (event) => {
    handleCanvasNodeAttached(event.detail);
  });

  window.addEventListener("capy:timeline-opened", (event) => {
    handleTimelineOpened(event.detail);
  });
}

export function installNativeWindowDrag(topbar) {
  topbar?.addEventListener("mousedown", (event) => {
    if (event.button !== 0) return;
    const target = event.target;
    if (target instanceof HTMLElement && target.closest("button, input, a, select, [role=button]")) return;
    if (!window.ipc) return;
    window.ipc.postMessage(event.detail === 2 ? "maximize_toggle" : "drag_window");
  });
}
