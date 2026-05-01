import { semanticStateSource } from "./verify-video-clip-semantics-evals.mjs";

export function proposalLoadEval(projectPath, compositionPath = "") {
  return `new Promise(async resolve => {
    ${proposalStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await waitForWorkbench(wait);
    await loadProjectVideoQueue(wait, ${JSON.stringify(projectPath)});
    await openVideoWorkspace(wait, ${JSON.stringify(compositionPath)}, "camera-a");
    await waitForQueue(wait, 2);
    await waitForSemantics(wait, 2);
    resolve(await proposalState("loaded"));
  })`;
}

export function proposalHistoryReopenEval(projectPath, compositionPath = "") {
  return `new Promise(async resolve => {
    ${proposalStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await waitForWorkbench(wait);
    await loadProjectVideoQueue(wait, ${JSON.stringify(projectPath)});
    await openVideoWorkspace(wait, ${JSON.stringify(compositionPath)}, "camera-a");
    await waitForQueue(wait, 2);
    await waitForProposalHistory(wait, 3);
    resolve(await proposalState("proposal-history-reopened"));
  })`;
}

export function proposalSaveFeedbackEval(text) {
  return `new Promise(async resolve => {
    ${proposalStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    const card = document.querySelector(".video-queue-card");
    const input = card?.querySelector("[data-video-feedback-text]");
    if (input) {
      input.value = ${JSON.stringify(text)};
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }
    card?.querySelector("[data-video-feedback-save]")?.click();
    await waitForFeedback(wait, ${JSON.stringify(text)});
    resolve(await proposalState("feedback-saved"));
  })`;
}

export function proposalGenerateEval() {
  return `new Promise(async resolve => {
    ${proposalStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("#video-suggestion-generate")?.click();
    await waitForFeedbackSuggestion(wait, 2);
    document.querySelector("[data-video-generate-proposal]")?.click();
    await waitForProposal(wait, "proposed");
    resolve(await proposalState("proposal-generated"));
  })`;
}

export function proposalRejectEval() {
  return `new Promise(async resolve => {
    ${proposalStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("[data-video-proposal-decision='reject']")?.click();
    await waitForProposal(wait, "rejected");
    resolve(await proposalState("proposal-rejected"));
  })`;
}

export function proposalAcceptEval() {
  return `new Promise(async resolve => {
    ${proposalStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("[data-video-generate-proposal]")?.click();
    await waitForProposal(wait, "proposed");
    document.querySelector("[data-video-proposal-decision='accept']")?.click();
    await waitForProposal(wait, "accepted");
    await waitForAcceptedQueue(wait, 2);
    resolve(await proposalState("proposal-accepted"));
  })`;
}

export function proposalAcceptCurrentEval(expectedStatus = "accepted") {
  return `new Promise(async resolve => {
    ${proposalStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("[data-video-proposal-decision='accept']")?.click();
    await waitForProposal(wait, ${JSON.stringify(expectedStatus)});
    resolve(await proposalState(${JSON.stringify(expectedStatus === "conflicted" ? "proposal-conflicted" : "proposal-accepted")}));
  })`;
}

function proposalStateSource() {
  return `${semanticStateSource()}
  async function waitForFeedback(wait, text) {
    for (let i = 0; i < 160; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const items = state.video.clipFeedback?.items || [];
      const domText = document.querySelector("#video-queue")?.innerText || "";
      if (items.some(item => item.feedback === text) && domText.includes("用户反馈")) return;
      await wait(100);
    }
  }
  async function waitForFeedbackSuggestion(wait, count) {
    for (let i = 0; i < 160; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const items = state.video.clipSuggestion?.items || [];
      const domText = document.querySelector("#video-suggestion")?.innerText || "";
      if (state.video.clipSuggestionStatus === "ready" && items.length >= count && items.some(item => item.feedback_reason) && domText.includes("反馈调整")) return;
      await wait(100);
    }
  }
  async function waitForProposal(wait, status) {
    for (let i = 0; i < 180; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const proposal = state.video.clipSuggestionProposal;
      const domText = document.querySelector("#video-suggestion")?.innerText || "";
      if (proposal?.status === status && domText.includes("修改提案")) return;
      await wait(100);
    }
  }
  async function waitForProposalHistory(wait, count) {
    for (let i = 0; i < 180; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const history = state.video.clipSuggestionProposalHistory || [];
      const domText = document.querySelector(".video-proposal-history")?.innerText || "";
      if (history.length >= count && domText.toLowerCase().includes("proposal history") && domText.includes("历史详情只读")) return;
      await wait(100);
    }
  }
  async function waitForAcceptedQueue(wait, count) {
    for (let i = 0; i < 180; i += 1) {
      const state = window.capyWorkbench.stateSnapshot();
      const queue = state.video.clipQueue || [];
      if (queue.length >= count && state.video.clipQueuePersistStatus === "saved") return;
      await wait(100);
    }
  }
  async function proposalState(stage) {
    const base = await semanticState(stage);
    const state = window.capyWorkbench?.stateSnapshot ? window.capyWorkbench.stateSnapshot() : {};
    const proposal = state.video?.clipSuggestionProposal || null;
    const proposalHistory = state.video?.clipSuggestionProposalHistory || [];
    const proposalHistoryManifest = state.video?.clipSuggestionProposalHistoryManifest || null;
    const proposalEl = document.querySelector(".video-proposal-diff")?.getBoundingClientRect();
    const historyEl = document.querySelector(".video-proposal-history")?.getBoundingClientRect();
    return {
      ...base,
      proposal,
      proposalHistory,
      proposalHistoryManifest,
      proposalStatus: state.video?.clipSuggestionProposalStatus || "",
      proposalError: state.video?.clipSuggestionProposalError || null,
      domProposalText: document.querySelector(".video-proposal-diff")?.innerText || "",
      domProposalHistoryText: document.querySelector(".video-proposal-history")?.innerText || "",
      historyDecisionButtonCount: document.querySelectorAll(".video-proposal-history [data-video-proposal-decision]").length,
      layout: {
        ...base.layout,
        proposal: { w: Math.round(proposalEl?.width || 0), h: Math.round(proposalEl?.height || 0) },
        proposalHistory: { w: Math.round(historyEl?.width || 0), h: Math.round(historyEl?.height || 0) }
      }
    };
  }`;
}
