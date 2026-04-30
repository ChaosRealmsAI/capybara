import {
  DEFAULT_COLLAPSED,
  claudeKindForBlock,
  claudeKindForTool,
  claudeSystemTitle,
  claudeToolText,
  claudeToolTitle,
  compact,
  normalizeSegment,
  resultText,
  splitFrontendArtifacts,
  summarizeJson,
  titleForKind,
  usageSummary,
} from "./segment-utils.mjs";
import { acceptCodexEvent, acceptCodexItem } from "./codex-segments.mjs";

export class SegmentNormalizer {
  constructor(provider, emit = null) {
    this.provider = provider;
    this.emit = emit;
    this.segments = [];
    this.byId = new Map();
    this.toolSegments = new Map();
    this.activeBlocks = new Map();
    this.currentClaudeMessageId = null;
    this.counter = 0;
  }

  acceptCodexEvent(event) {
    acceptCodexEvent(this, event);
  }

  acceptCodexItem(item, fallbackStatus = "completed", raw = item) {
    acceptCodexItem(this, item, fallbackStatus, raw);
  }

  acceptClaudeMessage(message) {
    if (!message || typeof message !== "object") return;
    if (message.type === "stream_event") {
      this.acceptClaudeStreamEvent(message);
      return;
    }
    if (message.type === "assistant") {
      const content = Array.isArray(message.message?.content) ? message.message.content : [];
      content.forEach((block, index) => this.acceptClaudeContentBlock(block, message, index));
      return;
    }
    if (message.type === "user") {
      const content = Array.isArray(message.message?.content) ? message.message.content : [];
      content.forEach((block) => this.acceptClaudeToolResult(block, message));
      return;
    }
    if (message.type === "result") {
      if (message.subtype === "success" && !message.is_error) {
        this.upsert("claude-usage", {
          kind: "usage",
          source_type: "result",
          title: "Usage",
          summary: usageSummary(message.usage, message.total_cost_usd),
          status: "completed",
          metadata: {
            usage: message.usage ?? null,
            total_cost_usd: message.total_cost_usd ?? null,
            num_turns: message.num_turns ?? null,
          },
          raw: message,
        });
      } else {
        this.upsert(`claude-error-${this.next()}`, {
          kind: "error",
          source_type: "result",
          title: "Claude error",
          text: (message.errors || []).join("\n") || message.result || message.subtype || "Claude SDK failed",
          status: "failed",
          raw: message,
        });
      }
      return;
    }
    if (message.type === "system") {
      this.upsert(`claude-system-${message.uuid || this.next()}`, {
        kind: "progress",
        source_type: `system/${message.subtype || "event"}`,
        title: claudeSystemTitle(message),
        text: message.output || message.text || "",
        summary: message.status || message.outcome || message.hook_event || message.subtype || "",
        status: message.outcome === "failure" ? "failed" : "completed",
        raw: message,
      });
      return;
    }
    if (message.type === "rate_limit_event") {
      this.upsert(`claude-rate-${message.uuid || this.next()}`, {
        kind: "progress",
        source_type: message.type,
        title: "Rate limit",
        summary: message.rate_limit_info?.status || "",
        status: "completed",
        metadata: { rate_limit_info: message.rate_limit_info ?? null },
        raw: message,
      });
      return;
    }
    this.upsert(`claude-progress-${message.uuid || this.next()}`, {
      kind: "progress",
      source_type: message.type || "unknown",
      title: message.type || "Claude event",
      text: summarizeJson(message),
      status: "completed",
      raw: message,
    });
  }

  acceptClaudeStreamEvent(message) {
    const event = message.event || {};
    if (event.type === "message_start") {
      this.currentClaudeMessageId = event.message?.id || message.uuid || `stream-${this.next()}`;
      return;
    }
    if (event.type === "content_block_start") {
      const block = event.content_block || {};
      const id = block.id ? `claude-tool-${block.id}` : `claude-${this.currentClaudeMessageId || message.uuid}-block-${event.index}`;
      const kind = claudeKindForBlock(block);
      this.activeBlocks.set(event.index, { id, kind, text: "", inputJson: "" });
      this.upsert(id, {
        kind,
        source_type: `stream/${block.type || "content"}`,
        title: block.name || titleForKind(kind),
        text: block.text || "",
        summary: block.name || "",
        status: "running",
        metadata: block.input ? { input: block.input } : {},
        raw: message,
      });
      if (block.type === "tool_use" && block.id) this.toolSegments.set(block.id, id);
      return;
    }
    if (event.type === "content_block_delta") {
      const active = this.activeBlocks.get(event.index);
      if (!active) return;
      const delta = event.delta || {};
      const current = this.byId.get(active.id) || {};
      if (delta.type === "text_delta") {
        active.text += delta.text || "";
        this.upsert(active.id, {
          ...current,
          text: active.text,
          status: "running",
          raw: message,
        });
      } else if (delta.type === "thinking_delta") {
        active.text += delta.thinking || "";
        this.upsert(active.id, {
          ...current,
          kind: "thinking",
          text: active.text,
          status: "running",
          raw: message,
        });
      } else if (delta.type === "input_json_delta") {
        active.inputJson += delta.partial_json || "";
        this.upsert(active.id, {
          ...current,
          summary: compact(active.inputJson),
          status: "running",
          metadata: {
            ...(current.metadata || {}),
            partial_json: active.inputJson,
          },
          raw: message,
        });
      }
      return;
    }
    if (event.type === "content_block_stop") {
      const active = this.activeBlocks.get(event.index);
      if (!active) return;
      const current = this.byId.get(active.id) || {};
      this.upsert(active.id, { ...current, status: "completed", raw: message });
      return;
    }
    if (event.type === "message_delta") {
      this.upsert(`claude-message-delta-${this.currentClaudeMessageId || this.next()}`, {
        kind: "usage",
        source_type: event.type,
        title: "Usage",
        summary: usageSummary(event.usage),
        status: "completed",
        metadata: { usage: event.usage ?? null, stop_reason: event.delta?.stop_reason ?? null },
        raw: message,
      });
    }
  }

  acceptClaudeContentBlock(block, message, index) {
    if (!block || typeof block !== "object") return;
    if (block.type === "text") {
      this.upsert(`claude-${message.message?.id || message.uuid || this.next()}-block-${index}`, {
        kind: "text",
        source_type: block.type,
        title: "Claude",
        text: block.text || "",
        status: "completed",
        collapsed: false,
        raw: message,
      });
      return;
    }
    if (block.type === "thinking" || block.type === "redacted_thinking") {
      this.upsert(`claude-${message.message?.id || message.uuid || this.next()}-block-${index}`, {
        kind: "thinking",
        source_type: block.type,
        title: block.type === "redacted_thinking" ? "Redacted thinking" : "Thinking",
        text: block.thinking || block.text || "",
        status: "completed",
        raw: message,
      });
      return;
    }
    if (block.type === "tool_use") {
      const segmentId = `claude-tool-${block.id || this.next()}`;
      this.toolSegments.set(block.id, segmentId);
      const kind = claudeKindForTool(block.name);
      this.upsert(segmentId, {
        kind,
        source_type: block.type,
        title: claudeToolTitle(block),
        text: claudeToolText(block),
        summary: block.name || "",
        status: "running",
        metadata: {
          tool_id: block.id,
          tool_name: block.name,
          input: block.input ?? null,
        },
        raw: message,
      });
      return;
    }
    this.upsert(`claude-${message.message?.id || message.uuid || this.next()}-block-${index}`, {
      kind: "tool_call",
      source_type: block.type || "content",
      title: block.type || "Claude content",
      text: summarizeJson(block),
      status: "completed",
      raw: message,
    });
  }

  acceptClaudeToolResult(block, message) {
    if (!block || block.type !== "tool_result") return;
    const segmentId = this.toolSegments.get(block.tool_use_id) || `claude-tool-${block.tool_use_id || this.next()}`;
    const current = this.byId.get(segmentId) || {
      id: segmentId,
      provider: this.provider,
      kind: "tool_call",
      source_type: "tool_result",
      title: "Tool result",
      collapsed: true,
    };
    const result = message.tool_use_result ?? null;
    const output = block.content || resultText(result);
    this.upsert(segmentId, {
      ...current,
      text: output || current.text || "",
      summary: block.is_error ? "failed" : "completed",
      status: block.is_error ? "failed" : "completed",
      metadata: {
        ...(current.metadata || {}),
        result,
        is_error: Boolean(block.is_error),
      },
      raw: message,
    });
  }

  upsert(id, value) {
    const next = normalizeSegment({
      id,
      provider: this.provider,
      collapsed: DEFAULT_COLLAPSED.has(value.kind),
      ...value,
    });
    const previous = this.byId.get(id);
    const merged = previous ? normalizeSegment({ ...previous, ...next }) : next;
    this.byId.set(id, merged);
    if (!previous) this.segments.push(merged);
    else this.segments = this.segments.map((segment) => (segment.id === id ? merged : segment));
    this.emit?.({ ok: true, type: "segment", op: "upsert", provider: this.provider, segment: merged });
    return merged;
  }

  next() {
    this.counter += 1;
    return this.counter;
  }

  finalSegments() {
    return this.segments.flatMap(splitFrontendArtifacts).filter((segment) => {
      return segment.kind !== "text" || Boolean(String(segment.text || "").trim());
    });
  }
}

export function segmentsFromCodexItems(items = [], usage = null) {
  const normalizer = new SegmentNormalizer("codex");
  for (const item of items || []) normalizer.acceptCodexItem(item, "completed", item);
  if (usage) {
    normalizer.upsert("codex-usage", {
      kind: "usage",
      source_type: "usage",
      title: "Usage",
      summary: usageSummary(usage),
      status: "completed",
      metadata: { usage },
    });
  }
  return normalizer.finalSegments();
}

export function segmentsFromClaudeMessages(messages = [], result = null) {
  const normalizer = new SegmentNormalizer("claude");
  for (const message of messages || []) normalizer.acceptClaudeMessage(message);
  if (result) normalizer.acceptClaudeMessage(result);
  return normalizer.finalSegments();
}

export function primaryContentFromSegments(segments = [], fallback = "") {
  const visible = segments
    .filter((segment) => ["text", "frontend_artifact", "error"].includes(segment.kind))
    .map((segment) => {
      if (segment.kind === "frontend_artifact") return `\`\`\`html\n${segment.text || ""}\n\`\`\``;
      return segment.text || segment.summary || "";
    })
    .filter((text) => text.trim());
  return visible.join("\n\n").trim() || fallback || "";
}
