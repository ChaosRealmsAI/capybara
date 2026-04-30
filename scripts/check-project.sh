#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

scripts/lint-spec.sh
export CAPY_SPEC_STRUCTURE_CHECKED=1
scripts/check-architecture.sh
scripts/check-large-files.sh
bash -n scripts/check-code-sign-clones.sh scripts/sign-capy-shell-app.sh scripts/open-debug-shell.sh scripts/verify-cef-shell.sh scripts/check-sdk-only-agent-runtime.sh scripts/check-project-design-language.sh
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
"$CAPY_BIN" agent sdk doctor >/dev/null
"$CAPY_BIN" cutout --help >/dev/null
"$CAPY_BIN" cutout doctor >/dev/null
"$CAPY_BIN" motion --help >/dev/null
"$CAPY_BIN" motion help agent >/dev/null
"$CAPY_BIN" motion help manifest >/dev/null
"$CAPY_BIN" motion help prompt-pack >/dev/null
"$CAPY_BIN" motion help qa >/dev/null
"$CAPY_BIN" motion help preview >/dev/null
motion_dry_run="$("$CAPY_BIN" motion cutout \
  --input tmp/nonexistent-motion-source.mp4 \
  --out target/capy-motion-dry-run \
  --dry-run)"
if ! jq -e '.schema == "capy.motion.cutout-plan.v1" and (.files | index("manifest.json")) != null and (.files | index("prompts/process.md")) != null' \
  <<<"$motion_dry_run" >/dev/null; then
  echo "project check failed: motion cutout dry-run must describe the motion package contract" >&2
  exit 1
fi
rm -rf target/capy-motion-prompt-pack
motion_prompt_pack="$("$CAPY_BIN" motion prompt-pack \
  --input tmp/nonexistent-motion-source.mp4 \
  --out target/capy-motion-prompt-pack)"
if ! jq -e '.schema == "capy.motion.prompt_pack.v1" and (.files | length) == 4' \
  <<<"$motion_prompt_pack" >/dev/null; then
  echo "project check failed: motion prompt-pack must write four AI handoff prompts" >&2
  exit 1
fi
test -f target/capy-motion-prompt-pack/README.md
test -f target/capy-motion-prompt-pack/process.md
test -f target/capy-motion-prompt-pack/qa-review.md
test -f target/capy-motion-prompt-pack/app-integration.md
"$CAPY_BIN" timeline doctor \
  --recorder tmp/nonexistent-capy-recorder >/dev/null
"$CAPY_BIN" image providers >/dev/null
"$CAPY_BIN" image doctor >/dev/null
"$CAPY_BIN" game-assets --help >/dev/null
"$CAPY_BIN" game-assets help agent >/dev/null
rm -rf target/capy-game-assets-sample
game_assets_sample="$("$CAPY_BIN" game-assets sample \
  --preset forest-action-rpg-compact \
  --out target/capy-game-assets-sample \
  --overwrite)"
if ! jq -e '.ok == true and .schema == "capy.game_assets.sample.v1" and .frame_count >= 16' \
  <<<"$game_assets_sample" >/dev/null; then
  echo "project check failed: game-assets sample must create a compact verifiable pack" >&2
  exit 1
fi
game_assets_verify="$("$CAPY_BIN" game-assets verify \
  --pack target/capy-game-assets-sample/pack.json)"
if ! jq -e '.verdict == "passed" and .asset_count >= 5 and .frame_count >= 16' \
  <<<"$game_assets_verify" >/dev/null; then
  echo "project check failed: game-assets verify must pass compact sample pack" >&2
  exit 1
fi
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
"$CAPY_BIN" component validate --path fixtures/components >/dev/null
"$CAPY_BIN" component inspect \
  --path fixtures/timeline/video-editing/components/html.capy-title >/dev/null
"$CAPY_BIN" poster export \
  --input fixtures/poster/v1/single-poster.json \
  --out target/capy-poster-v1-check \
  --formats svg,png,pdf,pptx,json >/dev/null
if ! jq -e '.schema == "capy.poster.export.v1" and (.pages | length) == 1 and (.pptx_path | length) > 0' \
  target/capy-poster-v1-check/manifest.json >/dev/null; then
  echo "project check failed: poster v1 export must emit SVG/PNG/PDF/PPTX manifest" >&2
  exit 1
fi
"$CAPY_BIN" poster export \
  --input fixtures/poster/v1/shared-component.json \
  --out target/capy-poster-shared-component-check \
  --formats svg,png,json >/dev/null
if ! jq -e '.schema == "capy.poster.export.v1" and (.pages | length) == 1 and (.pages[0].svg_path | length) > 0' \
  target/capy-poster-shared-component-check/manifest.json >/dev/null; then
  echo "project check failed: shared component poster fixture must export SVG/PNG from component package" >&2
  exit 1
fi
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
project_design_language_validate="$("$CAPY_BIN" project design-language validate \
  --project target/capy-project-html-context)"
if ! jq -e '.schema_version == "capy.design-language.validation.v1" and .ok == true and (.design_language_ref | startswith("dlpkg-fnv1a64-")) and .summary.token_count == 1 and .summary.reference_image_count == 1' \
  <<<"$project_design_language_validate" >/dev/null; then
  echo "project check failed: project design-language validate must expose a stable ref and local asset summary" >&2
  exit 1
fi
project_design_language_inspect="$("$CAPY_BIN" project design-language inspect \
  --project target/capy-project-html-context)"
if ! jq -e '.schema_version == "capy.design-language.inspection.v1" and (.manifest.assets | length) == 3 and .summary.rule_count >= 1' \
  <<<"$project_design_language_inspect" >/dev/null; then
  echo "project check failed: project design-language inspect must expose package metadata and rule summary" >&2
  exit 1
fi
project_workbench="$("$CAPY_BIN" project workbench \
  --project target/capy-project-html-context)"
if ! jq -e '.schema_version == "capy.project-workbench.v1" and (.cards | length) == 6 and any(.cards[]; .kind == "export_center") and (.design_language_summary.design_language_ref | startswith("dlpkg-fnv1a64-"))' \
  <<<"$project_workbench" >/dev/null; then
  echo "project check failed: project workbench must expose six cards and active design language summary" >&2
  exit 1
fi
"$CAPY_BIN" context build \
  --project target/capy-project-html-context \
  --artifact art_00000000000000000000000000000001 \
  --selector '[data-capy-section="hero-title"]' \
  --out target/capy-project-html-context/context.json >/dev/null
if ! jq -e '.schema_version == "capy.context.v1" and .artifact_id == "art_00000000000000000000000000000001" and (.design_language_ref | startswith("dlpkg-fnv1a64-")) and (.design_language_refs | length) == 2' \
  target/capy-project-html-context/context.json >/dev/null; then
  echo "project check failed: project context build must include artifact and design language package ref" >&2
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
generate_dry_run="$("$CAPY_BIN" project generate \
  --project target/capy-project-html-context \
  --artifact art_00000000000000000000000000000001 \
  --provider fixture \
  --prompt "Make launch copy clearer" \
  --dry-run)"
if ! jq -e '.run.schema_version == "capy.project-generate-run.v1" and .run.status == "planned" and .run.dry_run == true and (.run.design_language_ref | startswith("dlpkg-fnv1a64-"))' \
  <<<"$generate_dry_run" >/dev/null; then
  echo "project check failed: project generate dry-run must return planned generate run with design language ref" >&2
  exit 1
fi
if grep -q 'Capybara CLI draft' target/capy-project-html-context/web/index.html; then
  echo "project check failed: project generate dry-run must not mutate HTML source" >&2
  exit 1
fi
"$CAPY_BIN" patch apply \
  --project target/capy-project-html-context \
  --patch fixtures/project/html-context/patches/headline.json >/dev/null
if ! grep -q 'Project Context Locked' target/capy-project-html-context/web/index.html; then
  echo "project check failed: patch apply must mutate HTML source" >&2
  exit 1
fi
"$CAPY_BIN" project generate \
  --project target/capy-project-html-context \
  --artifact art_00000000000000000000000000000001 \
  --provider fixture \
  --prompt "Make launch copy clearer" \
  --write >/dev/null
if ! grep -q 'Capybara CLI draft' target/capy-project-html-context/web/index.html; then
  echo "project check failed: project generate --write must mutate HTML source" >&2
  exit 1
fi
rm -rf target/capy-project-ai-live
cp -R fixtures/project/html-context target/capy-project-ai-live
project_ai_dry_run="$("$CAPY_BIN" project generate \
  --project target/capy-project-ai-live \
  --artifact art_00000000000000000000000000000001 \
  --provider codex \
  --prompt "Make the launch page clearer" \
  --live \
  --sdk-response fixtures/project/html-context/sdk-response/project-ai-html.json \
  --save-prompt target/capy-project-ai-live/design-language-prompt.json \
  --dry-run)"
if ! jq -e '.run.schema_version == "capy.project-generate-run.v1" and .run.status == "planned" and .run.output.mode == "live" and (.run.design_language_ref | startswith("dlpkg-fnv1a64-"))' \
  <<<"$project_ai_dry_run" >/dev/null; then
  echo "project check failed: project AI dry-run must return a planned live run with design language ref" >&2
  exit 1
fi
if ! jq -e '.schema_version == "capy.project-ai-prompt.v1" and (.design_language_ref | startswith("dlpkg-fnv1a64-")) and (.prompt | contains("design_language_ref"))' \
  target/capy-project-ai-live/design-language-prompt.json >/dev/null; then
  echo "project check failed: saved project AI prompt must cite design language ref" >&2
  exit 1
fi
if grep -q 'Project Context Launch' target/capy-project-ai-live/web/index.html; then
  echo "project check failed: project AI dry-run must not mutate HTML source" >&2
  exit 1
fi
"$CAPY_BIN" project generate \
  --project target/capy-project-ai-live \
  --artifact art_00000000000000000000000000000001 \
  --provider codex \
  --prompt "Make the launch page clearer" \
  --live \
  --sdk-response fixtures/project/html-context/sdk-response/project-ai-html.json \
  --write >/dev/null
if ! grep -q 'Project Context Launch' target/capy-project-ai-live/web/index.html; then
  echo "project check failed: project AI --live --write must mutate HTML source" >&2
  exit 1
fi

echo "project check passed"
