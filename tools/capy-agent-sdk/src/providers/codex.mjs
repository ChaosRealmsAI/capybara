import { Codex } from "@openai/codex-sdk";

export async function runCodex(normalized) {
  const client = new Codex(normalized.codex.options);
  const thread = normalized.threadId
    ? client.resumeThread(normalized.threadId, normalized.codex.threadOptions)
    : client.startThread(normalized.codex.threadOptions);
  const turn = await thread.run(normalized.prompt, normalized.codex.turnOptions);
  const agentMessages = turn.items
    .filter((item) => item?.type === "agent_message")
    .map((item) => item.text ?? "")
    .filter(Boolean);
  return {
    ok: true,
    provider: "codex",
    thread_id: thread.id,
    primary_content: agentMessages[0] ?? turn.finalResponse,
    content: turn.finalResponse,
    agent_messages: agentMessages,
    usage: turn.usage,
    items: turn.items,
    normalized,
  };
}
