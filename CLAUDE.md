# Capybara · AI Entry

Capybara is a local-first AI design desktop workspace. This public repository contains the open-source core code only.

Private product specs, roadmap, research, concept assets, and evidence live at `spec/` inside this workspace. That directory is ignored by the public repository and is its own private git repository. Do not add `spec/` to the public repo index.

## Current State

`v0.2` mac shell baseline:

- `crates/capy-cli` - `capy` CLI and AI verification entrypoint.
- `crates/capy-shell` - wry/tao macOS shell, custom protocol, Unix socket IPC, native capture, traffic light alignment, SQLite store, and agent runtime adapters.
- `frontend/capy-app` - native HTML/CSS/JS webview UI.

## Commands

```bash
scripts/check-project.sh
cargo run -p capy-cli -- --help
cargo run -p capy-cli -- shell
target/debug/capy open --project=demo
target/debug/capy ps
target/debug/capy state --key=app.ready
target/debug/capy devtools --query=.topbar --get=bounding-rect
target/debug/capy screenshot --out=tmp/capy-dom.png
target/debug/capy capture --out=tmp/capy-window.png
target/debug/capy verify
target/debug/capy quit
```

## Public Repo Rules

- Do not commit `spec/`.
- Do not add Electron or a heavy desktop framework.
- Do not add React, Vue, Next.js, Tailwind, or shadcn.
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
