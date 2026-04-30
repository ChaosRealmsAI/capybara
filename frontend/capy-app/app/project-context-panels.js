export function renderDesignLanguageSummary(el, summary) {
  if (!el) return;
  if (!summary) {
    el.hidden = true;
    el.replaceChildren();
    return;
  }
  el.hidden = false;
  el.dataset.designLanguageRef = summary.design_language_ref || "";
  el.innerHTML = `
    <div>
      <span class="context-eyebrow">DESIGN LANGUAGE</span>
      <strong>${escapeText(summary.name || "Project Design Language")}</strong>
      <small>${escapeText(summary.version || "0.1.0")} · ${escapeText(shortRef(summary.design_language_ref))}</small>
    </div>
    <dl>
      <div><dt>tokens</dt><dd>${Number(summary.token_count || 0)}</dd></div>
      <div><dt>rules</dt><dd>${Number(summary.rule_count || 0)}</dd></div>
      <div><dt>refs</dt><dd>${Number(summary.reference_image_count || 0)}</dd></div>
      <div><dt>examples</dt><dd>${Number(summary.example_count || 0)}</dd></div>
    </dl>
  `;
}

export function renderSelectionContext(el, context) {
  if (!el) return;
  if (!context) {
    el.hidden = true;
    el.replaceChildren();
    return;
  }
  el.hidden = false;
  el.dataset.selectionKind = context.kind || "";
  el.dataset.selectionScope = context.scope || "";
  el.innerHTML = `
    <div>
      <span class="context-eyebrow">SELECTION CONTEXT</span>
      <strong>${escapeText(context.kind || "artifact")}</strong>
      <small>${escapeText(context.selector || context.json_pointer || context.fallback_reason || context.source_path || "")}</small>
    </div>
    <p>${escapeText(context.selected_text || context.fallback_reason || "Whole artifact context")}</p>
  `;
}

function escapeText(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function shortRef(value) {
  const text = String(value || "");
  if (text.length <= 22) return text || "no-ref";
  return `${text.slice(0, 18)}...`;
}
