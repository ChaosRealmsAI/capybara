export function createRuntimeControls({ state, dom }) {
  const {
    providerEl,
    cwdEl,
    modelEl,
    effortEl,
    policyEl,
    sandboxEl,
    writeCodeEl,
    configSummaryEl,
    runStatusEl,
    stopEl,
    runtimeFootEl,
    canvasStatusEl,
  } = dom;

  function currentConfig() {
    const provider = providerEl.value || "claude";
    const permission = policyEl.value || defaultPermission(provider);
    const config = {
      runtimeBackend: "sdk",
      capyCanvasTools: true,
      effort: effortEl.value || "medium",
      writeCode: true,
    };
    if (provider === "codex") {
      config.approvalPolicy = "never";
      config.sandbox = permission === "codex-project-auto" ? "workspace-write" : "danger-full-access";
      config.capyProjectInstructions = true;
    } else {
      config.permissionMode = permission === "auto" ? "auto" : "bypassPermissions";
      config.allowDangerouslySkipPermissions = config.permissionMode === "bypassPermissions";
      config.dangerouslySkipPermissions = config.permissionMode === "bypassPermissions";
    }
    return config;
  }

  function syncPolicyOptions() {
    const provider = providerEl.value || "claude";
    const previousPermission = policyEl.value;
    policyEl.innerHTML = "";
    for (const optionDef of permissionsFor(provider)) {
      const option = document.createElement("option");
      option.value = optionDef.value;
      option.textContent = optionDef.label;
      policyEl.append(option);
    }
    policyEl.value = [...policyEl.options].some((option) => option.value === previousPermission)
      ? previousPermission
      : defaultPermission(provider);
    syncModelOptions(modelEl, provider);
    syncSandbox(sandboxEl, policyEl);
    if (writeCodeEl) writeCodeEl.checked = true;
  }

  function applyWriteCodeDefaults() {
    if (writeCodeEl) writeCodeEl.checked = true;
    if (!effortEl.value) effortEl.value = "medium";
    if (!policyEl.value) policyEl.value = defaultPermission(providerEl.value || "claude");
    syncSandbox(sandboxEl, policyEl);
  }

  function updateConfigSummary() {
    if (!configSummaryEl) return;
    const provider = providerEl?.value === "codex" ? "Codex" : "Claude";
    const model = selectedModelLabel(modelEl, providerEl?.value || "claude");
    const effort = effortEl?.value || "medium";
    const permission = selectedPermissionLabel(policyEl);
    const cwd = cwdEl?.value.trim() || DEFAULT_CWD;
    configSummaryEl.textContent = `${provider} · ${model} · ${effort} · ${permission}`;
    configSummaryEl.title = `SDK 全自动 · ${cwd}`;
  }

  function setRunStatus(status) {
    const next = status || "idle";
    state.planner.runStatus = next;
    runStatusEl.textContent = next;
    runStatusEl.dataset.status = next;
    if (stopEl) stopEl.disabled = next !== "running";
  }

  function renderRuntimeFoot() {
    runtimeFootEl.hidden = true;
    runtimeFootEl.textContent = "";
  }

  function updateCanvasStatus(text) {
    if (canvasStatusEl) canvasStatusEl.textContent = text;
  }

  function selectedModelValue(provider = providerEl?.value || "claude") {
    return safeProviderModelValue(provider, modelEl?.value);
  }

  return {
    currentConfig,
    syncPolicyOptions,
    applyWriteCodeDefaults,
    updateConfigSummary,
    setRunStatus,
    renderRuntimeFoot,
    updateCanvasStatus,
    selectedModelValue,
  };
}

const DEFAULT_CWD = "/Users/Zhuanz/workspace/capybara";
export const PROVIDER_OPTIONS = {
  claude: {
    models: [
      { value: "sonnet", label: "sonnet-4.7" },
      { value: "opus", label: "opus-4.7" },
    ],
    permissions: [
      { value: "bypassPermissions", label: "全自动" },
      { value: "auto", label: "auto 模式" },
    ],
  },
  codex: {
    models: [
      { value: "gpt-5.5", label: "gpt-5.5" },
      { value: "gpt-5.4", label: "gpt-5.4" },
      { value: "gpt-5.4-mini", label: "gpt-5.4-mini" },
    ],
    permissions: [
      { value: "codex-full-auto", label: "全自动" },
      { value: "codex-project-auto", label: "项目内自动" },
    ],
  },
};

export function providerDefaultModel(provider) {
  return modelsFor(provider)[0]?.value || "";
}

export function isCrossProviderModel(provider, value) {
  const model = String(value || "").trim().toLowerCase();
  if (!model) return false;
  if (provider === "codex") return model === "sonnet" || model === "opus" || model.startsWith("claude");
  if (provider === "claude") return model.startsWith("gpt-") || /^o[0-9-]/.test(model);
  return false;
}

export function safeProviderModelValue(provider, value) {
  const model = String(value || "").trim();
  if (!model || isCrossProviderModel(provider, model)) return providerDefaultModel(provider);
  return model;
}

function syncModelOptions(modelEl, provider) {
  if (!modelEl) return;
  const previous = safeProviderModelValue(provider, modelEl.value);
  const models = modelsFor(provider);
  modelEl.innerHTML = "";
  for (const model of models) {
    const option = document.createElement("option");
    option.value = model.value;
    option.textContent = model.label;
    modelEl.append(option);
  }
  if (previous && !models.some((model) => model.value === previous)) {
    const option = document.createElement("option");
    option.value = previous;
    option.textContent = previous;
    modelEl.append(option);
  }
  modelEl.value = previous || models[0]?.value || "";
}

function modelsFor(provider) {
  return PROVIDER_OPTIONS[provider]?.models || PROVIDER_OPTIONS.claude.models;
}

function permissionsFor(provider) {
  return PROVIDER_OPTIONS[provider]?.permissions || PROVIDER_OPTIONS.claude.permissions;
}

function defaultPermission(provider) {
  return permissionsFor(provider)[0]?.value || "bypassPermissions";
}

function selectedModelLabel(modelEl, provider) {
  const model = safeProviderModelValue(provider, modelEl?.value);
  const option = [...(modelEl?.options || [])].find((item) => item.value === model);
  return option?.textContent || model || "默认模型";
}

function selectedPermissionLabel(policyEl) {
  return policyEl?.selectedOptions?.[0]?.textContent || policyEl?.value || "全自动";
}

function syncSandbox(sandboxEl, policyEl) {
  if (!sandboxEl) return;
  sandboxEl.value = policyEl.value === "codex-project-auto" ? "workspace-write" : "danger-full-access";
}
