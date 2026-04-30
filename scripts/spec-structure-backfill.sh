#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'USAGE'
Usage: scripts/spec-structure-backfill.sh

Use when: AI intentionally backfills missing legacy spec version handoff files.

Required params: none.
State effects: writes missing spec/standards/project/spec-structure.md and
missing version brief/bdd/status/bugs/report/evidence files.
Pitfalls: this is a repair tool; review all generated spec changes before commit.
Next step: prefer scripts/check-spec-structure.sh --fix, then rerun scripts/lint-spec.sh.
USAGE
  exit 0
fi

mkdir -p spec/standards/project

if [[ ! -e spec/standards/project/spec-structure.md ]]; then
  cat >spec/standards/project/spec-structure.md <<'MD'
# Spec Structure Standard

Capybara keeps private product truth in `spec/`, which is a nested private git
repository ignored by the public code repository.

Rules:

- `spec/.git` must exist locally, and the public repository must keep `spec/`
  ignored and untracked.
- `spec/README.md` is the first spec entry point and must explain version
  discovery through `spec/versions/REGISTRY.json` plus version `status.json`;
  it must not copy a current-version pointer.
- `spec/versions/REGISTRY.json` uses parallel registry semantics:
  `focus_version`, compatibility `active_version`, and `active_versions[]`.
- Every active/focus version must keep enhanced task metadata in `status.json`
  and `evidence/manifest.json` using `capy.evidence.manifest.v1`.
- `spec/contracts/` stores runnable JSON fixtures and schemas for shared
  handoff surfaces.
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
scripts/lint-spec.sh
scripts/check-spec-structure.sh
scripts/check-spec-structure.sh --fix
```
MD
fi

write_version_file_if_missing() {
  local version_dir="$1"
  local file="$2"
  local version_id path
  version_id="$(basename "$version_dir")"
  path="$version_dir/$file"
  [[ -e "$path" ]] && return
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
  "blockers": ["Backfilled minimal status; original version did not preserve a full status.json."],
  "risks": []
}
JSON
      ;;
    brief.md)
      cat >"$path" <<MD
# $version_id Brief

Archived legacy version directory backfilled by
\`scripts/check-spec-structure.sh --fix\`.

## Status

Archived incomplete. Use nearby evidence, devlog entries, and related version
reports for historical detail.
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
      "acceptance": ["Directory contains the standard version handoff files"],
      "test_cases": [],
      "self_verification_steps": []
    }
  ]
}
JSON
      ;;
    bugs.json)
      printf '{\n  "bugs": []\n}\n' >"$path"
      ;;
    evidence/index.html)
      cat >"$path" <<HTML
<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>$version_id Evidence</title></head>
<body>
  <h1>$version_id Evidence</h1>
  <p>Archived legacy evidence index backfilled for spec structure compliance.</p>
</body>
</html>
HTML
      ;;
    report.md)
      cat >"$path" <<MD
# $version_id Report

Archived legacy version directory. This report was backfilled to satisfy the
standard version handoff shape.
MD
      ;;
  esac
}

while IFS= read -r -d '' dir; do
  for file in status.json brief.md bdd.json bugs.json evidence/index.html report.md; do
    write_version_file_if_missing "$dir" "$file"
  done
done < <(find spec/versions -mindepth 1 -maxdepth 1 -type d -print0 | sort -z)

echo "spec structure backfill complete"
