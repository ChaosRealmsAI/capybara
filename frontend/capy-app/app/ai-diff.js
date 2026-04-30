export function createAiDiffReviewPanel(host) {
  let panel = host?.querySelector("[data-component='ai-diff-review']");
  if (!panel && host) {
    panel = document.createElement("section");
    panel.className = "ai-diff-review";
    panel.dataset.component = "ai-diff-review";
    host.append(panel);
  }

  function render(result, actions = {}) {
    if (!panel) return;
    const run = result?.run || null;
    const review = run?.review || null;
    const diff = review?.diff_summary || run?.output?.diff_summary || null;
    const hasReview = Boolean(run && review);
    panel.hidden = !hasReview;
    if (!hasReview) {
      panel.replaceChildren();
      return;
    }
    panel.dataset.status = review.status || run.status || "proposed";
    const status = review.status || run.status || "proposed";
    panel.innerHTML = `
      <header class="ai-diff-head">
        <div>
          <span class="context-eyebrow">AI DIFF</span>
          <h3>${escapeText(diff?.source_path || run.artifact_id || "Proposed artifact")}</h3>
        </div>
        <span class="ai-diff-status">${escapeText(status)}</span>
      </header>
      <div class="ai-diff-summary">
        <span>+${Number(diff?.added_lines || 0)}</span>
        <span>-${Number(diff?.removed_lines || 0)}</span>
        <span>${escapeText(shortHash(diff?.old_hash))} → ${escapeText(shortHash(diff?.new_hash))}</span>
      </div>
      <div class="ai-diff-preview">
        <pre>${escapeText(diff?.old_preview || "旧内容未显示")}</pre>
        <pre>${escapeText(diff?.new_preview || "新内容未显示")}</pre>
      </div>
      <div class="ai-diff-actions">
        <button type="button" data-ai-diff-action="accept">接受</button>
        <button type="button" data-ai-diff-action="reject">拒绝</button>
        <button type="button" data-ai-diff-action="retry">重试</button>
        <button type="button" data-ai-diff-action="undo">撤销</button>
      </div>
    `;
    const accept = panel.querySelector('[data-ai-diff-action="accept"]');
    const reject = panel.querySelector('[data-ai-diff-action="reject"]');
    const retry = panel.querySelector('[data-ai-diff-action="retry"]');
    const undo = panel.querySelector('[data-ai-diff-action="undo"]');
    accept.disabled = status !== "proposed";
    reject.disabled = status !== "proposed";
    retry.disabled = status !== "proposed" && status !== "rejected";
    undo.disabled = status !== "accepted";
    accept.addEventListener("click", () => actions.accept?.());
    reject.addEventListener("click", () => actions.reject?.());
    retry.addEventListener("click", () => actions.retry?.());
    undo.addEventListener("click", () => actions.undo?.());
  }

  return { render };
}

export function reviewRunId(result) {
  return result?.run?.id || "";
}

function shortHash(hash) {
  if (!hash) return "";
  const value = String(hash);
  return value.length > 16 ? value.slice(0, 16) : value;
}

function escapeText(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
