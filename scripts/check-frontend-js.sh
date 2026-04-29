#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

while IFS= read -r js_file; do
  node --input-type=module --check < "$js_file" >/dev/null
done < <(find frontend/capy-app -path 'frontend/capy-app/canvas-pkg' -prune -o -name '*.js' -print | sort)

echo "frontend js syntax check passed"
