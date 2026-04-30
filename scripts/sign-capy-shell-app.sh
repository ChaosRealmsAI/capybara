#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

APP="$ROOT/target/debug/capy-shell.app"
APP_SET=0
FORCE=0
ALLOW_RUNNING=0
IDENTITY="${CAPY_CODE_SIGN_IDENTITY:--}"

usage() {
  cat <<'USAGE'
Usage: scripts/sign-capy-shell-app.sh [options] [app-bundle]

Use when: AI needs to verify or sign target/debug/capy-shell.app before desktop
launch without causing unnecessary macOS code_sign_clone churn.

Required params: none; optional app bundle path.

Ensures the Capybara CEF app bundle is signed without re-signing it on every
debug launch. By default this script first runs codesign verification; it only
executes `codesign --force --deep --sign` when the bundle is unsigned, invalid,
or changed.

Options:
  --app <path>       App bundle to verify/sign. Default: target/debug/capy-shell.app
  --identity <id>    Signing identity. Default: CAPY_CODE_SIGN_IDENTITY or ad-hoc "-"
  --force            Re-sign even when codesign verification already passes.
  --allow-running    Permit re-signing while this app bundle has running processes.
  -h, --help         Show this help.

Examples:
  scripts/sign-capy-shell-app.sh
  scripts/sign-capy-shell-app.sh target/debug/capy-shell.app
  scripts/sign-capy-shell-app.sh --force --identity -

Why this exists:
  Repeated `codesign --force --deep` on the large CEF bundle can make macOS
  create large *.code_sign_clone temporary app copies under /private/var/folders.
  Debug and verification scripts must call this wrapper instead of calling
  codesign directly.

Pitfalls: do not pass --allow-running unless intentionally experimenting; if
the bundle is running and needs signing, quit those shells first.

Next step: run scripts/check-code-sign-clones.sh or scripts/verify-cef-shell.sh.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --app)
      shift
      APP="${1:?--app requires a path}"
      APP_SET=1
      ;;
    --identity)
      shift
      IDENTITY="${1:?--identity requires a value}"
      ;;
    --force)
      FORCE=1
      ;;
    --allow-running)
      ALLOW_RUNNING=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      echo "unknown arg: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ "$APP_SET" == "1" ]]; then
        echo "unexpected extra app bundle: $1" >&2
        usage >&2
        exit 2
      fi
      APP="$1"
      APP_SET=1
      ;;
  esac
  shift
done

case "$APP" in
  /*) ;;
  *) APP="$ROOT/$APP" ;;
esac

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "codesign skipped: not macOS"
  exit 0
fi

if [[ ! -d "$APP" ]]; then
  echo "app bundle not found: $APP" >&2
  exit 2
fi

if ! command -v codesign >/dev/null 2>&1; then
  echo "codesign not found on PATH" >&2
  exit 2
fi

if [[ "$FORCE" == "0" ]]; then
  if codesign --verify --deep --strict "$APP" >/dev/null 2>&1; then
    echo "codesign skipped: app bundle already valid: $APP"
    exit 0
  fi
  echo "codesign needed: app bundle is unsigned, invalid, or changed"
else
  echo "codesign forced: $APP"
fi

if [[ "$ALLOW_RUNNING" == "0" ]]; then
  if ps -axo pid,command | grep -F "$APP/Contents" | grep -v grep >/dev/null; then
    echo "app bundle needs signing but running processes are using it: $APP" >&2
    echo "quit those desktop shells first, or pass --allow-running only for a deliberate local experiment" >&2
    exit 2
  fi
fi

codesign --force --deep --sign "$IDENTITY" "$APP" >/dev/null
codesign --verify --deep --strict "$APP" >/dev/null
echo "codesign complete: $APP"
