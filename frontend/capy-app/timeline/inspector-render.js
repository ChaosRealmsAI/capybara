export function stageCard(title, status, rows) {
  const card = document.createElement("section");
  card.className = "inspector-stage";
  card.dataset.status = String(status || "idle");
  const head = document.createElement("header");
  head.innerHTML = `<span class="stage-icon"></span><h3></h3>`;
  head.querySelector("h3").textContent = title;
  const body = document.createElement("div");
  body.className = "stage-body";
  for (const row of rows) body.append(row);
  card.append(head, body);
  return card;
}

export function sourceRows(source) {
  const posterRefs = Array.isArray(source?.poster_refs) ? source.poster_refs : [];
  const scrollRefs = Array.isArray(source?.scroll_media_refs) ? source.scroll_media_refs : [];
  return [
    kvRow("poster", refsText(posterRefs)),
    kvRow("scroll-media", refsText(scrollRefs)),
    kvRow("brand_tokens", source?.brand_tokens?.source_path || source?.brand_tokens?.tokens_ref || "none")
  ];
}

export function compositionRows(composition) {
  return [
    linkRow("composition.json", composition?.path),
    codeRow(Array.isArray(composition?.preview_lines) ? composition.preview_lines.join("\n") : "")
  ];
}

export function compileRows(compile) {
  return [
    kvRow("render_source.json", compile?.status || "missing"),
    linkRow("path", compile?.render_source_path),
    kvRow("compile_mode", compile?.compile_mode || "unknown"),
    kvRow("timestamp", compile?.timestamp || "not recorded")
  ];
}

export function exportRows(jobs) {
  if (!Array.isArray(jobs) || jobs.length === 0) return [kvRow("jobs", "none")];
  return jobs.map((job) => {
    const row = document.createElement("div");
    row.className = "export-job-row";
    row.append(statusBadge(job.status), textSpan(job.output_path || job.job_id), textSpan(formatBytes(job.byte_size)));
    return row;
  });
}

export function evidenceRows(evidence) {
  if (!evidence?.exists) return [kvRow("evidence/index.html", "not found")];
  return [linkRow("evidence/index.html", evidence.index_html)];
}

export function kvRow(label, value) {
  const row = document.createElement("div");
  row.className = "stage-row";
  row.append(textSpan(label, "stage-key"), textSpan(value || "none", "stage-value"));
  return row;
}

export function linkRow(label, path) {
  const row = kvRow(label, path || "missing");
  if (path) {
    const value = row.querySelector(".stage-value");
    const link = document.createElement("a");
    link.href = path;
    link.textContent = path;
    value.replaceChildren(link);
  }
  return row;
}

export function codeRow(text) {
  const pre = document.createElement("pre");
  pre.className = "composition-preview";
  pre.textContent = text || "preview unavailable";
  return pre;
}

export function statusBadge(status) {
  const badge = textSpan(stageLabel(status), "status-badge");
  badge.dataset.status = stageLabel(status);
  return badge;
}

export function inspectorMessage(title, message) {
  const box = document.createElement("div");
  box.className = "inspector-message";
  box.append(textSpan(title, "message-title"), textSpan(message, "message-copy"));
  return box;
}

export function textSpan(value, className = "") {
  const span = document.createElement("span");
  if (className) span.className = className;
  span.textContent = String(value || "");
  return span;
}

export function refsText(refs) {
  if (!refs.length) return "none";
  return refs.map((item) => item.source_path || item.original_path || item.src || item.id).filter(Boolean).join(", ");
}

export function exportStatus(jobs) {
  if (!Array.isArray(jobs) || jobs.length === 0) return "idle";
  if (jobs.some((job) => stageLabel(job.status) === "failed")) return "failed";
  if (jobs.some((job) => stageLabel(job.status) === "running")) return "running";
  if (jobs.some((job) => stageLabel(job.status) === "done")) return "done";
  return stageLabel(jobs[0].status);
}

export function stageLabel(value) {
  if (!value) return "idle";
  if (typeof value === "string") return value;
  if (value.error) return "error";
  return String(value);
}

export function formatBytes(value) {
  const bytes = Number(value);
  if (!Number.isFinite(bytes) || bytes <= 0) return "bytes unknown";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

