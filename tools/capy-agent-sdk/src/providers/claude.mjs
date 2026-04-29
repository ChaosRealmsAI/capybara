import { query } from "@anthropic-ai/claude-agent-sdk";

export async function runClaude(normalized) {
  const messages = [];
  let result = null;
  let assistantText = "";
  for await (const message of query({ prompt: normalized.prompt, options: normalized.claude.options })) {
    messages.push(message);
    if (message.type === "assistant") assistantText += textFromAssistant(message);
    if (message.type === "result") result = message;
  }
  if (!result) throw new Error("Claude SDK ended without a result message");
  if (result.subtype !== "success") {
    throw new Error((result.errors ?? []).join("\n") || `Claude SDK failed: ${result.subtype}`);
  }
  return {
    ok: true,
    provider: "claude",
    session_id: result.session_id,
    primary_content: result.result || assistantText,
    content: result.result || assistantText,
    agent_messages: assistantText ? [assistantText] : [],
    usage: result.usage,
    total_cost_usd: result.total_cost_usd,
    num_turns: result.num_turns,
    messages,
    normalized,
  };
}

function textFromAssistant(message) {
  return (message.message?.content ?? [])
    .filter((block) => block?.type === "text")
    .map((block) => block.text ?? "")
    .join("");
}
