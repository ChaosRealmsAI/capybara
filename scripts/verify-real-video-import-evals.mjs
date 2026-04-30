export function artifactEval(projectPath) {
  return `new Promise(async resolve => {
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await window.capyWorkbench.loadProjectPackage(${JSON.stringify(projectPath)});
    for (let i = 0; i < 80; i += 1) {
      const card = document.querySelector('.project-workbench-card[data-project-card-kind="video"]');
      const img = card?.querySelector('.project-card-thumb');
      if (card && (!img || img.complete)) break;
      await wait(100);
    }
    const state = window.capyWorkbench.stateSnapshot();
    const cardData = state.projectPackage.workbench?.cards?.find(item => item.kind === "video");
    const card = document.querySelector('.project-workbench-card[data-project-card-kind="video"]');
    const img = card?.querySelector('.project-card-thumb');
    const workbench = document.querySelector("#project-workbench")?.getBoundingClientRect();
    const panel = document.querySelector("#project-package-panel")?.getBoundingClientRect();
    resolve({
      workspace: state.workspace.activeTab,
      projectPackage: state.projectPackage,
      card: {
        id: cardData?.id || card?.dataset?.projectCardId || "",
        kind: cardData?.preview?.kind || cardData?.kind || card?.dataset?.projectCardKind || "",
        text: card?.innerText || "",
        hasPoster: Boolean(img && img.complete && img.naturalWidth > 0),
        posterSize: img ? { w: img.naturalWidth, h: img.naturalHeight } : null,
        preview: cardData?.preview || null
      },
      layout: {
        viewport: { w: innerWidth, h: innerHeight },
        workbench: { w: Math.round(workbench?.width || 0), h: Math.round(workbench?.height || 0) },
        panel: { w: Math.round(panel?.width || 0), h: Math.round(panel?.height || 0) }
      },
      consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
      pageErrors: window.__capyPageErrors || []
    });
  })`;
}

export function rangeEval(projectPath, compositionPath, outputDirectory) {
  return `new Promise(async resolve => {
    const wait = ms => new Promise(done => setTimeout(done, ms));
    window.CAPY_VIDEO_EXPORT_DIR = ${JSON.stringify(outputDirectory)};
    await window.capyWorkbench.loadProjectPackage(${JSON.stringify(projectPath)});
    await window.capyWorkbench.openVideoComposition(${JSON.stringify(compositionPath)});
    for (let i = 0; i < 120; i += 1) {
      const preview = document.querySelector("#video-preview");
      if (preview?.dataset.previewReady === "true" && preview?.dataset.videoReady === "true") break;
      await wait(100);
    }
    window.capyWorkbench.setVideoSelectedRange({
      clip_id: "source",
      scene: "PM real source video",
      start_ms: 1000,
      end_ms: 3000,
      duration_ms: 2000
    });
    document.querySelector("#video-proposal-generate")?.click();
    await wait(350);
    const state = window.capyWorkbench.stateSnapshot();
    const preview = document.querySelector("#video-preview")?.getBoundingClientRect();
    const editor = document.querySelector("[data-section=video-editor]")?.getBoundingClientRect();
    const rangePanel = document.querySelector("[data-component=video-clip-delivery]")?.getBoundingClientRect();
    const video = document.querySelector("#video-preview video");
    resolve({
      workspace: state.workspace.activeTab,
      previewReady: document.querySelector("#video-preview")?.dataset.previewReady,
      videoReady: document.querySelector("#video-preview")?.dataset.videoReady,
      selectedRange: state.video.selectedRange,
      proposal: state.video.clipProposal,
      editorSourceVideo: state.video.editor?.source_video || null,
      videoElement: video ? {
        src: video.currentSrc || video.src || "",
        currentTime: video.currentTime,
        videoWidth: video.videoWidth,
        videoHeight: video.videoHeight
      } : null,
      layout: {
        viewport: { w: innerWidth, h: innerHeight },
        editor: { w: Math.round(editor?.width || 0), h: Math.round(editor?.height || 0) },
        preview: { w: Math.round(preview?.width || 0), h: Math.round(preview?.height || 0) },
        rangePanel: { w: Math.round(rangePanel?.width || 0), h: Math.round(rangePanel?.height || 0) }
      },
      consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
      pageErrors: window.__capyPageErrors || []
    });
  })`;
}

export function confirmExportEval() {
  return `new Promise(async resolve => {
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("[data-video-confirm-proposal]")?.click();
    for (let i = 0; i < 200; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      if (["done", "failed"].includes(state.video.exportJob?.status)) break;
      await wait(250);
    }
    const state = window.capyWorkbench.stateSnapshot();
    resolve({
      video: state.video,
      consoleErrors: (window.__capyConsoleEvents || []).filter(event => event.level === "error" || event.type === "error"),
      pageErrors: window.__capyPageErrors || []
    });
  })`;
}
