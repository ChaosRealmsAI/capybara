#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/lint-spec.sh

Use when: AI needs the private spec repo structure/registry lint gate.

Required params: none.

State effects: read-only. It delegates to scripts/check-spec-structure.sh.

Pitfalls: focus_version must match the current branch/worktree unless the
caller sets CAPY_FOCUS_VERSION intentionally for a parallel version check.

Next step: run scripts/check-spec-structure.sh --help for repair boundaries.
USAGE
  exit 0
fi

scripts/check-spec-structure.sh

echo "spec lint passed"
