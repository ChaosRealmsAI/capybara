import { codePre, htmlArtifact, isHtmlBlock, safeHref } from "./html-preview-renderer.js";

const BLOCK_TAGS = new Set(["p", "ul", "ol", "pre", "h1", "h2", "h3", "blockquote"]);

export function renderMessageContent(content, options = {}) {
  const root = document.createElement("div");
  root.className = "markdown-body";
  if (options.loading) {
    root.append(typingBubble());
    return root;
  }
  const text = String(content || "");
  const blocks = parseBlocks(text);
  if (blocks.some((block) => block.type === "code" && isHtmlBlock(block))) {
    root.classList.add("has-html-artifact");
  }
  for (const block of blocks) root.append(renderBlock(block));
  if (!root.childNodes.length) root.append(document.createTextNode(text));
  return root;
}

export function renderMessageSegments(segments = [], options = {}) {
  const root = document.createElement("div");
  root.className = "segment-stack";
  const normalized = Array.isArray(segments) ? segments.filter(Boolean) : [];
  const primary = normalized.filter(isPrimarySegment);
  const process = normalized.filter((segment) => !isPrimarySegment(segment));

  for (const segment of primary) root.append(primarySegment(segment));
  if (!primary.length && options.loading) root.append(typingBubble());
  if (process.length) root.append(processSegments(process));
  if (!root.childNodes.length) root.append(renderMessageContent(options.fallback || ""));
  return root;
}

function parseBlocks(text) {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  const blocks = [];
  let paragraph = [];
  let list = null;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const fence = line.match(/^```(\w+)?\s*$/);
    if (fence) {
      flushParagraph(blocks, paragraph);
      paragraph = [];
      if (list) {
        blocks.push(list);
        list = null;
      }
      const code = [];
      index += 1;
      while (index < lines.length && !/^```\s*$/.test(lines[index])) {
        code.push(lines[index]);
        index += 1;
      }
      blocks.push({ type: "code", lang: (fence[1] || "").toLowerCase(), text: code.join("\n") });
      continue;
    }
    if (!line.trim()) {
      flushParagraph(blocks, paragraph);
      paragraph = [];
      if (list) {
        blocks.push(list);
        list = null;
      }
      continue;
    }
    const heading = line.match(/^(#{1,3})\s+(.+)$/);
    if (heading) {
      flushParagraph(blocks, paragraph);
      paragraph = [];
      if (list) {
        blocks.push(list);
        list = null;
      }
      blocks.push({ type: "heading", level: heading[1].length, text: heading[2] });
      continue;
    }
    const bullet = line.match(/^[-*]\s+(.+)$/);
    const numbered = line.match(/^\d+\.\s+(.+)$/);
    if (bullet || numbered) {
      flushParagraph(blocks, paragraph);
      paragraph = [];
      const ordered = Boolean(numbered);
      if (!list || list.ordered !== ordered) {
        if (list) blocks.push(list);
        list = { type: "list", ordered, items: [] };
      }
      list.items.push((bullet || numbered)[1]);
      continue;
    }
    const quote = line.match(/^>\s?(.+)$/);
    if (quote) {
      flushParagraph(blocks, paragraph);
      paragraph = [];
      if (list) {
        blocks.push(list);
        list = null;
      }
      blocks.push({ type: "quote", text: quote[1] });
      continue;
    }
    if (list) {
      blocks.push(list);
      list = null;
    }
    paragraph.push(line.trim());
  }
  flushParagraph(blocks, paragraph);
  if (list) blocks.push(list);
  return blocks;
}

function flushParagraph(blocks, paragraph) {
  if (paragraph.length) blocks.push({ type: "paragraph", text: paragraph.join(" ") });
}

function renderBlock(block) {
  if (block.type === "heading") {
    const node = document.createElement(`h${block.level}`);
    renderInline(node, block.text);
    return node;
  }
  if (block.type === "list") {
    const node = document.createElement(block.ordered ? "ol" : "ul");
    for (const item of block.items) {
      const li = document.createElement("li");
      renderInline(li, item);
      node.append(li);
    }
    return node;
  }
  if (block.type === "quote") {
    const node = document.createElement("blockquote");
    renderInline(node, block.text);
    return node;
  }
  if (block.type === "code") return isHtmlBlock(block) ? htmlArtifact(block.text) : codeBlock(block);
  const node = document.createElement("p");
  renderInline(node, block.text);
  return node;
}

function renderInline(parent, text) {
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g;
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    if (match.index > last) parent.append(document.createTextNode(text.slice(last, match.index)));
    parent.append(inlineNode(match[0]));
    last = match.index + match[0].length;
  }
  if (last < text.length) parent.append(document.createTextNode(text.slice(last)));
}

function inlineNode(token) {
  if (token.startsWith("`")) {
    const node = document.createElement("code");
    node.textContent = token.slice(1, -1);
    return node;
  }
  if (token.startsWith("**")) {
    const node = document.createElement("strong");
    node.textContent = token.slice(2, -2);
    return node;
  }
  if (token.startsWith("*")) {
    const node = document.createElement("em");
    node.textContent = token.slice(1, -1);
    return node;
  }
  const link = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
  if (link) {
    const href = safeHref(link[2]);
    if (href) {
      const node = document.createElement("a");
      node.href = href;
      node.target = "_blank";
      node.rel = "noreferrer";
      node.textContent = link[1];
      return node;
    }
  }
  return document.createTextNode(token);
}

function codeBlock(block) {
  const wrap = document.createElement("div");
  wrap.className = "code-block-wrap";
  wrap.append(codePre(block));
  return wrap;
}

function isPrimarySegment(segment) {
  return ["text", "frontend_artifact", "error"].includes(segment?.kind);
}

function primarySegment(segment) {
  if (segment.kind === "frontend_artifact") {
    const wrap = document.createElement("section");
    wrap.className = "segment-primary segment-artifact";
    const title = document.createElement("div");
    title.className = "segment-title";
    title.textContent = segment.title || "前端预览";
    wrap.append(title, htmlArtifact(segment.text || ""));
    return wrap;
  }
  if (segment.kind === "error") return errorSegment(segment);
  const wrap = document.createElement("section");
  wrap.className = "segment-primary segment-text";
  wrap.append(renderMessageContent(segment.text || segment.summary || ""));
  return wrap;
}

function processSegments(segments) {
  const details = document.createElement("details");
  details.className = "segment-process";
  const summary = document.createElement("summary");
  const failed = segments.filter((segment) => segment.status === "failed").length;
  summary.textContent = failed ? `过程 · ${segments.length} 项 · ${failed} 项失败` : `过程 · ${segments.length} 项`;
  details.append(summary);
  const list = document.createElement("div");
  list.className = "segment-process-list";
  for (const segment of segments) list.append(processSegment(segment));
  details.append(list);
  return details;
}

function processSegment(segment) {
  const details = document.createElement("details");
  details.className = `segment-card segment-${segment.kind || "progress"} is-${segment.status || "completed"}`;
  if (segment.status === "failed") details.open = true;

  const summary = document.createElement("summary");
  const kind = document.createElement("span");
  kind.className = "segment-kind";
  kind.textContent = segmentKindLabel(segment.kind);
  const title = document.createElement("span");
  title.className = "segment-card-title";
  title.textContent = segment.title || segment.summary || segment.source_type || "Event";
  const status = document.createElement("span");
  status.className = "segment-status";
  status.textContent = segment.status || "completed";
  summary.append(kind, title, status);
  details.append(summary);

  const body = document.createElement("div");
  body.className = "segment-card-body";
  if (segment.kind === "command") {
    body.append(labeledCode("命令", segment.metadata?.command || segment.title || ""));
    if (segment.text) body.append(labeledCode("输出", segment.text));
  } else if (segment.kind === "file_change") {
    body.append(fileChangeList(segment.metadata?.changes || [], segment.text));
  } else if (segment.kind === "tool_call" || segment.kind === "web_search" || segment.kind === "todo") {
    body.append(renderMessageContent(segment.text || segment.summary || ""));
  } else if (segment.kind === "usage") {
    body.append(metadataTable(segment.metadata || {}, segment.summary));
  } else {
    body.append(renderMessageContent(segment.text || segment.summary || ""));
  }
  details.append(body);
  return details;
}

function errorSegment(segment) {
  const wrap = document.createElement("section");
  wrap.className = "segment-error-card";
  const title = document.createElement("strong");
  title.textContent = segment.title || "执行失败";
  const text = document.createElement("p");
  text.textContent = segment.text || segment.summary || "模型运行时返回了错误。";
  const next = document.createElement("p");
  next.className = "segment-next-step";
  next.textContent = "下一步：查看折叠的原始日志，按错误信息修复后重试。";
  wrap.append(title, text, next);
  return wrap;
}

function labeledCode(label, text) {
  const wrap = document.createElement("div");
  wrap.className = "segment-code";
  const head = document.createElement("span");
  head.textContent = label;
  wrap.append(head, codePre({ lang: "text", text: String(text || "") }));
  return wrap;
}

function fileChangeList(changes, fallback) {
  if (!changes.length) return renderMessageContent(fallback || "");
  const list = document.createElement("ul");
  list.className = "segment-file-list";
  for (const change of changes) {
    const item = document.createElement("li");
    item.textContent = `${change.kind || "change"} · ${change.path || ""}`;
    list.append(item);
  }
  return list;
}

function metadataTable(metadata, fallback) {
  const wrap = document.createElement("div");
  wrap.className = "segment-metadata";
  if (fallback) {
    const summary = document.createElement("p");
    summary.textContent = fallback;
    wrap.append(summary);
  }
  wrap.append(codePre({ lang: "json", text: JSON.stringify(metadata || {}, null, 2) }));
  return wrap;
}

function segmentKindLabel(kind) {
  return {
    thinking: "思考",
    todo: "Todo",
    command: "命令",
    file_change: "文件",
    tool_call: "工具",
    web_search: "搜索",
    progress: "状态",
    usage: "用量",
    error: "错误",
  }[kind] || "事件";
}

function typingBubble() {
  const node = document.createElement("span");
  node.className = "typing-dots";
  node.setAttribute("aria-label", "Codex 正在回复");
  node.append(dot(), dot(), dot());
  return node;
}

function dot() {
  const node = document.createElement("i");
  node.setAttribute("aria-hidden", "true");
  return node;
}

export function hasBlockChildren(node) {
  return [...node.childNodes].some((child) => child.nodeType === 1 && BLOCK_TAGS.has(child.localName));
}
