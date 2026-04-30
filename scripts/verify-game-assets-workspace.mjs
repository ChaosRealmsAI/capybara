#!/usr/bin/env node
import http from "node:http";
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.30-game-asset-workbench");
const assetsDir = process.env.CAPY_GAME_ASSETS_EVIDENCE_DIR
  ? path.resolve(process.env.CAPY_GAME_ASSETS_EVIDENCE_DIR)
  : path.join(versionDir, "evidence", "assets");
const packPath = process.env.CAPY_GAME_ASSETS_PACK || "target/capy-game-assets-sample/pack.json";
const screenshotDesktop = path.join(assetsDir, "game-assets-workspace-desktop.png");
const screenshotMobile = path.join(assetsDir, "game-assets-workspace-mobile.png");
const statePath = path.join(assetsDir, "game-assets-workspace-browser-state.json");

async function loadChromium() {
  try {
    return (await import("playwright")).chromium;
  } catch {
    const require = createRequire("/opt/homebrew/lib/node_modules/playwright/package.json");
    return require("playwright").chromium;
  }
}

await fs.access(path.join(root, packPath));
await fs.mkdir(assetsDir, { recursive: true });

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url || "/", "http://127.0.0.1");
  const pathname = url.pathname === "/" ? "/frontend/capy-app/index.html" : url.pathname;
  const file = path.normalize(path.join(root, pathname));
  if (!file.startsWith(root)) {
    res.writeHead(403);
    res.end("forbidden");
    return;
  }
  try {
    const bytes = await fs.readFile(file);
    const ext = path.extname(file);
    const type = {
      ".css": "text/css",
      ".html": "text/html",
      ".js": "text/javascript",
      ".json": "application/json",
      ".png": "image/png",
      ".svg": "image/svg+xml",
      ".wasm": "application/wasm",
    }[ext] || "application/octet-stream";
    res.writeHead(200, { "content-type": type });
    res.end(bytes);
  } catch {
    res.writeHead(404);
    res.end("not found");
  }
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const { port } = server.address();
const consoleEvents = [];
const failures = [];
const chromium = await loadChromium();
const browser = await chromium.launch();

try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
  page.on("console", (message) => consoleEvents.push({ type: message.type(), text: message.text() }));
  page.on("pageerror", (error) => consoleEvents.push({ type: "pageerror", text: error.message }));
  await page.addInitScript((cwd) => {
    window.CAPYBARA_SESSION = { cwd };
    window.ipc = {
      postMessage(raw) {
        let request;
        try {
          request = JSON.parse(raw);
        } catch {
          return;
        }
        const reply = (data) => {
          setTimeout(() => window.__capyReceive && window.__capyReceive({ req_id: request.id, ok: true, data }), 0);
        };
        if (request.op === "conversation-list") reply({ db_path: null, conversations: [] });
        else reply({});
      }
    };
  }, root);

  await page.goto(`http://127.0.0.1:${port}/frontend/capy-app/index.html`, { waitUntil: "networkidle" });
  await page.click('[data-workspace-tab="game-assets"]');
  await page.waitForFunction(() => window.capyWorkbench?.stateSnapshot?.().gameAssets?.assetCount >= 5);
  await page.click("#game-assets-verify");
  await page.waitForFunction(() => document.querySelector("#game-assets-status")?.textContent?.includes("通过"));
  await page.locator(".game-assets-row", { hasText: "Bramble Sentinel" }).click();
  await page.waitForTimeout(150);
  const desktopState = await gameAssetsState(page);
  await page.screenshot({ path: screenshotDesktop, fullPage: true });

  await page.setViewportSize({ width: 390, height: 844 });
  await page.waitForTimeout(250);
  const mobileState = await gameAssetsState(page);
  await page.screenshot({ path: screenshotMobile, fullPage: true });

  if (desktopState.activeTab !== "game-assets") failures.push(`expected game-assets tab, got ${desktopState.activeTab}`);
  if (desktopState.brandSubtitle !== "游戏素材") failures.push(`expected brand subtitle 游戏素材, got ${desktopState.brandSubtitle}`);
  if (desktopState.assetCount < 5) failures.push(`expected at least 5 assets, got ${desktopState.assetCount}`);
  if (desktopState.frameCount < 16) failures.push(`expected at least 16 frames, got ${desktopState.frameCount}`);
  if (!desktopState.status.includes("通过")) failures.push(`verify status missing pass text: ${desktopState.status}`);
  if (desktopState.previewImages < 1) failures.push("asset preview image did not render");
  if (desktopState.frameImages < 4) failures.push("selected animated asset did not render frames");
  if (desktopState.layout.workspace.w < 1100 || desktopState.layout.preview.w < 420) failures.push("desktop game assets workspace is too narrow");
  if (mobileState.layout.workspace.w < 320 || mobileState.layout.preview.w < 320) failures.push("mobile game assets workspace collapsed below usable width");
  if (consoleEvents.some((event) => event.type === "error" || event.type === "pageerror")) failures.push("console error or pageerror was emitted");

  await fs.writeFile(
    statePath,
    `${JSON.stringify({
      ok: failures.length === 0,
      url: `http://127.0.0.1:${port}/frontend/capy-app/index.html`,
      packPath,
      screenshots: { desktop: screenshotDesktop, mobile: screenshotMobile },
      desktopState,
      mobileState,
      consoleEvents,
      failures,
      verdict: failures.length ? "failed" : "passed",
    }, null, 2)}\n`
  );

  if (failures.length) {
    console.error(failures.join("\n"));
    process.exitCode = 1;
  }
} finally {
  await browser.close();
  server.close();
}

async function gameAssetsState(page) {
  return page.evaluate(() => {
    const rect = (selector) => {
      const el = document.querySelector(selector);
      if (!el) return { found: false, w: 0, h: 0 };
      const box = el.getBoundingClientRect();
      return { found: true, w: Math.round(box.width), h: Math.round(box.height) };
    };
    const snapshot = window.capyWorkbench?.stateSnapshot?.() || {};
    return {
      activeTab: snapshot.workspace?.activeTab || "",
      brandSubtitle: document.querySelector(".brand-subtitle")?.textContent?.trim() || "",
      status: document.querySelector("#game-assets-status")?.textContent || "",
      selectedAssetId: snapshot.gameAssets?.selectedAssetId || "",
      assetCount: snapshot.gameAssets?.assetCount || 0,
      frameCount: snapshot.gameAssets?.frameCount || 0,
      previewImages: document.querySelectorAll("#game-assets-preview img").length,
      frameImages: document.querySelectorAll("#game-assets-frames img").length,
      contactSheetLoaded: document.querySelector("#game-assets-contact-sheet")?.naturalWidth > 0,
      layout: {
        viewport: { w: innerWidth, h: innerHeight },
        workspace: rect('[data-section="game-assets-workspace"]'),
        preview: rect("#game-assets-preview"),
        frames: rect("#game-assets-frames"),
      },
    };
  });
}
