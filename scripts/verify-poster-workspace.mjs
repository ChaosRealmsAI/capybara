#!/usr/bin/env node
import http from "node:http";
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.27-json-poster-ppt-workspace");
const assetsDir = process.env.CAPY_POSTER_EVIDENCE_DIR
  ? path.resolve(process.env.CAPY_POSTER_EVIDENCE_DIR)
  : path.join(versionDir, "evidence", "assets");
const screenshotDesktop = path.join(assetsDir, "poster-workspace-desktop.png");
const screenshotMobile = path.join(assetsDir, "poster-workspace-mobile.png");
const statePath = path.join(assetsDir, "poster-workspace-browser-state.json");

async function loadChromium() {
  try {
    return (await import("playwright")).chromium;
  } catch {
    const require = createRequire("/opt/homebrew/lib/node_modules/playwright/package.json");
    return require("playwright").chromium;
  }
}

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
  await page.addInitScript(() => {
    window.CAPYBARA_SESSION = { cwd: "/Users/Zhuanz/workspace/capybara" };
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
      },
    };
  });

  await page.goto(`http://127.0.0.1:${port}/frontend/capy-app/index.html`, { waitUntil: "networkidle" });
  await page.click('[data-workspace-tab="poster"]');
  await page.waitForFunction(() => document.querySelector("#poster-preview")?.dataset.previewReady === "true");
  const autoState = await posterState(page);
  await page.click("#poster-open-single");
  await page.waitForFunction(() => document.querySelector("#poster-preview")?.dataset.previewReady === "true");
  await clickLayer(page, "headline");
  await page.fill('[data-poster-field="text"]', "每层可选中\nInspector 可编辑");
  await page.click("#poster-field-save");
  await page.waitForFunction(() => document.querySelector("#poster-source-json")?.textContent?.includes("Inspector 可编辑"));
  await page.click("#poster-verify");

  const desktopState = await posterState(page);
  await page.screenshot({ path: screenshotDesktop, fullPage: true });

  await page.click("#poster-open-deck");
  await page.waitForFunction(() => document.querySelector("#poster-status")?.textContent?.includes("Project Context Deck"));
  await page.click(".poster-page-row:nth-child(2)");
  await page.waitForFunction(() => document.querySelector("#poster-preview")?.dataset.previewReady === "true");
  const deckState = await posterState(page);

  await page.click("#poster-open-shared");
  await page.waitForFunction(() => document.querySelector("#poster-status")?.textContent?.includes("Shared Component Poster"));
  const sharedState = await posterState(page);

  await page.setViewportSize({ width: 390, height: 844 });
  await page.waitForTimeout(250);
  await page.screenshot({ path: screenshotMobile, fullPage: true });
  const mobileState = await posterState(page);

  if (autoState.previewReady !== "true") failures.push("poster tab did not auto-load real sample content");
  if (!autoState.previewText.includes("CAPYBARA")) failures.push("auto-loaded poster preview did not contain real poster text");
  if (autoState.pageCount !== 1 || autoState.layerCount < 5) failures.push("auto-loaded poster did not expose real page/layer content");
  if (desktopState.activeTab !== "poster") failures.push(`expected poster tab, got ${desktopState.activeTab}`);
  if (desktopState.activeTabLabel !== "海报") failures.push(`expected poster tab label 海报, got ${desktopState.activeTabLabel}`);
  if (desktopState.brandSubtitle !== "海报") failures.push(`expected brand subtitle 海报, got ${desktopState.brandSubtitle}`);
  if (desktopState.previewReady !== "true") failures.push("single poster preview was not ready");
  if (!desktopState.sourceText.includes("Inspector 可编辑")) failures.push("inspector edit did not patch source JSON");
  if (!desktopState.previewText.includes("Inspector 可编辑")) failures.push("inspector edit did not update preview text");
  if (!desktopState.exportStatus.includes("verified")) failures.push("verify button did not mark document verified");
  if (desktopState.layerCount < 5) failures.push("single poster should expose multiple selectable layers");
  if (desktopState.layout.workspace.w < 1100 || desktopState.layout.preview.w < 500) {
    failures.push("desktop workspace/preview is too narrow");
  }
  if (deckState.pageCount !== 3) failures.push(`expected 3 deck pages, got ${deckState.pageCount}`);
  if (!deckState.selectedPage.includes("p2")) failures.push(`expected selected deck page p2, got ${deckState.selectedPage}`);
  if (!sharedState.previewText.includes("组件跟视频共用")) failures.push("shared component poster did not render component text");
  if (mobileState.layout.workspace.w < 320 || mobileState.layout.preview.w < 320) {
    failures.push("mobile workspace/preview collapsed below usable width");
  }
  if (consoleEvents.some((event) => event.type === "error" || event.type === "pageerror")) {
    failures.push("console error or pageerror was emitted");
  }

  await fs.writeFile(
    statePath,
    `${JSON.stringify({
      ok: failures.length === 0,
      url: `http://127.0.0.1:${port}/frontend/capy-app/index.html`,
      screenshots: { desktop: screenshotDesktop, mobile: screenshotMobile },
      autoState,
      desktopState,
      deckState,
      sharedState,
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

async function clickLayer(page, layerId) {
  await page.locator(".poster-layer-row", { hasText: layerId }).click();
}

async function posterState(page) {
  return page.evaluate(() => {
    const rect = (selector) => {
      const el = document.querySelector(selector);
      if (!el) return { found: false, w: 0, h: 0 };
      const box = el.getBoundingClientRect();
      return {
        found: true,
        x: Math.round(box.x),
        y: Math.round(box.y),
        w: Math.round(box.width),
        h: Math.round(box.height),
      };
    };
    const snapshot = window.capyWorkbench?.stateSnapshot?.() || {};
    return {
      activeTab: snapshot.workspace?.activeTab || "",
      activeTabLabel: document.querySelector('[data-workspace-tab="poster"]')?.textContent?.trim() || "",
      brandSubtitle: document.querySelector(".brand-subtitle")?.textContent?.trim() || "",
      status: document.querySelector("#poster-status")?.textContent || "",
      exportStatus: document.querySelector("#poster-export-status")?.textContent || "",
      selectedPage: snapshot.posterWorkspace?.pageId || "",
      selectedLayer: snapshot.posterWorkspace?.layerPath || "",
      pageCount: snapshot.posterWorkspace?.pageCount || 0,
      previewReady: document.querySelector("#poster-preview")?.dataset.previewReady || "",
      previewText: document.querySelector("#poster-preview")?.innerText || "",
      sourceText: document.querySelector("#poster-source-json")?.textContent || "",
      layerCount: document.querySelectorAll("#poster-layers .poster-layer-row").length,
      previewLayerCount: document.querySelectorAll("#poster-preview [data-layer-id]").length,
      layout: {
        viewport: { w: innerWidth, h: innerHeight },
        workspace: rect('[data-section="poster-workspace"]'),
        preview: rect("#poster-preview"),
        inspector: rect(".poster-inspector-panel"),
      },
    };
  });
}
