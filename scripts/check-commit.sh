#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scripts/lint-spec.sh
export CAPY_SPEC_STRUCTURE_CHECKED=1
scripts/check-large-files.sh
scripts/check-architecture.sh
scripts/check-frontend-js.sh

echo "commit check passed"
