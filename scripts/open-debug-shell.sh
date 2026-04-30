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
REPLACE=0

usage() {
  cat <<'USAGE'
Usage: scripts/open-debug-shell.sh [options]

Starts an isolated Capybara desktop debug instance with its own socket and
launchctl label. This is the debug path for running multiple independent
desktop windows/instances at the same time.

Options:
  --name, --id <id>   Unique debug instance id. Default: debug-HHMMSS
  --socket <path>     Explicit CAPYBARA_SOCKET. Default: /tmp/capybara-<name>-<uid>.sock
  --label <label>     Explicit launchctl label. Default: com.capybara.debug.<name>
  --project <id>      Project to open on start. Default: demo
  --windows <n>       Number of desktop windows in this instance. Default: 1
  --cwd <path>        CAPY_DEFAULT_CWD. Default: repository root
  --skip-build        Reuse the existing app bundle and staged frontend
  --replace           Replace an existing debug instance with the same id/socket
  -h, --help          Show this help

Examples:
  # Start one named preview instance.
  scripts/open-debug-shell.sh --id poster-preview --project demo --skip-build
  CAPYBARA_SOCKET=/tmp/capybara-poster-preview-$(id -u).sock target/debug/capy ps

  # Start two independent instances for side-by-side debugging.
  scripts/open-debug-shell.sh --id v27-a --project demo --skip-build
  scripts/open-debug-shell.sh --id v27-b --project demo --skip-build
  CAPYBARA_SOCKET=/tmp/capybara-v27-a-$(id -u).sock target/debug/capy capture --out /tmp/v27-a.png
  CAPYBARA_SOCKET=/tmp/capybara-v27-b-$(id -u).sock target/debug/capy capture --out /tmp/v27-b.png

  # Open multiple windows inside one instance/socket.
  scripts/open-debug-shell.sh --id layout-debug --windows 2 --skip-build

  # Restart a known instance intentionally.
  scripts/open-debug-shell.sh --id poster-preview --replace --skip-build

  # Inspect exact commands and logs for an instance.
  cat tmp/capy-debug-shells/poster-preview/instance.json
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --name|--id)
      shift
      NAME="${1:?--name/--id requires a value}"
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
    --replace)
      REPLACE=1
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

if ! [[ "$NAME" =~ ^[A-Za-z0-9_.-]+$ ]]; then
  echo "--name/--id may contain only letters, numbers, dot, underscore, and dash: $NAME" >&2
  exit 2
fi

SAFE_NAME="$NAME"
SOCKET="${SOCKET:-/tmp/capybara-${SAFE_NAME}-$(id -u).sock}"
LABEL="${LABEL:-com.capybara.debug.${SAFE_NAME}}"
APP="$ROOT/target/debug/capy-shell.app"
LOG_DIR="$ROOT/tmp/capy-debug-shells/$SAFE_NAME"
MANIFEST="$LOG_DIR/instance.json"
mkdir -p "$LOG_DIR"

if [[ "$REPLACE" == "1" ]]; then
  launchctl remove "$LABEL" >/dev/null 2>&1 || true
  rm -f "$SOCKET"
else
  if launchctl print "gui/$(id -u)/$LABEL" >/dev/null 2>&1; then
    echo "debug instance id already exists: $SAFE_NAME" >&2
    echo "label: $LABEL" >&2
    echo "Use a unique --name/--id, or pass --replace to intentionally restart it." >&2
    exit 2
  fi

  if [[ -e "$SOCKET" ]]; then
    echo "debug socket already exists: $SOCKET" >&2
    echo "Use a unique --name/--id, or pass --replace to remove the stale socket." >&2
    exit 2
  fi
fi

cleanup_code_sign_clones() {
  scripts/check-code-sign-clones.sh --cleanup --apply --older-than-minutes 10 --keep-newest 2 >/dev/null || true
}

check_code_sign_clone_budget() {
  scripts/check-code-sign-clones.sh >/dev/null
}

cleanup_code_sign_clones
check_code_sign_clone_budget

if [[ "$SKIP_BUILD" == "0" ]]; then
  cargo build -p capy-cli
  cargo wef build -p capy-shell
fi

stage_frontend_assets() {
  local resources="$APP/Contents/Resources/capy-app"
  mkdir -p "$resources"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete "$ROOT/frontend/capy-app/" "$resources/"
  else
    rm -rf "$resources"
    mkdir -p "$resources"
    cp -Rp "$ROOT/frontend/capy-app/." "$resources/"
  fi
}

stage_frontend_assets
scripts/sign-capy-shell-app.sh "$APP"
cleanup_code_sign_clones
check_code_sign_clone_budget

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
  --arg id "$SAFE_NAME" \
  --arg name "$NAME" \
  --arg socket "$SOCKET" \
  --arg label "$LABEL" \
  --arg project "$PROJECT" \
  --arg cwd "$CWD" \
  --arg log_dir "$LOG_DIR" \
  --arg manifest "$MANIFEST" \
  --argjson windows "$(jq '.count' "$LOG_DIR/ps.json")" \
  '{
    ok: true,
    id: $id,
    name: $name,
    socket: $socket,
    label: $label,
    project: $project,
    cwd: $cwd,
    windows: $windows,
    log_dir: $log_dir,
    manifest: $manifest,
    commands: {
      ps: ("CAPYBARA_SOCKET=" + $socket + " target/debug/capy ps"),
      capture: ("CAPYBARA_SOCKET=" + $socket + " target/debug/capy capture --out <absolute-png>"),
      quit: ("CAPYBARA_SOCKET=" + $socket + " target/debug/capy quit")
    }
  }' | tee "$MANIFEST"
