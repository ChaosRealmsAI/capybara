export function installShellUi(ctx) {
  const {
    state,
    configDialogEl,
    configSummaryEl,
    configDialogCloseEl,
    configDialogDoneEl,
    cmdkTriggerEl,
    cmdPaletteEl,
    cmdSearchEl,
    cmdCloseEl,
    cmdListEl,
    cmdToolEl,
    cmdToolBackEl,
    imageToolPromptEl,
    updateConfigSummary,
    updateConversationConfig,
    renderRuntimeFoot,
    renderError,
    defaultImagePrompt,
  } = ctx;

/* ─── config dialog · 弹窗 ─── */
function openConfigDialog() {
  if (!configDialogEl) return;
  configDialogEl.classList.add("is-open");
  configDialogEl.style.display = "grid";
}
async function closeConfigDialog() {
  if (!configDialogEl) return;
  configDialogEl.classList.remove("is-open");
  configDialogEl.style.display = "none";
  updateConfigSummary();
  if (state.activeId) {
    try { await updateConversationConfig(); } catch (e) { renderError(e); }
  }
}
configSummaryEl?.addEventListener("click", openConfigDialog);
configDialogCloseEl?.addEventListener("click", () => closeConfigDialog());
configDialogDoneEl?.addEventListener("click", () => closeConfigDialog());
configDialogEl?.addEventListener("click", (e) => {
  if (e.target === configDialogEl) closeConfigDialog();
});

/* ─── cmd-K palette ─── */
function openCmdPalette() {
  if (!cmdPaletteEl) return;
  switchCmdView("list");
  cmdPaletteEl.classList.add("is-open");
  cmdPaletteEl.style.display = "grid";
  setTimeout(() => cmdSearchEl?.focus(), 30);
}
function closeCmdPalette() {
  if (!cmdPaletteEl) return;
  cmdPaletteEl.classList.remove("is-open");
  cmdPaletteEl.style.display = "none";
  if (cmdSearchEl) cmdSearchEl.value = "";
  switchCmdView("list");
}
function switchCmdView(view) {
  if (!cmdToolEl || !cmdListEl) return;
  if (view === "tool") {
    cmdListEl.style.display = "none";
    cmdToolEl.hidden = false;
    setTimeout(() => imageToolPromptEl?.focus(), 30);
  } else {
    cmdListEl.style.display = "";
    cmdToolEl.hidden = true;
  }
}
cmdkTriggerEl?.addEventListener("click", openCmdPalette);
cmdCloseEl?.addEventListener("click", closeCmdPalette);
cmdToolBackEl?.addEventListener("click", () => switchCmdView("list"));
cmdPaletteEl?.addEventListener("click", (e) => {
  if (e.target === cmdPaletteEl) closeCmdPalette();
});
cmdPaletteEl?.addEventListener("close", () => switchCmdView("list"));

cmdListEl?.addEventListener("click", (e) => {
  const row = e.target instanceof HTMLElement ? e.target.closest(".cmd-row") : null;
  if (!row) return;
  const cmd = row.dataset.cmd;
  runCmd(cmd);
});

cmdSearchEl?.addEventListener("input", () => {
  const q = cmdSearchEl.value.trim().toLowerCase();
  cmdListEl?.querySelectorAll(".cmd-row").forEach((row) => {
    const text = row.textContent.toLowerCase();
    row.style.display = !q || text.includes(q) ? "" : "none";
  });
});

cmdSearchEl?.addEventListener("keydown", (e) => {
  if (e.key === "Enter") {
    const visible = cmdListEl?.querySelector('.cmd-row:not([style*="display: none"])');
    if (visible) runCmd(visible.dataset.cmd);
  }
});

window.addEventListener("keydown", (e) => {
  const isCmdK = (e.metaKey || e.ctrlKey) && (e.key === "k" || e.key === "K");
  if (isCmdK) {
    e.preventDefault();
    if (cmdPaletteEl?.classList.contains("is-open")) closeCmdPalette(); else openCmdPalette();
  }
  if (e.key === "Escape") {
    if (cmdPaletteEl?.classList.contains("is-open")) closeCmdPalette();
    if (configDialogEl?.classList.contains("is-open")) closeConfigDialog();
  }
});

function runCmd(cmd) {
  if (cmd === "open-image-tool") {
    if (imageToolPromptEl && !imageToolPromptEl.value.trim()) {
      imageToolPromptEl.value = defaultImagePrompt();
    }
    switchCmdView("tool");
    return;
  }
  if (cmd === "dark-mode") {
    state.canvas.darkMode = !state.canvas.darkMode;
    document.documentElement.dataset.canvasDark = state.canvas.darkMode ? "true" : "false";
    closeCmdPalette();
    return;
  }
}


  return {
    openCmdPalette,
    closeCmdPalette,
    openConfigDialog,
    closeConfigDialog,
  };
}
