# Capybara

Capybara is a local-first AI design desktop workspace. The public repository is
the open-source core: Rust crates, a native HTML/CSS/JS desktop UI, CLI
automation, and local verification scripts.

Private product specs, roadmap, research, prompts, concept assets, and evidence
live in the nested private `spec/` repository. Do not add `spec/` to the public
repo index.

## AI Entry

- `AGENTS.md` is the source project entry for AI agents.
- `CLAUDE.md` is an exact plain-file copy for Claude Code compatibility.
- `README.md` is only the public repo overview; it is not the current-version
  truth source.
- Discover active and parallel versions from `spec/versions/REGISTRY.json`,
  then open the relevant version `status.json` before reading BDD, architecture,
  evidence, or devlog details.

This project assumes multi-model and multi-worktree development. Use progressive
disclosure: read the registry and status first, then only the task-relevant spec
layer.

## Architecture

- Desktop shell: CEF/Chromium + tao.
- Frontend: native HTML/CSS/JS, loaded by the shell.
- AI operation: `capy` CLI, Unix socket IPC, state/devtools/screenshot/capture
  commands, and repeatable verification scripts.
- Creative pipeline: Capybara-owned Timeline, poster, scroll-media, canvas,
  recorder, and provider-boundary crates.
- Persistence and runtime: local-first, CLI-observable, evidence-oriented.

## Repository Layout

```text
crates/capy-cli/              capy CLI and AI verification entrypoint
crates/capy-shell/            CEF desktop shell, IPC, capture, store, runtime
crates/capy-contracts/        shared CLI/shell wire contracts
crates/capy-canvas-core/      canvas data model and operations
crates/capy-canvas-web/       canvas web/WASM adapter
crates/capy-image-gen/        provider-neutral image generation boundary
crates/capy-poster/           Poster JSON parsing and composition helpers
crates/capy-scroll-media/     video/story media packaging and range serving
crates/capy-timeline/         Timeline product boundary
crates/capy-timeline-project/ internal Timeline project engine
crates/capy-recorder/         internal snapshot/export recorder
crates/capy-shell-mac/        macOS shell helpers
frontend/capy-app/            native desktop UI
scripts/                      local gates, verifiers, and repo checks
```

## Development

```bash
scripts/check-spec-structure.sh
scripts/check-architecture.sh
scripts/check-commit.sh
scripts/check-project.sh
cargo wef build -p capy-shell
cargo run -p capy-cli -- --help
```

For desktop verification, use the same socket for shell and CLI when multiple
worktrees are running:

```bash
CAPYBARA_SOCKET=/tmp/capybara-main-cef-$(id -u).sock scripts/verify-cef-shell.sh --launch launchctl --keep-open
```

## CLI Surface

```text
capy open
capy ps
capy state
capy devtools
capy screenshot
capy capture
capy verify
capy timeline
capy canvas
capy image
capy media
capy chat
capy agent doctor
capy quit
```

Run `cargo run -p capy-cli -- --help` and command-specific `--help` before using
new or unfamiliar commands.

## Maintenance Contract

- Update `AGENTS.md` first for project-entry changes, then copy it to
  `CLAUDE.md`.
- Keep volatile product state out of `README.md`; write it to `spec/README.md`,
  `spec/versions/REGISTRY.json`, and the relevant version `status.json`.
- When behavior, CLI output, IPC shape, runtime commands, module boundaries, or
  evidence process changes, update code and matching private spec in the same
  work unit.
- Run `scripts/check-spec-structure.sh` after entry/spec changes.
- Run `scripts/check-architecture.sh` after architecture, naming, module, or
  public README changes.
- Do not add Electron, Tauri, React, Vue, Next.js, Tailwind, or shadcn unless the
  private architecture spec changes.

## License

MIT
