#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scripts/lint-spec.sh
export CAPY_SPEC_STRUCTURE_CHECKED=1
scripts/check-large-files.sh
scripts/check-code-sign-clones.sh
scripts/check-architecture.sh
RUSTC_WRAPPER= cargo test -p capy-canvas-core --all-targets
RUSTC_WRAPPER= cargo check -p capy-canvas-web --target wasm32-unknown-unknown
scripts/check-frontend-js.sh

echo "commit check passed"
