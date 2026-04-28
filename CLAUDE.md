# Capybara · AI Entry

Capybara is a local-first AI design desktop workspace. This public repository contains the open-source core code only.

Private product specs, roadmap, research, concept assets, and evidence live at `spec/` inside this workspace. That directory is ignored by the public repository and is its own private git repository. Do not add `spec/` to the public repo index.

## Current State

`v0.4` is the desktop foundation: bundled CEF/Chromium + tao is merged on `main`. `v0.2` remains only as the legacy wry/tao baseline and rollback reference.

- `crates/capy-cli` - `capy` CLI and AI verification entrypoint.
- `crates/capy-shell` - CEF/Chromium + tao shell, browser adapter boundary, Unix socket IPC, native capture, traffic light alignment, SQLite store, and agent runtime adapters.
- `frontend/capy-app` - native HTML/CSS/JS desktop UI.

## Commands

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
target/debug/capy screenshot --out=tmp/capy-dom.png
target/debug/capy capture --out=tmp/capy-window.png
target/debug/capy verify
target/debug/capy quit
```

Use the same `CAPYBARA_SOCKET` value for both shell and CLI when multiple worktrees are running.

## AI 验证接口

```bash
target/debug/capy nextframe doctor
target/debug/capy nextframe compose-poster --input <poster.json> --out <dir> [--brand-tokens <css>]
target/debug/capy nextframe validate --composition <path>
target/debug/capy nextframe compile --composition <path>
target/debug/capy nextframe attach --canvas-node <id> --composition <path>
target/debug/capy nextframe state [--canvas-node <id>]
target/debug/capy nextframe open --canvas-node <id>
target/debug/capy nextframe snapshot --composition <path> [--frame <ms>]
target/debug/capy nextframe export --composition <path> --kind mp4 [--fps <int>]
target/debug/capy nextframe status --job <id>
target/debug/capy nextframe cancel --job <id>
target/debug/capy nextframe verify-export --composition <path>
target/debug/capy nextframe rebuild --composition <path>
```

## NextFrame Fusion (v0.13)

`capy-nextframe` is the single boundary between Capybara and the NextFrame engine. NextFrame ships as a git submodule at `external/NextFrame` and is consumed via direct crate dependencies (nf-project, nf-recorder).

### CLI commands(已在"AI 验证接口"段下增补)

- `capy nextframe doctor` · adapter health check (mode=crate-only)
- `capy nextframe compose-poster --input <poster.json> --out <dir> [--brand-tokens <css>]`
- `capy nextframe validate --composition <path>`
- `capy nextframe compile --composition <path>`
- `capy nextframe attach --canvas-node <id> --composition <path>`
- `capy nextframe state [--canvas-node <id>]`
- `capy nextframe open --canvas-node <id>`
- `capy nextframe snapshot --composition <path> [--frame <ms>]`
- `capy nextframe export --composition <path> --kind mp4 [--fps <int>]`
- `capy nextframe status --job <id>` / `cancel --job <id>`
- `capy nextframe verify-export --composition <path>` · 端到端 + evidence/index.html
- `capy nextframe rebuild --composition <path>` · token 变后重编

### Pipeline

`Poster JSON | Scroll-Media → capy nextframe compose-poster → composition.json → validate → compile → snapshot/export → evidence`

### Frontend integration

- Canvas node kind=`nextframe-composition` · attach 命令绑定
- iframe preview 由 capy-shell 内置 127.0.0.1 micro-server 服务
- PM inspector aside (`window.capyWorkbench.openNextFrameInspector`) 全链状态可视

## Public Repo Rules

- Do not commit `spec/`.
- Do not add Electron or a heavy desktop framework.
- Do not add React, Vue, Next.js, Tailwind, or shadcn.
- Future desktop shell work must target CEF/Chromium + tao, not deeper system WebView/wry expansion. Keep wry only as legacy baseline/rollback unless private architecture spec changes.
- Keep the frontend native HTML/CSS/JS unless the private architecture spec changes.
- Keep user-facing AI-operability through `capy` commands.
- Do not share a design preview, local HTML URL, browser UI, or desktop UI as usable until it has real visible verification: screenshot/capture, DOM or state checks, one interaction check, and console/error checks with evidence saved under private `spec/`.
- For localhost URLs, also verify the delivery surface itself: a process is listening, `curl -I <url>` returns `200`, and the service remains available after Playwright finishes. Do not rely on macOS `open` or a transient shell background process as proof.

## Private Spec Pairing

Expected local layout:

```text
/Users/Zhuanz/workspace/capybara
/Users/Zhuanz/workspace/capybara/spec  # private nested git repo, ignored by public git
```
