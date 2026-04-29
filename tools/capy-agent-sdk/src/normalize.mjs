import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const SDK_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const REPO_ROOT = path.resolve(SDK_ROOT, "..", "..");

export function parseArgs(argv) {
  const args = { _: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--") {
      args._.push(...argv.slice(index + 1));
      break;
    }
    if (!token.startsWith("--")) {
      args._.push(token);
      continue;
    }
    const eq = token.indexOf("=");
    const key = camel(eq === -1 ? token.slice(2) : token.slice(2, eq));
    let value = eq === -1 ? undefined : token.slice(eq + 1);
    if (value === undefined) {
      const next = argv[index + 1];
      if (next !== undefined && !next.startsWith("--")) {
        value = next;
        index += 1;
      } else {
        value = true;
      }
    }
    if (args[key] === undefined) args[key] = value;
    else if (Array.isArray(args[key])) args[key].push(value);
    else args[key] = [args[key], value];
  }
  return args;
}

export function normalize(raw = {}) {
  const provider = normalizeProvider(raw.provider ?? "claude");
  const cwd = path.resolve(str(raw.cwd) ?? process.cwd());
  const writeCode = bool(raw.writeCode);
  const outputSchema = parseJsonMaybe(raw.outputSchema ?? raw.jsonSchema);
  const mcpConfig = parseJsonMaybe(raw.mcpConfig);
  const common = {
    provider,
    runtimeBackend: "sdk",
    cwd,
    prompt: str(raw.prompt),
    model: str(raw.model),
    effort: normalizeEffort(raw.effort, provider),
    writeCode,
    maxTurns: int(raw.maxTurns),
    outputSchema,
    addDirs: list(raw.addDir ?? raw.addDirs).map((item) => path.resolve(cwd, item)),
    allowedTools: list(raw.allowedTools),
    disallowedTools: list(raw.disallowedTools),
    tools: normalizeTools(raw.tools),
    mcpConfig,
    search: bool(raw.search),
    threadId: str(raw.threadId),
    sessionId: str(raw.sessionId),
    resume: str(raw.resume),
    noSessionPersistence: bool(raw.noSessionPersistence),
    settingSources: normalizeSettingSources(raw.settingSource ?? raw.settingSources),
    codexPath: str(raw.codexPath),
    claudePath: str(raw.claudePath),
  };

  if (provider === "codex") {
    common.approvalPolicy = normalizeApproval(raw.approvalPolicy ?? (writeCode ? "never" : undefined));
    common.sandbox = normalizeSandbox(raw.sandbox ?? (writeCode ? "danger-full-access" : undefined));
    common.codexConfig = parseCodexConfig(list(raw.codexConfig));
    common.codex = {
      options: stripUndefined({
        codexPathOverride: common.codexPath,
        config: common.codexConfig,
      }),
      threadOptions: stripUndefined({
        model: common.model,
        workingDirectory: cwd,
        sandboxMode: common.sandbox,
        approvalPolicy: common.approvalPolicy,
        modelReasoningEffort: normalizeEffort(raw.effort, "codex"),
        webSearchMode: bool(raw.search) ? "live" : undefined,
        webSearchEnabled: bool(raw.search) ? true : undefined,
        networkAccessEnabled: bool(raw.search) ? true : undefined,
        additionalDirectories: common.addDirs.length ? common.addDirs : undefined,
        skipGitRepoCheck: bool(raw.skipGitRepoCheck) ? true : undefined,
      }),
      turnOptions: stripUndefined({ outputSchema }),
    };
    return common;
  }

  common.permissionMode = normalizePermission(raw.permissionMode ?? (writeCode ? "bypassPermissions" : undefined));
  common.allowDangerouslySkipPermissions =
    bool(raw.allowDangerouslySkipPermissions) || common.permissionMode === "bypassPermissions";
  common.claude = {
    options: stripUndefined({
      cwd,
      model: common.model,
      effort: normalizeEffort(raw.effort, "claude"),
      maxTurns: common.maxTurns,
      maxBudgetUsd: num(raw.maxBudgetUsd),
      permissionMode: common.permissionMode,
      allowDangerouslySkipPermissions: common.allowDangerouslySkipPermissions,
      additionalDirectories: common.addDirs.length ? common.addDirs : undefined,
      allowedTools: common.allowedTools.length ? common.allowedTools : undefined,
      disallowedTools: common.disallowedTools.length ? common.disallowedTools : undefined,
      tools: common.tools ?? (writeCode ? { type: "preset", preset: "claude_code" } : undefined),
      mcpServers: mcpConfig && typeof mcpConfig === "object" ? mcpConfig : undefined,
      outputFormat: outputSchema ? { type: "json_schema", schema: outputSchema } : undefined,
      pathToClaudeCodeExecutable: common.claudePath,
      persistSession: common.noSessionPersistence ? false : undefined,
      settingSources: common.settingSources,
      resume: common.resume,
      sessionId: common.sessionId,
    }),
  };
  return common;
}

function normalizeProvider(value) {
  const provider = String(value).trim().toLowerCase();
  if (["claude", "anthropic", "claude-code"].includes(provider)) return "claude";
  if (["codex", "openai", "gpt"].includes(provider)) return "codex";
  throw new Error(`unknown provider: ${value}`);
}

function normalizeApproval(value) {
  const raw = str(value);
  if (!raw) return undefined;
  const compact = raw.toLowerCase().replace(/_/g, "-");
  if (["auto", "always", "full-auto", "never"].includes(compact)) return "never";
  if (["onrequest", "on-request"].includes(compact)) return "on-request";
  if (["onfailure", "on-failure"].includes(compact)) return "on-failure";
  if (compact === "untrusted") return "untrusted";
  throw new Error(`unsupported Codex approval policy: ${value}`);
}

function normalizeSandbox(value) {
  const raw = str(value);
  if (!raw) return undefined;
  const compact = raw.replace(/[-_\s]/g, "").toLowerCase();
  if (compact === "readonly") return "read-only";
  if (compact === "workspacewrite") return "workspace-write";
  if (["dangerfullaccess", "fullauto", "danger", "none"].includes(compact)) return "danger-full-access";
  throw new Error(`unsupported Codex sandbox: ${value}`);
}

function normalizePermission(value) {
  const raw = str(value);
  if (!raw) return undefined;
  const compact = raw.replace(/[-_\s]/g, "").toLowerCase();
  if (["bypass", "bypasspermissions", "danger", "fullauto"].includes(compact)) return "bypassPermissions";
  if (compact === "acceptedits") return "acceptEdits";
  if (compact === "dontask") return "dontAsk";
  if (["default", "plan", "auto"].includes(compact)) return compact;
  throw new Error(`unsupported Claude permission mode: ${value}`);
}

function normalizeEffort(value, provider) {
  const raw = str(value);
  if (!raw) return undefined;
  const effort = raw.toLowerCase().replace(/[-_\s]/g, "");
  if (provider === "codex" && effort === "max") return "xhigh";
  if (provider === "claude" && effort === "minimal") return "low";
  if (["minimal", "low", "medium", "high", "xhigh", "max"].includes(effort)) return effort;
  throw new Error(`unsupported reasoning effort: ${value}`);
}

function normalizeTools(value) {
  if (value === undefined) return undefined;
  if (value === true) return { type: "preset", preset: "claude_code" };
  const values = list(value);
  if (values.length === 1 && ["all", "claudecode", "preset"].includes(values[0].replace(/[-_\s]/g, "").toLowerCase())) {
    return { type: "preset", preset: "claude_code" };
  }
  return values;
}

function normalizeSettingSources(value) {
  if (value === undefined) return [];
  const allowed = new Set(["user", "project", "local"]);
  return list(value).map((item) => {
    const source = item.toLowerCase();
    if (!allowed.has(source)) throw new Error(`unsupported Claude setting source: ${source}`);
    return source;
  });
}

function parseCodexConfig(values) {
  const config = {};
  for (const item of values) {
    const eq = item.indexOf("=");
    if (eq === -1) throw new Error(`Codex config must be key=value: ${item}`);
    setDotted(config, item.slice(0, eq).trim(), parseScalar(item.slice(eq + 1).trim()));
  }
  return Object.keys(config).length ? config : undefined;
}

function setDotted(target, key, value) {
  const parts = key.split(".").filter(Boolean);
  let cursor = target;
  for (const part of parts.slice(0, -1)) {
    cursor[part] = cursor[part] && typeof cursor[part] === "object" ? cursor[part] : {};
    cursor = cursor[part];
  }
  cursor[parts.at(-1)] = value;
}

function parseScalar(value) {
  if (value === "true") return true;
  if (value === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(value)) return Number(value);
  return parseJsonMaybe(value) ?? value;
}

function parseJsonMaybe(value) {
  const raw = str(value);
  if (!raw) return undefined;
  const maybePath = path.resolve(raw);
  if (fs.existsSync(maybePath) && fs.statSync(maybePath).isFile()) {
    return JSON.parse(fs.readFileSync(maybePath, "utf8"));
  }
  if (!raw.startsWith("{") && !raw.startsWith("[") && !raw.startsWith('"')) return undefined;
  return JSON.parse(raw);
}

function camel(value) {
  return value.replace(/-([a-z0-9])/g, (_, char) => char.toUpperCase());
}

function list(value) {
  if (value === undefined || value === false) return [];
  return (Array.isArray(value) ? value : [value])
    .flatMap((item) => String(item).split(","))
    .map((item) => item.trim())
    .filter(Boolean);
}

function bool(value) {
  if (value === true) return true;
  if (value === false || value === undefined || value === null) return false;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function str(value) {
  if (value === undefined || value === null || value === false) return undefined;
  const text = String(value).trim();
  return text ? text : undefined;
}

function int(value) {
  const text = str(value);
  if (!text) return undefined;
  const parsed = Number.parseInt(text, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(`expected positive integer, got: ${value}`);
  return parsed;
}

function num(value) {
  const text = str(value);
  if (!text) return undefined;
  const parsed = Number(text);
  if (!Number.isFinite(parsed)) throw new Error(`expected number, got: ${value}`);
  return parsed;
}

function stripUndefined(value) {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined));
}
