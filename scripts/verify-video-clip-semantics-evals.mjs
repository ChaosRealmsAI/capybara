export function semanticsBeforeEval(projectPath, compositionPath = "") {
  return `new Promise(async resolve => {
    ${semanticStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await waitForWorkbench(wait);
    await loadProjectVideoQueue(wait, ${JSON.stringify(projectPath)});
    await openVideoWorkspace(wait, ${JSON.stringify(compositionPath)}, "camera-a");
    await waitForQueue(wait, 2);
    resolve(await semanticState("before"));
  })`;
}

export function semanticsAnalyzeEval() {
  return `new Promise(async resolve => {
    ${semanticStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("#video-semantics-analyze")?.click();
    await waitForSemantics(wait, 2);
    resolve(await semanticState("analyzed"));
  })`;
}

export function semanticsSuggestEval() {
  return `new Promise(async resolve => {
    ${semanticStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("#video-suggestion-generate")?.click();
    await waitForSuggestion(wait, 2);
    resolve(await semanticState("suggested"));
  })`;
}

export function semanticsAdoptEval() {
  return `new Promise(async resolve => {
    ${semanticStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("[data-video-adopt-suggestion]")?.click();
    await waitForAdoptedQueue(wait, 2);
    resolve(await semanticState("adopted"));
  })`;
}

export function semanticsRestoreEval(projectPath, compositionPath = "") {
  return `new Promise(async resolve => {
    ${semanticStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await waitForWorkbench(wait);
    await loadProjectVideoQueue(wait, ${JSON.stringify(projectPath)});
    await openVideoWorkspace(wait, ${JSON.stringify(compositionPath)}, "camera-a");
    await waitForSemantics(wait, 2);
    await waitForAdoptedQueue(wait, 2);
    resolve(await semanticState("restored"));
  })`;
}

export function semanticStateSource() {
  return `async function waitForWorkbench(wait) {
    for (let i = 0; i < 120; i += 1) {
      if (window.capyWorkbench?.loadProjectVideoQueue && window.capyWorkbench?.stateSnapshot) return;
      await wait(100);
    }
  }
  async function loadProjectVideoQueue(wait, projectPath) {
    window.CAPYBARA_STATE.projectPackage.path = projectPath;
    await window.capyWorkbench.loadProjectVideoQueue(projectPath);
    for (let i = 0; i < 120; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if ((state.video.clipQueue || []).length >= 2) return;
      await wait(100);
    }
  }
  async function openVideoCard(wait, filename) {
    for (let i = 0; i < 160; i += 1) {
      const cards = [...document.querySelectorAll('.project-workbench-card[data-project-card-kind="video"]')];
      if (cards.length >= 2) break;
      await wait(100);
    }
    const cards = [...document.querySelectorAll('.project-workbench-card[data-project-card-kind="video"]')];
    const card = cards.find(item => (item.dataset.videoFilename || "").includes(filename)) || cards[0];
    card?.click();
    for (let i = 0; i < 180; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const preview = document.querySelector("#video-preview");
      const source = state.video.editor?.source_video?.filename || "";
      if (state.workspace.activeTab === "video" && source.includes(filename) && preview?.dataset.previewReady === "true" && preview?.dataset.videoReady === "true") return;
      await wait(100);
    }
  }
  async function openVideoWorkspace(wait, compositionPath, filename) {
    if (compositionPath === "__queue_only__") {
      window.capyWorkbench.switchWorkspaceTab("video");
      window.capyWorkbench.renderVideoEditor();
      for (let i = 0; i < 80; i += 1) {
        const state = window.capyWorkbench.stateSnapshot();
        if (state.workspace.activeTab === "video") return;
        await wait(100);
      }
      return;
    }
    if (compositionPath && window.capyWorkbench?.openVideoComposition) {
      await window.capyWorkbench.openVideoComposition(compositionPath);
      for (let i = 0; i < 120; i += 1) {
        const state = window.capyWorkbench.stateSnapshot();
        if (state.workspace.activeTab === "video" && state.video.editor) return;
        await wait(100);
      }
      return;
    }
    await openVideoCard(wait, filename);
  }
  async function waitForQueue(wait, count) {
    for (let i = 0; i < 140; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if ((state.video.clipQueue || []).length >= count) return;
      await wait(100);
    }
  }
  async function waitForSemantics(wait, count) {
    for (let i = 0; i < 160; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const items = state.video.clipSemantics?.items || [];
      const domItems = document.querySelectorAll(".video-semantic-card");
      if (items.length >= count && domItems.length >= count && ["saved", "loaded"].includes(state.video.clipSemanticsStatus)) return;
      await wait(100);
    }
  }
  async function waitForSuggestion(wait, count) {
    for (let i = 0; i < 160; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const items = state.video.clipSuggestion?.items || [];
      if (state.video.clipSuggestionStatus === "ready" && items.length >= count && items.every(item => item.semantic_reason)) return;
      await wait(100);
    }
  }
  async function waitForAdoptedQueue(wait, count) {
    for (let i = 0; i < 180; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const queue = state.video.clipQueue || [];
      const semantic = queue.length >= count && queue.every(item => item.semantic_reason && item.suggestion_reason);
      if (semantic && ["saved", "loaded"].includes(state.video.clipQueuePersistStatus)) return;
      await wait(100);
    }
  }
  async function captureAppView() {
    await new Promise(resolve => requestAnimationFrame(resolve));
    const width = 960;
    const height = 600;
    const dpr = 1;
    const canvas = document.createElement("canvas");
    canvas.width = Math.ceil(width * dpr);
    canvas.height = Math.ceil(height * dpr);
    const ctx = canvas.getContext("2d");
    ctx.scale(dpr, dpr);
    ctx.fillStyle = "#f6f8fb";
    ctx.fillRect(0, 0, width, height);
    ctx.fillStyle = "#101827";
    ctx.font = "700 28px -apple-system, BlinkMacSystemFont, sans-serif";
    ctx.fillText("Capybara · 视频片段语义分析", 36, 54);
    ctx.font = "14px -apple-system, BlinkMacSystemFont, sans-serif";
    ctx.fillStyle = "#64748b";
    ctx.fillText("inline DOM evidence · CEF stateSnapshot + visible text", 38, 82);
    drawPanel(ctx, 24, 104, 286, 452, "Clip queue", document.querySelector("#video-queue")?.innerText || "");
    drawPanel(ctx, 336, 104, 286, 452, "片段语义", document.querySelector("#video-semantics")?.innerText || "尚未分析");
    drawPanel(ctx, 648, 104, 288, 452, "AI 剪辑建议", document.querySelector("#video-suggestion")?.innerText || "尚未生成建议");
    return {
      dataUrl: canvas.toDataURL("image/png"),
      width: canvas.width,
      height: canvas.height,
      renderer: "cef-inline-dom-summary-render"
    };
  }
  function drawPanel(ctx, x, y, w, h, title, text) {
    ctx.fillStyle = "#ffffff";
    roundRect(ctx, x, y, w, h, 14);
    ctx.fill();
    ctx.strokeStyle = "#d8dee8";
    ctx.stroke();
    ctx.fillStyle = "#0f172a";
    ctx.font = "700 20px -apple-system, BlinkMacSystemFont, sans-serif";
    ctx.fillText(title, x + 20, y + 36);
    ctx.font = "14px -apple-system, BlinkMacSystemFont, sans-serif";
    ctx.fillStyle = "#334155";
    wrapText(ctx, String(text || "").replace(/\\s+/g, " ").slice(0, 1600), x + 20, y + 68, w - 40, 22, h - 88);
  }
  function wrapText(ctx, text, x, y, maxWidth, lineHeight, maxHeight) {
    const words = text.split(" ");
    let line = "";
    let offset = 0;
    for (const word of words) {
      const next = line ? line + " " + word : word;
      if (ctx.measureText(next).width > maxWidth && line) {
        if (offset + lineHeight > maxHeight) return;
        ctx.fillText(line, x, y + offset);
        line = word;
        offset += lineHeight;
      } else {
        line = next;
      }
    }
    if (line && offset + lineHeight <= maxHeight) ctx.fillText(line, x, y + offset);
  }
  function roundRect(ctx, x, y, w, h, r) {
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.arcTo(x + w, y, x + w, y + h, r);
    ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r);
    ctx.arcTo(x, y, x + w, y, r);
    ctx.closePath();
  }
  async function semanticState(stage) {
    const state = window.capyWorkbench?.stateSnapshot ? window.capyWorkbench.stateSnapshot() : {};
    const editor = document.querySelector("[data-section=video-editor]")?.getBoundingClientRect();
    const panel = document.querySelector("[data-component=video-clip-delivery]")?.getBoundingClientRect();
    const semantics = document.querySelector("#video-semantics")?.getBoundingClientRect();
    return {
      stage,
      workspace: state.workspace?.activeTab || "",
      queue: state.video?.clipQueue || [],
      queueManifest: state.video?.clipQueueManifest || null,
      persistStatus: state.video?.clipQueuePersistStatus || "",
      semantics: state.video?.clipSemantics || null,
      semanticsStatus: state.video?.clipSemanticsStatus || "",
      semanticsError: state.video?.clipSemanticsError || null,
      feedback: state.video?.clipFeedback || null,
      feedbackStatus: state.video?.clipFeedbackStatus || "",
      feedbackError: state.video?.clipFeedbackError || null,
      suggestion: state.video?.clipSuggestion || null,
      suggestionStatus: state.video?.clipSuggestionStatus || "",
      domSemanticsText: document.querySelector("#video-semantics")?.innerText || "",
      domSuggestionText: document.querySelector("#video-suggestion")?.innerText || "",
      domQueueText: document.querySelector("#video-queue")?.innerText || "",
      layout: {
        viewport: { w: innerWidth, h: innerHeight },
        editor: { w: Math.round(editor?.width || 0), h: Math.round(editor?.height || 0) },
        deliveryPanel: { w: Math.round(panel?.width || 0), h: Math.round(panel?.height || 0) },
        semantics: { w: Math.round(semantics?.width || 0), h: Math.round(semantics?.height || 0) }
      },
      consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
      pageErrors: window.__capyPageErrors || []
    };
  }`;
}
