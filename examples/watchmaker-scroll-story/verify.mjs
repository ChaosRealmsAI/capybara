#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const pageUrl = process.env.CAPY_WATCHMAKER_URL || "http://127.0.0.1:5264/index.html";
const evidenceDir =
  process.env.CAPY_WATCHMAKER_EVIDENCE_DIR ||
  "/Users/Zhuanz/workspace/capybara/spec/versions/v0.10-watchmaker-story/evidence/assets";

function loadPlaywright() {
  const candidates = [
    process.env.PLAYWRIGHT_MODULE,
    "playwright",
    "/opt/homebrew/lib/node_modules/playwright",
    "/usr/local/lib/node_modules/playwright",
  ].filter(Boolean);

  for (const candidate of candidates) {
    try {
      return require(candidate);
    } catch {
      // Try the next known install location.
    }
  }

  throw new Error(
    "Playwright runtime was not found. Install it or set PLAYWRIGHT_MODULE to the playwright package path.",
  );
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function ensureEvidenceDir() {
  fs.mkdirSync(evidenceDir, { recursive: true });
}

function watchErrors(page) {
  const events = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      events.push({ type: "console", text: message.text() });
    }
  });
  page.on("pageerror", (error) => {
    events.push({ type: "pageerror", text: error.message });
  });
  page.on("requestfailed", (request) => {
    events.push({
      type: "requestfailed",
      url: request.url(),
      text: request.failure()?.errorText || "request failed",
    });
  });
  return events;
}

async function waitForWatchmaker(page) {
  await page.goto(pageUrl, { waitUntil: "networkidle" });
  const title = await page.locator("#hero-title").textContent();
  assert(title && title.includes("Calibre Souverain"), "Hero title did not render");
  await page.waitForFunction(() => document.documentElement.dataset.watchmakerReady === "true");
}

async function readState(page) {
  return await page.evaluate(() => {
    const images = [...document.images].map((image) => ({
      src: image.getAttribute("src"),
      naturalWidth: image.naturalWidth,
      naturalHeight: image.naturalHeight,
      complete: image.complete,
    }));
    return {
      title: document.title,
      url: location.href,
      scrollY,
      viewport: { width: innerWidth, height: innerHeight },
      overflowX: document.documentElement.scrollWidth - innerWidth,
      watchmaker: window.__watchmakerState,
      imageCount: images.length,
      images,
    };
  });
}

async function verifyDesktop(browser) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 980 } });
  const errors = watchErrors(page);
  await waitForWatchmaker(page);

  const layerCount = await page.locator(".watch-layer").count();
  assert(layerCount === 6, `Expected 6 watch layers, got ${layerCount}`);
  const initialState = await readState(page);
  assert(initialState.imageCount === 7, `Expected 7 images, got ${initialState.imageCount}`);
  assert(
    initialState.images.every((image) => image.complete && image.naturalWidth > 0),
    "At least one image failed to load",
  );
  await page.screenshot({ path: path.join(evidenceDir, "watchmaker-desktop-hero.png"), fullPage: false });

  await page.getByRole("link", { name: "Enter the movement" }).click();
  await page.waitForFunction(() => window.scrollY > 200);

  await page.evaluate(() => {
    const story = document.querySelector(".assembly-story");
    window.scrollTo(0, story.offsetTop + window.innerHeight * 3.3);
  });
  await page.waitForFunction(() => window.__watchmakerState && window.__watchmakerState.currentProgress > 0.35);
  await page.waitForTimeout(650);
  const midState = await readState(page);
  assert(midState.watchmaker.activeChapter >= 2, "Active chapter did not advance by mid-scroll");
  assert(
    midState.watchmaker.layers.some((layer) => layer.progress > 0.45),
    "Layer progress did not advance by mid-scroll",
  );
  await page.screenshot({ path: path.join(evidenceDir, "watchmaker-mid-scroll.png"), fullPage: false });

  await page.evaluate(() => {
    const story = document.querySelector(".assembly-story");
    window.scrollTo(0, story.offsetTop + story.offsetHeight - window.innerHeight * 1.08);
  });
  await page.waitForFunction(() => window.__watchmakerState && window.__watchmakerState.currentProgress > 0.91);
  await page.waitForTimeout(650);
  const finalState = await readState(page);
  assert(finalState.watchmaker.activeChapter >= 5, "Active chapter did not reach the final section");
  assert(
    finalState.watchmaker.layers.every((layer) => layer.progress > 0.88),
    "Not every layer reached the assembled state",
  );
  await page.screenshot({ path: path.join(evidenceDir, "watchmaker-final-assembly.png"), fullPage: false });

  assert(errors.length === 0, `Browser errors: ${JSON.stringify(errors)}`);
  return { initialState, midState, finalState, errors };
}

async function verifyMobile(browser) {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const errors = watchErrors(page);
  await waitForWatchmaker(page);
  await page.evaluate(() => {
    const story = document.querySelector(".assembly-story");
    window.scrollTo(0, story.offsetTop + window.innerHeight * 2.4);
  });
  await page.waitForFunction(() => window.__watchmakerState && window.__watchmakerState.currentProgress > 0.24);
  await page.waitForTimeout(650);
  const mobileState = await readState(page);
  assert(mobileState.overflowX <= 1, `Mobile horizontal overflow is ${mobileState.overflowX}px`);
  assert(mobileState.watchmaker.layerCount === 6, "Mobile state did not report 6 layers");
  assert(mobileState.watchmaker.activeChapter >= 1, "Mobile active chapter did not advance");
  await page.screenshot({ path: path.join(evidenceDir, "watchmaker-mobile.png"), fullPage: false });
  assert(errors.length === 0, `Mobile browser errors: ${JSON.stringify(errors)}`);
  return { mobileState, errors };
}

async function main() {
  ensureEvidenceDir();
  const { chromium } = loadPlaywright();
  const browser = await chromium.launch({ headless: true });
  const result = {
    url: pageUrl,
    evidenceDir,
    startedAt: new Date().toISOString(),
    passed: false,
  };

  try {
    result.desktop = await verifyDesktop(browser);
    result.mobile = await verifyMobile(browser);
    result.finishedAt = new Date().toISOString();
    result.passed = true;
  } finally {
    await browser.close();
    fs.writeFileSync(
      path.join(evidenceDir, "watchmaker-verification-result.json"),
      JSON.stringify(result, null, 2),
    );
  }

  console.log(JSON.stringify(result, null, 2));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
