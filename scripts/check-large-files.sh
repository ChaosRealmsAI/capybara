#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/check-large-files.sh

Use when: AI needs the tracked-file size guardrail for maintainable modules.

Required params: none.
State effects: read-only.
Pitfalls: generated/vendor/build outputs are intentionally skipped; do not raise
caps to hide product code that should be split.
Next step: split oversized files, then rerun scripts/check-commit.sh.
USAGE
  exit 0
fi

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
    *.rs)
      case "$path" in
        crates/*/tests/*.rs|crates/*/tests/*/*.rs) echo "700:rust test file" ;;
        *) echo "500:rust source file" ;;
      esac
      ;;
    *.js|*.mjs|*.cjs|*.jsx|*.ts|*.tsx) echo "450:frontend/typescript module" ;;
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
  echo "next step · split the file; do not raise caps or hide product code behind a temporary debt allowlist." >&2
  exit 2
fi

echo "large-file check passed (${checked} checked, ${skipped} skipped)"
