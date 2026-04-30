# Capybara · AI Entry

Capybara is a local-first AI design desktop workspace. This public repository
contains the open-source core code only. Private product specs, roadmap,
research, prompts, generated assets, and evidence live in the nested private
repo at `spec/`; do not add `spec/` to the public repo index.

User-level AGENTS rules define how AI agents work. This file records only
Capybara-specific local facts. If documents disagree, follow this order:
user-level AGENTS rules, then `spec/`, then project scripts, then this entry.

`AGENTS.md` is the source project entry for this repo. `CLAUDE.md` is a
plain-file copy kept only for Claude Code compatibility; update `AGENTS.md`
first, then copy it to `CLAUDE.md`.

## Progressive Disclosure

Do not load the whole spec tree or hard-code one active version from this file.
Capybara assumes multi-model, multi-worktree work. Discover the current task
layer by layer, then read only the docs needed for that layer.

## Final Product Direction

Capybara's current design is locked by `spec/product-philosophy.md` and the
current `focus_version` directory discovered from `spec/versions/REGISTRY.json`:

```text
Project 承接一切
AI 根据项目级设计语言生产一切
桌面端是给 AI 提供 context 的入口
Capybara 负责展示、编辑、验证和证据
```

Do not treat Capybara as a low-code template engine or a fixed component
assembler. Project-level design language is AI context: markdown, JSON, CSS,
HTML, reference images, examples, approved outputs, constraints, and feedback
are all valid context assets. The desktop product's job is to display AI output
on surfaces, preserve source artifacts, turn user selection into precise context,
apply patches, verify results, export, and leave evidence.

## Start Here

1. Read `spec/README.md` for the truth map and write destinations.
2. Read `spec/versions/REGISTRY.json` to discover active and parallel versions,
   branches, worktrees, owners, stages, dependencies, and status.
3. Open the relevant version `status.json` first. Use it to choose the current
   task, owner, blockers, evidence directory, and next needed docs.
4. Then read only the task-relevant layer:
   - Scope or PM question: version `brief.md` and `bdd.json`.
   - Architecture or contracts: `spec/architecture.md`, `spec/data-model.md`,
     `spec/interfaces.md`, `spec/runtime.md`, and `spec/standards/`.
   - Implementation: the owning crate/module plus matching BDD/status entries.
   - Verification: `spec/ai-verify/`, version `evidence/`, and relevant gates.
   - History or root cause: `spec/devlog/`, `spec/pocs/`, and `bugs.json`.
5. For parallel agent work, respect `status.json` owners/dependencies and record
   new parallel tasks there before splitting work.
6. Check public repo and nested spec repo status before editing.

## What Goes Where

- Product principles and red lines: `spec/charter.md`
- Architecture and module ownership: `spec/architecture.md`, `spec/standards/project/module-ownership.md`
- Data models and state shapes: `spec/data-model.md`
- CLI, IPC, JS bridge, file contracts: `spec/interfaces.md`
- Runtime commands, env vars, gates, troubleshooting: `spec/runtime.md`
- AI verification commands and scenarios: `spec/ai-verify/`
- Version goals, BDD, bugs, evidence, report: `spec/versions/<version>/`
- Contract fixtures and evidence manifest schema: `spec/contracts/`
- Process decisions and root causes: `spec/devlog/`
- Design rules and visual examples: `spec/design/`
- Evidence retention, privacy, provider spend: `spec/standards/project/`

Code and spec move together. If behavior, data shape, CLI output, IPC contract,
runtime command, module boundary, or evidence process changes, update the
matching spec file in the same work unit.

## Code Map

- `crates/capy-cli`: `capy` CLI and AI verification entrypoint.
- `crates/capy-shell`: CEF/Chromium + tao shell, IPC, bridge, capture, store,
  and agent runtime orchestration.
- `crates/capy-contracts`: shared wire types.
- `crates/capy-canvas-core`, `crates/capy-canvas-web`: canvas model and web/WASM adapter.
- `crates/capy-image-gen`: provider-neutral image generation boundary.
- `crates/capy-poster`: Poster JSON parsing and render-source compilation.
- `crates/capy-scroll-media`: scroll/story media packaging and range serving.
- `crates/capy-timeline`: Capybara Timeline product boundary.
- `crates/capy-timeline-project`, `crates/capy-recorder`, `crates/capy-shell-mac`: Capybara-owned internal engine crates.
- `frontend/capy-app`: native HTML/CSS/JS desktop UI.

## Required Gates

```bash
scripts/lint-spec.sh
scripts/check-spec-structure.sh
scripts/check-architecture.sh
scripts/check-commit.sh
scripts/check-project.sh
CAPYBARA_SOCKET=/tmp/capybara-main-cef-$(id -u).sock scripts/verify-cef-shell.sh --launch launchctl --keep-open
```

Use the same `CAPYBARA_SOCKET` for shell and CLI when multiple worktrees run.

## Product CLI

All Capybara operations, including internal `[dev]` debugging tools and
PM-facing product workflows, use CLI progressive disclosure:

```bash
target/debug/capy --help
target/debug/capy help
target/debug/capy help <topic>
target/debug/capy <command> --help
target/debug/capy <command> help <topic>
```

Rules:

- `capy --help` is the compact total index. Do not turn it into a long manual.
- Internal automation and verification commands stay visible and carry `[dev]`.
- Real workflows and `[dev]` tools both point to self-contained topic help.
- Before using an unfamiliar Capybara CLI surface, read its `--help` and, when
  listed, the matching `help <topic>` first.
- Keep this contract green with `scripts/verify-capy-cli-help.sh`.

Common examples:

```bash
cargo run -p capy-cli -- --help
target/debug/capy open --project=demo
target/debug/capy ps
target/debug/capy state --key=app.ready
target/debug/capy devtools --eval='document.documentElement.dataset.capyBrowser'
target/debug/capy screenshot --out=tmp/capy-dom.png
target/debug/capy capture --out=tmp/capy-window.png
target/debug/capy verify
target/debug/capy quit
```

Debugging multiple desktop surfaces:

```bash
scripts/open-debug-shell.sh --name v19-a --windows 2
scripts/open-debug-shell.sh --name v19-b --socket=/tmp/capybara-v19-b-$(id -u).sock
```

Use `scripts/open-debug-shell.sh` for parallel desktop debugging. It launches a
separate app process with an explicit socket and launchctl label, so one debug
window cannot accidentally answer another window's CLI commands.

Timeline surface is canonical:

```bash
target/debug/capy timeline doctor
target/debug/capy timeline compose-poster --input <poster.json> --out <dir> [--brand-tokens <css>]
target/debug/capy timeline validate --composition <path>
target/debug/capy timeline compile --composition <path>
target/debug/capy timeline attach --canvas-node <id> --composition <path>
target/debug/capy timeline state [--canvas-node <id>]
target/debug/capy timeline open --canvas-node <id>
target/debug/capy timeline snapshot --composition <path> [--frame <ms>]
target/debug/capy timeline export --composition <path> --kind mp4 [--fps <int>]
target/debug/capy timeline verify-export --composition <path>
```

## Red Lines

- Do not commit `spec/` into the public repo.
- Do not add Electron, Tauri, React, Vue, Next.js, Tailwind, or shadcn.
- Keep desktop work on CEF/Chromium + tao; wry is legacy rollback only.
- Keep frontend native HTML/CSS/JS unless private architecture spec changes.
- Do not reintroduce external Timeline/old product aliases or submodules.
- Do not grow files past `scripts/check-large-files.sh`; split modules first.
- Do not expose provider secrets or live-spend provider calls in logs/evidence.
- Do not share UI/browser/desktop/local URL handoffs without real visible
  verification, service availability checks, and evidence saved under `spec/`.

## Common Pitfalls

- Reading only this file and missing `spec/README.md`.
- Updating code without active-version BDD/status/evidence.
- Treating Timeline as an external product instead of Capybara-owned capability.
- Saving mixed stdout/stderr into `.json`; use `.log`, `.txt`, or `.jsonl`.
- Relying on `macOS open` or a transient server as proof of a usable URL.
