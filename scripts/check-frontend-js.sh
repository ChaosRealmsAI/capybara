#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

while IFS= read -r js_file; do
  node --input-type=module --check < "$js_file" >/dev/null
done < <(find frontend/capy-app -path 'frontend/capy-app/canvas-pkg' -prune -o -name '*.js' -print | sort)

node scripts/verify-runtime-models.mjs >/dev/null
node scripts/verify-planner-message-whitelist.mjs >/dev/null

require_text() {
  local file="$1"
  local needle="$2"
  if ! rg -F -- "$needle" "$file" >/dev/null; then
    echo "missing frontend guard '$needle' in $file" >&2
    exit 1
  fi
}

require_canvas_tool() {
  local tool="$1"
  require_text frontend/capy-app/index.html "data-canvas-tool=\"$tool\""
}

require_text frontend/capy-app/script.js "setPlannerMessages"
require_text frontend/capy-app/script.js "setPlannerStreaming"
require_text frontend/capy-app/script.js "setPlannerRunStatus"
require_text frontend/capy-app/app/conversations.js "if (isRunning) messagesEl.append(loadingMessageNode())"
require_text frontend/capy-app/app/html-preview-renderer.js "html-artifact"
require_text frontend/capy-app/app/html-preview-renderer.js "html-source"
require_text frontend/capy-app/app/html-preview-renderer.js "allow-same-origin"
require_text frontend/capy-app/styles/planner.css "has-html-artifact"
require_text frontend/capy-app/styles/planner.css ".message.is-loading .bubble"
require_text frontend/capy-app/index.html 'value="/fixtures/poster/v1/single-poster.json"'
require_text frontend/capy-app/index.html 'id="poster-save-json"'
require_text frontend/capy-app/index.html 'id="poster-export-pptx"'
require_text frontend/capy-app/app/poster-workspace.js "function ensureDefaultDocument()"
require_text frontend/capy-app/app/poster-workspace.js '"poster-document-save"'
require_text frontend/capy-app/app/poster-workspace.js '"poster-document-export"'
require_text frontend/capy-app/app/video-editor.js "ensurePosterDocument && ensurePosterDocument()"
require_text crates/capy-shell/src/browser/assets.rs "fn workspace_fixture_response"
require_text crates/capy-poster/src/v1/export.rs "capy.poster.export.v1"
require_text frontend/capy-app/app/component-runtime.js "async function loadComponent"
require_text frontend/capy-app/app/component-runtime.js "resolveComponentDefinition"
require_text frontend/capy-app/app/poster-preview.js "componentRuntime.loadComponent"
require_text frontend/capy-app/app/video-preview.js "componentRuntime.loadComponent"
require_text crates/capy-components/src/lib.rs "capy.component.v1"
require_text crates/capy-cli/src/component.rs "capy.component.validation.v1"

require_text frontend/capy-app/index.html 'data-component="canvas-tool-bar"'
require_text frontend/capy-app/index.html 'class="canvas-bottom canvas-tool-island"'
require_text frontend/capy-app/index.html 'data-tool-group="navigation"'
require_text frontend/capy-app/index.html 'data-tool-group="shapes"'
require_text frontend/capy-app/index.html 'data-tool-group="lines"'
require_text frontend/capy-app/index.html 'data-tool-group="edit"'
require_text frontend/capy-app/index.html 'data-tool-group="style"'
require_text frontend/capy-app/styles/canvas-controls.css '--canvas-tool-size: 36px'
require_text frontend/capy-app/styles/canvas-controls.css 'left: 50%; top: 16px'
require_text frontend/capy-app/styles/canvas-controls.css 'transform: translateX(-50%)'
for tool in select rect ellipse triangle diamond line arrow freehand highlighter eraser lasso; do
  require_canvas_tool "$tool"
done
require_text frontend/capy-app/index.html 'id="canvas-mini-map"'
require_text frontend/capy-app/index.html 'data-canvas-zoom="fit"'
require_text frontend/capy-app/index.html 'data-canvas-color'

canvas_toolbar="$(sed -n '/data-component="canvas-tool-bar"/,/id="canvas-mini-map"/p' frontend/capy-app/index.html)"
if printf '%s\n' "$canvas_toolbar" | rg 'data-canvas-tool="(sticky_note|text)"' >/dev/null; then
  echo "canvas primary toolbar must stay Excalidraw-like; sticky/text belong outside the primary drawing toolbar" >&2
  exit 1
fi
top_canvas_toolbar="$(sed -n '/data-component="canvas-toolbar"/,/data-component="canvas-tool-bar"/p' frontend/capy-app/index.html)"
if printf '%s\n' "$top_canvas_toolbar" | rg 'data-canvas-tool=|>Select<|>Node<|>Link<' >/dev/null; then
  echo "top canvas toolbar must stay status/region-only; drawing tools and semantic Node/Link shortcuts belong outside it" >&2
  exit 1
fi

if rg -n 'Brand Kit|主视觉候选 A|Landing Draft|Storyboard|Seed demo nodes|data-cmd="seed-demo"' frontend/capy-app >/dev/null; then
  echo "default frontend must not ship demo Brand/Image/Web/Storyboard placeholders" >&2
  exit 1
fi

if rg -n 'export_png|perform_png_export|export_requested|canvas\.png|Export PNG|GPU readback|map_async' \
  crates/capy-canvas-core crates/capy-canvas-web frontend/capy-app/app frontend/capy-app/index.html >/dev/null; then
  echo "v0.24 canvas export must stay SVG/vector-first; remove PNG/GPU readback canvas export paths" >&2
  exit 1
fi

if rg -n '导出入口已连接|后续 shell adapter' frontend/capy-app/app/poster-workspace.js frontend/capy-app/index.html >/dev/null; then
  echo "poster export buttons must call real save/export IPC, not placeholder status copy" >&2
  exit 1
fi

seed_demo_body="$(sed -n '/function seedDemoCanvas()/,/^}/p' frontend/capy-app/app/canvas-workbench.js)"
if printf '%s\n' "$seed_demo_body" | rg 'create_content_card|loadPosterDocument|selectNode' >/dev/null; then
  echo "seedDemoCanvas must not create or select demo content; clean canvas starts as SVG/vector drawing only" >&2
  exit 1
fi

require_text frontend/capy-app/script.js "set_tool"
require_text frontend/capy-app/script.js "set_vector_style"
require_text frontend/capy-app/script.js "center_view_on"
require_text frontend/capy-app/script.js "zoom_view_at"
require_text frontend/capy-app/script.js "pan_view_by"
require_text frontend/capy-app/script.js "fit_view_to_content"
require_text frontend/capy-app/script.js "create_project_artifact_card"
require_text frontend/capy-app/script.js "resize_node_by_id"
require_text frontend/capy-app/app/project-artifact-nodes.js '"project-surface-nodes"'
require_text frontend/capy-app/app/project-artifact-nodes.js '"project-surface-node-update"'
require_text frontend/capy-app/app/canvas-controls.js 'miniMapEl?.addEventListener("pointerdown"'
require_text frontend/capy-app/app/canvas-controls.js "set_vector_style"
require_text frontend/capy-app/app/canvas-controls.js "zoom_view_at"
require_text frontend/capy-app/app/canvas-controls.js "center_view_on"
require_text crates/capy-canvas-web/src/web/exports.rs "pub fn set_tool"
require_text crates/capy-canvas-web/src/web/exports.rs "pub fn set_vector_style"
require_text crates/capy-canvas-web/src/web/exports.rs "pub fn center_view_on"
require_text crates/capy-canvas-web/src/web/viewport.rs "pub fn zoom_view_at"
require_text crates/capy-canvas-web/src/web/viewport.rs "pub fn pan_view_by"
require_text crates/capy-canvas-web/src/web/viewport.rs "pub fn fit_view_to_content"

echo "frontend js syntax check passed"
