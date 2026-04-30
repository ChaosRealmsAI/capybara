const HTML_FENCE_RE = /```([A-Za-z0-9_-]*)\s*\n([\s\S]*?)```/g;

export const DEFAULT_COLLAPSED = new Set([
  "thinking",
  "todo",
  "command",
  "file_change",
  "tool_call",
  "web_search",
  "progress",
  "usage",
]);

export function normalizeSegment(segment) {
  return {
    id: String(segment.id),
    provider: segment.provider,
    kind: segment.kind || "progress",
    source_type: segment.source_type || "unknown",
    title: segment.title || titleForKind(segment.kind),
    text: segment.text || "",
    summary: segment.summary || "",
    status: mapStatus(segment.status || "completed"),
    collapsed: Boolean(segment.collapsed),
    metadata: segment.metadata || {},
    raw: segment.raw ?? null,
  };
}

export function splitFrontendArtifacts(segment) {
  if (segment.kind !== "text" || !segment.text || !hasHtmlFence(segment.text)) return [segment];
  const result = [];
  let cursor = 0;
  let artifactIndex = 0;
  for (const match of segment.text.matchAll(HTML_FENCE_RE)) {
    const before = segment.text.slice(cursor, match.index).trim();
    if (before) result.push({ ...segment, id: `${segment.id}-text-${artifactIndex}`, text: before });
    const lang = (match[1] || "").toLowerCase();
    const source = match[2] || "";
    if (lang === "html" || looksLikeHtml(source)) {
      result.push({
        ...segment,
        id: `${segment.id}-artifact-${artifactIndex}`,
        kind: "frontend_artifact",
        source_type: "html_artifact",
        title: inferHtmlTitle(source),
        text: source.trim(),
        summary: "前端预览",
        collapsed: false,
        metadata: { ...(segment.metadata || {}), language: "html" },
      });
    } else {
      result.push({ ...segment, id: `${segment.id}-code-${artifactIndex}`, text: match[0] });
    }
    cursor = match.index + match[0].length;
    artifactIndex += 1;
  }
  const after = segment.text.slice(cursor).trim();
  if (after) result.push({ ...segment, id: `${segment.id}-text-${artifactIndex}`, text: after });
  return result.length ? result : [segment];
}

export function claudeKindForBlock(block) {
  if (block.type === "text") return "text";
  if (block.type === "thinking" || block.type === "redacted_thinking") return "thinking";
  if (block.type === "tool_use") return claudeKindForTool(block.name);
  return "tool_call";
}

export function claudeKindForTool(name = "") {
  const normalized = String(name).toLowerCase();
  if (normalized === "bash") return "command";
  if (["write", "edit", "multiedit", "notebookedit"].includes(normalized)) return "file_change";
  if (normalized === "todowrite") return "todo";
  if (normalized === "websearch" || normalized === "webfetch") return "web_search";
  return "tool_call";
}

export function claudeToolTitle(block) {
  if (block.name === "Bash") return block.input?.command || "Bash";
  if (["Write", "Edit", "MultiEdit", "NotebookEdit"].includes(block.name)) return block.input?.file_path || block.name;
  return block.name || "Tool";
}

export function claudeToolText(block) {
  if (block.name === "Bash") return block.input?.command || "";
  if (["Write", "Edit", "MultiEdit", "NotebookEdit"].includes(block.name)) {
    return [block.input?.file_path, block.input?.content].filter(Boolean).join("\n\n");
  }
  return summarizeJson(block.input);
}

export function claudeSystemTitle(message) {
  if (message.subtype === "init") return "Claude session";
  if (message.subtype === "status") return `Claude ${message.status || "status"}`;
  if (message.hook_name) return message.hook_name;
  return message.subtype ? `Claude ${message.subtype}` : "Claude system";
}

export function fileChangeTitle(changes) {
  if (!changes.length) return "File changes";
  if (changes.length === 1) return changes[0].path || "File change";
  return `${changes.length} file changes`;
}

export function commandSummary(command, exitCode, status) {
  const code = exitCode === undefined || exitCode === null ? "" : `exit ${exitCode}`;
  return [status, code, command].filter(Boolean).join(" · ");
}

export function usageSummary(usage, cost = null) {
  if (!usage && cost === null) return "";
  const parts = [];
  if (usage?.input_tokens !== undefined) parts.push(`input ${usage.input_tokens}`);
  if (usage?.output_tokens !== undefined) parts.push(`output ${usage.output_tokens}`);
  if (usage?.total_tokens !== undefined) parts.push(`total ${usage.total_tokens}`);
  if (cost !== null && cost !== undefined) parts.push(`$${Number(cost).toFixed(4)}`);
  return parts.join(" · ");
}

export function resultText(result) {
  if (!result) return "";
  if (typeof result === "string") return result;
  if (typeof result.stdout === "string" || typeof result.stderr === "string") {
    return [result.stdout, result.stderr].filter(Boolean).join("\n");
  }
  if (typeof result.content === "string") return result.content;
  return summarizeJson(result);
}

export function summarizeJson(value) {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  return compact(JSON.stringify(value, null, 2));
}

export function compact(value) {
  const text = String(value || "").trim();
  return text.length > 1200 ? `${text.slice(0, 1200)}...` : text;
}

export function mapStatus(status) {
  const value = String(status || "").toLowerCase();
  if (["failed", "error"].includes(value)) return "failed";
  if (["running", "started", "pending"].includes(value)) return "running";
  if (["stopped", "cancelled", "canceled"].includes(value)) return "stopped";
  return "completed";
}

function hasHtmlFence(text) {
  HTML_FENCE_RE.lastIndex = 0;
  for (const match of text.matchAll(HTML_FENCE_RE)) {
    const lang = (match[1] || "").toLowerCase();
    if (lang === "html" || looksLikeHtml(match[2] || "")) return true;
  }
  return false;
}

function looksLikeHtml(text) {
  return /^\s*<(?:!doctype\s+html|html|body|main|section|article|div|style)[\s>]/i.test(text || "");
}

function inferHtmlTitle(source) {
  const title = String(source || "").match(/<h[1-3][^>]*>(.*?)<\/h[1-3]>/is)?.[1]
    || String(source || "").match(/<title[^>]*>(.*?)<\/title>/is)?.[1];
  return stripTags(title || "前端预览");
}

function stripTags(value) {
  return String(value || "").replace(/<[^>]*>/g, "").trim();
}

export function titleForKind(kind) {
  return {
    text: "Assistant",
    frontend_artifact: "前端预览",
    thinking: "Thinking",
    todo: "Todo",
    command: "Command",
    file_change: "File changes",
    tool_call: "Tool",
    web_search: "Web search",
    progress: "Progress",
    usage: "Usage",
    error: "Error",
  }[kind] || "Event";
}
