#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-check}"
case "$MODE" in
  check|--check) ;;
  --fix) scripts/spec-structure-backfill.sh ;;
  *)
    echo "usage: scripts/check-spec-structure.sh [--fix]" >&2
    exit 64
    ;;
esac

failures=()

fail() {
  failures+=("$1")
}

ensure_dir() {
  local path="$1"
  if [[ -d "$path" ]]; then
    return
  fi
  fail "missing directory: $path"
}

ensure_required_path() {
  local path="$1"
  if [[ -e "$path" ]]; then
    return
  fi
  fail "missing required path: $path"
}

check_outer_git_boundary() {
  [[ -d spec/.git ]] || fail "spec/ must be a nested git repository with spec/.git"
  if ! git check-ignore -q spec/README.md 2>/dev/null &&
    ! git check-ignore -q spec 2>/dev/null &&
    ! git check-ignore -q spec/ 2>/dev/null; then
    fail "public repo must ignore spec/; git check-ignore spec/README.md failed"
  fi
  if [[ -n "$(git ls-files 'spec' 'spec/**')" ]]; then
    fail "public repo must not track files under spec/"
  fi
}

check_required_tree() {
  ensure_dir spec
  ensure_dir spec/standards/project

  local required=(
    spec/README.md
    spec/charter.md
    spec/human-ai-collaboration.md
    spec/architecture.md
    spec/data-model.md
    spec/interfaces.md
    spec/runtime.md
    spec/milestones.html
    spec/contracts/README.md
    spec/contracts/evidence/manifest.schema.json
    spec/contracts/evidence/manifest.example.json
    spec/contracts/registry/parallel-registry.example.json
    spec/contracts/registry/status-task.example.json
    spec/standards/00-index.md
    spec/standards/scorecard.md
    spec/standards/automation-map.md
    spec/standards/project/spec-structure.md
    spec/standards/project/agent-handoff.md
    spec/standards/project/module-ownership.md
    spec/standards/project/contracts-and-schemas.md
    spec/standards/project/evidence-retention.md
    spec/standards/project/security-privacy.md
    spec/standards/project/provider-cost.md
    spec/design/DESIGN.md
    spec/design/product.md
    spec/design/visual.md
    spec/design/tokens.css
    spec/design/examples/preview.html
    spec/design/examples/images
    spec/ai-verify/README.md
    spec/ai-verify/cli.md
    spec/ai-verify/scenarios.md
    spec/ai-verify/human-sim.md
    spec/ai-verify/debugging.md
    spec/versions/REGISTRY.json
    AGENTS.md
    CLAUDE.md
  )

  local path
  for path in "${required[@]}"; do
    ensure_required_path "$path"
  done
}

check_project_entry_docs() {
  [[ -f AGENTS.md && ! -L AGENTS.md ]] ||
    fail "AGENTS.md must be the regular-file source project entry"
  [[ -f CLAUDE.md && ! -L CLAUDE.md ]] ||
    fail "CLAUDE.md must be a regular-file copy of AGENTS.md"

  if ! cmp -s AGENTS.md CLAUDE.md; then
    fail "CLAUDE.md must be an exact copy of AGENTS.md"
  fi

  local entry required_text
  for entry in AGENTS.md CLAUDE.md; do
    for required_text in \
      "\`AGENTS.md\` is the source project entry" \
      "Progressive Disclosure" \
      "spec/README.md" \
      "spec/versions/REGISTRY.json" \
      "scripts/check-spec-structure.sh" \
      "What Goes Where" \
      "Do not commit \`spec/\`"; do
      grep -Fq "$required_text" "$entry" || fail "$entry missing required entry text: $required_text"
    done
  done

  for required_text in \
    "\`AGENTS.md\` is the source project entry" \
    "\`README.md\` is only the public repo overview" \
    "spec/versions/REGISTRY.json"; do
    grep -Fq "$required_text" README.md || fail "README.md missing required maintenance text: $required_text"
  done

  grep -Fq "## Write Destination Matrix" spec/README.md ||
    fail "spec/README.md must include ## Write Destination Matrix"
  grep -Fq "## Version Discovery" spec/README.md ||
    fail "spec/README.md must include ## Version Discovery"
  if grep -Fq "Registry active version:" spec/README.md; then
    fail "spec/README.md must not copy a current-version pointer; use REGISTRY.json and status.json discovery"
  fi

  local standard
  for standard in \
    agent-handoff.md \
    module-ownership.md \
    contracts-and-schemas.md \
    evidence-retention.md \
    security-privacy.md \
    provider-cost.md; do
    grep -Fq "project/$standard" spec/standards/00-index.md ||
      fail "spec/standards/00-index.md must link project/$standard"
  done
}

check_json_files() {
  local file
  while IFS= read -r -d '' file; do
    if ! jq empty "$file" >/dev/null 2>&1; then
      fail "invalid JSON file: $file"
    fi
  done < <(
    find spec \
      -path spec/.git -prune -o \
      -path '*/node_modules/*' -prune -o \
      -path '*/.venv/*' -prune -o \
      -path '*/model-cache/*' -prune -o \
      -path '*/model-cache-shootout/*' -prune -o \
      -name '*.json' -print0
  )
}

check_contract_fixtures() {
  local fixture
  for fixture in \
    spec/contracts/evidence/manifest.schema.json \
    spec/contracts/evidence/manifest.example.json \
    spec/contracts/registry/parallel-registry.example.json \
    spec/contracts/registry/status-task.example.json; do
    jq empty "$fixture" >/dev/null 2>&1 || fail "invalid contract fixture JSON: $fixture"
  done

  grep -Fq "capy.evidence.manifest.v1" spec/contracts/README.md ||
    fail "spec/contracts/README.md must name the evidence manifest schema"
  jq -e '.properties.schema.const == "capy.evidence.manifest.v1"' \
    spec/contracts/evidence/manifest.schema.json >/dev/null 2>&1 ||
    fail "evidence manifest schema must enforce capy.evidence.manifest.v1"
}

check_version_dirs() {
  local dir file
  while IFS= read -r -d '' dir; do
    for file in status.json brief.md bdd.json bugs.json evidence/index.html report.md; do
      [[ -e "$dir/$file" ]] || fail "$(basename "$dir") missing $file"
    done
    if [[ -f "$dir/bugs.json" ]] && ! jq -e '.bugs | type == "array"' "$dir/bugs.json" >/dev/null 2>&1; then
      fail "$(basename "$dir") bugs.json must contain a bugs array"
    fi
  done < <(find spec/versions -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)
}

check_version_consistency() {
  local version_id="$1"
  local require_current_worktree="$2"
  local version_dir="spec/versions/$version_id"
  local status_file="$version_dir/status.json"
  local bdd_file="$version_dir/bdd.json"
  local manifest_file="$version_dir/evidence/manifest.json"

  [[ -d "$version_dir" ]] || {
    fail "version directory missing: $version_dir"
    return
  }
  jq -e --arg version "$version_id" '.versions[] | select(.id == $version)' \
    spec/versions/REGISTRY.json >/dev/null 2>&1 ||
    fail "REGISTRY.json must include an entry for $version_id"
  [[ -f "$status_file" ]] || return
  [[ -f "$bdd_file" ]] || return

  local status_version current_branch current_worktree
  status_version="$(jq -r '.version // empty' "$status_file")"
  [[ "$status_version" == "$version_id" ]] || fail "$version_id status.json version must be $version_id"

  local status_branch status_worktree registry_branch registry_worktree status_stage registry_stage status_status registry_status
  status_branch="$(jq -r '.branch // empty' "$status_file")"
  status_worktree="$(jq -r '.worktree // empty' "$status_file")"
  status_stage="$(jq -r '.stage // empty' "$status_file")"
  status_status="$(jq -r '.status // empty' "$status_file")"
  registry_branch="$(jq -r --arg version "$version_id" '.versions[] | select(.id == $version) | .branch // empty' spec/versions/REGISTRY.json)"
  registry_worktree="$(jq -r --arg version "$version_id" '.versions[] | select(.id == $version) | .worktree // empty' spec/versions/REGISTRY.json)"
  registry_stage="$(jq -r --arg version "$version_id" '.versions[] | select(.id == $version) | .stage // empty' spec/versions/REGISTRY.json)"
  registry_status="$(jq -r --arg version "$version_id" '.versions[] | select(.id == $version) | .status // empty' spec/versions/REGISTRY.json)"

  [[ "$registry_branch" == "$status_branch" ]] || fail "$version_id registry branch must match status.json branch"
  [[ "$registry_worktree" == "$status_worktree" ]] || fail "$version_id registry worktree must match status.json worktree"
  [[ "$registry_stage" == "$status_stage" ]] || fail "$version_id registry stage must match status.json stage"
  [[ "$registry_status" == "$status_status" ]] || fail "$version_id registry status must match status.json status"

  if [[ "$require_current_worktree" == "true" ]]; then
    current_branch="$(git branch --show-current)"
    current_worktree="$(pwd -P)"
    [[ "$status_branch" == "$current_branch" ]] || fail "focus version $version_id branch $status_branch does not match current branch $current_branch"
    [[ "$status_worktree" == "$current_worktree" ]] || fail "focus version $version_id worktree $status_worktree does not match current worktree $current_worktree"
  fi

  jq -e '
    (.scenarios // []) as $scenarios |
    ($scenarios | length) > 0 and
    all($scenarios[]; (.delivery_status | type) == "object" and (.test_cases | type) == "array" and (.self_verification_steps | type) == "array")
  ' "$bdd_file" >/dev/null 2>&1 ||
    fail "$version_id bdd.json scenarios must include delivery_status, test_cases[], and self_verification_steps[]"

  jq -e '
    (.tasks // []) as $tasks |
    ($tasks | length) > 0 and
    all($tasks[];
      (.id | type) == "string" and
      (.title | type) == "string" and
      (.status | type) == "string" and
      (.owner | type) == "string" and
      (.agent | type) == "string" and
      (.session | type) == "string" and
      (.write_set | type) == "array" and
      (.blocked_by | type) == "array" and
      (.last_updated | type) == "string" and
      (.required_evidence | type) == "array" and
      (.handoff_notes | type) == "string"
    )
  ' "$status_file" >/dev/null 2>&1 ||
    fail "$version_id status.json tasks must include enhanced parallel handoff metadata"

  [[ -f "$manifest_file" ]] || fail "$version_id missing evidence/manifest.json"
  if [[ -f "$manifest_file" ]]; then
    jq -e --arg version "$version_id" '
      .schema == "capy.evidence.manifest.v1" and
      .version == $version and
      (.runs | type) == "array" and
      (.artifacts | type) == "array" and
      (.verdict | type) == "object"
    ' "$manifest_file" >/dev/null 2>&1 ||
      fail "$version_id evidence/manifest.json must match capy.evidence.manifest.v1"
  fi
}

check_active_version() {
  local active focus validation_focus
  active="$(jq -r '.active_version // empty' spec/versions/REGISTRY.json 2>/dev/null || true)"
  focus="$(jq -r '.focus_version // empty' spec/versions/REGISTRY.json 2>/dev/null || true)"
  validation_focus="${CAPY_FOCUS_VERSION:-$focus}"
  [[ -n "$active" ]] || {
    fail "REGISTRY.json active_version is missing"
    return
  }
  [[ -n "$focus" ]] || fail "REGISTRY.json focus_version is missing"
  [[ -n "$validation_focus" ]] || fail "validation focus version is missing"
  [[ "$active" == "$focus" ]] || fail "active_version compatibility default must match focus_version"

  jq -e '.registry_model == "parallel"' spec/versions/REGISTRY.json >/dev/null 2>&1 ||
    fail "REGISTRY.json registry_model must be parallel"
  jq -e '(.active_versions | type) == "array" and (.active_versions | length) > 0 and all(.active_versions[]; type == "string")' \
    spec/versions/REGISTRY.json >/dev/null 2>&1 ||
    fail "REGISTRY.json active_versions must be a non-empty string array"
  jq -e --arg focus "$focus" '.active_versions | index($focus)' spec/versions/REGISTRY.json >/dev/null 2>&1 ||
    fail "REGISTRY.json active_versions must include focus_version"
  jq -e --arg focus "$validation_focus" '.active_versions | index($focus)' spec/versions/REGISTRY.json >/dev/null 2>&1 ||
    fail "REGISTRY.json active_versions must include validation focus $validation_focus"

  local version_id
  while IFS= read -r version_id; do
    check_version_consistency "$version_id" "false"
  done < <(jq -r '.active_versions[]' spec/versions/REGISTRY.json)
  check_version_consistency "$validation_focus" "true"

  if grep -Fq "$active" AGENTS.md || grep -Fq "$active" CLAUDE.md || grep -Fq "$active" README.md || grep -Fq "$active" spec/README.md; then
    fail "entry docs and README files must discover active versions from REGISTRY.json, not hard-code $active"
  fi
}

check_outer_git_boundary
check_required_tree
check_project_entry_docs
check_version_dirs
check_json_files
check_contract_fixtures
check_active_version

if [[ "${#failures[@]}" -gt 0 ]]; then
  echo "spec structure check failed:" >&2
  printf ' - %s\n' "${failures[@]}" >&2
  echo "run scripts/check-spec-structure.sh --fix to backfill missing legacy version files, then review the result." >&2
  exit 2
fi

echo "spec structure check passed"
