#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  echo "architecture check failed: $*" >&2
  exit 1
}

require_file() {
  [[ -f "$1" ]] || fail "missing required file: $1"
}

for path in \
  Cargo.toml \
  crates/capy-canvas-core/Cargo.toml \
  crates/capy-canvas-web/Cargo.toml \
  crates/capy-image-gen/Cargo.toml \
  crates/capy-image-gen/src/lib.rs \
  scripts/image-provider-apimart.mjs \
  scripts/build-canvas-for-app.sh \
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
  spec/versions/v0.6.1-image-generation-tool/status.json
do
  require_file "$path"
done

rg -q '^wef = ' Cargo.toml || fail "workspace must depend on wef for CEF/Chromium"
rg -q '^wef\.workspace = true' crates/capy-shell/Cargo.toml || fail "capy-shell must use workspace wef"
rg -q 'data-capy-browser", "cef"' crates/capy-shell/src/browser/assets.rs || fail "CEF browser identity marker missing"
rg -q '"crates/capy-canvas-core"' Cargo.toml || fail "canvas core crate must be a workspace member"
rg -q '"crates/capy-canvas-web"' Cargo.toml || fail "canvas web crate must be a workspace member"
rg -q '"crates/capy-image-gen"' Cargo.toml || fail "image generation crate must be a workspace member"
rg -q '^capy-image-gen\.workspace = true' crates/capy-cli/Cargo.toml || fail "capy-cli must depend on capy-image-gen through the workspace boundary"
rg -q 'provider-adapter' crates/capy-image-gen/src/apimart.rs || fail "first image provider must remain an adapter, not the top-level abstraction"
rg -q 'default_no_spend_gate: true' crates/capy-image-gen/src/apimart.rs || fail "image provider must expose no-spend default gate metadata"

active_version="$(jq -r '.active_version // empty' spec/versions/REGISTRY.json)"
[[ -n "$active_version" ]] || fail "spec active_version is missing"
require_file "spec/versions/$active_version/bdd.json"
require_file "spec/versions/$active_version/status.json"
jq -e '.versions[] | select(.id == "v0.4-cef-shell-poc" and .status == "merged-verified")' \
  spec/versions/REGISTRY.json >/dev/null || fail "v0.4 CEF foundation must remain registered as merged-verified"
jq -e --arg active "$active_version" '
  .versions[] | select(.id == $active) |
  (.id == "v0.4-cef-shell-poc" or ((.depends_on // []) | index("v0.4-cef-shell-poc") != null) or ((.depends_on // []) | index("v0.5-desktop-foundation-hardening") != null) or ((.depends_on // []) | index("v0.6-canvas-chat-workbench") != null))
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
