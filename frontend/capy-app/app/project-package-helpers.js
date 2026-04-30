export function firstSelectableCard(workbench) {
  const cards = workbench?.cards || [];
  return cards.find((card) => card.kind === "web" && card.id?.startsWith("art_"))
    || cards.find((card) => card.id?.startsWith("art_"))
    || cards[0]
    || null;
}

export function escapeText(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

export function selectedCardSummary(rootState) {
  const packageState = rootState.projectPackage;
  const card = packageState?.workbench?.cards?.find((item) => item.id === packageState.selectedCardId);
  return card ? `${card.title} · ${card.status}` : "";
}

export function selectedArtifactSummary(rootState) {
  const packageState = rootState.projectPackage;
  const artifact = packageState?.inspection?.artifacts?.artifacts?.find((item) => item.id === packageState.selectedArtifactId);
  return artifact ? `${artifact.kind} · ${artifact.source_path}` : "";
}

export function previewFrameSource(artifact, source, packageState = {}) {
  if (artifact?.kind === "video") return videoPreviewFrameSource(artifact, packageState);
  if (!source) return "<!doctype html><p>No artifact preview</p>";
  if (artifact?.kind === "html" || source.trimStart().startsWith("<svg")) return source;
  return `<!doctype html><pre style="white-space:pre-wrap;font:12px ui-monospace,monospace;padding:16px;color:#2f2437">${escapeText(source)}</pre>`;
}

export function assetFileUrl(root, relativePath) {
  const path = absoluteProjectPath(root, relativePath);
  return path ? fileUrl(path) : "";
}

export function absoluteProjectPath(root, relativePath) {
  const rel = String(relativePath || "");
  if (!rel) return "";
  if (/^(file|https?|data|blob):/i.test(rel)) return rel;
  if (rel.startsWith("/")) return rel;
  const base = String(root || "").replace(/\/+$/, "");
  return base ? `${base}/${rel.replace(/^\/+/, "")}` : rel;
}

function videoPreviewFrameSource(artifact, packageState) {
  const meta = artifact?.provenance?.video_import || {};
  const frame = meta.poster_frame_path
    ? assetFileUrl(packageState.path, meta.poster_frame_path)
    : "";
  return `<!doctype html>
<style>
body{margin:0;font:13px -apple-system,BlinkMacSystemFont,Segoe UI,sans-serif;background:#111;color:#f6efe7}
.wrap{min-height:100vh;display:grid;grid-template-rows:auto 1fr;gap:12px;padding:14px;box-sizing:border-box}
img{width:100%;aspect-ratio:16/9;object-fit:cover;border-radius:10px;background:#000}
dl{display:grid;grid-template-columns:90px minmax(0,1fr);gap:6px 10px;margin:0}
dt{color:#b9aea4}dd{margin:0;overflow-wrap:anywhere}
</style>
<div class="wrap">
${frame ? `<img src="${escapeText(frame)}" alt="">` : "<div></div>"}
<dl>
<dt>文件</dt><dd>${escapeText(meta.filename || artifact.title || artifact.id)}</dd>
<dt>时长</dt><dd>${escapeText(formatDuration(meta.duration_ms))}</dd>
<dt>尺寸</dt><dd>${escapeText(meta.width && meta.height ? `${meta.width}x${meta.height}` : "unknown")}</dd>
<dt>Composition</dt><dd>${escapeText(meta.composition_path || "")}</dd>
</dl>
</div>`;
}

function fileUrl(path) {
  const value = String(path || "");
  if (/^(file|https?|data|blob):/i.test(value)) return value;
  const workspace = workspaceRelativeUrl(value);
  if (workspace) return workspace;
  return `file://${value.split("/").map((part, index) => index === 0 ? "" : encodeURIComponent(part)).join("/")}`;
}

function workspaceRelativeUrl(path) {
  const cwd = String(window.CAPYBARA_SESSION?.cwd || "").replace(/\/+$/, "");
  if (!cwd || !path.startsWith(`${cwd}/`)) return "";
  return `/${path.slice(cwd.length + 1).split("/").map(encodeURIComponent).join("/")}`;
}

function formatDuration(ms) {
  const seconds = Number(ms || 0) / 1000;
  return Number.isFinite(seconds) && seconds > 0 ? `${seconds.toFixed(seconds >= 10 ? 1 : 2)}s` : "unknown";
}
