#!/usr/bin/env bash
# Build capy-canvas-web with wasm-pack and copy the pkg/ output into
# frontend/capy-app/canvas-pkg/ so the desktop shell loads the same WASM build.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

wasm-pack build crates/capy-canvas-web --target web --release

DEST="frontend/capy-app/canvas-pkg"
mkdir -p "$DEST"
# Wipe stale artifacts so a release rebuild does not leave a half-renamed
# .js or .wasm behind.
rm -f "$DEST"/*
cp crates/capy-canvas-web/pkg/* "$DEST"/
echo "canvas-pkg synced to $DEST"
