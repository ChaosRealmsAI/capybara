#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

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
