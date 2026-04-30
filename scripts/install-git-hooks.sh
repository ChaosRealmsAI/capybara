#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/install-git-hooks.sh

Use when: AI needs to install the repository pre-commit hook that runs
scripts/check-commit.sh.

Required params: none.
State effects: writes .git/hooks/pre-commit for this worktree.
Pitfalls: installing the hook is not a substitute for running the gate now.
Next step: run scripts/check-commit.sh before committing.
USAGE
  exit 0
fi

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
