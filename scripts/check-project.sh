#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scripts/lint-spec.sh
export CAPY_SPEC_STRUCTURE_CHECKED=1
scripts/check-architecture.sh
scripts/check-large-files.sh
scripts/check-code-sign-clones.sh
scripts/build-canvas-for-app.sh >/dev/null
scripts/check-frontend-js.sh >/dev/null
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path crates/capy-recorder/Cargo.toml --lib
scripts/verify-capy-cli-help.sh
CAPY_BIN="${CAPY_BIN:-target/debug/capy}"
if [[ ! -x "$CAPY_BIN" ]]; then
  cargo build -p capy-cli >/dev/null
fi
"$CAPY_BIN" --help >/dev/null
"$CAPY_BIN" agent doctor >/dev/null
"$CAPY_BIN" cutout --help >/dev/null
"$CAPY_BIN" cutout doctor >/dev/null
"$CAPY_BIN" timeline doctor \
  --recorder tmp/nonexistent-capy-recorder >/dev/null
"$CAPY_BIN" image providers >/dev/null
"$CAPY_BIN" image doctor >/dev/null
rm -rf target/capy-timeline/sample-poster
"$CAPY_BIN" timeline compose-poster \
  --input fixtures/poster/v0.1/sample-poster.json \
  --out target/capy-timeline/sample-poster >/dev/null
"$CAPY_BIN" timeline validate \
  --composition target/capy-timeline/sample-poster/composition.json >/dev/null
"$CAPY_BIN" timeline compile \
  --composition target/capy-timeline/sample-poster/composition.json >/dev/null
if ! jq -e '.schema_version == "capy.timeline.render_source.v1" and (.tracks | length) == 1' \
  target/capy-timeline/sample-poster/render_source.json >/dev/null; then
  echo "project check failed: timeline compile must emit render_source.v1 with one component track" >&2
  exit 1
fi
"$CAPY_BIN" timeline validate \
  --composition fixtures/timeline/video-editing/compositions/main.json >/dev/null
"$CAPY_BIN" timeline compile \
  --composition fixtures/timeline/video-editing/compositions/main.json >/dev/null
if ! jq -e '.schema_version == "capy.timeline.render_source.v1" and .duration_ms == 4000 and (.tracks | length) == 2' \
  fixtures/timeline/video-editing/compositions/render_source.json >/dev/null; then
  echo "project check failed: clip-first video editing fixture must compile to 4000ms render_source with two tracks" >&2
  exit 1
fi
"$CAPY_BIN" tts --help >/dev/null
"$CAPY_BIN" tts --brief >/dev/null
"$CAPY_BIN" tts doctor >/dev/null
tts_init_dry_run="$("$CAPY_BIN" tts init --dry-run)"
if ! jq -e '.kind == "tts-init" and .dry_run == true and (.runtime.python | length) > 0' \
  <<<"$tts_init_dry_run" >/dev/null; then
  echo "project check failed: tts init dry-run must report selected runtime" >&2
  exit 1
fi
tts_dry_run="$(printf '[{"text":"hello","filename":"hello"}]' | "$CAPY_BIN" tts batch -d target/capy-tts-dry-run --dry-run)"
if ! grep -q '"text": "hello"' <<<"$tts_dry_run"; then
  echo "project check failed: tts batch dry-run must echo the planned job" >&2
  exit 1
fi
"$CAPY_BIN" clips --help >/dev/null
"$CAPY_BIN" clips doctor >/dev/null
"$CAPY_BIN" clips karaoke --help >/dev/null
test -f crates/capy-clips-transcribe/scripts/whisper_transcribe.py
test -f crates/capy-clips-align/scripts/align_ffa.py
"$CAPY_BIN" media --help >/dev/null
media_dry_run="$("$CAPY_BIN" media scroll-pack \
  --input tmp/nonexistent-scroll-media-dry-run.mp4 \
  --out target/capy-scroll-media-dry-run \
  --name dry-run \
  --dry-run)"
if ! grep -q '"scroll-hq.html"' <<<"$media_dry_run"; then
  echo "project check failed: scroll media dry-run must include scroll-hq.html" >&2
  exit 1
fi
story_dry_run="$("$CAPY_BIN" media story-pack \
  --manifest crates/capy-scroll-media/examples/inputs/watch-story-dry-run.json \
  --out target/capy-scroll-story-dry-run \
  --dry-run)"
if ! grep -q '"story.html"' <<<"$story_dry_run"; then
  echo "project check failed: scroll story dry-run must include story.html" >&2
  exit 1
fi
test -f crates/capy-scroll-media/README.md
test -f crates/capy-scroll-media/examples/inputs/watch-story-dry-run.json
test -f crates/capy-scroll-media/examples/inputs/card-pan-2s.mp4
test -f crates/capy-scroll-media/examples/outputs/card-pan-2s/scroll-hq.html
test -f crates/capy-scroll-media/examples/outputs/card-pan-2s/manifest.json
"$CAPY_BIN" media inspect \
  --manifest crates/capy-scroll-media/examples/outputs/card-pan-2s/manifest.json >/dev/null
"$CAPY_BIN" image generate --dry-run \
  "Scene: Warm studio tabletop. Subject: One ceramic cup centered, 40% frame height. Important details: Product photo, soft key light from upper left, cream and lavender palette. Use case: Hero card, 1:1 crop-safe. Constraints: No text, no watermark, no extra objects." \
  --size 1:1 --resolution 1k >/dev/null
if "$CAPY_BIN" image generate --dry-run "cute cat" >/dev/null 2>&1; then
  echo "project check failed: bad image prompt should be rejected" >&2
  exit 1
fi
rm -rf target/capy-project-html-context
mkdir -p target
cp -R fixtures/project/html-context target/capy-project-html-context
"$CAPY_BIN" project inspect \
  --project target/capy-project-html-context >/dev/null
"$CAPY_BIN" context build \
  --project target/capy-project-html-context \
  --artifact art_00000000000000000000000000000001 \
  --selector '[data-capy-section="hero-title"]' \
  --out target/capy-project-html-context/context.json >/dev/null
if ! jq -e '.schema_version == "capy.context.v1" and .artifact_id == "art_00000000000000000000000000000001" and (.design_language_refs | length) == 2' \
  target/capy-project-html-context/context.json >/dev/null; then
  echo "project check failed: project context build must include artifact and design language refs" >&2
  exit 1
fi
"$CAPY_BIN" patch apply \
  --project target/capy-project-html-context \
  --patch fixtures/project/html-context/patches/headline.json \
  --dry-run >/dev/null
if grep -q 'Project Context Locked' target/capy-project-html-context/web/index.html; then
  echo "project check failed: patch dry-run must not mutate HTML source" >&2
  exit 1
fi
"$CAPY_BIN" patch apply \
  --project target/capy-project-html-context \
  --patch fixtures/project/html-context/patches/headline.json >/dev/null
if ! grep -q 'Project Context Locked' target/capy-project-html-context/web/index.html; then
  echo "project check failed: patch apply must mutate HTML source" >&2
  exit 1
fi

echo "project check passed"
