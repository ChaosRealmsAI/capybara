#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const versionDir = path.resolve(process.argv[2] || "spec/versions/v0.40-remotion-video-loop");
const assetsDir = process.env.CAPY_VERIFY_ASSETS
  ? path.resolve(process.env.CAPY_VERIFY_ASSETS)
  : path.join(versionDir, "evidence", "assets");
const pocDir = path.join(root, "spec", "pocs", "remotion-composition-contract");
const capy = path.join(root, "target", "debug", "capy");
const storyboardPath = path.join(root, "fixtures", "project", "html-context", "video", "storyboard.json");
const componentSourcePath = path.join(root, "fixtures", "timeline", "video-editing", "components", "html.capy-title.js");
const compositionPath = path.join(pocDir, "composition.json");
const summaryPath = path.join(pocDir, "contract-summary.json");
const compileEvidencePath = path.join(assetsDir, "remotion-poc-compile.json");
const renderSourceEvidencePath = path.join(assetsDir, "remotion-poc-render-source.json");

mkdirSync(assetsDir, { recursive: true });
mkdirSync(pocDir, { recursive: true });

const report = {
  ok: false,
  started_at: new Date().toISOString(),
  poc_dir: pocDir,
  storyboard_path: storyboardPath,
  composition_path: compositionPath,
  commands: [],
  checks: {}
};

try {
  assert(existsSync(capy), `missing CLI binary: ${capy}`);
  const storyboard = JSON.parse(readFileSync(storyboardPath, "utf8"));
  mkdirSync(path.join(pocDir, "components"), { recursive: true });
  cpSync(componentSourcePath, path.join(pocDir, "components", "html.capy-title.js"));
  const composition = storyboardToComposition(storyboard);
  writeJson(compositionPath, composition);
  writeJson(summaryPath, {
    schema: "capy.remotion_contract_poc.v1",
    adopted_runtime: "capy.timeline.composition.v2",
    remotion_boundary: "POC proves data-to-composition contract only; product Remotion UI/export remains blocked behind v0.38 and v0.39.",
    source_storyboard: "fixtures/project/html-context/video/storyboard.json",
    scene_count: storyboard.scenes.length,
    duration_ms: storyboard.duration_ms,
    composition_path: "spec/pocs/remotion-composition-contract/composition.json"
  });

  report.checks.validate = capyJson(["timeline", "validate", "--composition", compositionPath]);
  assert(report.checks.validate.ok === true, "composition validate must pass");
  assert(report.checks.validate.schema_version === "capy.composition.v2", "composition schema must be v2");
  assert(report.checks.validate.components.includes("html.capy-title"), "composition must use html.capy-title component");

  report.checks.compile = capyJson(["timeline", "compile", "--composition", compositionPath]);
  assert(report.checks.compile.ok === true, "composition compile must pass");
  assert(report.checks.compile.render_source_schema === "capy.timeline.render_source.v1", "compile must emit render_source.v1");
  assert(Number(report.checks.compile.duration_ms) === storyboard.duration_ms, "compiled duration must match storyboard");
  assert(Number(report.checks.compile.track_count) === storyboard.scenes.length, "compiled track count must match scenes");

  const renderSourcePath = path.resolve(root, report.checks.compile.render_source_path);
  const renderSource = JSON.parse(readFileSync(renderSourcePath, "utf8"));
  writeJson(renderSourceEvidencePath, renderSource);
  report.checks.render_source = {
    schema_version: renderSource.schema_version,
    duration_ms: renderSource.duration_ms,
    track_count: renderSource.tracks?.length || 0,
    component_count: Object.keys(renderSource.components || {}).length
  };
  assert(report.checks.render_source.track_count === storyboard.scenes.length, "render source must keep one component track per scene");

  report.ok = true;
  report.finished_at = new Date().toISOString();
  writeJson(compileEvidencePath, report);
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  report.ok = false;
  report.error = error instanceof Error ? error.message : String(error);
  report.finished_at = new Date().toISOString();
  writeJson(compileEvidencePath, report);
  console.error(JSON.stringify(report, null, 2));
  process.exit(1);
}

function storyboardToComposition(storyboard) {
  return {
    schema: "capy.timeline.composition.v2",
    schema_version: "capy.composition.v2",
    id: "remotion-contract-poc",
    name: "Remotion Contract POC",
    viewport: { w: 1920, h: 1080, ratio: "16:9" },
    theme: "default",
    export: { resolution: "1080p" },
    clips: storyboard.scenes.map((scene, index) => sceneToClip(scene, index))
  };
}

function sceneToClip(scene, index) {
  const duration = Math.max(1, Number(scene.end_ms) - Number(scene.start_ms));
  return {
    id: scene.id,
    name: scene.text,
    duration: `${duration}ms`,
    tracks: [
      {
        id: "stage",
        kind: "component",
        component: "html.capy-title",
        z: 0,
        items: [
          {
            id: `${scene.id}-title`,
            time: { start: "in", end: "out" },
            params: {
              eyebrow: "CAPYBARA REMOTION POC",
              title: scene.text,
              subtitle: `Scene ${index + 1} · ${scene.start_ms}-${scene.end_ms}ms`,
              status: "contract only",
              metric: `${duration}ms`,
              clip: scene.id,
              accent: ["lavender", "mint", "coral"][index % 3]
            }
          }
        ]
      }
    ]
  };
}

function capyJson(args) {
  const started = Date.now();
  const stdout = execFileSync(capy, args, {
    cwd: root,
    env: process.env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    maxBuffer: 24 * 1024 * 1024
  });
  const parsed = JSON.parse(stdout);
  report.commands.push({
    cmd: ["target/debug/capy", ...args].join(" "),
    elapsed_ms: Date.now() - started,
    output_summary: summarize(parsed)
  });
  return parsed;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function summarize(value) {
  if (value && typeof value === "object") {
    const keys = Object.keys(value).slice(0, 8);
    return Object.fromEntries(keys.map((key) => [key, value[key]]));
  }
  return value;
}

function writeJson(file, value) {
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}
