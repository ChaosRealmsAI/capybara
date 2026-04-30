export function queueCreateAndPersistEval(projectPath, outputDirectory) {
  return `new Promise(async resolve => {
    ${queueStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    window.CAPY_VIDEO_EXPORT_DIR = ${JSON.stringify(outputDirectory)};
    await waitForWorkbench(wait);
    await window.capyWorkbench.loadProjectPackage(${JSON.stringify(projectPath)});
    await openVideoCard(wait, "camera-a");
    window.capyWorkbench.setVideoSelectedRange({
      clip_id: "source",
      scene: "Camera A opening detail",
      start_ms: 500,
      end_ms: 2000,
      duration_ms: 1500
    });
    document.querySelector("#video-queue-add")?.click();
    await waitForQueueSave(wait, 1);
    await openVideoCard(wait, "camera-b");
    window.capyWorkbench.setVideoSelectedRange({
      clip_id: "source",
      scene: "Camera B product closeup",
      start_ms: 1000,
      end_ms: 3000,
      duration_ms: 2000
    });
    document.querySelector("#video-queue-add")?.click();
    await waitForQueueSave(wait, 2);
    window.capyWorkbench.setVideoSelectedRange({
      clip_id: "source",
      scene: "Camera B temporary tail",
      start_ms: 3200,
      end_ms: 4000,
      duration_ms: 800
    });
    document.querySelector("#video-queue-add")?.click();
    await waitForQueueSave(wait, 3);
    resolve(queueState("created"));
  })`;
}

export function queueRestoreEval(projectPath, stage = "restored") {
  return `new Promise(async resolve => {
    ${queueStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await waitForWorkbench(wait);
    await window.capyWorkbench.loadProjectPackage(${JSON.stringify(projectPath)});
    await openVideoCard(wait, "camera-a");
    await waitForQueue(wait, 3);
    resolve(queueState(${JSON.stringify(stage)}));
  })`;
}

export function queueModifyAndPersistEval() {
  return `new Promise(async resolve => {
    ${queueStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector('.video-queue-card[data-sequence="2"] [data-video-queue-move="-1"]')?.click();
    await waitForQueueSave(wait, 3);
    document.querySelector('.video-queue-card[data-sequence="3"] [data-video-queue-remove]')?.click();
    await waitForQueueSave(wait, 2);
    await openVideoCard(wait, "camera-a");
    window.capyWorkbench.setVideoSelectedRange({
      clip_id: "source",
      scene: "Camera A ending detail",
      start_ms: 2200,
      end_ms: 3500,
      duration_ms: 1300
    });
    document.querySelector("#video-queue-add")?.click();
    await waitForQueueSave(wait, 3);
    resolve(queueState("modified"));
  })`;
}

export function queueExportEval() {
  return `new Promise(async resolve => {
    ${queueStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("#video-proposal-generate")?.click();
    await wait(250);
    document.querySelector("[data-video-confirm-proposal]")?.click();
    for (let i = 0; i < 320; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if (["done", "failed"].includes(state.video.exportJob?.status)) break;
      await wait(250);
    }
    resolve(queueState("exported"));
  })`;
}

function queueStateSource() {
  return `async function waitForWorkbench(wait) {
    for (let i = 0; i < 120; i += 1) {
      if (window.capyWorkbench?.loadProjectPackage && window.capyWorkbench?.stateSnapshot) return;
      await wait(100);
    }
  }
  async function openVideoCard(wait, filename) {
    for (let i = 0; i < 140; i += 1) {
      const cards = [...document.querySelectorAll('.project-workbench-card[data-project-card-kind="video"]')];
      if (cards.length >= 2) break;
      await wait(100);
    }
    const cards = [...document.querySelectorAll('.project-workbench-card[data-project-card-kind="video"]')];
    const card = cards.find(item => (item.dataset.videoFilename || "").includes(filename)) || cards[0];
    card?.click();
    for (let i = 0; i < 160; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const preview = document.querySelector("#video-preview");
      const source = state.video.editor?.source_video?.filename || "";
      if (state.workspace.activeTab === "video" && source.includes(filename) && preview?.dataset.previewReady === "true" && preview?.dataset.videoReady === "true") return;
      await wait(100);
    }
  }
  async function waitForQueueSave(wait, count) {
    for (let i = 0; i < 120; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if ((state.video.clipQueue || []).length === count && ["saved", "loaded"].includes(state.video.clipQueuePersistStatus)) return;
      await wait(100);
    }
  }
  async function waitForQueue(wait, count) {
    for (let i = 0; i < 120; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if ((state.video.clipQueue || []).length === count) return;
      await wait(100);
    }
  }
  function queueState(stage) {
    const state = window.capyWorkbench?.stateSnapshot ? window.capyWorkbench.stateSnapshot() : {};
    const editor = document.querySelector("[data-section=video-editor]")?.getBoundingClientRect();
    const panel = document.querySelector("[data-component=video-clip-delivery]")?.getBoundingClientRect();
    const queue = [...document.querySelectorAll(".video-queue-card")].map(card => ({
      sequence: Number(card.dataset.sequence || 0),
      id: card.dataset.queueItemId || "",
      text: card.innerText || ""
    }));
    return {
      stage,
      workspace: state.workspace?.activeTab || "",
      selectedRange: state.video?.selectedRange || null,
      queue: state.video?.clipQueue || [],
      queueManifest: state.video?.clipQueueManifest || null,
      persistStatus: state.video?.clipQueuePersistStatus || "",
      persistError: state.video?.clipQueuePersistError || null,
      proposal: state.video?.clipProposal || null,
      exportJob: state.video?.exportJob || null,
      lastExport: state.video?.lastExport || null,
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
