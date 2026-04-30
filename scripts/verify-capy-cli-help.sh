#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CAPY_BIN="${CAPY_BIN:-target/debug/capy}"
WITH_SMOKE=0

usage() {
  cat <<'USAGE'
Usage: scripts/verify-capy-cli-help.sh [--with-smoke]

Verifies the Capybara CLI progressive-disclosure contract:
  1. capy --help is the compact index.
  2. capy help <topic> is a self-contained workflow.
  3. capy <command> --help names common commands, required params, pitfalls, and next help topics.
  4. capy <command> help <topic> works for command-local topics.

--with-smoke also runs no-spend doctor/dry-run commands that are safe for gates.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-smoke)
      WITH_SMOKE=1
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

if [[ ! -x "$CAPY_BIN" ]]; then
  cargo build -p capy-cli >/dev/null
fi

TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/capy-help-verify.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

run_capture() {
  local name="$1"
  shift
  local out="$TMP_DIR/$name.txt"
  "$@" >"$out"
  echo "$out"
}

require_text() {
  local file="$1"
  local text="$2"
  if ! grep -Fq "$text" "$file"; then
    echo "help verification failed: missing '$text' in $file" >&2
    echo "command output:" >&2
    sed -n '1,140p' "$file" >&2
    exit 1
  fi
}

require_json() {
  jq empty "$1" >/dev/null
}

echo "[help] top-level index"
top_help="$(run_capture capy-help "$CAPY_BIN" --help)"
require_text "$top_help" "capy --help is the index"
require_text "$top_help" "Command tag: [dev]"
require_text "$top_help" "Help topics:"

topic_index="$(run_capture capy-help-topics "$CAPY_BIN" help)"
require_text "$topic_index" "Available self-contained help topics:"
for topic in \
  dev doctor interaction desktop canvas chat agent image image-cutout cutout \
  tts tts-karaoke tts-batch clips media timeline; do
  require_text "$topic_index" "$topic"
done

echo "[help] top-level commands"
for command in \
  shell open ps state devtools screenshot capture doctor click type cutout verify \
  chat canvas image help media timeline tts clips agent quit; do
  file="$(run_capture "cmd-$command-help" "$CAPY_BIN" "$command" --help)"
  require_text "$file" "Usage:"
  require_text "$file" "AI quick start:"
  require_text "$file" "Required params:"
  require_text "$file" "Pitfalls:"
done

check_topic() {
  local name="$1"
  shift
  local file
  file="$(run_capture "$name" "$@")"
  require_text "$file" "Use when"
  require_text "$file" "Required"
  require_text "$file" "Recommended"
  require_text "$file" "Do not"
  require_text "$file" "Next step"
}

echo "[help] global topics"
for topic in \
  dev doctor interaction desktop canvas chat agent image image-cutout cutout \
  tts tts-karaoke tts-batch clips media timeline; do
  check_topic "topic-$topic" "$CAPY_BIN" help "$topic"
done

echo "[help] command-local topic indexes"
for command in image cutout canvas chat agent clips media timeline tts; do
  file="$(run_capture "command-$command-topic-index" "$CAPY_BIN" "$command" help)"
  require_text "$file" "Available self-contained help topics:"
  require_text "$file" "Run \`capy $command help <topic>\`."
done

echo "[help] command-local topics"
command_topics=(
  "image agent"
  "image cutout-ready"
  "cutout agent"
  "cutout manifest"
  "canvas agent"
  "canvas context"
  "canvas images"
  "chat agent"
  "chat canvas-tools"
  "agent doctor"
  "agent sdk"
  "clips pipeline"
  "clips youtube"
  "media scroll-pack"
  "media story-pack"
  "timeline poster-export"
  "timeline live"
  "tts agent"
  "tts karaoke"
  "tts batch"
  "tts playbook"
)
for item in "${command_topics[@]}"; do
  read -r command topic <<<"$item"
  check_topic "command-$command-topic-$topic" "$CAPY_BIN" "$command" help "$topic"
done

if [[ "$WITH_SMOKE" == "1" ]]; then
  echo "[smoke] no-spend commands"
  doctor_json="$(run_capture doctor-json "$CAPY_BIN" doctor)"
  require_json "$doctor_json"
  image_providers="$(run_capture image-providers "$CAPY_BIN" image providers)"
  require_json "$image_providers"
  image_doctor="$(run_capture image-doctor "$CAPY_BIN" image doctor)"
  require_json "$image_doctor"
  cutout_doctor="$(run_capture cutout-doctor "$CAPY_BIN" cutout doctor)"
  require_json "$cutout_doctor"
  tts_init="$(run_capture tts-init "$CAPY_BIN" tts init --dry-run)"
  require_json "$tts_init"
  if "$CAPY_BIN" image generate --dry-run "cute cat" >/dev/null 2>&1; then
    echo "help verification failed: bad image prompt should be rejected" >&2
    exit 1
  fi
  "$CAPY_BIN" image generate --dry-run \
    "Scene: Warm studio tabletop. Subject: One ceramic cup centered, 40% frame height. Important details: Product photo, soft key light from upper left, cream and lavender palette. Use case: Hero card, 1:1 crop-safe. Constraints: No text, no watermark, no extra objects." \
    --size 1:1 --resolution 1k >/dev/null
fi

echo "capy CLI help verification passed"
