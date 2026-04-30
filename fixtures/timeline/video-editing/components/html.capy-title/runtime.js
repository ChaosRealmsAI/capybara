export function mount(root) {
  root.textContent = "";
  root.style.position = "absolute";
  root.style.inset = "0";
  root.style.overflow = "hidden";
}

export function update(root, ctx) {
  const p = ctx?.params || {};
  const progress = clamp01(Number(ctx?.progress || 0));
  const accent = accentColor(p.accent, ctx?.theme?.accent);
  const surface = ctx?.surface?.kind || ctx?.mode || "component";
  const playhead = Math.round(10 + progress * 80);
  root.innerHTML = `<section style="position:absolute;inset:0;overflow:hidden;background:linear-gradient(135deg,#fffaf0 0%,#fef3c7 48%,#ede9fe 100%);font-family:PingFang SC,Source Han Sans CN,-apple-system,BlinkMacSystemFont,sans-serif;color:#1c1917;">
    <div style="position:absolute;inset:7%;border:1px solid rgba(120,113,108,.22);border-radius:52px;background:rgba(255,250,240,.76);box-shadow:0 30px 80px rgba(28,25,23,.14);padding:6.5%;">
      <p style="margin:0 0 24px;color:#78716c;font-size:34px;font-weight:850;">${escapeHtml(p.eyebrow || "CAPYBARA COMPONENT")}</p>
      <h1 style="margin:0;max-width:1120px;font-size:102px;line-height:1.02;font-weight:900;">${escapeHtml(p.title || "同一组件用于海报、视频和网页")}</h1>
      <p style="margin:34px 0 0;max-width:920px;font-size:42px;line-height:1.32;color:#57534e;font-weight:650;">${escapeHtml(p.subtitle || "组件协议统一 mount/update/destroy 和 ctx。")}</p>
      <span style="position:absolute;left:6.5%;bottom:6.5%;display:inline-flex;height:76px;align-items:center;padding:0 34px;border-radius:999px;background:${accent};font-size:32px;font-weight:900;">${escapeHtml(p.metric || surface)}</span>
      <span style="position:absolute;right:6.5%;bottom:7.3%;width:280px;height:18px;border-radius:999px;background:rgba(28,25,23,.10);overflow:hidden;"><i style="display:block;width:${playhead}%;height:100%;background:#1c1917;"></i></span>
    </div>
  </section>`;
}

export function destroy(root) {
  root.textContent = "";
}

function accentColor(name, fallback) {
  if (name === "mint") return "#84cc16";
  if (name === "peach") return "#fdba74";
  return fallback || "#a78bfa";
}

function clamp01(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
