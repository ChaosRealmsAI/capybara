export function createComponentRuntime() {
  const modules = new Map();
  const urls = new Map();
  const definitions = new Map();

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

  async function loadComponent(key, definition, baseUrl = globalThis.location?.href || "") {
    const resolved = await resolveComponentDefinition(definition, baseUrl);
    return loadModule(`${key}::${resolved.cacheKey}`, resolved.runtime);
  }

  async function resolveComponentDefinition(definition, baseUrl = globalThis.location?.href || "") {
    if (typeof definition === "string") {
      return { runtime: definition, cacheKey: "inline" };
    }
    if (definition?.runtime) {
      return { runtime: definition.runtime, cacheKey: definition.package || "inline-object" };
    }
    if (definition?.package) {
      const manifestUrl = absoluteUrl(definition.package, baseUrl);
      if (definitions.has(manifestUrl)) return definitions.get(manifestUrl);
      const manifest = await fetchJson(manifestUrl);
      const runtimeEntry = manifest.entrypoints?.runtime || "runtime.js";
      const runtimeUrl = absoluteUrl(runtimeEntry, manifestUrl);
      const runtime = await fetchText(runtimeUrl);
      const resolved = {
        manifest,
        runtime,
        cacheKey: `${manifest.id || manifestUrl}@${manifest.version || "0"}`,
      };
      definitions.set(manifestUrl, resolved);
      return resolved;
    }
    throw new Error("missing component runtime source");
  }

  function clear() {
    for (const url of urls.values()) URL.revokeObjectURL(url);
    modules.clear();
    urls.clear();
    definitions.clear();
  }

  return { loadModule, loadComponent, resolveComponentDefinition, clear };
}

function absoluteUrl(value, baseUrl) {
  const base = baseUrl
    ? new URL(baseUrl, globalThis.location?.href || "http://localhost/").href
    : globalThis.location?.href || "http://localhost/";
  return new URL(value, base).href;
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`component manifest load failed: ${url}: ${response.status}`);
  return response.json();
}

async function fetchText(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`component runtime load failed: ${url}: ${response.status}`);
  return response.text();
}

export function destroyMounted(mounted) {
  for (const entry of mounted.values()) {
    entry.module?.destroy && entry.module.destroy(entry.el);
    entry.el.remove();
  }
  mounted.clear();
}
