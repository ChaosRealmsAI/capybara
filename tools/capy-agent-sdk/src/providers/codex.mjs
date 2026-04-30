import { Codex } from "@openai/codex-sdk";
import { SegmentNormalizer, primaryContentFromSegments, segmentsFromCodexItems } from "../segments.mjs";

export async function runCodex(normalized) {
  const client = new Codex(normalized.codex.options);
  const thread = normalized.threadId
    ? client.resumeThread(normalized.threadId, normalized.codex.threadOptions)
    : client.startThread(normalized.codex.threadOptions);
  const turn = await thread.run(normalized.prompt, normalized.codex.turnOptions);
  const segments = segmentsFromCodexItems(turn.items, turn.usage);
  const agentMessages = turn.items
    .filter((item) => item?.type === "agent_message")
    .map((item) => item.text ?? "")
    .filter(Boolean);
  return {
    ok: true,
    provider: "codex",
    thread_id: thread.id,
    primary_content: primaryContentFromSegments(segments, turn.finalResponse || agentMessages.at(-1) || agentMessages[0]),
    content: turn.finalResponse,
    segments,
    agent_messages: agentMessages,
    usage: turn.usage,
    items: turn.items,
    normalized,
  };
}

export async function runCodexStream(normalized, emit) {
  const client = new Codex(normalized.codex.options);
  const thread = normalized.threadId
    ? client.resumeThread(normalized.threadId, normalized.codex.threadOptions)
    : client.startThread(normalized.codex.threadOptions);
  const normalizer = new SegmentNormalizer("codex", emit);
  const streamed = await thread.runStreamed(normalized.prompt, normalized.codex.turnOptions);
  let usage = null;
  let threadId = thread.id ?? normalized.threadId ?? null;
  for await (const event of streamed.events) {
    if (event.thread_id) threadId = event.thread_id;
    if (event.usage) usage = event.usage;
    normalizer.acceptCodexEvent(event);
  }
  const segments = normalizer.finalSegments();
  const content = primaryContentFromSegments(segments);
  return {
    ok: true,
    provider: "codex",
    thread_id: thread.id ?? threadId,
    primary_content: content,
    content,
    segments,
    usage,
    normalized,
  };
}
