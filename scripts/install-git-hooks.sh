#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

hook_path="$(git rev-parse --git-path hooks/pre-commit)"
mkdir -p "$(dirname "$hook_path")"

tmp_hook="${hook_path}.tmp"
{
  printf '%s\n' '#!/usr/bin/env bash'
  printf '%s\n' 'set -euo pipefail'
  printf '%s\n' ''
  printf '%s\n' 'root="$(git rev-parse --show-toplevel)"'
  printf '%s\n' 'cd "$root"'
  printf '%s\n' ''
  printf '%s\n' 'if [[ -x scripts/check-commit.sh ]]; then'
  printf '%s\n' '  scripts/check-commit.sh'
  printf '%s\n' 'else'
  printf '%s\n' '  echo "pre-commit: scripts/check-commit.sh not found; skipping project fast gate"'
  printf '%s\n' 'fi'
} > "$tmp_hook"

mv "$tmp_hook" "$hook_path"
chmod +x "$hook_path"

echo "installed pre-commit hook: $hook_path"
