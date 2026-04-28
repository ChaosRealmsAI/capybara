#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scripts/check-architecture.sh
scripts/build-canvas-for-app.sh >/dev/null
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p capy-cli -- --help >/dev/null
cargo run -p capy-cli -- agent doctor >/dev/null
cargo run -p capy-cli -- image providers >/dev/null
cargo run -p capy-cli -- image doctor >/dev/null
cargo run -p capy-cli -- media --help >/dev/null
media_dry_run="$(cargo run -p capy-cli -- media scroll-pack \
  --input tmp/nonexistent-scroll-media-dry-run.mp4 \
  --out target/capy-scroll-media-dry-run \
  --name dry-run \
  --dry-run)"
if ! grep -q '"scroll-hq.html"' <<<"$media_dry_run"; then
  echo "project check failed: scroll media dry-run must include scroll-hq.html" >&2
  exit 1
fi
cargo run -p capy-cli -- image generate --dry-run \
  "Scene: Warm studio tabletop. Subject: One ceramic cup centered, 40% frame height. Important details: Product photo, soft key light from upper left, cream and lavender palette. Use case: Hero card, 1:1 crop-safe. Constraints: No text, no watermark, no extra objects." \
  --size 1:1 --resolution 1k >/dev/null
if cargo run -p capy-cli -- image generate --dry-run "cute cat" >/dev/null 2>&1; then
  echo "project check failed: bad image prompt should be rejected" >&2
  exit 1
fi

echo "project check passed"
