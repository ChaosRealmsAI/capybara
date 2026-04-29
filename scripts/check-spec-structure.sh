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
  if ! git check-ignore -q spec/README.md; then
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
  if [[ -f AGENTS.md && -f CLAUDE.md ]] && ! cmp -s AGENTS.md CLAUDE.md; then
    fail "AGENTS.md and CLAUDE.md must stay identical"
  fi

  local entry required_text
  for entry in AGENTS.md CLAUDE.md; do
    [[ -f "$entry" ]] || continue
    for required_text in \
      "spec/README.md" \
      "spec/versions/REGISTRY.json" \
      "scripts/check-spec-structure.sh" \
      "What Goes Where" \
      "Do not commit \`spec/\`"; do
      grep -Fq "$required_text" "$entry" || fail "$entry missing required entry text: $required_text"
    done
  done

  grep -Fq "## Write Destination Matrix" spec/README.md ||
    fail "spec/README.md must include ## Write Destination Matrix"

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

check_active_version() {
  local active
  active="$(jq -r '.active_version // empty' spec/versions/REGISTRY.json 2>/dev/null || true)"
  [[ -n "$active" ]] || {
    fail "REGISTRY.json active_version is missing"
    return
  }

  local active_dir="spec/versions/$active"
  [[ -d "$active_dir" ]] || fail "active version directory missing: $active_dir"
  jq -e --arg active "$active" '.versions[] | select(.id == $active)' spec/versions/REGISTRY.json >/dev/null 2>&1 ||
    fail "REGISTRY.json must include an entry for active_version $active"
  grep -Fq "Registry active version: \`$active\`" spec/README.md ||
    fail "spec/README.md Current Version must name active version $active"

  local status_file="$active_dir/status.json"
  local bdd_file="$active_dir/bdd.json"
  [[ -f "$status_file" ]] || return
  [[ -f "$bdd_file" ]] || return

  local status_version current_branch current_worktree
  status_version="$(jq -r '.version // empty' "$status_file")"
  [[ "$status_version" == "$active" ]] || fail "active status.json version must be $active"

  current_branch="$(git branch --show-current)"
  current_worktree="$(pwd -P)"
  local status_branch status_worktree registry_branch registry_worktree status_stage registry_stage status_status registry_status
  status_branch="$(jq -r '.branch // empty' "$status_file")"
  status_worktree="$(jq -r '.worktree // empty' "$status_file")"
  status_stage="$(jq -r '.stage // empty' "$status_file")"
  status_status="$(jq -r '.status // empty' "$status_file")"
  registry_branch="$(jq -r --arg active "$active" '.versions[] | select(.id == $active) | .branch // empty' spec/versions/REGISTRY.json)"
  registry_worktree="$(jq -r --arg active "$active" '.versions[] | select(.id == $active) | .worktree // empty' spec/versions/REGISTRY.json)"
  registry_stage="$(jq -r --arg active "$active" '.versions[] | select(.id == $active) | .stage // empty' spec/versions/REGISTRY.json)"
  registry_status="$(jq -r --arg active "$active" '.versions[] | select(.id == $active) | .status // empty' spec/versions/REGISTRY.json)"

  [[ "$status_branch" == "$current_branch" ]] || fail "active status branch $status_branch does not match current branch $current_branch"
  [[ "$status_worktree" == "$current_worktree" ]] || fail "active status worktree $status_worktree does not match current worktree $current_worktree"
  [[ "$registry_branch" == "$status_branch" ]] || fail "active registry branch must match status.json branch"
  [[ "$registry_worktree" == "$status_worktree" ]] || fail "active registry worktree must match status.json worktree"
  [[ "$registry_stage" == "$status_stage" ]] || fail "active registry stage must match status.json stage"
  [[ "$registry_status" == "$status_status" ]] || fail "active registry status must match status.json status"

  jq -e '
    (.scenarios // []) as $scenarios |
    ($scenarios | length) > 0 and
    all($scenarios[]; (.delivery_status | type) == "object" and (.test_cases | type) == "array" and (.self_verification_steps | type) == "array")
  ' "$bdd_file" >/dev/null 2>&1 ||
    fail "active bdd.json scenarios must include delivery_status, test_cases[], and self_verification_steps[]"

  grep -Fq "$active" AGENTS.md || fail "AGENTS.md must name active version $active"
  grep -Fq "$active" CLAUDE.md || fail "CLAUDE.md must name active version $active"
}

check_outer_git_boundary
check_required_tree
check_project_entry_docs
check_version_dirs
check_json_files
check_active_version

if [[ "${#failures[@]}" -gt 0 ]]; then
  echo "spec structure check failed:" >&2
  printf ' - %s\n' "${failures[@]}" >&2
  echo "run scripts/check-spec-structure.sh --fix to backfill missing legacy version files, then review the result." >&2
  exit 2
fi

echo "spec structure check passed"
