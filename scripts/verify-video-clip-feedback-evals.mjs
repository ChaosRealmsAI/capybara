import { semanticStateSource } from "./verify-video-clip-semantics-evals.mjs";

export function feedbackLoadEval(projectPath, compositionPath = "") {
  return `new Promise(async resolve => {
    ${feedbackStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await waitForWorkbench(wait);
    await loadProjectVideoQueue(wait, ${JSON.stringify(projectPath)});
    await openVideoWorkspace(wait, ${JSON.stringify(compositionPath)}, "camera-a");
    await waitForQueue(wait, 2);
    await waitForSemantics(wait, 2);
    resolve(await semanticState("loaded"));
  })`;
}

export function feedbackSaveEval(text) {
  return `new Promise(async resolve => {
    ${feedbackStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    const card = document.querySelector(".video-queue-card");
    const input = card?.querySelector("[data-video-feedback-text]");
    if (input) {
      input.value = ${JSON.stringify(text)};
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }
    card?.querySelector("[data-video-feedback-save]")?.click();
    await waitForFeedback(wait, ${JSON.stringify(text)});
    resolve(await semanticState("feedback-saved"));
  })`;
}

export function feedbackSuggestEval() {
  return `new Promise(async resolve => {
    ${feedbackStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    document.querySelector("#video-suggestion-generate")?.click();
    await waitForFeedbackSuggestion(wait, 2);
    resolve(await semanticState("feedback-suggested"));
  })`;
}

export function feedbackRestoreEval(projectPath, compositionPath = "") {
  return `new Promise(async resolve => {
    ${feedbackStateSource()}
    const wait = ms => new Promise(done => setTimeout(done, ms));
    await waitForWorkbench(wait);
    await loadProjectVideoQueue(wait, ${JSON.stringify(projectPath)});
    await openVideoWorkspace(wait, ${JSON.stringify(compositionPath)}, "camera-a");
    await waitForSemantics(wait, 2);
    await waitForFeedback(wait, "这段不适合开场");
    document.querySelector("#video-suggestion-generate")?.click();
    await waitForFeedbackSuggestion(wait, 2);
    resolve(await semanticState("restored"));
  })`;
}

function feedbackStateSource() {
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
  }`;
}
