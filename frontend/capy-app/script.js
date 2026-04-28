import { STATE } from "./mock.js";

window.CAPYBARA_STATE = STATE;

const topbar = document.querySelector(".topbar");
const listEl = document.querySelector("#conversation-list");
const messagesEl = document.querySelector("#message-list");
const titleEl = document.querySelector("#chat-title");
const subtitleEl = document.querySelector("#chat-subtitle");
const newChatEl = document.querySelector("#new-chat");
const stopEl = document.querySelector("#stop-run");
const formEl = document.querySelector("#composer");
const promptEl = document.querySelector("#prompt");
const providerEl = document.querySelector("#provider");
const cwdEl = document.querySelector("#cwd");
const modelEl = document.querySelector("#model");
const effortEl = document.querySelector("#effort");
const policyEl = document.querySelector("#policy");

const pending = new Map();
const state = {
  conversations: [],
  activeId: null,
  messages: [],
  streaming: new Map()
};

topbar?.addEventListener("mousedown", (event) => {
  if (event.button !== 0) return;
  const target = event.target;
  if (target instanceof HTMLElement && target.closest("button, input, a, select, [role=button]")) {
    return;
  }
  if (!window.ipc) return;
  window.ipc.postMessage(event.detail === 2 ? "maximize_toggle" : "drag_window");
});

window.__capyReceive = (response) => {
  const entry = pending.get(response.req_id);
  if (!entry) return;
  pending.delete(response.req_id);
  if (response.ok) {
    entry.resolve(response.data);
  } else {
    entry.reject(response.error || { error: "request failed" });
  }
};

window.addEventListener("capy:agent-event", (event) => {
  const detail = event.detail;
  if (!detail || detail.conversation_id !== state.activeId) return;
  if (detail.kind === "assistant_delta") {
    const current = state.streaming.get(detail.run_id) || "";
    state.streaming.set(detail.run_id, current + (detail.delta || ""));
    renderMessages();
  } else if (detail.kind === "assistant_done" || detail.kind === "error") {
    state.streaming.delete(detail.run_id);
    openConversation(state.activeId);
  }
});

newChatEl?.addEventListener("click", async () => {
  await createConversation();
});

stopEl?.addEventListener("click", async () => {
  if (!state.activeId) return;
  await rpc("conversation-stop", { id: state.activeId });
});

formEl?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const prompt = promptEl.value.trim();
  if (!prompt) return;
  if (!state.activeId) {
    await createConversation();
  }
  if (!state.activeId) return;
  promptEl.value = "";
  state.messages.push({
    id: `local-${Date.now()}`,
    role: "user",
    content: prompt
  });
  renderMessages();
  await rpc("conversation-send", { id: state.activeId, prompt });
});

providerEl?.addEventListener("change", () => {
  syncPolicyOptions();
});

init();

async function init() {
  cwdEl.value = window.CAPYBARA_SESSION?.cwd || "/Users/Zhuanz/workspace/capybara";
  syncPolicyOptions();
  try {
    const data = await rpc("conversation-list", {});
    state.conversations = data.conversations || [];
    renderConversations();
    if (state.conversations[0]) {
      await openConversation(state.conversations[0].id);
    } else {
      renderMessages();
    }
  } catch (error) {
    renderError(error);
  }
}

async function createConversation() {
  const data = await rpc("conversation-create", {
    provider: providerEl.value,
    cwd: cwdEl.value.trim() || "/Users/Zhuanz/workspace/capybara",
    model: modelEl.value.trim() || null,
    config: currentConfig()
  });
  await refreshList();
  await openConversation(data.conversation.id);
}

async function refreshList() {
  const data = await rpc("conversation-list", {});
  state.conversations = data.conversations || [];
  renderConversations();
}

async function openConversation(id) {
  const detail = await rpc("conversation-open", { id });
  state.activeId = detail.conversation.id;
  state.messages = detail.messages || [];
  titleEl.textContent = detail.conversation.title;
  subtitleEl.textContent = `${detail.conversation.provider} · ${detail.conversation.cwd}`;
  providerEl.value = detail.conversation.provider;
  cwdEl.value = detail.conversation.cwd;
  modelEl.value = detail.conversation.model || "";
  effortEl.value = detail.conversation.config?.effort || "";
  const policy = detail.conversation.provider === "claude"
    ? detail.conversation.config?.permissionMode
    : detail.conversation.config?.approvalPolicy;
  policyEl.value = policy || "";
  syncPolicyOptions();
  renderConversations();
  renderMessages();
}

function renderConversations() {
  listEl.innerHTML = "";
  for (const item of state.conversations) {
    const button = document.createElement("button");
    button.className = `conversation-item${item.id === state.activeId ? " active" : ""}`;
    button.type = "button";
    button.innerHTML = `
      <span class="title"></span>
      <span class="meta"></span>
    `;
    button.querySelector(".title").textContent = item.title;
    button.querySelector(".meta").textContent = `${item.provider} · ${item.status}`;
    button.addEventListener("click", () => openConversation(item.id));
    listEl.append(button);
  }
}

function renderMessages() {
  messagesEl.innerHTML = "";
  if (state.messages.length === 0 && state.streaming.size === 0) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.textContent = "Create a chat, choose Claude or Codex, then send a task.";
    messagesEl.append(empty);
    return;
  }
  for (const message of state.messages) {
    messagesEl.append(messageNode(message.role, message.content));
  }
  for (const content of state.streaming.values()) {
    messagesEl.append(messageNode("assistant", content || "…"));
  }
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function messageNode(role, content) {
  const node = document.createElement("article");
  node.className = `message ${role}`;
  const label = document.createElement("div");
  label.className = "role";
  label.textContent = role;
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = content;
  node.append(label, bubble);
  return node;
}

function renderError(error) {
  state.messages = [{
    role: "system",
    content: typeof error === "string" ? error : JSON.stringify(error, null, 2)
  }];
  renderMessages();
}

function currentConfig() {
  const config = {};
  if (effortEl.value) config.effort = effortEl.value;
  if (providerEl.value === "claude" && policyEl.value) config.permissionMode = policyEl.value;
  if (providerEl.value === "codex" && policyEl.value) config.approvalPolicy = policyEl.value;
  return config;
}

function syncPolicyOptions() {
  const provider = providerEl.value;
  const options = provider === "claude"
    ? [["", "policy"], ["default", "default"], ["acceptEdits", "accept edits"], ["plan", "plan"], ["dontAsk", "dont ask"], ["bypassPermissions", "bypass"]]
    : [["", "policy"], ["on-request", "on request"], ["never", "never"], ["untrusted", "untrusted"]];
  const current = policyEl.value;
  policyEl.innerHTML = "";
  for (const [value, label] of options) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = label;
    policyEl.append(option);
  }
  policyEl.value = options.some(([value]) => value === current) ? current : "";
}

function rpc(op, params) {
  return new Promise((resolve, reject) => {
    if (!window.ipc) {
      reject({ error: "Capybara shell IPC unavailable" });
      return;
    }
    const id = `ui-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    pending.set(id, { resolve, reject });
    window.ipc.postMessage(JSON.stringify({ kind: "rpc", id, op, params }));
  });
}
