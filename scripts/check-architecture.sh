#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${CAPY_SPEC_STRUCTURE_CHECKED:-0}" != "1" ]]; then
  scripts/check-spec-structure.sh
fi

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

check_no_external_timeline_engine() {
  local matches
  matches="$(rg -n 'external/Timeline|/Users/Zhuanz/workspace/Timeline|Timeline/target/debug' Cargo.toml crates frontend scripts README.md AGENTS.md CLAUDE.md | rg -v 'scripts/check-architecture.sh' || true)"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "Timeline engine code must live inside Capybara, not external/Timeline" \
      "move required code into crates/capy-* and use Capybara path dependencies"
  fi
}

check_timeline_engine_dependency_boundary() {
  local matches
  matches="$(
    rg -n 'Command::new\("nf"\)|Command::new\("nf-recorder"\)|\bnf_project::|\bnf_recorder::|nf-shell-mac|nf-recorder|nf-project|CAPY_NF' crates frontend scripts \
      | rg -v 'scripts/check-architecture.sh' || true
  )"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "Timeline engine dependencies must use Capybara-owned crate names" \
      "rename to capy-timeline-project, capy-recorder, CAPY_RECORDER, or typed in-process adapters"
  fi
}

check_no_new_render_source_builders() {
  local matches
  matches="$(
    rg -n '"capy\.timeline\.render_source\.v1"' crates \
      | rg -v 'capy-timeline|capy-recorder|capy-timeline-project|tests|fixtures' || true
  )"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "render_source.v1 must be produced by Timeline compile, not new Capybara builders" \
      "route generation through capy-timeline only"
  fi
}

check_no_old_timeline_compat_surface() {
  local matches
  matches="$(rg -n 'NextFrame|nextframe|legacy_nextframe|CAPY_NEXTFRAME|OP_NEXTFRAME|KIND_NEXTFRAME|attachNextFrame|openNextFrame|name = "nextframe"|capy nextframe' crates frontend scripts README.md AGENTS.md CLAUDE.md | rg -v 'scripts/check-architecture.sh' || true)"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "old Timeline product/compat surface must not remain" \
      "use Timeline/Capybara names only; no legacy CLI command, old IPC aliases, or old frontend aliases"
  fi
}

check_no_legacy_poster_render_source() {
  local matches
  matches="$(rg -n 'compile_render_source|write_render_source' crates || true)"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "capy-poster legacy render_source builders must be removed" \
      "delete legacy render_source APIs and route poster composition through capy timeline compose-poster"
  fi
}

check_no_binary_adapter() {
  local matches
  matches="$(rg -n 'BinaryAdapter|adapter/binary' crates || true)"
  if [[ -n "$matches" ]]; then
    echo "$matches" >&2
    fail_guardrail \
      "Timeline binary adapter path must be removed" \
      "delete BinaryAdapter references and keep in-process Capybara engine adapters only"
  fi
}

check_v15_contract_boundary() {
  rg -q '"crates/capy-contracts"' Cargo.toml || fail "capy-contracts must be a workspace member"
  rg -q '"crates/capy-creative-core"' Cargo.toml || fail "capy-creative-core must be a workspace member"
  rg -q '"crates/capy-timeline-project"' Cargo.toml || fail "capy-timeline-project must be a workspace member"
  rg -F -q 'capy-recorder = { path = "../capy-recorder"' crates/capy-timeline/Cargo.toml || fail "capy-timeline must depend on migrated capy-recorder"
  rg -q 'capy-timeline-project.workspace = true' crates/capy-timeline/Cargo.toml || fail "capy-timeline must use migrated capy-timeline-project"
  rg -q '^capy-contracts\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-contracts"
  rg -q '^capy-contracts\.workspace = true' crates/capy-shell/Cargo.toml || fail "capy-shell must depend on capy-contracts"
  rg -q 'pub struct IpcRequest' crates/capy-contracts/src/ipc.rs || fail "IpcRequest contract must live in capy-contracts"
  rg -q 'pub struct IpcResponse' crates/capy-contracts/src/ipc.rs || fail "IpcResponse contract must live in capy-contracts"
  rg -q 'OP_TIMELINE_ATTACH' crates/capy-contracts/src/timeline.rs || fail "Timeline live IPC contracts must live in capy-contracts"
  rg -q 'TrackKind::Tts' crates/capy-creative-core/src/lib.rs || fail "Creative core must reserve TTS track contract"

  local duplicate_ipc
  duplicate_ipc="$(rg -n 'pub struct Ipc(Request|Response)' crates/capy-cli/src crates/capy-shell/src || true)"
  if [[ -n "$duplicate_ipc" ]]; then
    echo "$duplicate_ipc" >&2
    fail_guardrail \
      "CLI and shell must not redefine IPC wire structs" \
      "use capy_contracts::ipc::{IpcRequest, IpcResponse}"
  fi
}

check_v16_tts_clips_boundary() {
  for member in \
    '"crates/capy-tts"' \
    '"crates/capy-clips-core"' \
    '"crates/capy-clips-download"' \
    '"crates/capy-clips-transcribe"' \
    '"crates/capy-clips-align"' \
    '"crates/capy-clips-cut"'; do
    rg -q "$member" Cargo.toml || fail "v0.16 workspace member missing: $member"
  done

  for dep in \
    '^capy-tts\.workspace = true' \
    '^capy-clips-core\.workspace = true' \
    '^capy-clips-download\.workspace = true' \
    '^capy-clips-transcribe\.workspace = true' \
    '^capy-clips-align\.workspace = true' \
    '^capy-clips-cut\.workspace = true'; do
    rg -q "$dep" crates/capy-cli/Cargo.toml || fail "capy-cli missing v0.16 dependency: $dep"
  done

  rg -q 'Command::Tts' crates/capy-cli/src/main.rs || fail "capy tts CLI must remain wired"
  rg -q 'Command::Clips' crates/capy-cli/src/main.rs || fail "capy clips CLI must remain wired"
  rg -q 'CAPY_CLIPS_WHISPER_SCRIPT' crates/capy-clips-transcribe/src/lib.rs crates/capy-cli/src/clips.rs ||
    fail "clips transcription must use Capybara-owned helper env vars"
  rg -q 'CAPY_CLIPS_ALIGN_SCRIPT' crates/capy-clips-align/src/script.rs crates/capy-cli/src/clips.rs ||
    fail "clips alignment must use Capybara-owned helper env vars"
  rg -q 'CAPY_TTS_ALIGN_SCRIPT' crates/capy-tts/src/whisper ||
    fail "TTS alignment must use Capybara-owned helper env vars"

  local legacy_matches
  legacy_matches="$(
    rg -n 'nf-tts|nf-source|VIDEOCUT_|VOX_ALIGN_SCRIPT|\.vox-cache|name = "vox"|capy cue' \
      Cargo.toml crates \
      | rg -v 'target/' || true
  )"
  if [[ -n "$legacy_matches" ]]; then
    echo "$legacy_matches" >&2
    fail_guardrail \
      "TTS and clips migration must not expose old NextFrame/standalone product names" \
      "route through capy tts / capy clips and Capybara-owned CAPY_* environment variables"
  fi
}

check_no_external_timeline_engine
check_timeline_engine_dependency_boundary
check_no_new_render_source_builders
check_no_old_timeline_compat_surface
check_no_legacy_poster_render_source
check_no_binary_adapter
check_v15_contract_boundary
check_v16_tts_clips_boundary

for path in \
  Cargo.toml \
  crates/capy-contracts/Cargo.toml \
  crates/capy-contracts/src/lib.rs \
  crates/capy-contracts/src/ipc.rs \
  crates/capy-contracts/src/timeline.rs \
  crates/capy-creative-core/Cargo.toml \
  crates/capy-creative-core/src/lib.rs \
  crates/capy-timeline-project/Cargo.toml \
  crates/capy-timeline-project/src/lib.rs \
  crates/capy-recorder/Cargo.toml \
  crates/capy-recorder/src/lib.rs \
  crates/capy-shell-mac/Cargo.toml \
  crates/capy-shell-mac/src/lib.rs \
  crates/capy-canvas-core/Cargo.toml \
  crates/capy-canvas-web/Cargo.toml \
  crates/capy-image-gen/Cargo.toml \
  crates/capy-image-gen/src/lib.rs \
  crates/capy-timeline/Cargo.toml \
  crates/capy-timeline/src/lib.rs \
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
  frontend/capy-app/workbench/geometry.js \
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
rg -q '"crates/capy-timeline-project"' Cargo.toml || fail "timeline project engine crate must be a workspace member"
rg -q '"crates/capy-timeline"' Cargo.toml || fail "Timeline crate must be a workspace member"
rg -q '"crates/capy-poster"' Cargo.toml || fail "poster adapter crate must be a workspace member"
rg -q '"crates/capy-scroll-media"' Cargo.toml || fail "scroll media crate must be a workspace member"
rg -q '^capy-image-gen\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-image-gen through the workspace boundary"
rg -q '^capy-timeline\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-timeline through the workspace boundary"
rg -q '^capy-image-gen\.workspace = true' crates/capy-shell/Cargo.toml || fail "capy-shell must use capy-image-gen for desktop canvas tool calls"
rg -q '^capy-scroll-media\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-scroll-media through the workspace boundary"
rg -q 'CanvasCommand::GenerateImage' crates/capy-cli/src/canvas.rs || fail "capy canvas generate-image command must exist"
rg -q 'CanvasCommand::LoadPoster' crates/capy-cli/src/canvas.rs || fail "capy canvas load-poster command must exist"
rg -q 'CanvasContentKind::Poster' crates/capy-canvas-core/src crates/capy-canvas-web/src || fail "poster content kind must exist in canvas core/web boundary"
rg -q 'create_poster_document_card' crates/capy-canvas-core/src crates/capy-canvas-web/src frontend/capy-app/script.js || fail "poster document card creation must cross Rust/WASM/frontend boundary"
rg -q 'poster-overlay-layer' frontend/capy-app/index.html frontend/capy-app/script.js frontend/capy-app/style.css frontend/capy-app/styles frontend/capy-app/app || fail "poster renderer must use current canvas overlay layer"
rg -q 'verifyPosterRenderer' frontend/capy-app/script.js frontend/capy-app/app scripts/verify-poster-json-renderer.mjs || fail "poster renderer visible verification hook must exist"
rg -q 'CanvasCommand::Context' crates/capy-cli/src/canvas.rs || fail "capy canvas context export command must exist"
rg -q 'context export --selected' crates/capy-shell/src/agent_tools.rs scripts/verify-canvas-context-interface.mjs || fail "agent canvas context contract must require context export"
rg -q 'canvas_context' crates/capy-cli/src/main.rs crates/capy-cli/src/chat.rs crates/capy-shell/src/app.rs crates/capy-shell/src/app crates/capy-shell/src/agent.rs frontend/capy-app/script.js frontend/capy-app/app || fail "chat messages must persist canvas_context metadata"
rg -q 'canvas-generate-image' crates/capy-shell/src/app.rs crates/capy-shell/src/app || fail "desktop canvas image tool RPC must exist"
rg -q 'capyCanvasTools' crates/capy-cli/src/main.rs crates/capy-cli/src/chat.rs crates/capy-shell/src/agent.rs frontend/capy-app/script.js frontend/capy-app/app || fail "agent canvas CLI tool contract must be wired for chat runtimes"
rg -q 'CAPY_TOOL_CALL_LOG' crates/capy-cli/src/canvas.rs crates/capy-shell/src/agent.rs || fail "agent canvas CLI calls must support JSONL evidence logging"
rg -q 'v0.10-agent-canvas-image-placement' scripts/verify-agent-canvas-image-placement.mjs || fail "agent canvas placement verifier must target v0.10 evidence"
rg -q 'provider-adapter' crates/capy-image-gen/src/apimart.rs || fail "first image provider must remain an adapter, not the top-level abstraction"
rg -q 'default_no_spend_gate: true' crates/capy-image-gen/src/apimart.rs || fail "image provider must expose no-spend default gate metadata"
rg -q 'http_range' crates/capy-scroll-media/src/types.rs || fail "scroll media manifest must record HTTP Range requirement"
rg -q '206' crates/capy-scroll-media/src/range_server.rs || fail "scroll media server must support HTTP 206 Partial Content"
rg -q 'capy-multi-video-scroll-story' crates/capy-scroll-media/src/types.rs || fail "multi-video story manifest kind must be explicit"
rg -q 'StoryPack' crates/capy-cli/src/media.rs || fail "capy media story-pack CLI must remain wired"
rg -q 'Timeline\(timeline::TimelineArgs\)' crates/capy-cli/src/main.rs || fail "capy timeline CLI must remain wired"
if rg -n 'Nextframe\(timeline::TimelineArgs\)|name = "nextframe"|capy nextframe' crates/capy-cli/src scripts README.md AGENTS.md CLAUDE.md | rg -v 'scripts/check-architecture.sh'; then
  fail "capy nextframe legacy alias must not remain wired"
fi
rg -q 'capy.poster-document' crates/capy-poster/src/component.rs || fail "poster component id must remain explicit"
if rg -n '/Users/Zhuanz/workspace/NextFrame|NextFrame/target/debug|external/NextFrame' crates fixtures Cargo.toml; then
  fail "Timeline adapter must not depend on external Timeline paths"
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

if rg -n '\bwry\b|javascriptcore|WKWebView|WebKit' Cargo.toml Cargo.lock; then
  fail "desktop mainline must not reintroduce wry/WebKit dependencies"
fi

if rg -n '\bwry\b|objc2-web-kit|javascriptcore|WKWebView|WebKit' \
  Cargo.toml crates/capy-shell/Cargo.toml crates/capy-shell/src; then
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
