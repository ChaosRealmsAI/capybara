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
  modelEl.value = detail.conversation.model || "";
  effortEl.value = detail.conversation.config?.effort || "";
  const policy = detail.conversation.provider === "claude"
    ? detail.conversation.config?.permissionMode
    : detail.conversation.config?.approvalPolicy;
  policyEl.value = policy || "";
  sandboxEl.value = detail.conversation.config?.sandbox || "";
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
  syncPolicyOptions();
  applyWriteCodeDefaults();
  renderConversations();
  renderMessages();
  renderRuntimeFoot();
  updateConfigSummary();
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
  if (state.messages.length === 0 && state.streaming.size === 0) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.textContent = "选中画布上的节点 · Planner 会围绕该对象工作。试试 ⌘K 打开生图工具。";
    messagesEl.append(empty);
    return;
  }
  for (const message of state.messages) messagesEl.append(messageNode(message.role, message.content));
  for (const content of state.streaming.values()) messagesEl.append(messageNode("assistant", content || "..."));
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function messageNode(role, content) {
  const node = document.createElement("article");
  node.className = `message ${role}`;
  if (role !== "user") {
    const label = document.createElement("div");
    label.className = "role";
    label.textContent = role;
    node.append(label);
  }
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = content;
  node.append(bubble);
  return node;
}

function renderError(error) {
  state.messages = [{ role: "system", content: stringifyError(error) }];
  renderMessages();
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
