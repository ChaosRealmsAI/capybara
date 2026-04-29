#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scripts/check-large-files.sh
scripts/check-architecture.sh
scripts/check-frontend-js.sh

echo "commit check passed"
