import {
  commandSummary,
  fileChangeTitle,
  mapStatus,
  summarizeJson,
  usageSummary,
} from "./segment-utils.mjs";

export function acceptCodexEvent(normalizer, event) {
  if (!event || typeof event !== "object") return;
  if (event.type === "thread.started") {
    normalizer.upsert("codex-thread", {
      kind: "progress",
      source_type: event.type,
      title: "Codex thread",
      summary: event.thread?.id || "thread started",
      status: "running",
      raw: event,
    });
    return;
  }
  if (event.type === "turn.started") {
    normalizer.upsert("codex-turn", {
      kind: "progress",
      source_type: event.type,
      title: "Codex turn",
      summary: "model is working",
      status: "running",
      raw: event,
    });
    return;
  }
  if (event.type === "item.started" || event.type === "item.updated" || event.type === "item.completed") {
    acceptCodexItem(normalizer, event.item, event.type === "item.completed" ? "completed" : "running", event);
    return;
  }
  if (event.type === "turn.completed") {
    normalizer.upsert("codex-usage", {
      kind: "usage",
      source_type: event.type,
      title: "Usage",
      summary: usageSummary(event.usage),
      status: "completed",
      metadata: { usage: event.usage ?? null },
      raw: event,
    });
    return;
  }
  if (event.type === "turn.failed" || event.type === "error") {
    normalizer.upsert(`codex-error-${normalizer.next()}`, {
      kind: "error",
      source_type: event.type,
      title: "Codex error",
      text: event.message || event.error?.message || JSON.stringify(event),
      status: "failed",
      raw: event,
    });
  }
}

export function acceptCodexItem(normalizer, item, fallbackStatus = "completed", raw = item) {
  if (!item || typeof item !== "object") return;
  const id = `codex-${item.id || normalizer.next()}`;
  const status = mapStatus(item.status || fallbackStatus);
  if (item.type === "agent_message") {
    normalizer.upsert(id, {
      kind: "text",
      source_type: item.type,
      title: "Codex",
      text: item.text || "",
      status,
      collapsed: false,
      raw,
    });
    return;
  }
  if (item.type === "reasoning") {
    normalizer.upsert(id, {
      kind: "thinking",
      source_type: item.type,
      title: "Thinking",
      text: item.text || item.summary || "",
      status,
      raw,
    });
    return;
  }
  if (item.type === "command_execution") {
    normalizer.upsert(id, {
      kind: "command",
      source_type: item.type,
      title: item.command || "Command",
      text: item.aggregated_output || "",
      summary: commandSummary(item.command, item.exit_code, item.status),
      status: mapStatus(item.status || (Number(item.exit_code) === 0 ? "completed" : "failed")),
      metadata: {
        command: item.command || "",
        exit_code: item.exit_code ?? null,
        output: item.aggregated_output || "",
      },
      raw,
    });
    return;
  }
  if (item.type === "file_change") {
    const changes = Array.isArray(item.changes) ? item.changes : [];
    normalizer.upsert(id, {
      kind: "file_change",
      source_type: item.type,
      title: fileChangeTitle(changes),
      text: changes.map((change) => `${change.kind || "change"} ${change.path || ""}`.trim()).join("\n"),
      summary: `${changes.length} file change${changes.length === 1 ? "" : "s"}`,
      status,
      metadata: { changes },
      raw,
    });
    return;
  }
  acceptCodexAuxiliaryItem(normalizer, item, id, status, raw);
}

function acceptCodexAuxiliaryItem(normalizer, item, id, status, raw) {
  if (item.type === "mcp_tool_call") {
    normalizer.upsert(id, {
      kind: "tool_call",
      source_type: item.type,
      title: [item.server, item.tool].filter(Boolean).join(" / ") || "MCP tool",
      text: item.error?.message || summarizeJson(item.result) || summarizeJson(item.arguments),
      summary: item.error?.message || item.status || "",
      status: item.error || item.status === "failed" ? "failed" : status,
      metadata: {
        server: item.server,
        tool: item.tool,
        arguments: item.arguments,
        result: item.result,
      },
      raw,
    });
    return;
  }
  if (item.type === "web_search") {
    normalizer.upsert(id, {
      kind: "web_search",
      source_type: item.type,
      title: item.query || "Web search",
      text: summarizeJson(item.action),
      status,
      metadata: { query: item.query, action: item.action },
      raw,
    });
    return;
  }
  if (item.type === "todo_list") {
    const todos = Array.isArray(item.items) ? item.items : [];
    normalizer.upsert(id, {
      kind: "todo",
      source_type: item.type,
      title: "Todo",
      text: todos.map((todo) => `${todo.completed ? "[x]" : "[ ]"} ${todo.text || ""}`).join("\n"),
      summary: `${todos.filter((todo) => todo.completed).length}/${todos.length} completed`,
      status,
      metadata: { items: todos },
      raw,
    });
    return;
  }
  normalizer.upsert(id, {
    kind: item.type === "error" ? "error" : "progress",
    source_type: item.type || "unknown",
    title: item.type === "error" ? "Codex error" : item.type || "Codex item",
    text: item.message || summarizeJson(item),
    status: item.type === "error" ? "failed" : status,
    raw,
  });
}
