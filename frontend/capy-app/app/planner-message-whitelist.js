const INTERNAL_FIELD_PATTERN = /(^|\n)\s*[-*]\s*(Provider|Artifact|Changed|Status|Run)\s*:/i;
const RAW_ARTIFACT_PATTERN = /\b(?:art|surf_art|proj)_[a-z0-9_]{16,}\b/i;
const RAW_RUN_PATTERN = /\b(?:gen|run)_[a-f0-9]{16,}\b/i;
const RUN_PATH_PATTERN = /(^|\s)\.capy\/runs\/[^\s)]+/i;
const RAW_ARTIFACT_REPLACE_PATTERN = /\b(?:art|surf_art|proj)_[a-z0-9_]{16,}\b/gi;
const RAW_RUN_REPLACE_PATTERN = /\b(?:gen|run)_[a-f0-9]{16,}\b/gi;
const RUN_PATH_REPLACE_PATTERN = /(^|\s)\.capy\/runs\/[^\s)]+/gi;
const PROVIDER_TOKEN_PATTERN = /\b(?:codex|claude)\b/gi;

export function projectGenerateMessageContent(result, artifact = {}) {
  const summary = sanitizeVisibleText(result?.run?.output?.summary_zh, "项目源文件已生成。");
  const artifactTitle = sanitizeVisibleText(artifact.title || artifact.kind, "选中的项目内容");
  const status = String(result?.run?.status || "completed").toLowerCase();
  return [
    `### ${summary}`,
    "",
    visibleStatusSentence(status),
    "",
    `- 对象：${artifactTitle}`
  ].join("\n");
}

export function sanitizePlannerMessageText(content) {
  const text = String(content || "");
  if (!looksLikeInternalPlannerStatus(text)) return text;

  const heading = extractHeading(text);
  const artifactTitle = sanitizeVisibleText(extractField(text, "Artifact"), "");
  const status = inferStatus(text);
  if (/^AI Diff\b/i.test(heading)) {
    return [
      `### AI Diff ${statusTitle(status)}`,
      "",
      visibleStatusSentence(status)
    ].join("\n");
  }

  const title = sanitizeVisibleText(heading, "项目修改已生成。");
  const lines = [`### ${title}`, "", visibleStatusSentence(status)];
  if (artifactTitle) lines.push("", `- 对象：${artifactTitle}`);
  return lines.join("\n");
}

export function hasPlannerInternalLeak(content) {
  const text = String(content || "");
  return INTERNAL_FIELD_PATTERN.test(text)
    || RAW_ARTIFACT_PATTERN.test(text)
    || RAW_RUN_PATTERN.test(text)
    || RUN_PATH_PATTERN.test(text);
}

function looksLikeInternalPlannerStatus(text) {
  const hasPlannerHeading = /^#{1,3}\s+/m.test(text) || /\bAI Diff\b/i.test(text);
  return hasPlannerHeading && (INTERNAL_FIELD_PATTERN.test(text) || hasPlannerInternalLeak(text));
}

function extractHeading(text) {
  const heading = String(text || "").match(/^#{1,3}\s+(.+)$/m);
  return heading?.[1]?.trim() || "";
}

function extractField(text, field) {
  const lines = String(text || "").replace(/\r\n/g, "\n").split("\n");
  const fieldPattern = new RegExp(`^\\s*[-*]\\s*${field}:\\s*(.*)$`, "i");
  for (let index = 0; index < lines.length; index += 1) {
    const match = lines[index].match(fieldPattern);
    if (!match) continue;
    const value = [match[1] || ""];
    for (let next = index + 1; next < lines.length; next += 1) {
      const line = lines[next];
      if (!/^\s+/.test(line) || /^\s*[-*]\s+\w+\s*:/.test(line)) break;
      value.push(line.trim());
    }
    return value.join(" ").trim();
  }
  return "";
}

function inferStatus(text) {
  const normalized = String(text || "").toLowerCase();
  if (/撤销|reverted/.test(normalized)) return "reverted";
  if (/拒绝|rejected/.test(normalized)) return "rejected";
  if (/proposed|planned|待审核|待确认|提出/.test(normalized)) return "proposed";
  if (/completed|applied|已应用|已写回/.test(normalized)) return "completed";
  return "completed";
}

function statusTitle(status) {
  return {
    reverted: "已撤销",
    rejected: "已拒绝",
    proposed: "待审核",
    planned: "待审核",
    completed: "已应用"
  }[status] || "已更新";
}

function visibleStatusSentence(status) {
  return {
    reverted: "这次 AI 变更已撤销。",
    rejected: "这次 AI 变更已拒绝，未应用到项目。",
    proposed: "AI 已提出一版修改，等待你审核。",
    planned: "AI 已提出一版修改，等待你审核。",
    completed: "修改已应用，预览已刷新。"
  }[status] || "修改状态已更新。";
}

function sanitizeVisibleText(value, fallback) {
  const text = String(value || "")
    .replace(RUN_PATH_REPLACE_PATTERN, " ")
    .replace(RAW_ARTIFACT_REPLACE_PATTERN, " ")
    .replace(RAW_RUN_REPLACE_PATTERN, " ")
    .replace(PROVIDER_TOKEN_PATTERN, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^[·:：,\-\s]+|[·:：,\-\s]+$/g, "");
  return text || fallback;
}
