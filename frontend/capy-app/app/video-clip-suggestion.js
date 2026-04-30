import { queueFromManifest } from "./video-clip-queue.js";

export function createVideoClipSuggestionController(ctx) {
  const {
    state,
    dom,
    rpc,
    projectPath,
    stringifyError,
    renderVideoEditor,
    renderDelivery,
    formatTime,
    escapeHtml
  } = ctx;

  function render() {
    if (!dom.videoSuggestionEl) return;
    const status = state.video.clipSuggestionStatus || "idle";
    const suggestion = state.video.clipSuggestion;
    if (status === "planning") {
      dom.videoSuggestionEl.hidden = false;
      dom.videoSuggestionEl.innerHTML = "<p>正在生成本地 AI 剪辑建议...</p>";
      return;
    }
    if (status === "error") {
      dom.videoSuggestionEl.hidden = false;
      dom.videoSuggestionEl.innerHTML = `<p>AI 建议生成失败：${escapeHtml(state.video.clipSuggestionError || "unknown")}</p>`;
      return;
    }
    if (!suggestion) {
      dom.videoSuggestionEl.hidden = true;
      dom.videoSuggestionEl.replaceChildren();
      return;
    }
    dom.videoSuggestionEl.hidden = false;
    const proposal = state.video.clipSuggestionProposal;
    const proposalStatus = state.video.clipSuggestionProposalStatus || proposal?.status || "idle";
    const rows = (suggestion.items || []).map((item) => `
      <li>
        <strong>${String(item.sequence).padStart(2, "0")} · ${escapeHtml(item.source_video?.filename || item.scene || item.clip_id)}</strong>
        <span>${formatTime(item.start_ms)} - ${formatTime(item.end_ms)} · ${formatTime(item.duration_ms)}</span>
        ${item.semantic_summary ? `<small>摘要：${escapeHtml(item.semantic_summary)}</small>` : ""}
        ${item.semantic_tags?.length ? `<small>标签：${item.semantic_tags.map(escapeHtml).join(" · ")}</small>` : ""}
        ${item.feedback_text ? `<small>用户反馈：${escapeHtml(item.feedback_text)}</small>` : ""}
        <em>${escapeHtml(item.reason || "本地 planner 建议")}</em>
        ${item.semantic_reason ? `<em>语义理由：${escapeHtml(item.semantic_reason)}</em>` : ""}
        ${item.feedback_reason ? `<em>反馈调整：${escapeHtml(item.feedback_reason)}</em>` : ""}
      </li>
    `).join("");
    dom.videoSuggestionEl.innerHTML = `
      <header class="video-suggestion-head">
        <div>
          <span>AI 剪辑建议</span>
          <strong>${escapeHtml(suggestion.suggestion_id || "suggestion")}</strong>
        </div>
        <button class="tool-button primary" type="button" data-video-generate-proposal>${proposalStatus === "planning" ? "生成中" : "生成修改提案"}</button>
      </header>
      <p>${escapeHtml(suggestion.rationale || "本地 deterministic planner 基于项目素材和队列生成。")}</p>
      <ol class="video-suggestion-list">${rows}</ol>
      ${renderProposalDiff(proposal, proposalStatus)}
    `;
    const generateProposal = dom.videoSuggestionEl.querySelector("[data-video-generate-proposal]");
    if (generateProposal) {
      generateProposal.disabled = proposalStatus === "planning";
      generateProposal.addEventListener("click", () => generateProposalDiff());
    }
    dom.videoSuggestionEl.querySelector("[data-video-proposal-decision=\"accept\"]")
      ?.addEventListener("click", () => decideProposal("accept"));
    dom.videoSuggestionEl.querySelector("[data-video-proposal-decision=\"reject\"]")
      ?.addEventListener("click", () => decideProposal("reject"));
  }

  async function generate() {
    const project = projectPath?.();
    if (!project || !rpc) {
      state.video.clipSuggestionStatus = "error";
      state.video.clipSuggestionError = "缺少项目路径";
      renderDelivery();
      return;
    }
    state.video.clipSuggestionStatus = "planning";
    state.video.clipSuggestionError = null;
    renderDelivery();
    try {
      const suggestion = await rpc("project-video-clip-queue-suggest", { project });
      state.video.clipSuggestion = suggestion;
      state.video.clipSuggestionStatus = "ready";
      state.video.clipSuggestionError = null;
      state.video.clipSuggestionProposal = null;
      state.video.clipSuggestionProposalStatus = "idle";
      state.video.clipSuggestionProposalError = null;
      renderDelivery();
    } catch (error) {
      state.video.clipSuggestionStatus = "error";
      state.video.clipSuggestionError = stringifyError ? stringifyError(error) : String(error);
      renderDelivery();
    }
  }

  async function generateProposalDiff() {
    const project = projectPath?.();
    if (!project || !rpc || !state.video.clipSuggestion) return;
    state.video.clipSuggestionProposalStatus = "planning";
    state.video.clipSuggestionProposalError = null;
    renderDelivery();
    try {
      const proposal = await rpc("project-video-clip-proposal-generate", { project });
      state.video.clipSuggestionProposal = proposal;
      state.video.clipSuggestionProposalStatus = proposal.status || "proposed";
      state.video.clipSuggestionProposalError = null;
      renderDelivery();
    } catch (error) {
      state.video.clipSuggestionProposalStatus = "error";
      state.video.clipSuggestionProposalError = stringifyError ? stringifyError(error) : String(error);
      renderDelivery();
    }
  }

  async function decideProposal(decision) {
    const project = projectPath?.();
    const proposal = state.video.clipSuggestionProposal;
    if (!project || !rpc || !proposal?.proposal_id) return;
    state.video.clipSuggestionProposalStatus = decision === "accept" ? "accepting" : "rejecting";
    state.video.clipSuggestionProposalError = null;
    renderDelivery();
    try {
      const result = await rpc("project-video-clip-proposal-decide", {
        project,
        proposal: proposal.proposal_id,
        decision,
        reason: decision === "accept" ? "PM accepted proposal diff in desktop UI" : "PM rejected proposal diff in desktop UI"
      });
      state.video.clipSuggestionProposal = result.proposal || proposal;
      state.video.clipSuggestionProposalStatus = result.proposal?.status || (decision === "accept" ? "accepted" : "rejected");
      if (result.queue_manifest) {
        state.video.clipQueueManifest = result.queue_manifest;
        state.video.clipQueue = queueFromManifest(result.queue_manifest, project);
        state.video.clipQueuePersistStatus = "saved";
        state.video.clipQueuePersistError = null;
        state.video.clipProposal = null;
        state.video.proposalStatus = "idle";
      }
      renderVideoEditor();
    } catch (error) {
      state.video.clipSuggestionProposalStatus = "error";
      state.video.clipSuggestionProposalError = stringifyError ? stringifyError(error) : String(error);
      renderDelivery();
    }
  }

  return { render, generate, generateProposalDiff, decideProposal };
}

function renderProposalDiff(proposal, status) {
  if (status === "planning") return `<div class="video-proposal-diff"><p>正在生成剪辑修改提案...</p></div>`;
  if (status === "error") return `<div class="video-proposal-diff"><p>修改提案生成失败。</p></div>`;
  if (!proposal) return "";
  const decided = ["accepted", "rejected"].includes(proposal.status);
  const rows = (proposal.changes || []).map((change) => `
    <li data-video-proposal-change="${escapeAttr(change.id || "")}">
      <strong>${escapeHtml(change.action_label_zh || change.action || "调整")} · ${escapeHtml(change.scene || "片段")}</strong>
      <span>Before ${positionLabel(change.before_sequence)} → After ${positionLabel(change.after_sequence)} · ${change.applicable ? "可应用" : "不可直接应用"} · ${escapeHtml(change.apply_status || "pending")}</span>
      <small>${escapeHtml(change.reason_summary || "")}</small>
      ${change.feedback_text ? `<em>用户反馈：${escapeHtml(change.feedback_text)}</em>` : ""}
      ${change.semantic_reason ? `<em>语义理由：${escapeHtml(change.semantic_reason)}</em>` : ""}
    </li>
  `).join("");
  return `
    <section class="video-proposal-diff" data-video-proposal-id="${escapeAttr(proposal.proposal_id || "")}" data-video-proposal-status="${escapeAttr(proposal.status || status || "")}">
      <header class="video-proposal-diff-head">
        <div>
          <span>修改提案</span>
          <strong>${escapeHtml(proposal.proposal_id || "proposal")}</strong>
        </div>
        <div class="video-proposal-diff-actions">
          <button class="tool-button secondary" type="button" data-video-proposal-decision="reject" ${decided ? "disabled" : ""}>拒绝提案</button>
          <button class="tool-button primary" type="button" data-video-proposal-decision="accept" ${decided ? "disabled" : ""}>接受提案</button>
        </div>
      </header>
      <p>${escapeHtml(proposal.rationale || "本地 proposal diff 等待 PM 决策。")}</p>
      <p>${escapeHtml(proposal.safety_note || "生成提案不会自动修改 queue。")}</p>
      <ol class="video-proposal-diff-list">${rows}</ol>
    </section>
  `;
}

function positionLabel(value) {
  return value ? `#${value}` : "无";
}

function escapeAttr(value) {
  return escapeHtml(value).replaceAll("'", "&#39;");
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
