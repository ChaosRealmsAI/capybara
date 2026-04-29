export function createRuntimeControls({ state, dom }) {
  const {
    providerEl,
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
    configSummaryEl,
    runStatusEl,
    runtimeFootEl,
    canvasStatusEl,
  } = dom;

  function currentConfig() {
    const config = { capyCanvasTools: true };
    if (effortEl.value) config.effort = effortEl.value;
    if (providerEl.value === "claude" && policyEl.value) config.permissionMode = policyEl.value;
    if (providerEl.value === "codex" && policyEl.value) config.approvalPolicy = policyEl.value;
    if (sandboxEl.value) config.sandbox = sandboxEl.value;
    if (serviceTierEl.value.trim()) config.serviceTier = serviceTierEl.value.trim();
    if (systemPromptEl.value.trim()) config.systemPrompt = systemPromptEl.value.trim();
    if (appendSystemPromptEl.value.trim()) config.appendSystemPrompt = appendSystemPromptEl.value.trim();
    if (developerInstructionsEl.value.trim()) config.developerInstructions = developerInstructionsEl.value.trim();
    const addDirs = addDirsEl.value.split(",").map((value) => value.trim()).filter(Boolean);
    if (addDirs.length) config.addDirs = addDirs;
    if (allowedToolsEl.value.trim()) config.allowedTools = allowedToolsEl.value.trim();
    if (disallowedToolsEl.value.trim()) config.disallowedTools = disallowedToolsEl.value.trim();
    if (mcpConfigEl.value.trim()) config.mcpConfig = mcpConfigEl.value.trim();
    if (modelProviderEl.value.trim()) config.modelProvider = modelProviderEl.value.trim();
    if (approvalsReviewerEl.value) config.approvalsReviewer = approvalsReviewerEl.value;
    if (reasoningSummaryEl.value) config.reasoningSummary = reasoningSummaryEl.value;
    if (outputSchemaEl.value.trim()) config.outputSchema = outputSchemaEl.value.trim();
    if (bareEl.checked) config.bare = true;
    if (searchEl.checked) config.search = true;
    if (writeCodeEl.checked) {
      config.writeCode = true;
      if (providerEl.value === "codex" && !config.approvalPolicy) config.approvalPolicy = "never";
      if (providerEl.value === "claude" && !config.permissionMode) config.permissionMode = "bypassPermissions";
      if (!config.sandbox) config.sandbox = "danger-full-access";
      config.allowDangerouslySkipPermissions = true;
      config.dangerouslySkipPermissions = true;
    }
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

  function applyWriteCodeDefaults() {
    if (!writeCodeEl.checked) return;
    if (providerEl.value === "codex") policyEl.value = "never";
    else policyEl.value = "bypassPermissions";
    sandboxEl.value = "danger-full-access";
  }

  function updateConfigSummary() {
    if (!configSummaryEl) return;
    const provider = providerEl?.value || "claude";
    const effort = effortEl?.value || "default";
    const policy = policyEl?.value || "default";
    configSummaryEl.textContent = `${provider} · ${effort} · ${policy}`;
  }

  function setRunStatus(status) {
    runStatusEl.textContent = status || "idle";
    runStatusEl.dataset.status = status || "idle";
  }

  function renderRuntimeFoot() {
    const provider = providerEl.value === "claude" ? "Claude Code" : "Codex CLI";
    runtimeFootEl.textContent = `${provider} · Canvas CLI tools active · ${state.dbPath || "SQLite store pending"}`;
  }

  function updateCanvasStatus(text) {
    if (canvasStatusEl) canvasStatusEl.textContent = text;
  }

  return {
    currentConfig,
    syncPolicyOptions,
    applyWriteCodeDefaults,
    updateConfigSummary,
    setRunStatus,
    renderRuntimeFoot,
    updateCanvasStatus,
  };
}
