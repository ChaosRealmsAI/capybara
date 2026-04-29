#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-check}"
FIX=0
case "$MODE" in
  check|--check) ;;
  --fix) FIX=1 ;;
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
  if [[ "$FIX" == 1 ]]; then
    mkdir -p "$path"
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

write_milestones_if_missing() {
  local path="spec/milestones.html"
  if [[ -e "$path" || "$FIX" != 1 ]]; then
    return
  fi
  cat >"$path" <<'HTML'
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Capybara Milestones</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif; margin: 32px; color: #171717; line-height: 1.5; }
    h1 { font-size: 28px; }
    table { border-collapse: collapse; width: 100%; max-width: 980px; }
    th, td { border: 1px solid #e5e7eb; padding: 8px 10px; text-align: left; vertical-align: top; }
    th { background: #f8fafc; }
    code { background: #f3f4f6; padding: 2px 5px; border-radius: 4px; }
  </style>
</head>
<body>
  <h1>Capybara Milestones</h1>
  <p>This page is the PM-readable roadmap index. The authoritative multi-version state remains <code>spec/versions/REGISTRY.json</code>; each version owns its own <code>brief.md</code>, <code>bdd.json</code>, <code>status.json</code>, <code>bugs.json</code>, evidence report, and closeout report.</p>
  <table>
    <thead>
      <tr><th>Milestone</th><th>Status</th><th>Truth Source</th></tr>
    </thead>
    <tbody>
      <tr><td>Desktop CEF foundation</td><td>merged verified</td><td><code>spec/versions/v0.4-cef-shell-poc/</code></td></tr>
      <tr><td>Canvas and creative tools</td><td>merged verified</td><td><code>spec/versions/v0.6-canvas-chat-workbench/</code> through <code>v0.14-canvas-context-interface/</code></td></tr>
      <tr><td>Architecture migration</td><td>active</td><td><code>spec/versions/v0.15-project-architecture-migration/</code></td></tr>
    </tbody>
  </table>
</body>
</html>
HTML
}

write_project_standard_if_missing() {
  local path="spec/standards/project/spec-structure.md"
  if [[ -e "$path" || "$FIX" != 1 ]]; then
    return
  fi
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<'MD'
# Spec Structure Standard

Capybara keeps private product truth in `spec/`, which is a nested private git
repository ignored by the public code repository.

Rules:

- `spec/.git` must exist locally, and the public repository must keep `spec/`
  ignored and untracked.
- `spec/README.md` is the first entry point and must name the same active
  version as `spec/versions/REGISTRY.json`.
- Global current-truth files must exist: charter, collaboration, architecture,
  data model, interfaces, runtime, milestones, design, standards, and AI verify.
- Every directory under `spec/versions/` must contain `brief.md`, `bdd.json`,
  `status.json`, `bugs.json`, `report.md`, and `evidence/index.html`.
- The active version's `bdd.json` scenarios must include `delivery_status`,
  `test_cases[]`, and `self_verification_steps[]`.
- Files ending in `.json` must be parseable JSON. Mixed command logs belong in
  `.txt`, `.log`, or `.jsonl`, not `.json`.

Automation:

```bash
scripts/check-spec-structure.sh
```

Use this repair helper only for archived legacy gaps, then review the generated
stubs before committing:

```bash
scripts/check-spec-structure.sh --fix
```
MD
}

write_version_file_if_missing() {
  local version_dir="$1"
  local file="$2"
  local version_id
  version_id="$(basename "$version_dir")"
  local path="$version_dir/$file"

  if [[ -e "$path" || "$FIX" != 1 ]]; then
    return
  fi

  mkdir -p "$(dirname "$path")"
  case "$file" in
    status.json)
      cat >"$path" <<JSON
{
  "version": "$version_id",
  "stage": "archived",
  "status": "archived-incomplete",
  "owner": "codex",
  "branch": "main",
  "worktree": "$ROOT",
  "current_focus": "Archived legacy version directory backfilled so the spec tree has a complete handoff shape.",
  "artifacts": {},
  "tasks": [],
  "blockers": [
    "Backfilled minimal status; original version did not preserve a full status.json."
  ],
  "risks": []
}
JSON
      ;;
    brief.md)
      cat >"$path" <<MD
# $version_id Brief

This is an archived legacy version directory backfilled by
\`scripts/check-spec-structure.sh --fix\` so future agents can rely on the
standard version file set.

## Scope

Original scope was not fully preserved in this directory. Use nearby evidence,
devlog entries, and related version reports for historical detail.

## Status

Archived incomplete.
MD
      ;;
    bdd.json)
      cat >"$path" <<JSON
{
  "version": "$version_id",
  "status": "archived-incomplete",
  "scenarios": [
    {
      "id": "SCN-ARCHIVE-001",
      "title": "Archived legacy version keeps discoverable handoff files",
      "priority": "p3",
      "status": "archived",
      "delivery_status": {
        "implementation": "unknown",
        "tests": "unknown",
        "self_verification": "unknown",
        "evidence": "partial",
        "final_verdict": "archived-incomplete"
      },
      "acceptance": [
        "Directory contains the standard version handoff files",
        "Existing evidence files remain untouched"
      ],
      "test_cases": [],
      "self_verification_steps": []
    }
  ]
}
JSON
      ;;
    bugs.json)
      cat >"$path" <<'JSON'
{
  "bugs": []
}
JSON
      ;;
    evidence/index.html)
      cat >"$path" <<HTML
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>$version_id Evidence</title>
</head>
<body>
  <h1>$version_id Evidence</h1>
  <p>Archived legacy evidence index backfilled for spec structure compliance. Review sibling files in this evidence directory for the original raw artifacts.</p>
</body>
</html>
HTML
      ;;
    report.md)
      cat >"$path" <<MD
# $version_id Report

Archived legacy version directory. This report was backfilled to satisfy the
standard version handoff shape. Original conclusions, if any, remain in sibling
evidence files, devlog entries, or related version documents.
MD
      ;;
  esac
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
  write_milestones_if_missing
  write_project_standard_if_missing

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
  )

  local path
  for path in "${required[@]}"; do
    ensure_required_path "$path"
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
      write_version_file_if_missing "$dir" "$file"
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
}

check_outer_git_boundary
check_required_tree
check_version_dirs
check_json_files
check_active_version

if [[ "${#failures[@]}" -gt 0 ]]; then
  echo "spec structure check failed:" >&2
  printf ' - %s\n' "${failures[@]}" >&2
  if [[ "$FIX" != 1 ]]; then
    echo "run scripts/check-spec-structure.sh --fix to backfill missing legacy version files, then review the result." >&2
  fi
  exit 2
fi

echo "spec structure check passed"
