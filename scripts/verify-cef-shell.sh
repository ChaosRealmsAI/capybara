#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

KEEP_OPEN=0
SKIP_BUILD=0
for arg in "$@"; do
  case "$arg" in
    --keep-open) KEEP_OPEN=1 ;;
    --skip-build) SKIP_BUILD=1 ;;
    *)
      echo "unknown arg: $arg" >&2
      exit 2
      ;;
  esac
done

VERSION_DIR="${CAPY_VERIFY_VERSION_DIR:-$ROOT/spec/versions/v0.4-cef-shell-poc}"
ASSETS="${CAPY_VERIFY_ASSETS:-$VERSION_DIR/evidence/assets}"
SOCKET="${CAPYBARA_SOCKET:-/tmp/capybara-main-cef-$(id -u).sock}"
LABEL="${CAPY_LAUNCH_LABEL:-com.capybara.cef.poc}"
ROOT_JSON="$(printf '%s' "$ROOT" | jq -Rs .)"
mkdir -p "$ASSETS" "$ROOT/tmp"

run_capy() {
  CAPYBARA_SOCKET="$SOCKET" target/debug/capy "$@"
}

stop_service() {
  launchctl remove "$LABEL" 2>/dev/null || true
  for _ in $(seq 1 40); do
    if ! ps -axo command | grep -F "$ROOT/target/debug/capy-shell.app/Contents" | grep -v grep >/dev/null; then
      break
    fi
    sleep 0.25
  done
  ps -axo pid,command | grep -F "$ROOT/target/debug/capy-shell.app/Contents" | grep -v grep | awk '{print $1}' | while read -r pid; do
    [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
  done || true
  sleep 0.5
  ps -axo pid,command | grep -F "$ROOT/target/debug/capy-shell.app/Contents" | grep -v grep | awk '{print $1}' | while read -r pid; do
    [[ -n "$pid" ]] && kill -9 "$pid" 2>/dev/null || true
  done || true
  rm -f "$SOCKET"
}

if [[ "$KEEP_OPEN" == "0" ]]; then
  trap 'run_capy quit >/dev/null 2>&1 || true; stop_service' EXIT
fi

stop_service

if [[ "$SKIP_BUILD" == "0" ]]; then
  cargo wef build -p capy-shell
  cargo build -p capy-cli
  codesign --force --deep --sign - target/debug/capy-shell.app
  codesign --verify --deep --strict target/debug/capy-shell.app
fi

launchctl submit \
  -l "$LABEL" \
  -o "$ASSETS/capy-cef-launchctl.out.log" \
  -e "$ASSETS/capy-cef-launchctl.err.log" \
  -- /usr/bin/env CAPYBARA_SOCKET="$SOCKET" "$ROOT/target/debug/capy-shell.app/Contents/MacOS/capy-shell"

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

run_capy open --project=demo > "$ASSETS/capy-cef-live-open.json"

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

run_capy devtools --eval='({consoleEvents:(window.__capyConsoleEvents||[]).slice(-20),pageErrors:window.__capyPageErrors||[]})' > "$ASSETS/capy-cef-console.json"
jq -e '(.pageErrors | length) == 0' "$ASSETS/capy-cef-console.json" >/dev/null

run_capy capture --out="$ASSETS/capy-cef-window.png" > "$ASSETS/capy-cef-capture.json"
jq -e '.bytes > 100000 and .width > 0 and .height > 0' "$ASSETS/capy-cef-capture.json" >/dev/null

{
  echo "socket=$SOCKET"
  echo "label=$LABEL"
  echo "app=$ROOT/target/debug/capy-shell.app"
  du -sh target/debug/capy-shell.app
  find target/debug/capy-shell.app/Contents/Frameworks -maxdepth 2 -type d | sed -n '1,40p'
} > "$ASSETS/capy-cef-bundle.txt"

ps -axo pid,ppid,command | grep -E 'capy-shell\.app/Contents|capy-shell Helper' | grep -v grep > "$ASSETS/capy-cef-processes.txt" || true
run_capy ps > "$ASSETS/capy-cef-live-ps.json"

echo "CEF shell verified with socket $SOCKET"
if [[ "$KEEP_OPEN" == "1" ]]; then
  echo "CEF shell left running under launchctl label $LABEL"
fi
