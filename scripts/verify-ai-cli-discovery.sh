#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CATALOG="spec/ai-verify/cli-catalog.json"
ONLY=""
LIST=0
NO_BUILD=0

usage() {
  cat <<'USAGE'
Usage: scripts/verify-ai-cli-discovery.sh [--catalog <json>] [--only <id>] [--list] [--no-build]

Use when: AI changes or audits any project-owned CLI, private spec harness, help
topic, verification script, or internal adapter that future agents may discover.

Required params: none. The default catalog is spec/ai-verify/cli-catalog.json.

What it verifies:
  - every catalog entry has id, help_command[], kind, state_effect, and required_markers[]
  - each help command exits successfully and prints non-empty help
  - each help output contains the markers declared by that entry

Options:
  --catalog <json>  Alternate catalog path.
  --only <id>       Verify only one catalog entry id.
  --list            Print catalog ids and help commands without running them.
  --no-build        Do not build target/debug/capy when it is missing.
  -h, --help        Show this help.

Pitfalls:
  - This is a help-discovery gate, not a product behavior gate.
  - Do not add a tool here before its own --help or help <topic> is self-contained.
  - Do not use this to approve live provider spend or visible UI delivery.

Next step: run target/debug/capy help harness, then the specific help command
shown in the catalog entry before using that tool in a version workflow.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --catalog)
      shift
      CATALOG="${1:?--catalog requires a path}"
      ;;
    --only)
      shift
      ONLY="${1:?--only requires an id}"
      ;;
    --list)
      LIST=1
      ;;
    --no-build)
      NO_BUILD=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

fail() {
  echo "AI CLI discovery check failed: $*" >&2
  exit 2
}

[[ -f "$CATALOG" ]] || fail "catalog not found: $CATALOG"
jq -e '.schema_version == 1 and (.entries | type) == "array"' "$CATALOG" >/dev/null ||
  fail "catalog must have schema_version=1 and entries[]"

if [[ "$NO_BUILD" == "0" && ! -x target/debug/capy ]]; then
  cargo build -p capy-cli >/dev/null
fi

if [[ "$LIST" == "1" ]]; then
  jq -r '.entries[] | [.id, .kind, (.help_command | join(" "))] | @tsv' "$CATALOG"
  exit 0
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/capy-ai-cli-discovery.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

entry_filter='.entries[]'
if [[ -n "$ONLY" ]]; then
  entry_filter='.entries[] | select(.id == $only)'
fi

count=0
while IFS= read -r entry; do
  [[ -n "$entry" ]] || continue
  id="$(jq -r '.id // empty' <<<"$entry")"
  kind="$(jq -r '.kind // empty' <<<"$entry")"
  [[ -n "$id" ]] || fail "entry missing id"
  [[ -n "$kind" ]] || fail "$id missing kind"
  jq -e '(.help_command | type) == "array" and (.help_command | length) > 0' <<<"$entry" >/dev/null ||
    fail "$id missing help_command[]"
  jq -e '(.required_markers | type) == "array" and (.required_markers | length) > 0' <<<"$entry" >/dev/null ||
    fail "$id missing required_markers[]"
  jq -e '(.state_effect | type) == "string" and (.state_effect | length) > 0' <<<"$entry" >/dev/null ||
    fail "$id missing state_effect"

  command_parts=()
  while IFS= read -r part; do
    command_parts+=("$part")
  done < <(jq -r '.help_command[]' <<<"$entry")
  output_file="$TMP_DIR/${id//[^A-Za-z0-9_.-]/_}.txt"
  echo "[ai-cli] $id · ${command_parts[*]}"
  if ! "${command_parts[@]}" >"$output_file" 2>&1; then
    sed -n '1,160p' "$output_file" >&2
    fail "$id help command failed"
  fi
  [[ -s "$output_file" ]] || fail "$id help output is empty"

  while IFS= read -r marker; do
    [[ -n "$marker" ]] || continue
    if ! grep -Fq "$marker" "$output_file"; then
      echo "missing marker '$marker' in $id output" >&2
      sed -n '1,180p' "$output_file" >&2
      exit 2
    fi
  done < <(jq -r '.required_markers[]' <<<"$entry")

  count=$((count + 1))
done < <(jq -c --arg only "$ONLY" "$entry_filter" "$CATALOG")

if [[ -n "$ONLY" && "$count" -eq 0 ]]; then
  fail "no catalog entry matched --only $ONLY"
fi

echo "AI CLI discovery verification passed ($count entries)"
