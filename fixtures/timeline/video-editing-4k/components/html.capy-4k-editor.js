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
  const accent = accentColor(params.accent);
  const soft = accentSoft(params.accent);
  const playhead = Math.round(8 + progress * 78);
  root.innerHTML = `
    <section style="
      position: relative;
      width: 100%;
      height: 100%;
      overflow: hidden;
      color: #1c1917;
      font-family: 'PingFang SC', 'Source Han Sans CN', -apple-system, BlinkMacSystemFont, sans-serif;
      background: linear-gradient(135deg, #fdba74 0%, #fef3c7 48%, #c4b5fd 100%);
    ">
      <div style="
        position: absolute;
        inset: 110px 128px;
        display: grid;
        grid-template-columns: 520px 1fr 560px;
        grid-template-rows: 170px 1fr 260px;
        gap: 44px;
      ">
        <header style="
          grid-column: 1 / -1;
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 0 54px;
          border-radius: 56px;
          border: 2px solid rgba(255, 250, 240, .72);
          background: rgba(255, 250, 240, .68);
          box-shadow: 0 28px 78px rgba(28, 25, 23, .14);
          backdrop-filter: blur(26px);
        ">
          <div style="display: flex; align-items: center; gap: 30px;">
            <span style="
              width: 88px;
              height: 88px;
              display: grid;
              place-items: center;
              border-radius: 28px;
              background: #1c1917;
              color: #fffaf0;
              font-size: 42px;
              font-weight: 900;
            ">C</span>
            <div>
              <p style="margin: 0; color: #78716c; font-size: 26px; line-height: 1.1;">Capybara</p>
              <strong style="display: block; margin-top: 10px; font-size: 48px; line-height: 1;">视频剪辑工作台</strong>
            </div>
          </div>
          <div style="display: flex; align-items: center; gap: 18px;">
            ${tab("画布", false)}
            ${tab("视频剪辑", true, accent)}
            ${statusPill(escapeHtml(params.status || "Ready"), soft)}
          </div>
        </header>

        <aside style="${panelStyle()} padding: 42px;">
          <p style="${eyebrowStyle()}">Clips</p>
          ${clipRow("01", "打开 JSON", params.clip === "打开 JSON", "#a78bfa")}
          ${clipRow("02", "在线预览", params.clip === "在线预览", "#fdba74")}
          ${clipRow("03", "轨道检查", params.clip === "轨道检查", "#84cc16")}
          ${clipRow("04", "严格录制", params.clip === "严格录制", "#a78bfa")}
          ${clipRow("05", "4K 导出", params.clip === "4K 导出", "#fdba74")}
          ${clipRow("06", "证据留档", params.clip === "证据留档", "#84cc16")}
        </aside>

        <main style="
          position: relative;
          overflow: hidden;
          border-radius: 68px;
          background: #1c1917;
          box-shadow: 0 46px 140px rgba(28, 25, 23, .30);
        ">
          <div style="
            position: absolute;
            inset: 58px;
            border-radius: 54px;
            border: 2px solid rgba(255, 250, 240, .82);
            background:
              linear-gradient(135deg, rgba(255, 250, 240, .96), rgba(254, 243, 199, .90)),
              ${soft};
            padding: 148px 156px;
            display: flex;
            flex-direction: column;
            justify-content: center;
          ">
            <p style="
              margin: 0 0 46px;
              color: #78716c;
              font-size: 54px;
              font-weight: 900;
              letter-spacing: 0;
            ">${escapeHtml(params.eyebrow || "CAPYBARA TIMELINE")}</p>
            <h1 style="
              margin: 0;
              max-width: 1740px;
              color: #1c1917;
              font-size: 176px;
              line-height: 1.02;
              letter-spacing: 0;
              font-weight: 900;
            ">${escapeHtml(params.title || "视频剪辑从 JSON 打开")}</h1>
            <p style="
              margin: 64px 0 0;
              max-width: 1620px;
              color: #57534e;
              font-size: 64px;
              line-height: 1.34;
            ">${escapeHtml(params.subtitle || "")}</p>
            <div style="display: flex; align-items: center; gap: 24px; margin-top: 86px;">
              ${solidPill(escapeHtml(params.metric || "4K · 30fps"), accent)}
              ${outlinePill("Recorder only")}
              ${outlinePill("PM 可见证据")}
            </div>
          </div>
        </main>

        <aside style="${panelStyle()} padding: 42px;">
          <p style="${eyebrowStyle()}">Inspector</p>
          ${fieldRow("component", "html.capy-4k-editor")}
          ${fieldRow("resolution", "3840 × 2160")}
          ${fieldRow("fps", "30")}
          ${fieldRow("duration", "30s")}
          <p style="
            margin: 34px 0 0;
            color: #78716c;
            font-size: 30px;
            line-height: 1.46;
          ">同一份 render_source 负责在线预览、字段检查和 recorder 导出。</p>
        </aside>

        <footer style="
          grid-column: 1 / -1;
          display: grid;
          grid-template-columns: 430px 1fr 360px;
          gap: 42px;
          align-items: center;
          padding: 44px;
          border-radius: 60px;
          border: 2px solid rgba(255, 250, 240, .72);
          background: rgba(255, 250, 240, .72);
          box-shadow: 0 28px 82px rgba(28, 25, 23, .14);
          backdrop-filter: blur(26px);
        ">
          <div>
            <p style="margin: 0; color: #78716c; font-size: 28px;">Timeline</p>
            <strong style="display: block; margin-top: 14px; font-size: 54px;">00:00 - 00:30</strong>
          </div>
          <div style="position: relative; height: 118px; border-radius: 36px; background: rgba(28, 25, 23, .08); overflow: hidden;">
            ${timelineBlock(3, 14, "#a78bfa")}
            ${timelineBlock(18, 14, "#fdba74")}
            ${timelineBlock(33, 14, "#84cc16")}
            ${timelineBlock(48, 14, "#a78bfa")}
            ${timelineBlock(63, 14, "#fdba74")}
            ${timelineBlock(78, 14, "#84cc16")}
            <span style="position: absolute; left: ${playhead}%; top: 10px; width: 8px; height: 98px; border-radius: 99px; background: #fffaf0; box-shadow: 0 0 0 4px rgba(28, 25, 23, .22);"></span>
          </div>
          <div style="display: flex; justify-content: flex-end; gap: 18px;">
            ${button("录制", false)}
            ${button("导出", true)}
          </div>
        </footer>
      </div>
    </section>
  `;
}

export function destroy(root) {
  root.textContent = "";
}

function panelStyle() {
  return `
    border-radius: 56px;
    border: 2px solid rgba(255, 250, 240, .72);
    background: rgba(255, 250, 240, .64);
    box-shadow: 0 28px 80px rgba(28, 25, 23, .12);
    backdrop-filter: blur(24px);
  `;
}

function eyebrowStyle() {
  return "margin: 0 0 28px; color: #78716c; font-size: 28px; font-weight: 800;";
}

function tab(label, active, color = "#a78bfa") {
  return `<span style="
    display: inline-flex;
    align-items: center;
    height: 74px;
    padding: 0 36px;
    border-radius: 999px;
    background: ${active ? color : "rgba(28, 25, 23, .06)"};
    color: ${active ? "#1c1917" : "#78716c"};
    font-size: 32px;
    font-weight: 900;
  ">${label}</span>`;
}

function statusPill(label, background) {
  return `<span style="
    display: inline-flex;
    align-items: center;
    height: 74px;
    padding: 0 36px;
    border-radius: 999px;
    background: ${background};
    color: #57534e;
    font-size: 30px;
    font-weight: 800;
  ">${label}</span>`;
}

function clipRow(index, label, active, color) {
  return `<div style="
    display: flex;
    align-items: center;
    gap: 24px;
    margin-bottom: 20px;
    padding: 26px;
    border-radius: 38px;
    background: ${active ? "rgba(255, 250, 240, .90)" : "rgba(28, 25, 23, .05)"};
    box-shadow: ${active ? "0 18px 44px rgba(28, 25, 23, .12)" : "none"};
  ">
    <span style="
      width: 76px;
      height: 76px;
      display: grid;
      place-items: center;
      border-radius: 24px;
      background: ${active ? color : "rgba(28, 25, 23, .10)"};
      color: #1c1917;
      font-size: 32px;
      font-weight: 900;
    ">${index}</span>
    <strong style="font-size: 36px; line-height: 1.12;">${label}</strong>
  </div>`;
}

function fieldRow(label, value) {
  return `<div style="padding: 28px 0; border-bottom: 2px solid rgba(120, 113, 108, .14);">
    <p style="margin: 0 0 12px; color: #78716c; font-size: 26px;">${label}</p>
    <strong style="display: block; font-size: 38px; line-height: 1.15;">${value}</strong>
  </div>`;
}

function solidPill(label, color) {
  return `<span style="display: inline-flex; align-items: center; height: 86px; padding: 0 34px; border-radius: 999px; background: ${color}; color: #1c1917; font-size: 38px; font-weight: 900;">${label}</span>`;
}

function outlinePill(label) {
  return `<span style="display: inline-flex; align-items: center; height: 86px; padding: 0 34px; border-radius: 999px; border: 2px solid rgba(120, 113, 108, .24); background: rgba(255, 250, 240, .66); color: #57534e; font-size: 38px; font-weight: 800;">${label}</span>`;
}

function timelineBlock(left, width, color) {
  return `<span style="position: absolute; left: ${left}%; top: 24px; width: ${width}%; height: 70px; border-radius: 26px; background: ${color}; box-shadow: 0 16px 42px ${color}55;"></span>`;
}

function button(label, primary) {
  return `<span style="
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 88px;
    min-width: 146px;
    border-radius: 999px;
    background: ${primary ? "#1c1917" : "rgba(28, 25, 23, .08)"};
    color: ${primary ? "#fffaf0" : "#1c1917"};
    font-size: 38px;
    font-weight: 900;
  ">${label}</span>`;
}

function accentColor(name) {
  if (name === "mint") return "#84cc16";
  if (name === "peach") return "#fdba74";
  return "#a78bfa";
}

function accentSoft(name) {
  if (name === "mint") return "rgba(132, 204, 22, .18)";
  if (name === "peach") return "rgba(253, 186, 116, .20)";
  return "rgba(167, 139, 250, .20)";
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
