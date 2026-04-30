const HTML_PREVIEW_ALLOWED_TAGS = new Set([
  "article",
  "br",
  "code",
  "dd",
  "div",
  "dl",
  "dt",
  "em",
  "footer",
  "h1",
  "h2",
  "h3",
  "h4",
  "header",
  "hr",
  "li",
  "ol",
  "p",
  "pre",
  "section",
  "small",
  "span",
  "strong",
  "table",
  "tbody",
  "td",
  "th",
  "thead",
  "tr",
  "ul",
]);
const HTML_PREVIEW_DROP_TAGS = new Set([
  "audio",
  "base",
  "button",
  "canvas",
  "embed",
  "form",
  "iframe",
  "img",
  "input",
  "link",
  "meta",
  "object",
  "option",
  "picture",
  "script",
  "select",
  "source",
  "style",
  "svg",
  "textarea",
  "video",
]);
const HTML_PREVIEW_CLASSES = new Set([
  "capy-card",
  "capy-section",
  "capy-kicker",
  "capy-title",
  "capy-subtitle",
  "capy-grid",
  "capy-stat",
  "capy-table",
  "capy-timeline",
  "capy-step",
  "capy-badges",
  "capy-badge",
  "capy-note",
  "capy-risk",
  "capy-callout",
  "is-good",
  "is-warn",
  "is-bad",
  "is-muted",
]);
const HTML_PREVIEW_CSS = `
:root {
  color-scheme: light;
  --capy-bg: #fffaf5;
  --capy-panel: rgba(255, 255, 255, 0.82);
  --capy-border: rgba(68, 64, 60, 0.13);
  --capy-text: #1f1d1b;
  --capy-muted: #7c756e;
  --capy-accent: #7c5cff;
  --capy-green: #1d8f63;
  --capy-amber: #a05c00;
  --capy-red: #bd3b2e;
}
* { box-sizing: border-box; }
html, body {
  margin: 0;
  min-height: 100%;
  background: transparent;
  color: var(--capy-text);
  font: 13px/1.48 -apple-system, BlinkMacSystemFont, "PingFang SC", "Hiragino Sans GB", sans-serif;
}
body { padding: 12px; }
.capy-html-root {
  display: grid;
  gap: 10px;
}
.capy-html-root :is(h1, h2, h3, h4, p, ul, ol, dl, pre, table) { margin: 0; }
.capy-html-root :is(h1, h2, h3, h4) {
  line-height: 1.18;
  letter-spacing: 0;
}
.capy-html-root h1 { font-size: 22px; }
.capy-html-root h2 { font-size: 18px; }
.capy-html-root h3 { font-size: 15px; }
.capy-html-root h4 { font-size: 13px; }
.capy-html-root p,
.capy-html-root li,
.capy-html-root dd,
.capy-html-root td {
  color: var(--capy-muted);
}
.capy-html-root strong,
.capy-html-root th,
.capy-html-root dt {
  color: var(--capy-text);
  font-weight: 760;
}
.capy-html-root ul,
.capy-html-root ol { padding-left: 18px; }
.capy-html-root li + li { margin-top: 4px; }
.capy-card,
.capy-section,
.capy-callout,
.capy-note,
.capy-risk {
  border: 1px solid var(--capy-border);
  border-radius: 16px;
  background: var(--capy-panel);
  padding: 14px;
}
.capy-card {
  display: grid;
  gap: 10px;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.76), 0 12px 28px rgba(68, 64, 60, 0.08);
}
.capy-section { display: grid; gap: 8px; }
.capy-kicker {
  color: var(--capy-accent);
  font: 700 10px/1.2 ui-monospace, SFMono-Regular, Menlo, monospace;
  letter-spacing: .08em;
  text-transform: uppercase;
}
.capy-title {
  color: var(--capy-text);
  font-size: 19px;
  font-weight: 820;
}
.capy-subtitle { color: var(--capy-muted); }
.capy-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 8px;
}
.capy-stat {
  display: grid;
  gap: 4px;
  border: 1px solid rgba(68, 64, 60, 0.1);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.62);
  padding: 10px;
}
.capy-stat span:first-child {
  color: var(--capy-muted);
  font-size: 11px;
}
.capy-stat strong:last-child {
  font-size: 16px;
}
.capy-table {
  width: 100%;
  border-collapse: collapse;
  overflow: hidden;
  border-radius: 12px;
}
.capy-table :is(th, td) {
  border-top: 1px solid rgba(68, 64, 60, 0.1);
  padding: 8px;
  text-align: left;
  vertical-align: top;
}
.capy-table tr:first-child :is(th, td) { border-top: 0; }
.capy-timeline {
  display: grid;
  gap: 8px;
  list-style: none;
  padding-left: 0;
}
.capy-step {
  border-left: 3px solid rgba(124, 92, 255, 0.42);
  padding-left: 10px;
}
.capy-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.capy-badge {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  border: 1px solid rgba(124, 92, 255, 0.18);
  border-radius: 999px;
  background: rgba(124, 92, 255, 0.1);
  color: var(--capy-accent);
  padding: 3px 8px;
  font-weight: 760;
}
.capy-note,
.capy-callout { border-color: rgba(124, 92, 255, 0.2); }
.capy-risk { border-color: rgba(189, 59, 46, 0.22); }
.is-good { color: var(--capy-green) !important; }
.is-warn { color: var(--capy-amber) !important; }
.is-bad { color: var(--capy-red) !important; }
.is-muted { color: var(--capy-muted) !important; }
code,
pre {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
code {
  border-radius: 6px;
  background: rgba(68, 64, 60, 0.08);
  padding: 1px 5px;
}
pre {
  overflow: auto;
  border-radius: 12px;
  background: rgba(31, 29, 27, 0.9);
  color: #fffaf0;
  padding: 10px;
}
`;

export function isHtmlBlock(block) {
  return block.lang === "html" || looksLikeHtml(block.text);
}

export function codePre(block) {
  const pre = document.createElement("pre");
  const code = document.createElement("code");
  if (block.lang) code.dataset.lang = block.lang;
  code.textContent = block.text;
  pre.append(code);
  return pre;
}

export function htmlArtifact(source) {
  const wrap = document.createElement("section");
  wrap.className = "html-artifact";
  wrap.append(htmlPreview(source), htmlSource(source));
  return wrap;
}

export function safeHref(value) {
  const trimmed = String(value || "").trim();
  if (/^(https?:|mailto:)/i.test(trimmed)) return trimmed;
  if (/^[./#]/.test(trimmed)) return trimmed;
  return null;
}

function htmlPreview(source) {
  const wrap = document.createElement("div");
  wrap.className = "html-preview";
  const head = document.createElement("div");
  head.className = "html-preview-head";
  const label = document.createElement("span");
  label.className = "html-preview-label";
  label.textContent = "HTML";
  const note = document.createElement("span");
  note.className = "html-preview-note";
  note.textContent = "Capybara styled";
  head.append(label, note);
  const frame = document.createElement("iframe");
  frame.setAttribute("sandbox", "allow-same-origin");
  frame.setAttribute("referrerpolicy", "no-referrer");
  frame.addEventListener("load", () => fitHtmlPreviewFrame(frame), { once: true });
  frame.srcdoc = htmlPreviewDocument(source);
  wrap.append(head, frame);
  return wrap;
}

function fitHtmlPreviewFrame(frame) {
  try {
    const doc = frame.contentDocument;
    if (!doc) return;
    const height = Math.max(
      doc.documentElement?.scrollHeight || 0,
      doc.body?.scrollHeight || 0,
      doc.documentElement?.offsetHeight || 0,
      doc.body?.offsetHeight || 0,
    );
    if (height) frame.style.height = `${Math.min(420, Math.max(180, height))}px`;
  } catch {
    // Keep the CSS fallback if the sandbox blocks inspection.
  }
}

function htmlSource(source) {
  const details = document.createElement("details");
  details.className = "html-source";
  const summary = document.createElement("summary");
  summary.textContent = "查看 HTML DOM";
  details.append(summary, codePre({ lang: "html", text: source }));
  return details;
}

function htmlPreviewDocument(source) {
  return `<!doctype html><html><head><meta charset="utf-8"><style>${HTML_PREVIEW_CSS}</style></head><body><main class="capy-html-root">${sanitizeHtmlFragment(source)}</main></body></html>`;
}

function sanitizeHtmlFragment(source) {
  const raw = String(source || "");
  const parsed = new DOMParser().parseFromString(raw, "text/html");
  const sourceRoot = /<body[\s>]/i.test(raw) || /<html[\s>]/i.test(raw)
    ? parsed.body
    : new DOMParser().parseFromString(`<body>${raw}</body>`, "text/html").body;
  const clean = document.createElement("div");
  for (const child of [...sourceRoot.childNodes]) {
    const sanitized = sanitizePreviewNode(child);
    if (sanitized) clean.append(sanitized);
  }
  return clean.innerHTML.trim() || `<p>${escapeHtml(raw.trim())}</p>`;
}

function sanitizePreviewNode(node) {
  if (node.nodeType === Node.TEXT_NODE) return document.createTextNode(node.textContent || "");
  if (node.nodeType !== Node.ELEMENT_NODE) return null;

  const tag = node.localName.toLowerCase();
  if (HTML_PREVIEW_DROP_TAGS.has(tag)) return null;

  if (!HTML_PREVIEW_ALLOWED_TAGS.has(tag)) {
    const fragment = document.createDocumentFragment();
    for (const child of [...node.childNodes]) {
      const sanitized = sanitizePreviewNode(child);
      if (sanitized) fragment.append(sanitized);
    }
    return fragment;
  }

  const clean = document.createElement(tag);
  const className = sanitizeClassName(node.getAttribute("class"));
  if (className) clean.className = className;
  copyTableSpanAttribute(clean, node, "colspan");
  copyTableSpanAttribute(clean, node, "rowspan");
  for (const child of [...node.childNodes]) {
    const sanitized = sanitizePreviewNode(child);
    if (sanitized) clean.append(sanitized);
  }
  return clean;
}

function sanitizeClassName(value) {
  return String(value || "")
    .split(/\s+/)
    .filter((name) => HTML_PREVIEW_CLASSES.has(name))
    .join(" ");
}

function copyTableSpanAttribute(clean, source, name) {
  if (clean.localName !== "td" && clean.localName !== "th") return;
  const value = Number(source.getAttribute(name));
  if (Number.isInteger(value) && value > 1 && value <= 6) clean.setAttribute(name, String(value));
}

function escapeHtml(value) {
  return String(value || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function looksLikeHtml(text) {
  return /^\s*<(?:!doctype\s+html|html|body|main|section|article|div|style)[\s>]/i.test(text);
}
