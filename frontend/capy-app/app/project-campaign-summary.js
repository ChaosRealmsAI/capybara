export function renderProjectCampaignSummary(el, campaign) {
  if (!el) return;
  const run = campaign?.run;
  if (!run) {
    el.hidden = true;
    el.replaceChildren();
    return;
  }
  const proposalCount = Number(campaign?.proposals?.length || 0);
  const targetCount = Number(campaign?.plan?.targets?.length || run.artifact_runs?.length || 0);
  el.hidden = false;
  el.dataset.campaignStatus = run.status || "";
  el.dataset.campaignId = run.id || "";
  el.innerHTML = `
    <div>
      <span class="context-eyebrow">CAMPAIGN</span>
      <strong>${escapeText(run.status || "proposed")} · ${proposalCount} proposals</strong>
      <small>${targetCount} artifacts · review required</small>
    </div>
    <p>${escapeText(run.brief || "")}</p>
  `;
}

export function projectCampaignMessage(result) {
  const proposalCount = Number(result?.proposals?.length || 0);
  return {
    role: "assistant",
    content: `### Campaign 已生成\n\nAI 已为 ${proposalCount} 个项目内容提出一组待审核修改。`
  };
}

function escapeText(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
