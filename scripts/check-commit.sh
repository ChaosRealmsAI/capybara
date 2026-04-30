#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/check-commit.sh

Use when: AI needs the fast pre-commit gate for code/spec changes.

Required params: none.

State effects: mostly read-only, but cargo may write target/ build artifacts.

Pitfalls: this is not a visible UI proof; run scripts/check-project.sh and
product capture/evidence commands for final browser or desktop delivery.

Next step: fix the first failure, then rerun this gate before committing.
USAGE
  exit 0
fi

scripts/lint-spec.sh
export CAPY_SPEC_STRUCTURE_CHECKED=1
scripts/check-large-files.sh
bash -n scripts/check-code-sign-clones.sh scripts/sign-capy-shell-app.sh scripts/open-debug-shell.sh scripts/verify-cef-shell.sh scripts/verify-ai-cli-discovery.sh
scripts/check-code-sign-clones.sh
scripts/check-architecture.sh
scripts/verify-ai-cli-discovery.sh
node --test scripts/tests/*.test.mjs
RUSTC_WRAPPER= cargo test -p capy-canvas-core --all-targets
RUSTC_WRAPPER= cargo check -p capy-canvas-web --target wasm32-unknown-unknown
scripts/check-frontend-js.sh

echo "commit check passed"
