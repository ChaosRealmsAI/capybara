#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "architecture check failed: $*" >&2
  exit 1
}

fail_guardrail() {
  local message="$1"
  local next_step="$2"
  echo "architecture check failed: $message" >&2
  echo "next step · $next_step" >&2
  exit 2
}

require_file() {
  [[ -f "$1" ]] || fail "missing required file: $1"
}

check_no_nextframe_local_path() {
  local matches
  matches="$(rg -n '/Users/Zhuanz/workspace/NextFrame|NextFrame/target/debug' crates frontend scripts | rg -v 'scripts/check-architecture.sh' || true)"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "NextFrame local path must not be hardcoded in product code" \
      "move the path to --nf/--recorder, CAPY_NF/CAPY_NF_RECORDER, or private spec evidence"
  fi
}

check_nextframe_adapter_boundary() {
  local matches
  matches="$(
    rg -n 'Command::new\("nf"\)|Command::new\("nf-recorder"\)|nf_project::|nf_recorder::' crates frontend \
      | rg -v 'capy-nextframe|nextframe.rs' || true
  )"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "NextFrame process/crate calls must go through capy-nextframe" \
      "move the call behind crates/capy-nextframe or crates/capy-cli/src/nextframe.rs"
  fi
}

check_no_new_render_source_builders() {
  local matches
  matches="$(
    rg -n '"nf\.render_source\.v1"' crates \
      | rg -v 'capy-nextframe|capy-poster|tests|fixtures' || true
  )"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "render_source.v1 must be produced by NextFrame compile, not new Capybara builders" \
      "route generation through capy-nextframe or keep it in legacy capy-poster/tests/fixtures only"
  fi
}

check_no_nextframe_local_path
check_nextframe_adapter_boundary
check_no_new_render_source_builders

for path in \
  Cargo.toml \
  crates/capy-canvas-core/Cargo.toml \
  crates/capy-canvas-web/Cargo.toml \
  crates/capy-image-gen/Cargo.toml \
  crates/capy-image-gen/src/lib.rs \
  crates/capy-nextframe/Cargo.toml \
  crates/capy-nextframe/src/lib.rs \
  crates/capy-poster/Cargo.toml \
  crates/capy-poster/src/lib.rs \
  crates/capy-poster/src/component.rs \
  fixtures/poster/v0.1/sample-poster.json \
  crates/capy-scroll-media/Cargo.toml \
  crates/capy-scroll-media/src/lib.rs \
  scripts/image-provider-apimart.mjs \
  scripts/capy-focus-cutout.py \
  scripts/build-canvas-for-app.sh \
  scripts/verify-canvas-context-interface.mjs \
  scripts/verify-agent-canvas-image-placement.mjs \
  scripts/verify-poster-json-renderer.mjs \
  fixtures/poster/sample-poster.json \
  crates/capy-shell/Cargo.toml \
  crates/capy-shell/src/browser.rs \
  crates/capy-shell/src/browser/assets.rs \
  crates/capy-shell/src/browser/runtime.rs \
  scripts/verify-cef-shell.sh \
  spec/README.md \
  spec/architecture.md \
  spec/development-flow.md \
  spec/interfaces.md \
  spec/runtime.md \
  spec/versions/REGISTRY.json \
  spec/versions/v0.4-cef-shell-poc/bdd.json \
  spec/versions/v0.4-cef-shell-poc/status.json \
  spec/versions/v0.6-canvas-chat-workbench/bdd.json \
  spec/versions/v0.6-canvas-chat-workbench/status.json \
  spec/versions/v0.6.1-image-generation-tool/bdd.json \
  spec/versions/v0.6.1-image-generation-tool/status.json \
  spec/versions/v0.8-canvas-image-tool/bdd.json \
  spec/versions/v0.8-canvas-image-tool/status.json \
  spec/versions/v0.9-poster-json-renderer/bdd.json \
  spec/versions/v0.9-poster-json-renderer/status.json \
  spec/versions/v0.10-agent-canvas-image-placement/bdd.json \
  spec/versions/v0.10-agent-canvas-image-placement/status.json
do
  require_file "$path"
done

rg -q '^wef = ' Cargo.toml || fail "workspace must depend on wef for CEF/Chromium"
rg -q '^wef\.workspace = true' crates/capy-shell/Cargo.toml || fail "capy-shell must use workspace wef"
rg -q 'data-capy-browser", "cef"' crates/capy-shell/src/browser/assets.rs || fail "CEF browser identity marker missing"
rg -q '"crates/capy-canvas-core"' Cargo.toml || fail "canvas core crate must be a workspace member"
rg -q '"crates/capy-canvas-web"' Cargo.toml || fail "canvas web crate must be a workspace member"
rg -q '"crates/capy-image-gen"' Cargo.toml || fail "image generation crate must be a workspace member"
rg -q '"crates/capy-nextframe"' Cargo.toml || fail "NextFrame adapter crate must be a workspace member"
rg -q '"crates/capy-poster"' Cargo.toml || fail "poster adapter crate must be a workspace member"
rg -q '"crates/capy-scroll-media"' Cargo.toml || fail "scroll media crate must be a workspace member"
rg -q '^capy-image-gen\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-image-gen through the workspace boundary"
rg -q '^capy-nextframe\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-nextframe through the workspace boundary"
rg -q '^capy-poster\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-poster through the workspace boundary"
rg -q '^capy-image-gen\.workspace = true' crates/capy-shell/Cargo.toml || fail "capy-shell must use capy-image-gen for desktop canvas tool calls"
rg -q '^capy-scroll-media\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-scroll-media through the workspace boundary"
rg -q 'CanvasCommand::GenerateImage' crates/capy-cli/src/canvas.rs || fail "capy canvas generate-image command must exist"
rg -q 'CanvasCommand::LoadPoster' crates/capy-cli/src/canvas.rs || fail "capy canvas load-poster command must exist"
rg -q 'CanvasContentKind::Poster' crates/capy-canvas-core/src crates/capy-canvas-web/src || fail "poster content kind must exist in canvas core/web boundary"
rg -q 'create_poster_document_card' crates/capy-canvas-core/src crates/capy-canvas-web/src frontend/capy-app/script.js || fail "poster document card creation must cross Rust/WASM/frontend boundary"
rg -q 'poster-overlay-layer' frontend/capy-app/index.html frontend/capy-app/script.js frontend/capy-app/style.css || fail "poster renderer must use current canvas overlay layer"
rg -q 'verifyPosterRenderer' frontend/capy-app/script.js scripts/verify-poster-json-renderer.mjs || fail "poster renderer visible verification hook must exist"
rg -q 'CanvasCommand::Context' crates/capy-cli/src/canvas.rs || fail "capy canvas context export command must exist"
rg -q 'context export --selected' crates/capy-shell/src/agent_tools.rs scripts/verify-canvas-context-interface.mjs || fail "agent canvas context contract must require context export"
rg -q 'canvas_context' crates/capy-cli/src/main.rs crates/capy-shell/src/app.rs crates/capy-shell/src/agent.rs frontend/capy-app/script.js || fail "chat messages must persist canvas_context metadata"
rg -q 'canvas-generate-image' crates/capy-shell/src/app.rs || fail "desktop canvas image tool RPC must exist"
rg -q 'capyCanvasTools' crates/capy-cli/src/main.rs crates/capy-shell/src/agent.rs frontend/capy-app/script.js || fail "agent canvas CLI tool contract must be wired for chat runtimes"
rg -q 'CAPY_TOOL_CALL_LOG' crates/capy-cli/src/canvas.rs crates/capy-shell/src/agent.rs || fail "agent canvas CLI calls must support JSONL evidence logging"
rg -q 'v0.10-agent-canvas-image-placement' scripts/verify-agent-canvas-image-placement.mjs || fail "agent canvas placement verifier must target v0.10 evidence"
rg -q 'provider-adapter' crates/capy-image-gen/src/apimart.rs || fail "first image provider must remain an adapter, not the top-level abstraction"
rg -q 'default_no_spend_gate: true' crates/capy-image-gen/src/apimart.rs || fail "image provider must expose no-spend default gate metadata"
rg -q 'http_range' crates/capy-scroll-media/src/types.rs || fail "scroll media manifest must record HTTP Range requirement"
rg -q '206' crates/capy-scroll-media/src/range_server.rs || fail "scroll media server must support HTTP 206 Partial Content"
rg -q 'capy-multi-video-scroll-story' crates/capy-scroll-media/src/types.rs || fail "multi-video story manifest kind must be explicit"
rg -q 'StoryPack' crates/capy-cli/src/media.rs || fail "capy media story-pack CLI must remain wired"
rg -q 'Poster\(poster::PosterArgs\)' crates/capy-cli/src/main.rs || fail "capy poster CLI must remain wired"
rg -q 'Nextframe\(nextframe::NextFrameArgs\)' crates/capy-cli/src/main.rs || fail "capy nextframe CLI must remain wired"
rg -q 'nf.render_source.v1' crates/capy-poster/src/render_source.rs || fail "poster adapter must compile to NextFrame render_source.v1"
rg -q 'capy.poster-document' crates/capy-poster/src/component.rs crates/capy-poster/src/render_source.rs || fail "poster adapter must use an explicit poster component id"
if rg -n '/Users/Zhuanz/workspace/NextFrame|NextFrame/target/debug/nf-recorder' crates fixtures; then
  fail "poster adapter must not hardcode a local NextFrame checkout or recorder path"
fi
rg -q 'withoutbg/focus' crates/capy-cli/src/main.rs crates/capy-cli/src/cutout.rs scripts/capy-focus-cutout.py || fail "cutout CLI must use withoutbg/focus"
rg -q 'CutoutCommand::Run' crates/capy-cli/src/cutout.rs || fail "capy cutout run command must remain wired"
rg -q 'CutoutCommand::Batch' crates/capy-cli/src/cutout.rs || fail "capy cutout batch command must remain wired"
if rg -n 'CutoutRequest|cutout::execute|flood|tolerance|feather_radius|hole_min_area|DEFAULT_BACKGROUND|fixed-background' \
  crates/capy-cli/src/main.rs crates/capy-cli/src/cutout.rs; then
  fail "cutout CLI must not expose the old fixed-background algorithm"
fi

active_version="$(jq -r '.active_version // empty' spec/versions/REGISTRY.json)"
[[ -n "$active_version" ]] || fail "spec active_version is missing"
require_file "spec/versions/$active_version/bdd.json"
require_file "spec/versions/$active_version/status.json"
jq -e '.versions[] | select(.id == "v0.4-cef-shell-poc" and .status == "merged-verified")' \
  spec/versions/REGISTRY.json >/dev/null || fail "v0.4 CEF foundation must remain registered as merged-verified"
jq -e --arg active "$active_version" '
  . as $registry |
  def deps($id):
    ($registry.versions[] | select(.id == $id) | (.depends_on // [])) as $direct
    | $direct + ([$direct[]? | deps(.)] | add // []);
  ($active == "v0.4-cef-shell-poc") or
  ((deps($active) | unique) as $deps |
    ($deps | index("v0.4-cef-shell-poc") != null) or
    ($deps | index("v0.5-desktop-foundation-hardening") != null) or
    ($deps | index("v0.6-canvas-chat-workbench") != null))
' spec/versions/REGISTRY.json >/dev/null || fail "active version must be v0.4 CEF foundation or depend on the CEF desktop foundation chain"

if rg -n '\bwry\b|objc2-web-kit|javascriptcore|WKWebView|WebKit' \
  Cargo.toml Cargo.lock crates/capy-shell/Cargo.toml crates/capy-shell/src; then
  fail "desktop mainline must not reintroduce wry/WebKit dependencies"
fi

if rg -n -i '\b(electron|tailwind|shadcn|react|vue|next\.js|nextjs)\b' \
  Cargo.toml crates frontend; then
  fail "forbidden product framework dependency found"
fi

check_rust_file_size() {
  local file="$1"
  local lines
  lines="$(wc -l < "$file" | tr -d ' ')"
  case "$file" in
    crates/capy-canvas-web/src/lib.rs)
      [[ "$lines" -le 1200 ]] || fail "$file has $lines lines; split wasm adapter before crossing 1200"
      ;;
    *)
      [[ "$lines" -le 900 ]] || fail "$file has $lines lines; split module before crossing 900"
      ;;
  esac
}

while IFS= read -r file; do
  check_rust_file_size "$file"
done < <(find crates -path '*/src/*.rs' -type f | sort)

echo "architecture check passed"
