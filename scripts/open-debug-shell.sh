#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

NAME="debug-$(date +%H%M%S)"
PROJECT="demo"
WINDOWS=1
SOCKET=""
LABEL=""
CWD="$ROOT"
SKIP_BUILD=0

usage() {
  cat <<'USAGE'
Usage: scripts/open-debug-shell.sh [options]

Starts an isolated Capybara desktop debug instance with its own socket and
launchctl label. This is the debug path for running multiple independent
desktop windows/instances at the same time.

Options:
  --name <name>       Stable debug instance name. Default: debug-HHMMSS
  --socket <path>     Explicit CAPYBARA_SOCKET. Default: /tmp/capybara-<name>-<uid>.sock
  --label <label>     Explicit launchctl label. Default: com.capybara.debug.<name>
  --project <id>      Project to open on start. Default: demo
  --windows <n>       Number of desktop windows in this instance. Default: 1
  --cwd <path>        CAPY_DEFAULT_CWD. Default: repository root
  --skip-build        Reuse the existing app bundle and staged frontend
  -h, --help          Show this help

Examples:
  scripts/open-debug-shell.sh --name v19-a --windows 2
  scripts/open-debug-shell.sh --name v19-b --socket /tmp/capybara-v19-b-$(id -u).sock
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --name)
      shift
      NAME="${1:?--name requires a value}"
      ;;
    --socket)
      shift
      SOCKET="${1:?--socket requires a value}"
      ;;
    --label)
      shift
      LABEL="${1:?--label requires a value}"
      ;;
    --project)
      shift
      PROJECT="${1:?--project requires a value}"
      ;;
    --windows)
      shift
      WINDOWS="${1:?--windows requires a value}"
      ;;
    --cwd)
      shift
      CWD="${1:?--cwd requires a value}"
      ;;
    --skip-build)
      SKIP_BUILD=1
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

if ! [[ "$WINDOWS" =~ ^[0-9]+$ ]] || [[ "$WINDOWS" -lt 1 ]]; then
  echo "--windows must be a positive integer" >&2
  exit 2
fi

SAFE_NAME="$(printf '%s' "$NAME" | tr -c 'A-Za-z0-9_.-' '-')"
SOCKET="${SOCKET:-/tmp/capybara-${SAFE_NAME}-$(id -u).sock}"
LABEL="${LABEL:-com.capybara.debug.${SAFE_NAME}}"
APP="$ROOT/target/debug/capy-shell.app"
LOG_DIR="$ROOT/tmp/capy-debug-shells/$SAFE_NAME"
mkdir -p "$LOG_DIR"

scripts/check-code-sign-clones.sh --cleanup --apply --older-than-minutes 10 --keep-newest 2 >/dev/null || true

if [[ "$SKIP_BUILD" == "0" ]]; then
  cargo build -p capy-cli
  cargo wef build -p capy-shell
fi

RESOURCES="$APP/Contents/Resources/capy-app"
rm -rf "$RESOURCES"
mkdir -p "$RESOURCES"
cp -R "$ROOT/frontend/capy-app/." "$RESOURCES/"
codesign --force --deep --sign - "$APP" >/dev/null
codesign --verify --deep --strict "$APP"
scripts/check-code-sign-clones.sh --cleanup --apply --older-than-minutes 10 --keep-newest 2 >/dev/null || true

launchctl remove "$LABEL" >/dev/null 2>&1 || true
rm -f "$SOCKET"

launchctl submit \
  -l "$LABEL" \
  -o "$LOG_DIR/stdout.log" \
  -e "$LOG_DIR/stderr.log" \
  -- /usr/bin/env \
    CAPYBARA_SOCKET="$SOCKET" \
    CAPY_DEFAULT_CWD="$CWD" \
    CAPY_OPEN_ON_START="$PROJECT" \
    "$APP/Contents/MacOS/capy-shell"

for _ in $(seq 1 80); do
  if [[ -S "$SOCKET" ]]; then
    break
  fi
  sleep 0.25
done

if [[ ! -S "$SOCKET" ]]; then
  echo "Capybara debug socket not ready: $SOCKET" >&2
  echo "stdout: $LOG_DIR/stdout.log" >&2
  echo "stderr: $LOG_DIR/stderr.log" >&2
  exit 1
fi

run_capy() {
  CAPYBARA_SOCKET="$SOCKET" target/debug/capy "$@"
}

for _ in $(seq 1 40); do
  if run_capy ps > "$LOG_DIR/ps.json" && jq -e '.count > 0' "$LOG_DIR/ps.json" >/dev/null; then
    break
  fi
  sleep 0.5
done

if ! jq -e '.count > 0' "$LOG_DIR/ps.json" >/dev/null; then
  echo "Capybara debug window did not open: $SOCKET" >&2
  exit 1
fi

if [[ "$WINDOWS" -gt 1 ]]; then
  for _ in $(seq 2 "$WINDOWS"); do
    run_capy open --project="$PROJECT" --new-window > /dev/null
  done
  run_capy ps > "$LOG_DIR/ps.json"
fi

jq -n \
  --arg name "$NAME" \
  --arg socket "$SOCKET" \
  --arg label "$LABEL" \
  --arg project "$PROJECT" \
  --arg cwd "$CWD" \
  --arg log_dir "$LOG_DIR" \
  --argjson windows "$(jq '.count' "$LOG_DIR/ps.json")" \
  '{
    ok: true,
    name: $name,
    socket: $socket,
    label: $label,
    project: $project,
    cwd: $cwd,
    windows: $windows,
    log_dir: $log_dir,
    commands: {
      ps: ("CAPYBARA_SOCKET=" + $socket + " target/debug/capy ps"),
      capture: ("CAPYBARA_SOCKET=" + $socket + " target/debug/capy capture --out <absolute-png>"),
      quit: ("CAPYBARA_SOCKET=" + $socket + " target/debug/capy quit")
    }
  }'
