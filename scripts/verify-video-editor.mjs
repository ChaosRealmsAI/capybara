import http from "node:http";
import fs from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const appRoot = path.join(root, "frontend/capy-app");
const evidence = path.resolve(root, process.env.CAPY_VIDEO_EVIDENCE_DIR || "spec/versions/v0.25-video-editing-tab/evidence/assets");
const compositionPath = path.resolve(root, process.env.CAPY_VIDEO_COMPOSITION || "fixtures/timeline/video-editing/compositions/main.json");
const renderSourcePath = path.resolve(
  root,
  process.env.CAPY_VIDEO_RENDER_SOURCE || path.join(path.dirname(compositionPath), "render_source.json")
);
const screenshotName = process.env.CAPY_VIDEO_SCREENSHOT || "video-editor-browser-preview.png";
const stateName = process.env.CAPY_VIDEO_STATE || "video-editor-browser-state.json";
const expectedText = process.env.CAPY_VIDEO_EXPECT_TEXT || "视频剪辑像画布一样打开";

async function loadChromium() {
  try {
    return (await import("playwright")).chromium;
  } catch {
    const require = createRequire("/opt/homebrew/lib/node_modules/playwright/package.json");
    return require("playwright").chromium;
  }
}

const composition = JSON.parse(await fs.readFile(compositionPath, "utf8"));
const renderSource = JSON.parse(await fs.readFile(renderSourcePath, "utf8"));
const openResult = {
  ok: true,
  stage: "composition-open",
  composition_path: compositionPath,
  render_source_path: renderSourcePath,
  preview_url: "http://127.0.0.1/preview/index.html",
  schema_version: composition.schema_version || composition.schema || "capy.composition.v2",
  track_count: Array.isArray(renderSource.tracks) ? renderSource.tracks.length : 0,
  asset_count: 0,
  editor: editorSummary(composition, renderSource),
  render_source: renderSource
};

function editorSummary(document, source) {
  let cursor = 0;
  const clips = [];
  const tracks = [];
  for (const clip of Array.isArray(document.clips) ? document.clips : []) {
    const duration = durationMs(clip.duration_ms ?? clip.duration ?? clip.length);
    const start = Number(clip.start_ms ?? cursor);
    const clipTracks = Array.isArray(clip.tracks) ? clip.tracks : [];
    clips.push({
      id: String(clip.id || `clip-${clips.length + 1}`),
      name: String(clip.name || clip.id || `Clip ${clips.length + 1}`),
      start_ms: start,
      duration_ms: duration,
      end_ms: start + duration,
      track_count: clipTracks.length
    });
    for (const track of clipTracks) {
      const item = Array.isArray(track.items) && track.items.length ? track.items[0] : {};
      const params = item.params || track.params || {};
      tracks.push({
        id: `${clip.id || clips.length}.${track.id || "track"}`,
        clip_id: String(clip.id || `clip-${clips.length}`),
        local_id: String(track.id || "track"),
        label: String(params.title || track.name || track.id || "track"),
        kind: String(track.kind || "component"),
        component: String(track.component || ""),
        z: Number(track.z || 0),
        start_ms: start,
        duration_ms: duration,
        end_ms: start + duration,
        fields: Object.entries(params).map(([field, value]) => ({
          field,
          kind: typeof value === "number" ? "number" : "text",
          value
        }))
      });
    }
    cursor = start + duration;
  }
  return {
    id: String(document.id || "composition"),
    name: String(document.name || document.title || "Composition"),
    duration_ms: Number(source.duration_ms || cursor),
    viewport: document.viewport || source.viewport || { w: 1920, h: 1080, ratio: "16:9" },
    clips,
    tracks,
    render_source_tracks: Array.isArray(source.tracks) ? source.tracks.length : tracks.length
  };
}

function durationMs(value) {
  if (typeof value === "number" && Number.isFinite(value)) return Math.max(0, Math.round(value));
  const raw = String(value || "0").trim();
  if (raw.endsWith("ms")) return Math.max(0, Math.round(Number(raw.slice(0, -2)) || 0));
  if (raw.endsWith("s")) return Math.max(0, Math.round((Number(raw.slice(0, -1)) || 0) * 1000));
  return Math.max(0, Math.round(Number(raw) || 0));
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url || "/", "http://127.0.0.1");
  const pathname = url.pathname === "/" ? "/index.html" : url.pathname;
  const file = path.normalize(path.join(appRoot, pathname));
  if (!file.startsWith(appRoot)) {
    res.writeHead(403);
    res.end("forbidden");
    return;
  }
  try {
    const bytes = await fs.readFile(file);
    const ext = path.extname(file);
    const type = ext === ".js" ? "text/javascript" : ext === ".css" ? "text/css" : ext === ".wasm" ? "application/wasm" : "text/html";
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
const chromium = await loadChromium();
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
  page.on("console", (message) => consoleEvents.push({ type: message.type(), text: message.text() }));
  page.on("pageerror", (error) => consoleEvents.push({ type: "pageerror", text: error.message }));
  await page.addInitScript((result) => {
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
        else if (request.op === "timeline-composition-open") reply(result);
        else if (request.op === "timeline-export-start") reply({ job: { status: "done", output_path: "/tmp/capy-video.mp4" } });
        else reply({});
      }
    };
  }, openResult);
  await page.goto(`http://127.0.0.1:${port}/index.html`, { waitUntil: "networkidle" });
  await page.evaluate((composition) => window.capyWorkbench.openVideoComposition(composition), compositionPath);
  await page.waitForFunction(() => document.querySelector("#video-preview")?.dataset.previewReady === "true");
  await page.screenshot({ path: path.join(evidence, screenshotName), fullPage: true });
  const state = await page.evaluate(() => ({
    tab: window.capyWorkbench.stateSnapshot().workspace.activeTab,
    status: document.querySelector("#video-status")?.textContent,
    previewReady: document.querySelector("#video-preview")?.dataset.previewReady,
    previewText: document.querySelector("#video-preview")?.innerText,
    previewLayers: document.querySelectorAll(".video-preview-layer").length,
    stage: (() => {
      const rect = document.querySelector(".video-preview-stage")?.getBoundingClientRect();
      return rect ? { w: Math.round(rect.width), h: Math.round(rect.height) } : null;
    })(),
    layout: (() => {
      const editor = document.querySelector("[data-section=video-editor]")?.getBoundingClientRect();
      const preview = document.querySelector("#video-preview")?.getBoundingClientRect();
      return {
        viewport: { w: innerWidth, h: innerHeight },
        editor: { w: Math.round(editor?.width || 0), h: Math.round(editor?.height || 0) },
        preview: { w: Math.round(preview?.width || 0), h: Math.round(preview?.height || 0) }
      };
    })()
  }));
  const failures = [];
  if (state.tab !== "video") failures.push(`expected video tab, got ${state.tab}`);
  if (state.previewReady !== "true") failures.push("preview was not ready");
  if (!state.previewText?.includes(expectedText)) failures.push("preview text did not render expected title");
  if (state.previewLayers < 1) failures.push("expected at least one preview layer");
  if (state.layout.editor.w < 1000 || state.layout.preview.w < 600) failures.push("editor/preview layout is too narrow for desktop verification");
  if (consoleEvents.some((event) => event.type === "error" || event.type === "pageerror")) failures.push("console error or pageerror was emitted");
  await fs.writeFile(
    path.join(evidence, stateName),
    `${JSON.stringify({ ...state, consoleEvents, failures, verdict: failures.length ? "failed" : "passed" }, null, 2)}\n`
  );
  if (failures.length) {
    console.error(failures.join("\n"));
    process.exitCode = 1;
  }
} finally {
  await browser.close();
  server.close();
}
