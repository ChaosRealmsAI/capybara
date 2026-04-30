#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/check-project-design-language.sh

Use when: AI changes project design-language contracts, prompt context, or
desktop summary wiring and needs the architecture guard for that slice.

Required params: none.
State effects: read-only.
Pitfalls: this is a contract/string guard, not a browser proof.
Next step: run scripts/check-project.sh for the full gate.
USAGE
  exit 0
fi

fail() {
  echo "project design-language check failed: $*" >&2
  exit 1
}

rg -q 'DesignLanguageValidationV1' crates/capy-project/src/model.rs crates/capy-project/src/design_language.rs ||
  fail "validation contract must live in capy-project"
rg -q 'design-language validate' crates/capy-cli/src/project.rs crates/capy-cli/src/help_topics/docs.rs scripts/check-project.sh ||
  fail "validate command must remain wired and discoverable"
rg -q 'design_language_ref' crates/capy-project/src/model.rs crates/capy-project/src/ai.rs crates/capy-project/src/generate.rs ||
  fail "AI prompt and generate runs must record design_language_ref"
rg -q 'project-design-language-summary' frontend/capy-app/index.html frontend/capy-app/app/project-package.js scripts/verify-project-design-language.mjs ||
  fail "desktop project package must expose active summary"
