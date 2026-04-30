#!/usr/bin/env bash
# Build capy-canvas-web with wasm-pack and copy the pkg/ output into
# frontend/capy-app/canvas-pkg/ so the desktop shell loads the same WASM build.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/build-canvas-for-app.sh

Use when: AI changes capy-canvas-web or desktop frontend assets and must stage
the wasm-pack output into frontend/capy-app/canvas-pkg/.

Required params: none. Requires wasm-pack on PATH.
State effects: rebuilds wasm release package and replaces canvas-pkg contents.
Pitfalls: this mutates generated frontend assets; do not run as a proof of UI.
Next step: run scripts/check-frontend-js.sh or the full scripts/check-project.sh.
USAGE
  exit 0
fi

wasm-pack build crates/capy-canvas-web --target web --release

DEST="frontend/capy-app/canvas-pkg"
mkdir -p "$DEST"
# Wipe stale artifacts so a release rebuild does not leave a half-renamed
# .js or .wasm behind.
rm -f "$DEST"/*
cp crates/capy-canvas-web/pkg/* "$DEST"/
echo "canvas-pkg synced to $DEST"
