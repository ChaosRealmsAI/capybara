export function suggestionGenerateEval(projectPath, outputDirectory) {
  return `new Promise(async resolve => {
    ${suggestionStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    window.CAPY_VIDEO_EXPORT_DIR = ${JSON.stringify(outputDirectory)};
    await waitForWorkbench(wait);
    await window.capyWorkbench.loadProjectPackage(${JSON.stringify(projectPath)});
    await openVideoCard(wait, "camera-a");
    await waitForQueue(wait, 2);
    document.querySelector("#video-suggestion-generate")?.click();
    await waitForSuggestion(wait, 3);
    resolve(suggestionState("suggested"));
  })`;
}

export function suggestionAdoptEval() {
  return `new Promise(async resolve => {
    ${suggestionStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("[data-video-adopt-suggestion]")?.click();
    await waitForAdoptedQueue(wait, 3);
    resolve(suggestionState("adopted"));
  })`;
}

export function suggestionRestoreEval(projectPath) {
  return `new Promise(async resolve => {
    ${suggestionStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await waitForWorkbench(wait);
    await window.capyWorkbench.loadProjectPackage(${JSON.stringify(projectPath)});
    await openVideoCard(wait, "camera-a");
    await waitForAdoptedQueue(wait, 3);
    resolve(suggestionState("restored"));
  })`;
}

export function suggestionExportEval(outputDirectory) {
  return `new Promise(async resolve => {
    ${suggestionStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    window.CAPY_VIDEO_EXPORT_DIR = ${JSON.stringify(outputDirectory)};
    document.querySelector("#video-proposal-generate")?.click();
    await wait(250);
    document.querySelector("[data-video-confirm-proposal]")?.click();
    for (let i = 0; i < 360; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if (["done", "failed"].includes(state.video.exportJob?.status)) break;
      await wait(250);
    }
    resolve(suggestionState("exported"));
  })`;
}

function suggestionStateSource() {
  return `async function waitForWorkbench(wait) {
    for (let i = 0; i < 120; i += 1) {
      if (window.capyWorkbench?.loadProjectPackage && window.capyWorkbench?.stateSnapshot) return;
      await wait(100);
    }
  }
  async function openVideoCard(wait, filename) {
    for (let i = 0; i < 160; i += 1) {
      const cards = [...document.querySelectorAll('.project-workbench-card[data-project-card-kind="video"]')];
      if (cards.length >= 3) break;
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
  async function waitForQueue(wait, count) {
    for (let i = 0; i < 140; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if ((state.video.clipQueue || []).length === count) return;
      await wait(100);
    }
  }
  async function waitForSuggestion(wait, count) {
    for (let i = 0; i < 140; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const items = state.video.clipSuggestion?.items || [];
      const domItems = document.querySelectorAll(".video-suggestion-list li");
      if (state.video.clipSuggestionStatus === "ready" && items.length >= count && domItems.length >= count) return;
      await wait(100);
    }
  }
  async function waitForAdoptedQueue(wait, count) {
    for (let i = 0; i < 180; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const queue = state.video.clipQueue || [];
      const hasSuggestion = queue.length >= count && queue.every(item => item.suggestion_id && item.suggestion_reason);
      if (hasSuggestion && ["saved", "loaded"].includes(state.video.clipQueuePersistStatus)) return;
      await wait(100);
    }
  }
  function suggestionState(stage) {
    const state = window.capyWorkbench?.stateSnapshot ? window.capyWorkbench.stateSnapshot() : {};
    const editor = document.querySelector("[data-section=video-editor]")?.getBoundingClientRect();
    const panel = document.querySelector("[data-component=video-clip-delivery]")?.getBoundingClientRect();
    const suggestion = document.querySelector("#video-suggestion")?.getBoundingClientRect();
    return {
      stage,
      workspace: state.workspace?.activeTab || "",
      queue: state.video?.clipQueue || [],
      queueManifest: state.video?.clipQueueManifest || null,
      persistStatus: state.video?.clipQueuePersistStatus || "",
      persistError: state.video?.clipQueuePersistError || null,
      suggestion: state.video?.clipSuggestion || null,
      suggestionStatus: state.video?.clipSuggestionStatus || "",
      suggestionError: state.video?.clipSuggestionError || null,
      proposal: state.video?.clipProposal || null,
      exportJob: state.video?.exportJob || null,
      lastExport: state.video?.lastExport || null,
      domSuggestionText: document.querySelector("#video-suggestion")?.innerText || "",
      domQueueText: document.querySelector("#video-queue")?.innerText || "",
      queueSummary: document.querySelector("#video-queue-summary")?.textContent || "",
      proposalText: document.querySelector("#video-proposal")?.innerText || "",
      layout: {
        viewport: { w: innerWidth, h: innerHeight },
        editor: { w: Math.round(editor?.width || 0), h: Math.round(editor?.height || 0) },
        deliveryPanel: { w: Math.round(panel?.width || 0), h: Math.round(panel?.height || 0) },
        suggestion: { w: Math.round(suggestion?.width || 0), h: Math.round(suggestion?.height || 0) }
      },
      consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
      pageErrors: window.__capyPageErrors || []
    };
  }`;
}
