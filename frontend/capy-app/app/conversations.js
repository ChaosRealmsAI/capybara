import { renderMessageContent, renderMessageSegments } from "./message-renderer.js";

export function createConversations(ctx) {
  const {
    state,
    rpc,
    currentConfig,
    syncPolicyOptions,
    applyWriteCodeDefaults,
    updateConfigSummary,
    setRunStatus,
    renderRuntimeFoot,
    stringifyError,
    listEl,
    messagesEl,
    providerEl,
    cwdEl,
    modelEl,
    effortEl,
    policyEl,
    sandboxEl,
    serviceTierEl,
    systemPromptEl,
    appendSystemPromptEl,
    developerInstructionsEl,
    addDirsEl,
    allowedToolsEl,
    disallowedToolsEl,
    mcpConfigEl,
    modelProviderEl,
    approvalsReviewerEl,
    reasoningSummaryEl,
    outputSchemaEl,
    bareEl,
    searchEl,
    writeCodeEl,
  } = ctx;

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
  state.dbPath = data.db_path || state.dbPath;
  state.conversations = data.conversations || [];
  renderConversations();
  renderRuntimeFoot();
}

async function openConversation(id) {
  if (!id) return;
  const detail = await rpc("conversation-open", { id });
  state.activeId = detail.conversation.id;
  state.messages = detail.messages || [];
  providerEl.value = detail.conversation.provider;
  cwdEl.value = detail.conversation.cwd;
  effortEl.value = detail.conversation.config?.effort || "";
  syncPolicyOptions();
  setSelectValue(modelEl, detail.conversation.model || modelEl.value);
  const policy = detail.conversation.provider === "claude"
    ? detail.conversation.config?.permissionMode
    : codexPermissionPreset(detail.conversation.config);
  setSelectValue(policyEl, policy || policyEl.value);
  sandboxEl.value = detail.conversation.config?.sandbox || sandboxEl.value;
  serviceTierEl.value = detail.conversation.config?.serviceTier || "";
  systemPromptEl.value = detail.conversation.config?.systemPrompt || "";
  appendSystemPromptEl.value = detail.conversation.config?.appendSystemPrompt || "";
  developerInstructionsEl.value = detail.conversation.config?.developerInstructions || "";
  addDirsEl.value = (detail.conversation.config?.addDirs || []).join(", ");
  allowedToolsEl.value = detail.conversation.config?.allowedTools || "";
  disallowedToolsEl.value = detail.conversation.config?.disallowedTools || "";
  mcpConfigEl.value = detail.conversation.config?.mcpConfig || "";
  modelProviderEl.value = detail.conversation.config?.modelProvider || "";
  approvalsReviewerEl.value = detail.conversation.config?.approvalsReviewer || "";
  reasoningSummaryEl.value = detail.conversation.config?.reasoningSummary || "";
  outputSchemaEl.value = detail.conversation.config?.outputSchema || "";
  bareEl.checked = Boolean(detail.conversation.config?.bare);
  searchEl.checked = Boolean(detail.conversation.config?.search);
  writeCodeEl.checked = Boolean(detail.conversation.config?.writeCode);
  setRunStatus(detail.conversation.status || "idle");
  applyWriteCodeDefaults();
  renderConversations();
  renderMessages();
  renderRuntimeFoot();
  updateConfigSummary();
}

function codexPermissionPreset(config = {}) {
  if (config?.sandbox === "workspace-write") return "codex-project-auto";
  return "codex-full-auto";
}

function setSelectValue(select, value) {
  if (!select || !value) return;
  if (![...select.options].some((option) => option.value === value)) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = value;
    select.append(option);
  }
  select.value = value;
}

async function updateConversationConfig() {
  if (!state.activeId) return;
  await rpc("conversation-update-config", {
    id: state.activeId,
    model: modelEl.value.trim() || null,
    config: currentConfig()
  });
  await refreshList();
}

function renderConversations() {
  listEl.innerHTML = "";
  for (const item of state.conversations) {
    const button = document.createElement("button");
    button.type = "button";
    const isRunning = (item.status || "").toLowerCase() === "running";
    button.className = `conversation-item${item.id === state.activeId ? " active" : ""}${isRunning ? " is-running" : ""}`;
    button.innerHTML = `<span class="dot"></span><span class="title"></span><span class="meta"></span>`;
    button.querySelector(".title").textContent = item.title;
    button.querySelector(".meta").textContent = `${item.provider} · ${item.status}`;
    button.addEventListener("click", () => openConversation(item.id).catch((error) => renderError(error)));
    listEl.append(button);
  }
}

function renderMessages() {
  messagesEl.innerHTML = "";
  const isRunning = state.planner.runStatus === "running";
  if (state.messages.length === 0 && state.streaming.size === 0 && !isRunning) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.dataset.component = "chat-empty-state";
    empty.textContent = "选中画布上的节点 · Planner 会围绕该对象工作。试试 ⌘K 打开生图工具。";
    messagesEl.append(empty);
    return;
  }
  for (const message of state.messages) messagesEl.append(messageNode(message.role, message.content, message.event_json));
  for (const entry of state.streaming.values()) {
    if (entry) messagesEl.append(messageNode("assistant", streamingContent(entry), streamingEventJson(entry), true));
  }
  if (isRunning) messagesEl.append(loadingMessageNode());
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function messageNode(role, content, eventJson = null, streaming = false) {
  const node = document.createElement("article");
  node.className = `message ${role}`;
  node.dataset.role = role;
  if (streaming) node.classList.add("is-streaming");
  if (role !== "user") {
    const label = document.createElement("div");
    label.className = "role";
    label.textContent = role;
    node.append(label);
  }
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.dataset.component = "message-bubble";
  const segments = eventJson?.segments;
  if (Array.isArray(segments) && segments.length) {
    bubble.append(renderMessageSegments(segments, { loading: streaming, fallback: content }));
  } else {
    bubble.append(renderMessageContent(content));
  }
  node.append(bubble);
  return node;
}

function loadingMessageNode() {
  const node = messageNode("assistant", "");
  node.classList.add("is-loading");
  node.querySelector(".bubble")?.replaceChildren(renderMessageContent("", { loading: true }));
  return node;
}

function renderError(error) {
  state.messages = [{ role: "system", content: stringifyError(error) }];
  renderMessages();
}

function streamingContent(entry) {
  if (typeof entry === "string") return entry;
  return entry?.content || "";
}

function streamingEventJson(entry) {
  if (typeof entry === "string") return null;
  return { segments: entry?.segments || [] };
}


  return {
    createConversation,
    refreshList,
    openConversation,
    updateConversationConfig,
    renderConversations,
    renderMessages,
    renderError,
  };
}
