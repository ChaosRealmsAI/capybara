export function createComponentRuntime() {
  const modules = new Map();
  const urls = new Map();

  async function loadModule(key, sourceText) {
    if (modules.has(key)) return modules.get(key);
    if (!sourceText) throw new Error(`missing component source: ${key}`);
    const blob = new Blob([String(sourceText)], { type: "text/javascript" });
    const url = URL.createObjectURL(blob);
    urls.set(key, url);
    const module = await import(url);
    modules.set(key, module);
    return module;
  }

  function clear() {
    for (const url of urls.values()) URL.revokeObjectURL(url);
    modules.clear();
    urls.clear();
  }

  return { loadModule, clear };
}

export function destroyMounted(mounted) {
  for (const entry of mounted.values()) {
    entry.module?.destroy && entry.module.destroy(entry.el);
    entry.el.remove();
  }
  mounted.clear();
}
