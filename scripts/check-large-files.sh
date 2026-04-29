#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

failures=""
checked=0
skipped=0

line_rule() {
  local path="$1"

  case "$path" in
    Cargo.lock|*/Cargo.lock|*.wasm|*.png|*.jpg|*.jpeg|*.mp4|*.mov|*.gif|*.webp)
      echo "skip:binary-or-lock"
      return
      ;;
    spec/*|target/*|tmp/*|vendor/*|frontend/capy-app/canvas-pkg/*)
      echo "skip:private-generated-or-vendor"
      return
      ;;
    crates/capy-recorder/assets/runtime/*|crates/capy-recorder/assets/tracks/*)
      echo "skip:runtime-asset"
      return
      ;;
  esac

  case "$path" in
    frontend/capy-app/script.js) echo "1721:continue splitting native workbench JS modules" ;;
    frontend/capy-app/styles/canvas.css) echo "450:canvas stylesheet module" ;;
    crates/capy-canvas-web/src/lib.rs) echo "1196:split wasm binding facade" ;;
    crates/capy-shell-mac/src/headless/mac.rs) echo "893:split mac headless capture/launch plumbing" ;;
    crates/capy-recorder/src/pipeline/vt_wrap.rs) echo "844:split VideoToolbox wrappers" ;;
    crates/capy-shell/src/agent.rs) echo "765:split agent stream/provider/runtime ownership" ;;
    crates/capy-recorder/src/verify_mp4.rs) echo "759:split mp4 verification probes" ;;
    crates/capy-recorder/src/cef_osr.rs) echo "755:split cef osr lifecycle/render handlers" ;;
    crates/capy-canvas-core/src/state_shapes.rs) echo "746:split canvas state shape mutations" ;;
    crates/capy-canvas-core/src/shape.rs) echo "722:split shape model and geometry helpers" ;;
    crates/capy-recorder/src/record_loop.rs) echo "717:split recorder loop phases" ;;
    crates/capy-shell/src/app.rs) echo "701:split shell event-loop services" ;;
    crates/capy-cli/src/timeline.rs) echo "671:split timeline CLI command handlers" ;;
    crates/capy-timeline-project/src/episode_compile.rs) echo "666:split episode compile phases" ;;
    crates/capy-shell/src/store.rs) echo "656:split store repositories" ;;
    crates/capy-recorder/src/snapshot.rs) echo "646:split snapshot capture/output helpers" ;;
    crates/capy-timeline-project/src/clip_composition.rs) echo "634:split clip composition builders" ;;
    crates/capy-cli/tests/timeline_cli.rs) echo "625:split timeline CLI integration tests" ;;
    crates/capy-scroll-media/src/templates.rs) echo "590:split scroll media templates" ;;
    crates/capy-cli/src/main.rs) echo "584:keep CLI root shrinking into command modules" ;;
    crates/capy-cli/src/canvas.rs) echo "575:split canvas CLI command handlers" ;;
    crates/capy-scroll-media/src/packager.rs) echo "554:split scroll media packager phases" ;;
    crates/capy-recorder/src/export_api.rs) echo "554:split export API entrypoints" ;;
    crates/capy-timeline/src/snapshot/embedded.rs) echo "519:split embedded snapshot flow" ;;
    crates/capy-recorder/src/main.rs) echo "516:split recorder binary command dispatch" ;;
    crates/capy-canvas-core/src/ui.rs) echo "504:split canvas UI orchestration" ;;
    examples/watchmaker-scroll-story/styles.css) echo "520:example style debt" ;;
    *.rs)
      case "$path" in
        crates/*/tests/*.rs|crates/*/tests/*/*.rs) echo "700:rust test file" ;;
        *) echo "500:rust source file" ;;
      esac
      ;;
    *.js|*.mjs|*.cjs) echo "450:javascript module" ;;
    *.css) echo "450:stylesheet" ;;
    *.sh|*.py) echo "400:script" ;;
    *) echo "skip:untracked-extension" ;;
  esac
}

while IFS= read -r -d '' path; do
  [[ -f "$path" ]] || continue
  rule="$(line_rule "$path")"
  if [[ "$rule" == skip:* ]]; then
    skipped=$((skipped + 1))
    continue
  fi
  cap="${rule%%:*}"
  reason="${rule#*:}"
  lines="$(wc -l < "$path" | tr -d ' ')"
  checked=$((checked + 1))
  if (( lines > cap )); then
    failures+="${path} has ${lines} lines; cap is ${cap} (${reason})"$'\n'
  fi
done < <(git ls-files -z)

if [[ -n "$failures" ]]; then
  echo "large-file check failed:" >&2
  printf '%s' "$failures" >&2
  echo "next step · split the file, or add a temporary debt cap with an owner/reason and lower it after the split." >&2
  exit 2
fi

echo "large-file check passed (${checked} checked, ${skipped} skipped)"
