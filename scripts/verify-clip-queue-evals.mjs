export function queueAddedEval(projectPath, outputDirectory) {
  return `new Promise(async resolve => {
    ${queueStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    window.CAPY_VIDEO_EXPORT_DIR = ${JSON.stringify(outputDirectory)};
    for (let i = 0; i < 100; i += 1) {
      if (window.capyWorkbench?.loadProjectPackage && window.capyWorkbench?.stateSnapshot) break;
      await wait(100);
    }
    if (!window.capyWorkbench?.loadProjectPackage) {
      resolve({
        stage: "added",
        workspace: "",
        queue: [],
        domQueue: [],
        error: "window.capyWorkbench.loadProjectPackage unavailable",
        consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
        pageErrors: window.__capyPageErrors || []
      });
      return;
    }
    await window.capyWorkbench.loadProjectPackage(${JSON.stringify(projectPath)});
    for (let i = 0; i < 100; i += 1) {
      const cards = [...document.querySelectorAll('.project-workbench-card[data-project-card-kind="video"]')];
      if (cards.length >= 2) break;
      await wait(100);
    }
    const cards = [...document.querySelectorAll('.project-workbench-card[data-project-card-kind="video"]')];
    const cardA = cards.find(card => (card.dataset.videoFilename || "").includes("camera-a")) || cards[0];
    const cardB = cards.find(card => (card.dataset.videoFilename || "").includes("camera-b")) || cards[1];
    async function openCard(card, filename) {
      card?.click();
      for (let i = 0; i < 140; i += 1) {
        const state = window.capyWorkbench.stateSnapshot();
        const preview = document.querySelector("#video-preview");
        const source = state.video.editor?.source_video?.filename || "";
        if (state.workspace.activeTab === "video" && source.includes(filename) && preview?.dataset.previewReady === "true" && preview?.dataset.videoReady === "true") break;
        await wait(100);
      }
    }
    await openCard(cardA, "camera-a");
    window.capyWorkbench.setVideoSelectedRange({
      clip_id: "source",
      scene: "Camera A opening detail",
      start_ms: 500,
      end_ms: 2000,
      duration_ms: 1500
    });
    document.querySelector("#video-queue-add")?.click();
    await wait(150);
    await openCard(cardB, "camera-b");
    window.capyWorkbench.setVideoSelectedRange({
      clip_id: "source",
      scene: "Camera B product closeup",
      start_ms: 1000,
      end_ms: 3000,
      duration_ms: 2000
    });
    document.querySelector("#video-queue-add")?.click();
    await wait(150);
    window.capyWorkbench.setVideoSelectedRange({
      clip_id: "source",
      scene: "Camera B temporary tail",
      start_ms: 3200,
      end_ms: 4000,
      duration_ms: 800
    });
    document.querySelector("#video-queue-add")?.click();
    await wait(250);
    resolve(queueState("added"));
  })`;
}

export function queueReorderedEval() {
  return `new Promise(async resolve => {
    ${queueStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector('.video-queue-card[data-sequence="2"] [data-video-queue-move="-1"]')?.click();
    await wait(200);
    document.querySelector('.video-queue-card[data-sequence="3"] [data-video-queue-remove]')?.click();
    await wait(250);
    resolve(queueState("reordered"));
  })`;
}

export function queueProposalExportEval() {
  return `new Promise(async resolve => {
    ${queueStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("#video-proposal-generate")?.click();
    await wait(250);
    document.querySelector("[data-video-confirm-proposal]")?.click();
    for (let i = 0; i < 260; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if (["done", "failed"].includes(state.video.exportJob?.status)) break;
      await wait(250);
    }
    resolve(queueState("exported"));
  })`;
}

function queueStateSource() {
  return `function queueState(stage) {
    const state = window.capyWorkbench.stateSnapshot();
    const editor = document.querySelector("[data-section=video-editor]")?.getBoundingClientRect();
    const panel = document.querySelector("[data-component=video-clip-delivery]")?.getBoundingClientRect();
    const queue = [...document.querySelectorAll(".video-queue-card")].map(card => ({
      sequence: Number(card.dataset.sequence || 0),
      id: card.dataset.queueItemId || "",
      text: card.innerText || ""
    }));
    return {
      stage,
      workspace: state.workspace.activeTab,
      selectedRange: state.video.selectedRange,
      queue: state.video.clipQueue || [],
      proposal: state.video.clipProposal,
      exportJob: state.video.exportJob,
      lastExport: state.video.lastExport,
      domQueue: queue,
      queueSummary: document.querySelector("#video-queue-summary")?.textContent || "",
      proposalText: document.querySelector("#video-proposal")?.innerText || "",
      layout: {
        viewport: { w: innerWidth, h: innerHeight },
        editor: { w: Math.round(editor?.width || 0), h: Math.round(editor?.height || 0) },
        deliveryPanel: { w: Math.round(panel?.width || 0), h: Math.round(panel?.height || 0) }
      },
      consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
      pageErrors: window.__capyPageErrors || []
    };
  }`;
}
