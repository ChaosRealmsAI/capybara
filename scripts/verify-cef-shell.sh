#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

usage() {
  cat <<'USAGE'
Usage: scripts/verify-cef-shell.sh [--launch launchctl|open] [--keep-open] [--skip-build]

Use when: AI must prove the real CEF/Chromium desktop shell opens, renders,
accepts an interaction, exposes JS bridge state, has no blocking page errors,
and can produce app-view capture evidence.

Required params: none. Optional env: CAPYBARA_SOCKET, CAPY_VERIFY_VERSION_DIR,
CAPY_VERIFY_ASSETS, CAPY_VERIFY_OPEN_PROJECT, CAPY_LAUNCH_LABEL.

State effects: builds/stages/signs target/debug/capy-shell.app unless
--skip-build, runs CEF code-sign clone preflight cleanup/checks, launches a
desktop shell, writes evidence JSON/PNG/log files, and closes it unless
--keep-open is passed.

Pitfalls: use an isolated CAPYBARA_SOCKET for parallel worktrees; do not use
this as a generic browser screenshot tool; --keep-open leaves a running app.
If stale CodeSigningHelper clones cannot be cleaned, inspect
capy-cef-code-sign-preflight.log in the evidence assets directory.

Next step: inspect the written capy-cef-*.json/png assets or rerun
target/debug/capy help desktop for focused CLI probes.
USAGE
}

KEEP_OPEN=0
SKIP_BUILD=0
LAUNCH_MODE="launchctl"
while [[ $# -gt 0 ]]; do
  arg="$1"
  case "$arg" in
    -h|--help)
      usage
      exit 0
      ;;
    --keep-open) KEEP_OPEN=1 ;;
    --skip-build) SKIP_BUILD=1 ;;
    --launch=*) LAUNCH_MODE="${arg#--launch=}" ;;
    --launch)
      shift
      [[ $# -gt 0 ]] || {
        echo "--launch requires launchctl or open" >&2
        exit 2
      }
      LAUNCH_MODE="$1"
      ;;
    *)
      echo "unknown arg: $arg" >&2
      exit 2
      ;;
  esac
  shift
done

case "$LAUNCH_MODE" in
  launchctl|open) ;;
  *)
    echo "unknown --launch mode: $LAUNCH_MODE" >&2
    exit 2
    ;;
esac

VERSION_DIR="${CAPY_VERIFY_VERSION_DIR:-$ROOT/spec/versions/v0.4-cef-shell-poc}"
ASSETS="${CAPY_VERIFY_ASSETS:-$VERSION_DIR/evidence/assets}"
SOCKET="${CAPYBARA_SOCKET:-/tmp/capybara-main-cef-$(id -u).sock}"
LABEL="${CAPY_LAUNCH_LABEL:-com.capybara.cef.poc}"
OPEN_PROJECT="${CAPY_VERIFY_OPEN_PROJECT:-${CAPY_OPEN_ON_START:-demo}}"
ROOT_JSON="$(printf '%s' "$ROOT" | jq -Rs .)"
APP="$ROOT/target/debug/capy-shell.app"
case "$VERSION_DIR" in
  /*) ;;
  *) VERSION_DIR="$ROOT/$VERSION_DIR" ;;
esac
case "$ASSETS" in
  /*) ;;
  *) ASSETS="$ROOT/$ASSETS" ;;
esac
mkdir -p "$ASSETS" "$ROOT/tmp"
CODE_SIGN_PREFLIGHT_LOG="$ASSETS/capy-cef-code-sign-preflight.log"
: > "$CODE_SIGN_PREFLIGHT_LOG"

run_capy() {
  CAPYBARA_SOCKET="$SOCKET" target/debug/capy "$@"
}

stop_service() {
  launchctl remove "$LABEL" 2>/dev/null || true
  launchctl unsetenv CAPYBARA_SOCKET 2>/dev/null || true
  launchctl unsetenv CAPY_DEFAULT_CWD 2>/dev/null || true
  launchctl unsetenv CAPY_OPEN_ON_START 2>/dev/null || true
  for _ in $(seq 1 40); do
    if ! ps -axo command | grep -F "$APP/Contents" | grep -v grep >/dev/null; then
      break
    fi
    sleep 0.25
  done
  ps -axo pid,command | grep -F "$APP/Contents" | grep -v grep | awk '{print $1}' | while read -r pid; do
    [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
  done || true
  sleep 0.5
  ps -axo pid,command | grep -F "$APP/Contents" | grep -v grep | awk '{print $1}' | while read -r pid; do
    [[ -n "$pid" ]] && kill -9 "$pid" 2>/dev/null || true
  done || true
  rm -f "$SOCKET"
}

cleanup_code_sign_clones_best_effort() {
  {
    echo "== $(date -u +%Y-%m-%dT%H:%M:%SZ) exit-cleanup =="
    echo "$ scripts/check-code-sign-clones.sh --cleanup --apply --older-than-minutes 10 --keep-newest 2"
  } >> "$CODE_SIGN_PREFLIGHT_LOG"
  scripts/check-code-sign-clones.sh --cleanup --apply --older-than-minutes 10 --keep-newest 2 \
    >> "$CODE_SIGN_PREFLIGHT_LOG" 2>&1 || true
}

run_code_sign_clone_preflight() {
  local stage="$1"
  {
    echo "== $(date -u +%Y-%m-%dT%H:%M:%SZ) $stage =="
    echo "$ scripts/check-code-sign-clones.sh --cleanup --apply --older-than-minutes 10 --keep-newest 2"
  } >> "$CODE_SIGN_PREFLIGHT_LOG"
  if ! scripts/check-code-sign-clones.sh --cleanup --apply --older-than-minutes 10 --keep-newest 2 \
    >> "$CODE_SIGN_PREFLIGHT_LOG" 2>&1; then
    echo "CEF code-sign clone preflight failed during $stage: active or blocked clone remains." >&2
    echo "Inspect evidence log: $CODE_SIGN_PREFLIGHT_LOG" >&2
    exit 2
  fi

  echo "$ scripts/check-code-sign-clones.sh --check" >> "$CODE_SIGN_PREFLIGHT_LOG"
  if ! scripts/check-code-sign-clones.sh --check >> "$CODE_SIGN_PREFLIGHT_LOG" 2>&1; then
    echo "CEF code-sign clone preflight failed during $stage: clone budget check failed." >&2
    echo "Inspect evidence log: $CODE_SIGN_PREFLIGHT_LOG" >&2
    exit 2
  fi
}

if [[ "$KEEP_OPEN" == "0" ]]; then
  trap 'run_capy quit >/dev/null 2>&1 || true; stop_service; cleanup_code_sign_clones_best_effort' EXIT
fi

stop_service
run_code_sign_clone_preflight "before-build"

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

if [[ "$SKIP_BUILD" == "0" ]]; then
  scripts/build-canvas-for-app.sh
  cargo wef build -p capy-shell
  cargo build -p capy-cli
fi
stage_frontend_assets
scripts/sign-capy-shell-app.sh "$APP"
run_code_sign_clone_preflight "after-sign"

if [[ "$LAUNCH_MODE" == "launchctl" ]]; then
  : > "$ASSETS/capy-cef-launchctl.out.log"
  : > "$ASSETS/capy-cef-launchctl.err.log"
  launchctl submit \
    -l "$LABEL" \
    -o "$ASSETS/capy-cef-launchctl.out.log" \
    -e "$ASSETS/capy-cef-launchctl.err.log" \
    -- /usr/bin/env CAPYBARA_SOCKET="$SOCKET" CAPY_DEFAULT_CWD="$ROOT" CAPY_OPEN_ON_START="$OPEN_PROJECT" "$APP/Contents/MacOS/capy-shell"
else
  launchctl setenv CAPYBARA_SOCKET "$SOCKET"
  launchctl setenv CAPY_DEFAULT_CWD "$ROOT"
  launchctl setenv CAPY_OPEN_ON_START "$OPEN_PROJECT"
  open -n "$APP"
fi

for _ in $(seq 1 80); do
  if [[ -S "$SOCKET" ]]; then
    break
  fi
  sleep 0.25
done
if [[ ! -S "$SOCKET" ]]; then
  echo "CEF shell socket not ready: $SOCKET" >&2
  exit 1
fi

if ! run_capy ps > "$ASSETS/capy-cef-live-open.json" \
  || ! jq -e '.count > 0' "$ASSETS/capy-cef-live-open.json" >/dev/null; then
  run_capy open --project="$OPEN_PROJECT" > "$ASSETS/capy-cef-explicit-open.json" 2>/dev/null || true
fi

OPEN_OK=0
for _ in $(seq 1 40); do
  if run_capy ps > "$ASSETS/capy-cef-live-open.json" \
    && jq -e '.count > 0' "$ASSETS/capy-cef-live-open.json" >/dev/null; then
    OPEN_OK=1
    break
  fi
  sleep 0.5
done
if [[ "$OPEN_OK" == "0" ]]; then
  echo "CEF shell did not auto-open a window" >&2
  exit 1
fi

for _ in $(seq 1 40); do
  if run_capy devtools --eval='({browser:document.documentElement.dataset.capyBrowser,native:document.documentElement.dataset.capybaraNative,ready:document.readyState,title:document.title,topbar:!!document.querySelector(".topbar"),ua:navigator.userAgent})' > "$ASSETS/capy-cef-browser.json"; then
    break
  fi
  sleep 0.5
done

jq -e '.browser == "cef" and (.ua | contains("Chrome"))' "$ASSETS/capy-cef-browser.json" >/dev/null

run_capy devtools --eval='({ipc:typeof window.ipc?.postMessage, bridge:!!window.jsBridge, native:document.documentElement.dataset.capybaraNative})' > "$ASSETS/capy-cef-bridge.json"
jq -e '.ipc == "function" and .bridge == true' "$ASSETS/capy-cef-bridge.json" >/dev/null

run_capy devtools --query=.topbar --get=bounding-rect > "$ASSETS/capy-cef-topbar-rect.json"
jq -e '.ok == true and .value.width > 0 and .value.height > 0' "$ASSETS/capy-cef-topbar-rect.json" >/dev/null

run_capy devtools --eval='new Promise(resolve=>{setTimeout(()=>{const verifyCwd='"$ROOT_JSON"'; const cwd=document.querySelector("#cwd"); const provider=document.querySelector("#provider"); if(cwd) cwd.value=verifyCwd; if(provider) provider.value="codex"; const before=document.querySelectorAll(".conversation-item").length; document.querySelector("#new-chat")?.click(); setTimeout(()=>resolve({before,after:document.querySelectorAll(".conversation-item").length,title:document.querySelector("#chat-title")?.textContent,subtitle:document.querySelector("#chat-subtitle")?.textContent,cwd:document.querySelector("#cwd")?.value,browser:document.documentElement.dataset.capyBrowser,pageErrors:window.__capyPageErrors||[]}),700);},300);})' > "$ASSETS/capy-cef-interaction.json"
jq -e '.browser == "cef" and .after > .before and (.pageErrors | length) == 0' "$ASSETS/capy-cef-interaction.json" >/dev/null

run_capy verify --profile=desktop --capture-out="$ASSETS/capy-cef-window.png" > "$ASSETS/capy-cef-desktop-verify.json"
jq -e '.ok == true and .checks.capture.bytes > 100000' "$ASSETS/capy-cef-desktop-verify.json" >/dev/null
jq '.checks.console' "$ASSETS/capy-cef-desktop-verify.json" > "$ASSETS/capy-cef-console.json"
jq '.checks.capture' "$ASSETS/capy-cef-desktop-verify.json" > "$ASSETS/capy-cef-capture.json"
cp "$ASSETS/capy-cef-desktop-verify.json" "$ASSETS/capy-cef-$LAUNCH_MODE-desktop-verify.json"
cp "$ASSETS/capy-cef-window.png" "$ASSETS/capy-cef-$LAUNCH_MODE-window.png"

{
  echo "socket=$SOCKET"
  echo "label=$LABEL"
  echo "launch=$LAUNCH_MODE"
  echo "app=$APP"
  du -sh "$APP"
  find "$APP/Contents/Frameworks" -maxdepth 2 -type d | sed -n '1,40p'
  find "$APP/Contents/Resources/capy-app" -maxdepth 1 -type f | sort
} > "$ASSETS/capy-cef-bundle.txt"

ps -axo pid,ppid,command | grep -E 'capy-shell\.app/Contents|capy-shell Helper' | grep -v grep > "$ASSETS/capy-cef-processes.txt" || true
run_capy ps > "$ASSETS/capy-cef-live-ps.json"

echo "CEF shell verified with socket $SOCKET via $LAUNCH_MODE"
if [[ "$KEEP_OPEN" == "1" ]]; then
  if [[ "$LAUNCH_MODE" == "launchctl" ]]; then
    echo "CEF shell left running under launchctl label $LABEL"
  else
    echo "CEF shell left running from LaunchServices open path"
  fi
fi
