# Capybara

Capybara is a local-first AI design desktop workspace.

The open-source core is a thin Rust desktop shell with a CLI-first control surface. The product direction is: desktop first, headless first, AI-operable, and no Electron.

## Status

`v0.2` is a macOS shell baseline:

- `capy` CLI for AI-friendly operation.
- `capy-shell` wry/tao desktop shell.
- Unix socket NDJSON IPC.
- Native macOS window capture.
- DOM inspection and probe screenshot commands.
- Local SQLite conversation persistence for Claude/Codex runtime experiments.
- Native HTML/CSS/JS frontend, no React/Vue/Next.js.

## Repository Layout

```text
crates/capy-cli/      CLI entrypoint
crates/capy-shell/    desktop shell, IPC, capture, store, agent runtime
frontend/capy-app/    native webview UI loaded by the shell
scripts/              local developer gates
```

Private product specs, research, evidence, and concept assets are not tracked in this public repository. They live in a separate private repository.

## Development

```bash
scripts/check-project.sh
cargo run -p capy-cli -- --help
cargo run -p capy-cli -- shell
target/debug/capy open --project=demo
target/debug/capy ps
target/debug/capy state --key=app.ready
target/debug/capy devtools --query=.topbar --get=bounding-rect
target/debug/capy verify
target/debug/capy capture --out=tmp/capy-window.png
target/debug/capy quit
```

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
capy chat
capy agent doctor
capy quit
```

## License

MIT
