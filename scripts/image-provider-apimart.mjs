#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const DEFAULT_ROOT = "/Users/Zhuanz/workspace/apimart-image-gen";

function readStdin() {
  return new Promise((resolve, reject) => {
    let text = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => { text += chunk; });
    process.stdin.on("end", () => resolve(text));
    process.stdin.on("error", reject);
  });
}

async function loadProvider(root) {
  const modulePath = path.join(root, "scripts", "apimart.mjs");
  await fs.access(modulePath);
  return import(pathToFileURL(modulePath).href);
}

function payload(input) {
  return {
    prompt: input.prompt,
    size: input.size || "1:1",
    resolution: input.resolution || "1k",
    imageUrls: Array.isArray(input.refs) && input.refs.length ? input.refs : undefined,
    outputDir: input.out || undefined,
    filename: input.name || undefined,
    download: input.download !== false,
  };
}

function write(value) {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
}

function fail(error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${JSON.stringify({ ok: false, error: { message } }, null, 2)}\n`);
  process.exit(1);
}

try {
  const operation = process.argv[2] || "doctor";
  const inputText = await readStdin();
  const input = inputText.trim() ? JSON.parse(inputText) : {};
  const root = process.env.CAPY_IMAGE_GEN_APIMART_ROOT || input.provider_root || DEFAULT_ROOT;
  const provider = await loadProvider(root);

  if (operation === "balance") {
    const [token, user] = await Promise.all([
      provider.getTokenBalance(),
      provider.getUserBalance(),
    ]);
    write({
      ok: true,
      provider: "apimart-gpt-image-2",
      model: "gpt-image-2",
      token,
      user,
    });
  } else if (operation === "submit") {
    const taskId = await provider.submit(payload(input));
    write({
      ok: true,
      provider: "apimart-gpt-image-2",
      model: "gpt-image-2",
      mode: "submit-only",
      task_id: taskId,
    });
  } else if (operation === "resume") {
    const result = await provider.resumeFromTaskId({
      ...payload(input),
      taskId: input.task_id,
    });
    write({
      ok: true,
      provider: "apimart-gpt-image-2",
      model: "gpt-image-2",
      mode: "resume",
      result,
    });
  } else if (operation === "generate") {
    const result = await provider.generate(payload(input));
    write({
      ok: true,
      provider: "apimart-gpt-image-2",
      model: "gpt-image-2",
      mode: "generate",
      result,
    });
  } else {
    throw new Error(`unknown operation: ${operation}`);
  }
} catch (error) {
  fail(error);
}
