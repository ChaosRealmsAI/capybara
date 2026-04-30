#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "desktop signing boundary check failed: $*" >&2
  exit 1
}

fail_guardrail() {
  local message="$1"
  local next_step="$2"
  echo "desktop signing boundary check failed: $message" >&2
  echo "next step · $next_step" >&2
  exit 2
}

require_file() {
  [[ -f "$1" ]] || fail "missing required file: $1"
}

require_file scripts/sign-capy-shell-app.sh
rg -q 'codesign --verify --deep --strict' scripts/sign-capy-shell-app.sh ||
  fail "capy-shell signing wrapper must verify before signing"
rg -q 'codesign --force --deep --sign' scripts/sign-capy-shell-app.sh ||
  fail "capy-shell signing wrapper must own force signing"
rg -q 'scripts/sign-capy-shell-app.sh "\$APP"' scripts/open-debug-shell.sh ||
  fail "open-debug-shell must use scripts/sign-capy-shell-app.sh"
rg -q 'scripts/sign-capy-shell-app.sh "\$APP"' scripts/verify-cef-shell.sh ||
  fail "verify-cef-shell must use scripts/sign-capy-shell-app.sh"

direct_codesign="$(
  rg -n 'codesign --force --deep|codesign --verify --deep' scripts \
    | rg -v '^scripts/sign-capy-shell-app.sh:' \
    | rg -v '^scripts/check-desktop-signing-boundary.sh:' || true
)"
if [[ -n "$direct_codesign" ]]; then
  echo "$direct_codesign" >&2
  fail_guardrail \
    "desktop scripts must not call codesign directly" \
    "route capy-shell.app signing through scripts/sign-capy-shell-app.sh so valid bundles are skipped"
fi
