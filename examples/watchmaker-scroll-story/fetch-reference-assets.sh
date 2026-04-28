#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$ROOT_DIR/assets/watch"
BASE_URL="https://openai-landing-page-examples.vercel.app/haute-horlogerie/assets/watch"

mkdir -p "$OUT_DIR"

assets=(
  "hero-tilted.png"
  "base-plate.png"
  "gear-train.png"
  "tourbillon.png"
  "moonphase.png"
  "case-ring.png"
  "hands.png"
)

for asset in "${assets[@]}"; do
  curl -fL "$BASE_URL/$asset" -o "$OUT_DIR/$asset"
done

echo "Downloaded ${#assets[@]} reference assets into $OUT_DIR"
echo "These files are for private visual reference only and are gitignored."

