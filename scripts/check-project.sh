#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scripts/check-architecture.sh
scripts/build-canvas-for-app.sh >/dev/null
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p capy-cli -- --help >/dev/null
cargo run -p capy-cli -- agent doctor >/dev/null

echo "project check passed"
