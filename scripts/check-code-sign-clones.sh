#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="check"
APPLY=0
MAX_TOTAL="${CAPY_CODE_SIGN_CLONE_MAX_TOTAL:-4G}"
OLDER_THAN_MINUTES="${CAPY_CODE_SIGN_CLONE_CLEAN_OLDER_THAN_MINUTES:-30}"
KEEP_NEWEST="${CAPY_CODE_SIGN_CLONE_KEEP_NEWEST:-2}"
SCOPE="capybara"

usage() {
  cat <<'USAGE'
Usage: scripts/check-code-sign-clones.sh [options]

Use when: AI verifies or cleans Capybara-owned macOS CodeSigningHelper temp app
bundle clones before or after desktop shell launch/signing work.

Required params: none for read-only check.

Detects macOS CodeSigningHelper temporary app bundle clones created under:
  /private/var/folders/*/*/X/*.code_sign_clone

The default check is read-only and fails when Capybara-owned clones exceed the
configured total size. Cleanup is opt-in and lsof-aware.

Options:
  --check                 Read-only threshold check. Default.
  --cleanup               Plan cleanup of old clone directories.
  --apply                 Actually delete cleanup candidates. Without this,
                          --cleanup is a dry run.
  --max-total <size>      Failure threshold for --check. Default: 4G.
  --older-than-minutes N  Cleanup only candidates older than N minutes.
                          Default: 30.
  --keep-newest N         Always keep the newest N clone dirs per bundle id.
                          Default: 2.
  --scope <name>          capybara or all. Default: capybara.
  -h, --help              Show this help.

Examples:
  scripts/check-code-sign-clones.sh
  scripts/check-code-sign-clones.sh --cleanup
  scripts/check-code-sign-clones.sh --cleanup --apply --older-than-minutes 10

Exit codes:
  0  no leak over threshold
  2  clone total exceeds threshold or cleanup found active candidates

Pitfalls: cleanup only deletes planned candidates when --apply is passed; quit
desktop shells first if active clones block cleanup.

Next step: rerun scripts/verify-cef-shell.sh or scripts/check-project.sh after cleanup.
USAGE
}

parse_size_bytes() {
  local raw="$1"
  local unit value
  value="${raw%[KkMmGgTt]}"
  unit="${raw:${#raw}-1:1}"
  case "$unit" in
    K|k) awk -v n="$value" 'BEGIN{printf "%.0f", n*1024}' ;;
    M|m) awk -v n="$value" 'BEGIN{printf "%.0f", n*1024*1024}' ;;
    G|g) awk -v n="$value" 'BEGIN{printf "%.0f", n*1024*1024*1024}' ;;
    T|t) awk -v n="$value" 'BEGIN{printf "%.0f", n*1024*1024*1024*1024}' ;;
    *) printf '%s\n' "$raw" ;;
  esac
}

human_bytes() {
  awk -v b="$1" 'BEGIN{
    if (b < 1024) printf "%dB", b;
    else if (b < 1024*1024) printf "%.1fK", b/1024;
    else if (b < 1024*1024*1024) printf "%.1fM", b/1024/1024;
    else printf "%.1fG", b/1024/1024/1024;
  }'
}

clone_owned_by_scope() {
  local name="$1"
  case "$SCOPE" in
    all) return 0 ;;
    capybara)
      case "$name" in
        io.github.wef.capy-shell.code_sign_clone|\
        ai.capybara.capy.code_sign_clone)
          return 0
          ;;
      esac
      ;;
  esac
  return 1
}

clone_roots() {
  local base root name
  for base in /private/var/folders/*/*/X; do
    [[ -d "$base" ]] || continue
    for root in "$base"/*.code_sign_clone; do
      [[ -d "$root" ]] || continue
      name="$(basename "$root")"
      clone_owned_by_scope "$name" || continue
      printf '%s\n' "$root"
    done
  done
}

dir_size_bytes() {
  local path="$1"
  local kb
  kb="$(du -sk "$path" 2>/dev/null | awk '{print $1}')"
  [[ -n "${kb:-}" ]] || kb=0
  printf '%s\n' "$((kb * 1024))"
}

open_file_paths() {
  lsof -Fn 2>/dev/null | sed -n 's/^n//p' | grep -F '.code_sign_clone/' || true
}

is_open_clone() {
  local path="$1"
  grep -F -q "$path/" <<<"$OPEN_PATHS"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) MODE="check" ;;
    --cleanup) MODE="cleanup" ;;
    --apply) APPLY=1 ;;
    --max-total)
      shift
      MAX_TOTAL="${1:?--max-total requires a value}"
      ;;
    --older-than-minutes)
      shift
      OLDER_THAN_MINUTES="${1:?--older-than-minutes requires a value}"
      ;;
    --keep-newest)
      shift
      KEEP_NEWEST="${1:?--keep-newest requires a value}"
      ;;
    --scope)
      shift
      SCOPE="${1:?--scope requires a value}"
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

case "$SCOPE" in
  capybara|all) ;;
  *) echo "unknown --scope: $SCOPE" >&2; exit 2 ;;
esac

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "code-sign clone check skipped: not macOS"
  exit 0
fi

MAX_BYTES="$(parse_size_bytes "$MAX_TOTAL")"
OPEN_PATHS="$(open_file_paths)"

roots=()
while IFS= read -r root; do
  roots+=("$root")
done < <(clone_roots | sort)
if [[ "${#roots[@]}" -eq 0 ]]; then
  echo "code-sign clone check passed: no $SCOPE clone roots"
  exit 0
fi

total=0
for root in "${roots[@]}"; do
  size="$(dir_size_bytes "$root")"
  total=$((total + size))
  printf '%8s  %s\n' "$(human_bytes "$size")" "$root"
done

if [[ "$MODE" == "check" ]]; then
  if (( total > MAX_BYTES )); then
    echo "code-sign clone check failed: $(human_bytes "$total") exceeds max $MAX_TOTAL for scope=$SCOPE" >&2
    echo "next step · quit Capybara desktop shells, then run:" >&2
    echo "  scripts/check-code-sign-clones.sh --cleanup --apply" >&2
    exit 2
  fi
  echo "code-sign clone check passed: $(human_bytes "$total") <= $MAX_TOTAL"
  exit 0
fi

cutoff_epoch="$(( $(date +%s) - OLDER_THAN_MINUTES * 60 ))"
deleted=0
kept=0
blocked=0
planned=0

for root in "${roots[@]}"; do
  clones=()
  while IFS= read -r clone; do
    clones+=("$clone")
  done < <(find "$root" -maxdepth 1 -type d -name 'code_sign_clone.*' -print0 \
    | xargs -0 stat -f '%m	%N' 2>/dev/null \
    | sort -rn \
    | cut -f2-)
  if (( ${#clones[@]} == 0 )); then
    continue
  fi
  index=0
  for clone in "${clones[@]}"; do
    index=$((index + 1))
    if (( index <= KEEP_NEWEST )); then
      kept=$((kept + 1))
      continue
    fi
    mtime="$(stat -f '%m' "$clone" 2>/dev/null || echo 0)"
    if (( mtime > cutoff_epoch )); then
      kept=$((kept + 1))
      continue
    fi
    if is_open_clone "$clone"; then
      echo "keep active clone: $clone" >&2
      blocked=$((blocked + 1))
      continue
    fi
    planned=$((planned + 1))
    if [[ "$APPLY" == "1" ]]; then
      rm -rf "$clone"
      deleted=$((deleted + 1))
    else
      echo "would delete: $clone"
    fi
  done
done

if [[ "$APPLY" == "1" ]]; then
  echo "code-sign clone cleanup deleted=$deleted kept=$kept active_blocked=$blocked"
else
  echo "code-sign clone cleanup dry-run planned=$planned kept=$kept active_blocked=$blocked"
fi

if (( blocked > 0 )); then
  exit 2
fi
