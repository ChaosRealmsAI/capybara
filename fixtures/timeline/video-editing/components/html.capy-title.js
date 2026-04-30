export function mount(root) {
  root.textContent = "";
  root.style.position = "absolute";
  root.style.inset = "0";
  root.style.display = "block";
  root.style.overflow = "hidden";
}

export function update(root, ctx) {
  const params = ctx && ctx.params ? ctx.params : {};
  const progress = clamp01(Number(ctx && ctx.progress ? ctx.progress : 0));
  const accent = params.accent === "mint" ? "#84cc16" : "#a78bfa";
  const accentSoft = params.accent === "mint" ? "rgba(132, 204, 22, .20)" : "rgba(167, 139, 250, .22)";
  const playhead = Math.round(18 + progress * 64);
  const activeClipWidth = Math.round(38 + progress * 18);
  root.innerHTML = `
    <section style="
      width: 100%;
      height: 100%;
      position: relative;
      overflow: hidden;
      color: #1c1917;
      font-family: 'PingFang SC', 'Source Han Sans CN', -apple-system, BlinkMacSystemFont, sans-serif;
      background:
        linear-gradient(135deg, rgba(253, 186, 116, .96) 0%, rgba(254, 243, 199, .96) 48%, rgba(196, 181, 253, .98) 100%);
    ">
      <div style="
        position: absolute;
        inset: 46px 52px;
        display: grid;
        grid-template-columns: 300px 1fr 330px;
        grid-template-rows: 82px 1fr 160px;
        gap: 22px;
      ">
        <header style="
          grid-column: 1 / -1;
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 0 24px;
          border: 1px solid rgba(120, 113, 108, .20);
          border-radius: 28px;
          background: rgba(255, 250, 240, .70);
          box-shadow: 0 8px 24px rgba(28, 25, 23, .08);
          backdrop-filter: blur(18px);
        ">
          <div style="display: flex; align-items: center; gap: 14px;">
            <span style="
              width: 42px;
              height: 42px;
              display: grid;
              place-items: center;
              border-radius: 14px;
              background: #1c1917;
              color: #fffaf0;
              font-weight: 800;
              font-size: 18px;
            ">C</span>
            <div>
              <p style="margin: 0; color: #78716c; font-size: 15px; line-height: 1.2;">CAPYBARA</p>
              <strong style="display: block; margin-top: 4px; font-size: 24px; line-height: 1;">视频剪辑工作台</strong>
            </div>
          </div>
          <div style="display: flex; gap: 10px; align-items: center;">
            ${chip("画布", false)}
            ${chip("视频剪辑", true, accent)}
            ${chip(escapeHtml(params.status || "在线预览"), false)}
          </div>
        </header>

        <aside style="
          display: flex;
          flex-direction: column;
          gap: 14px;
          padding: 20px;
          border: 1px solid rgba(120, 113, 108, .18);
          border-radius: 28px;
          background: rgba(255, 250, 240, .64);
          backdrop-filter: blur(16px);
          box-shadow: 0 12px 28px rgba(28, 25, 23, .08);
        ">
          <p style="margin: 0; color: #78716c; font-size: 16px;">Clips</p>
          ${clipRow("01", escapeHtml(params.clip || "JSON 进入工作台"), true, accent)}
          ${clipRow("02", params.accent === "mint" ? "导出证据" : "在线预览与导出", params.accent === "mint", "#84cc16")}
          <div style="margin-top: auto; padding: 16px; border-radius: 20px; background: rgba(28, 25, 23, .90); color: #fffaf0;">
            <p style="margin: 0 0 8px; color: rgba(255, 250, 240, .62); font-size: 15px;">JSON source</p>
            <p style="margin: 0; font-family: 'JetBrains Mono', 'SF Mono', Menlo, monospace; font-size: 18px;">composition.v2</p>
          </div>
        </aside>

        <main style="
          position: relative;
          overflow: hidden;
          border-radius: 34px;
          background: #1c1917;
          box-shadow: 0 22px 60px rgba(28, 25, 23, .24);
        ">
          <div style="
            position: absolute;
            inset: 28px;
            border-radius: 28px;
            background:
              linear-gradient(135deg, rgba(255, 250, 240, .96), rgba(254, 243, 199, .90)),
              ${accentSoft};
            border: 1px solid rgba(255, 250, 240, .72);
            padding: 58px 64px;
            display: flex;
            flex-direction: column;
            justify-content: center;
          ">
            <p style="
              margin: 0 0 24px;
              color: #78716c;
              font-size: 28px;
              font-weight: 800;
              letter-spacing: 0;
            ">${escapeHtml(params.eyebrow || "CAPYBARA TIMELINE")}</p>
            <h1 style="
              margin: 0;
              max-width: 980px;
              color: #1c1917;
              font-size: 88px;
              line-height: 1.04;
              letter-spacing: 0;
              font-weight: 850;
            ">${escapeHtml(params.title || "视频剪辑像画布一样打开")}</h1>
            <p style="
              margin: 30px 0 0;
              max-width: 900px;
              color: #57534e;
              font-size: 34px;
              line-height: 1.34;
            ">${escapeHtml(params.subtitle || "")}</p>
            <div style="display: flex; gap: 14px; margin-top: 44px; align-items: center;">
              ${solidPill(escapeHtml(params.metric || "2 clips · 4s"), accent)}
              ${outlinePill("AI 可操作")}
              ${outlinePill("PM 可见证据")}
            </div>
          </div>
        </main>

        <aside style="
          padding: 20px;
          border: 1px solid rgba(120, 113, 108, .18);
          border-radius: 28px;
          background: rgba(255, 250, 240, .64);
          backdrop-filter: blur(16px);
          box-shadow: 0 12px 28px rgba(28, 25, 23, .08);
        ">
          <p style="margin: 0 0 14px; color: #78716c; font-size: 16px;">Inspector</p>
          ${fieldRow("component", "html.capy-title")}
          ${fieldRow("status", escapeHtml(params.status || "ready"))}
          ${fieldRow("export", params.accent === "mint" ? "mp4 ready" : "preview first")}
          <div style="height: 1px; background: rgba(120, 113, 108, .18); margin: 18px 0;"></div>
          <p style="margin: 0; color: #78716c; font-size: 16px; line-height: 1.45;">同一份 render_source 同时服务在线查看、字段检查和导出证据。</p>
        </aside>

        <footer style="
          grid-column: 1 / -1;
          display: grid;
          grid-template-columns: 260px 1fr 220px;
          align-items: center;
          gap: 22px;
          padding: 22px;
          border: 1px solid rgba(120, 113, 108, .18);
          border-radius: 30px;
          background: rgba(255, 250, 240, .72);
          backdrop-filter: blur(18px);
          box-shadow: 0 12px 32px rgba(28, 25, 23, .10);
        ">
          <div>
            <p style="margin: 0; color: #78716c; font-size: 15px;">Timeline</p>
            <strong style="display: block; margin-top: 8px; font-size: 28px;">00:00 - 00:04</strong>
          </div>
          <div style="position: relative; height: 72px; border-radius: 22px; background: rgba(28, 25, 23, .08); overflow: hidden;">
            <span style="position: absolute; left: 3%; top: 14px; width: ${activeClipWidth}%; height: 44px; border-radius: 16px; background: ${accent}; box-shadow: 0 8px 22px ${accentSoft};"></span>
            <span style="position: absolute; left: 56%; top: 14px; width: 38%; height: 44px; border-radius: 16px; background: rgba(28, 25, 23, .86);"></span>
            <span style="position: absolute; left: ${playhead}%; top: 6px; width: 4px; height: 60px; border-radius: 99px; background: #fffaf0; box-shadow: 0 0 0 2px rgba(28, 25, 23, .18);"></span>
          </div>
          <div style="display: flex; justify-content: flex-end; gap: 10px;">
            ${button("录制", false)}
            ${button("导出", true, accent)}
          </div>
        </footer>
      </div>
    </section>
  `;
}

export function destroy(root) {
  root.textContent = "";
}

function chip(label, active, accent = "#a78bfa") {
  return `<span style="
    display: inline-flex;
    align-items: center;
    height: 38px;
    padding: 0 16px;
    border-radius: 999px;
    color: ${active ? "#1c1917" : "#78716c"};
    background: ${active ? accent : "rgba(28, 25, 23, .06)"};
    font-size: 16px;
    font-weight: ${active ? 800 : 650};
  ">${label}</span>`;
}

function clipRow(index, label, active, accent) {
  return `<div style="
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border-radius: 20px;
    background: ${active ? "rgba(255, 250, 240, .88)" : "rgba(28, 25, 23, .05)"};
    box-shadow: ${active ? "0 8px 20px rgba(28, 25, 23, .08)" : "none"};
  ">
    <span style="
      width: 42px;
      height: 42px;
      display: grid;
      place-items: center;
      border-radius: 14px;
      background: ${active ? accent : "rgba(28, 25, 23, .10)"};
      color: ${active ? "#1c1917" : "#78716c"};
      font-weight: 850;
      font-size: 17px;
    ">${index}</span>
    <strong style="font-size: 19px; line-height: 1.18;">${label}</strong>
  </div>`;
}

function fieldRow(label, value) {
  return `<div style="padding: 14px 0; border-bottom: 1px solid rgba(120, 113, 108, .14);">
    <p style="margin: 0 0 7px; color: #78716c; font-size: 14px;">${label}</p>
    <strong style="display: block; font-size: 20px; line-height: 1.2;">${value}</strong>
  </div>`;
}

function solidPill(label, color) {
  return `<span style="display: inline-flex; align-items: center; height: 46px; padding: 0 18px; border-radius: 999px; background: ${color}; color: #1c1917; font-size: 20px; font-weight: 850;">${label}</span>`;
}

function outlinePill(label) {
  return `<span style="display: inline-flex; align-items: center; height: 46px; padding: 0 18px; border-radius: 999px; border: 1px solid rgba(120, 113, 108, .24); background: rgba(255, 250, 240, .64); color: #57534e; font-size: 20px; font-weight: 700;">${label}</span>`;
}

function button(label, primary, accent = "#a78bfa") {
  return `<span style="
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 48px;
    min-width: 86px;
    border-radius: 999px;
    background: ${primary ? "#1c1917" : "rgba(28, 25, 23, .08)"};
    color: ${primary ? "#fffaf0" : "#1c1917"};
    box-shadow: ${primary ? `0 10px 24px ${accent}44` : "none"};
    font-size: 20px;
    font-weight: 850;
  ">${label}</span>`;
}

function clamp01(value) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(1, value));
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
