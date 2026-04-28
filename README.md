# Capybara

Capybara is a local-first AI design desktop workspace.

The open-source core is a thin Rust desktop shell with a CLI-first control surface. The product direction is: desktop first, headless first, AI-operable, and no Electron.

## Status

`v0.4` is the desktop foundation: bundled CEF/Chromium + tao is merged on `main`. `v0.2` remains only as the legacy wry/tao baseline and rollback reference.

**v0.13** · NextFrame fusion complete. Capybara is now a creative AI workstation built on a single timeline/composition engine. Poster JSON, scroll media, brand tokens all flow through `capy nextframe compose-poster` into the NextFrame pipeline.

- `capy` CLI for AI-friendly operation.
- `capy-image-gen` provider-neutral image generation tool module.
- `capy-scroll-media` video-to-scroll-HTML packager with small committed examples.
- `capy-shell` CEF/Chromium + tao desktop shell POC.
- Unix socket NDJSON IPC.
- Native macOS window capture.
- DOM inspection and probe screenshot commands.
- Local SQLite conversation persistence for Claude/Codex runtime experiments.
- Native HTML/CSS/JS frontend, no React/Vue/Next.js.
- Future desktop mainline: CEF/Chromium + tao, with wry retained only as legacy baseline/rollback.

## Repository Layout

```text
crates/capy-cli/      CLI entrypoint
crates/capy-image-gen/ Image generation provider contract and adapters
crates/capy-scroll-media/ Video in, scroll HTML package out
crates/capy-shell/    CEF desktop shell, IPC, capture, store, agent runtime
frontend/capy-app/    native HTML/CSS/JS UI loaded by the shell
scripts/              local developer gates
```

Private product specs, research, evidence, and concept assets are not tracked in this public repository. They live in a separate private repository.

## Development

```bash
scripts/check-project.sh
scripts/check-architecture.sh
CAPYBARA_SOCKET=/tmp/capybara-main-cef-$(id -u).sock scripts/verify-cef-shell.sh --keep-open
cargo wef build -p capy-shell
cargo run -p capy-cli -- --help
target/debug/capy open --project=demo
target/debug/capy ps
target/debug/capy state --key=app.ready
target/debug/capy devtools --eval='document.documentElement.dataset.capyBrowser'
target/debug/capy devtools --query=.topbar --get=bounding-rect
target/debug/capy verify
target/debug/capy image providers
target/debug/capy image doctor
target/debug/capy image generate --dry-run 'Scene: Warm studio. Subject: One ceramic cup centered. Important details: Product photo, soft light. Use case: Hero card. Constraints: No text, no watermark.'
crates/capy-scroll-media/examples/build-examples.sh
target/debug/capy media serve --root crates/capy-scroll-media/examples/outputs/card-pan-2s --port 5202
target/debug/capy capture --out=tmp/capy-window.png
target/debug/capy quit
```

Use the same `CAPYBARA_SOCKET` value for both shell and CLI when multiple worktrees are running.

## Current CLI

```text
capy shell
capy open
capy ps
capy state
capy devtools
capy screenshot
capy capture
capy verify
capy image
capy media
capy chat
capy agent doctor
capy quit
```

## License

MIT
