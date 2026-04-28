#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

KEEP_OPEN=0
SKIP_BUILD=0
LAUNCH_MODE="launchctl"
while [[ $# -gt 0 ]]; do
  arg="$1"
  case "$arg" in
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

if [[ "$KEEP_OPEN" == "0" ]]; then
  trap 'run_capy quit >/dev/null 2>&1 || true; stop_service' EXIT
fi

stop_service

stage_frontend_assets() {
  local resources="$APP/Contents/Resources/capy-app"
  rm -rf "$resources"
  mkdir -p "$resources"
  cp -R "$ROOT/frontend/capy-app/." "$resources/"
}

if [[ "$SKIP_BUILD" == "0" ]]; then
  cargo wef build -p capy-shell
  cargo build -p capy-cli
fi
stage_frontend_assets
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

if [[ "$LAUNCH_MODE" == "launchctl" ]]; then
  : > "$ASSETS/capy-cef-launchctl.out.log"
  : > "$ASSETS/capy-cef-launchctl.err.log"
  launchctl submit \
    -l "$LABEL" \
    -o "$ASSETS/capy-cef-launchctl.out.log" \
    -e "$ASSETS/capy-cef-launchctl.err.log" \
    -- /usr/bin/env CAPYBARA_SOCKET="$SOCKET" CAPY_DEFAULT_CWD="$ROOT" CAPY_OPEN_ON_START="demo" "$APP/Contents/MacOS/capy-shell"
else
  launchctl setenv CAPYBARA_SOCKET "$SOCKET"
  launchctl setenv CAPY_DEFAULT_CWD "$ROOT"
  launchctl setenv CAPY_OPEN_ON_START "demo"
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
