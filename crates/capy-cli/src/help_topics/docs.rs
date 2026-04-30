pub(super) const DEV_HELP: &str = r#"
Topic: capy dev

Use when: AI needs the internal operation index before verifying or automating Capybara.
Required parameters: none.
Meaning of `[dev]`: internal AI/dev verification or automation command. It is safe to expose in `capy --help`, but it is not a PM-facing product workflow.
Registered `[dev]` commands:
1. Lifecycle: `shell`, `open`, `ps`, `quit`
2. Read/inspect: `doctor`, `state`, `devtools`, `verify`
3. Visible evidence: `screenshot`, `capture`
4. UI automation: `click`, `type`
5. Runtime inspection: `agent`, `agent sdk`
Product workflow commands without `[dev]`: `chat`, `canvas`, `image`, `cutout`, `motion`, `tts`, `clips`, `media`, `timeline`.
Recommended commands:
1. `target/debug/capy --help`
2. `target/debug/capy help`
3. `target/debug/capy help desktop`
4. `target/debug/capy help interaction`
Project context commands: `project`, `context`, `patch`.
Do not: hide these commands from help; run `click`/`type` on a user's active window without an isolated `CAPYBARA_SOCKET`; treat `[dev]` commands as a substitute for final product evidence.
Next step: read `capy help doctor`, `capy help interaction`, or `capy help desktop` for the exact workflow.
"#;

pub(super) const PROJECT_HELP: &str = r#"
Topic: capy project

Use when: AI or a workflow needs a local `.capy` file package that carries project metadata, design-language assets, source artifacts, runs, and evidence.
Required parameters: every command needs `--project <dir>`. `generate` needs `--artifact <id>`, `--provider fixture|codex|claude`, and `--prompt <text>`. Add `--live` to actually call the Claude/Codex SDK.
Recommended commands:
1. `target/debug/capy project init --project <dir> --name "Campaign"`
2. `target/debug/capy project add-design --project <dir> --path design/tokens.css --kind css --title "Tokens"`
3. `target/debug/capy project add-artifact --project <dir> --path web/index.html --kind html --title "Landing" --design-ref <dl_id>`
4. `target/debug/capy project inspect --project <dir>`
5. `target/debug/capy project workbench --project <dir>`
6. `target/debug/capy project generate --project <dir> --artifact <art_id> --provider fixture --prompt "Make this clearer" --dry-run`
7. Copy the project, then run `target/debug/capy project generate --project <copy> --artifact <art_id> --provider fixture --prompt "Make this clearer" --write`
8. For real AI output, run `target/debug/capy project generate --project <copy> --artifact <art_id> --provider codex --prompt "Make this clearer" --live --write`
Do not: place project source outside the project root; treat `.capy` as generated garbage; register derived screenshots as the editable source artifact; let models edit `.capy` metadata directly; run live `codex` or `claude` provider commands when no-spend fixture mode is enough.
Next step: build context with `capy context build --project <dir> --artifact <art_id>`, or open the desktop workbench for visible card evidence.
"#;

pub(super) const PROJECT_CONTEXT_HELP: &str = r#"
Topic: capy context

Use when: AI needs a precise project context package before editing an artifact.
Required parameters: `build` needs `--project <dir>` and `--artifact <art_id>`.
Optional parameters: `--selector <css>` records the user-selected DOM target; `--canvas-node <id>` records the visible surface node; `--out <json>` saves the packet.
Recommended command: `target/debug/capy context build --project <dir> --artifact <art_id> --selector '[data-capy-section="hero-title"]' --out target/context.json`
Do not: call a model from this command; paste screenshots without the JSON packet; invent artifact ids.
Next step: create a `capy.patch.v1` document and run `capy patch apply --dry-run`.
"#;

pub(super) const PROJECT_PATCH_HELP: &str = r#"
Topic: capy patch

Use when: AI or UI has a proposed edit to a real project source artifact.
Required parameters: `apply` needs `--project <dir>` and `--patch <json>`.
Patch schema: `capy.patch.v1`; first operation is `replace_exact_text` with `artifact_id`, `old_text`, `new_text`, optional `source_path`, and optional `selector_hint`.
Recommended commands:
1. `target/debug/capy patch apply --project <dir> --patch patch.json --dry-run`
2. `target/debug/capy patch apply --project <dir> --patch patch.json`
3. `target/debug/capy context build --project <dir> --artifact <art_id> --selector <css>`
Do not: patch derived screenshots or outputs; skip dry-run for AI-generated patches; use vague text that matches multiple places.
Next step: reopen or refresh the surface and capture evidence.
"#;

pub(super) const DOCTOR_HELP: &str = r#"
Topic: capy doctor

Use when: AI needs to decide whether Capybara is ready before running desktop, asset, agent, TTS, clips, media, or Timeline workflows.
Required parameters: none.
Recommended commands:
1. `target/debug/capy doctor`
2. Read `domain_doctors[]` in the JSON.
3. Run the domain doctor for the workflow you will perform next, for example `target/debug/capy clips doctor` or `target/debug/capy tts doctor`.
Do not: treat `capy doctor` as proof that the desktop UI is visible; use `capy verify --profile desktop --capture-out <png>` for real UI evidence from built-in app-view capture.
Next step: save the JSON into version evidence, then run the workflow-specific doctor.
"#;

pub(super) const INTERACTION_HELP: &str = r#"
Topic: capy interaction

Use when: AI needs to click or type in the live Capybara desktop UI.
Required parameters: `click` needs `--query <css>`; `type` needs `--query <css> --text <text>`; both need the shell running on the same `CAPYBARA_SOCKET`.
Recommended commands:
1. `target/debug/capy devtools --query <css> --get=bounding-rect`
2. `target/debug/capy click --query <css>`
3. `target/debug/capy type --query <css> --text "hello" --clear`
4. `target/debug/capy devtools --query <css> --get=value`
Do not: skip the selector probe; use browser-coordinate guesses; mutate product state with ad hoc `devtools --eval` when click/type can express the action.
Next step: capture state with `capy state`, `capy devtools`, or `capy capture` and save it into evidence.
"#;

pub(super) const DESKTOP_HELP: &str = r#"
Topic: capy desktop

Use when: AI must open, inspect, capture, or verify the desktop shell.
Required parameters: `capture`/`screenshot` need `--out`; `verify --profile desktop` needs `--capture-out`; keep one `CAPYBARA_SOCKET` across shell and CLI.
Recommended commands:
0. `target/debug/capy doctor`
1. `target/debug/capy open --project=demo`
2. `target/debug/capy ps`
3. `target/debug/capy state --key=app.ready`
4. `target/debug/capy devtools --eval='document.documentElement.dataset.capyBrowser'`
5. `target/debug/capy verify --profile desktop --capture-out target/capy-desktop.png`
Do not: claim UI verified from build/tests alone; mix sockets; run `devtools` without `--query` or `--eval`; use macOS Screen Recording/global screen capture as the default evidence path.
Next step: save JSON output and PNGs into version evidence.
"#;

pub(super) const CANVAS_HELP: &str = r#"
Topic: capy canvas agent

Use when: AI needs live canvas state or node manipulation.
Required parameters: shell must be running; `select`/`move` need `--id`; `move` needs `--x --y`; `create-card` needs `--kind --title --x --y`.
Recommended commands:
1. `target/debug/capy canvas snapshot`
2. `target/debug/capy canvas create-card --kind image --title "Reference" --x 360 --y 140`
3. `target/debug/capy canvas select --id <node_id>`
4. `target/debug/capy canvas move --id <node_id> --x 420 --y 180`
Do not: guess node ids from z-order; skip `snapshot`; use screen pixels as canvas coordinates without checking.
Next step: read `capy canvas help context` or `capy canvas help images`.
"#;

pub(super) const CANVAS_CONTEXT_HELP: &str = r#"
Topic: capy canvas context

Use when: selected canvas content or a region must become an AI-readable context packet.
Required parameters: `capy canvas context export --out <dir>`; optional `--region <x,y,w,h>`.
Recommended command: `target/debug/capy canvas context export --out target/canvas-context`
Do not: send screenshots alone when metadata/geometry can be exported; invent region coordinates blindly.
Next step: attach the packet with `capy chat send --canvas-context <context.json>`.
"#;

pub(super) const CANVAS_IMAGES_HELP: &str = r#"
Topic: capy canvas images

Use when: AI needs to insert a local image or generate one into the live canvas.
Required parameters: insert needs `--path`; generate needs one five-section prompt; add `--cutout-ready` for alpha-cutout sources.
Recommended commands:
1. `target/debug/capy canvas insert-image --path <image.png> --title "Source image"`
2. `target/debug/capy canvas generate-image --dry-run --out target/capy-canvas-image --name demo "<five-section prompt>"`
Do not: use `--live` for smoke tests; omit `--out` and `--name` when later steps need a file.
Next step: for cutout sources, read `capy image help cutout-ready`.
"#;

pub(super) const CHAT_HELP: &str = r#"
Topic: capy chat agent

Use when: AI needs persistent Claude/Codex conversations with events and export.
Required parameters: `send/open/events/stop/export` need `--id`; `send` also needs a prompt. SDK is the default and only runtime.
Recommended commands:
1. `target/debug/capy chat list`
2. `target/debug/capy chat new --provider codex --cwd <repo>`
3. `target/debug/capy chat send --id <id> "Summarize current state"`
4. `target/debug/capy chat events --id <id>`
5. `target/debug/capy chat export --id <id>`
Do not: create throwaway sessions when continuity matters; use `--runtime-backend=cli`; use `--write-code` casually; attach canvas context as prose when `--canvas-context` exists.
Next step: read `capy chat help canvas-tools` for canvas-aware runs.
"#;

pub(super) const CHAT_CANVAS_TOOLS_HELP: &str = r#"
Topic: capy chat canvas-tools

Use when: Claude or Codex should operate Capybara canvas through project-owned CLI commands.
Required parameters: add `--capy-canvas-tools`; optional `--capy-tool-log <jsonl>` records tool calls.
Recommended command: `target/debug/capy chat send --id <id> --capy-canvas-tools --capy-tool-log target/capy-tool-calls.jsonl "Inspect the selected node."`
Do not: ask agents to guess DOM internals; omit tool logging when validating behavior.
Next step: use `capy canvas snapshot` plus the JSONL log as evidence.
"#;

pub(super) const AGENT_HELP: &str = r#"
Topic: capy agent doctor

Use when: AI needs to know whether Capybara's SDK-only agent runtime is available.
Required parameters: none.
Recommended command: `target/debug/capy agent doctor`
Do not: start a long agent run before checking SDK package and provider availability; do not reintroduce direct Claude/Codex CLI backends.
Next step: use `capy agent sdk doctor` for the explicit SDK readiness path, or create a conversation with `capy chat new --provider claude|codex`.
"#;

pub(super) const AGENT_SDK_HELP: &str = r#"
Topic: capy agent sdk

Use when: AI needs to call Claude Agent SDK or Codex SDK through Capybara's public CLI.
Required parameters: `normalize` and `run` need `--provider claude|codex`; `run` also needs `--prompt` or positional prompt text.
Recommended commands:
1. `target/debug/capy agent sdk doctor`
2. `target/debug/capy agent sdk normalize --provider codex --write-code`
3. `target/debug/capy agent sdk normalize --provider claude --write-code`
4. `target/debug/capy agent sdk run --provider codex --cwd <repo> --write-code --prompt "Reply with exactly: ok" --json`
5. `target/debug/capy agent sdk run --provider claude --cwd <repo> --write-code --prompt "Reply with exactly: ok" --json`
Meaning of `--write-code`: full-auto local coding authority. Codex gets `approvalPolicy=never` and `sandbox=danger-full-access`; Claude gets `permissionMode=bypassPermissions` and `allowDangerouslySkipPermissions=true`.
Shared parameters:
- `--cwd <path>` working directory.
- `--model <name>` provider model.
- `--effort <minimal|low|medium|high|xhigh|max>` normalized reasoning effort.
- `--add-dir <path>` extra filesystem root.
- `--allowed-tools`, `--disallowed-tools`, `--tools` Claude tool controls.
- `--mcp-config <json-or-path>` MCP servers.
- `--output-schema <json-or-path>` structured output schema.
- `--max-turns <n>` Claude turn cap.
- `--raw` include native SDK messages/items in JSON output.
Codex parameters:
- `--approval-policy <never|on-request|on-failure|untrusted>`
- `--sandbox <read-only|workspace-write|danger-full-access>`
- `--thread-id <id>` resume SDK thread.
- `--search` enable web search/network flags.
- `--skip-git-repo-check`
- `--codex-config <key=value>` repeated config override.
- `--codex-path <path>` Codex CLI path override.
Claude parameters:
- `--permission-mode <default|acceptEdits|bypassPermissions|plan|dontAsk|auto>`
- `--max-budget-usd <usd>`
- `--setting-source <user|project|local>` repeated setting source.
- `--session-id <uuid>` new session id.
- `--resume <uuid>` resume session.
- `--no-session-persistence`
- `--claude-path <path>` Claude executable path override.
Known provider boundary: Codex SDK rejects reasoning effort `minimal` when image_gen/web_search tools are present; use `low` for smoke runs.
Do not: call `tools/capy-agent-sdk/src/cli.mjs` as the product entrypoint; re-enable the removed direct CLI backend; use `--runtime-backend=cli`.
Next step: for persistent chat, use `capy chat new --provider claude|codex --write-code`.
"#;

pub(super) const IMAGE_HELP: &str = r#"
Topic: capy image

Use when: AI needs provider-neutral image generation with JSON output.
Required parameters: `generate` needs five prompt sections: Scene, Subject, Important details, Use case, Constraints. Use `--out` and `--name` when later steps need a file.
Recommended commands:
1. `target/debug/capy image providers`
2. `target/debug/capy image doctor`
3. `target/debug/capy image generate --dry-run "<five-section prompt>" --size 1:1 --resolution 1k --out <dir> --name <slug>`
4. `target/debug/capy image balance`
Do not: call provider CLIs directly; use short unstructured prompts; run live generation unless spending credits is intended; assume `--cutout-ready` proves the generated pixels are cutout-safe.
Next step: for alpha cutout, read `capy help image-cutout`.
"#;

pub(super) const IMAGE_CUTOUT_HELP: &str = r##"
Topic: image-cutout

Use when: generated image will be passed to `capy cutout run` or `batch`.
Required parameters: add `--cutout-ready`; prompt must include five sections plus a neutral gray background strategy: `#E0E0E0` default, `#E8E8E8` for dark subjects, or `#B8BEC3` for white/light subjects. It must also include `one`/`single`, `fully visible`/`uncropped`, clean edges or strong separation, no extra objects, no text, no watermark, no green screen, no blue screen.
Recommended command: `target/debug/capy image generate --cutout-ready "<prompt>" --size 1:1 --resolution 1k --out target/capy-image --name object`
Prompt template:
```text
Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source.
Subject: One single <object> centered, fully visible, uncropped, 70% frame height.
Important details: Clean silhouette, clear edges, soft even light, strong separation from background.
Use case: Source for automated alpha cutout and transparent PNG UI composition.
Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.
```
Background rules: default to neutral gray `#E0E0E0`; use `#E8E8E8` for dark subjects; use `#B8BEC3` for white/light subjects; always keep background and subject colors clearly separated.
Do not: use green/blue screen, pure white, pure black, gradients, vignettes, floors, cast shadows, reflections, busy backgrounds, or a background color that collides with the subject color; assume the provider obeyed every background/shadow instruction without visual QA.
Next step: run `capy cutout doctor`, `capy cutout init` if needed, then `capy cutout run`; inspect `qa-white.png` and `qa-black.png` before PM-visible use.
"##;

pub(super) const CUTOUT_HELP: &str = r#"
Topic: capy cutout

Use when: a local image must become a transparent PNG.
Required parameters: first setup `capy cutout init`; readiness `capy cutout doctor`; single run needs `--input --output`; recommended `--mask-out --qa-dir --report`.
Recommended command: `target/debug/capy cutout run --input <image.png> --output <cutout.png> --mask-out <mask.png> --qa-dir <qa-dir> --report <report.json>`
Do not: use old fixed-background removal; skip doctor/init; skip QA previews for PM-visible assets; treat a generated source as clean just because it passed `capy image generate --cutout-ready`; use source backgrounds that collide with the subject color.
Next step: open `qa-white.png` and `qa-black.png`; confirm `sips -g hasAlpha <cutout.png>` is `yes`.
"#;

pub(super) const CUTOUT_MANIFEST_HELP: &str = r#"
Topic: capy cutout manifest

Use when: multiple generated assets need cutout in one command.
Required parameters: `--manifest <json> --out-dir <dir>`; manifest has `items[]` with `id`, `input`, optional `output` and `mask`.
Recommended command: `target/debug/capy cutout batch --manifest <manifest.json> --out-dir <out-dir> --report <summary.json>`
Do not: put directories in item `input`; assume quality without checking QA previews.
Next step: read summary JSON and inspect `qa/`.
"#;

pub(super) const MOTION_HELP: &str = r#"
Topic: capy motion

Use when: a real MP4 must become a high-quality moving transparent asset for APP, game, or animation use.
Required parameters: `cutout` needs `--input <mp4>` and `--out <dir>`; use `--quality animation --target all --verify --overwrite` for the full package.
Recommended commands:
1. `target/debug/capy motion doctor`
2. `target/debug/capy motion cutout --input /Users/Zhuanz/Downloads/d_f_d_d_a_bc_be_a_mp_.mp4 --out spec/versions/v0.32-animation-grade-video-cutout/evidence/assets/motion-asset --quality animation --target all --verify --overwrite --evidence-index spec/versions/v0.32-animation-grade-video-cutout/evidence/index.html`
3. `target/debug/capy motion verify --manifest spec/versions/v0.32-animation-grade-video-cutout/evidence/assets/motion-asset/manifest.json`
4. Serve `qa/preview.html` over HTTP and capture browser evidence on multiple backgrounds.
Reuse command: after a full cutout exists, `--reuse-existing` rebuilds QA, manifest, preview HTML, and exports from existing frames without rerunning Focus.
Do not: claim ordinary H.264 MP4 is transparent; judge quality from one still frame; use fixed-background/chroma-key removal; skip `qa/report.json` or `manifest.json`.
Next step: if QA verdict is `draft`, inspect `qa/report.json` warnings and improve masks or source before calling the asset app-ready.
"#;

pub(super) const MOTION_MANIFEST_HELP: &str = r#"
Topic: capy motion manifest

Use when: AI or a runtime needs to consume the generated motion package.
Required fields: `schema=capy.motion_asset.manifest.v1`, `source`, `strategy`, `outputs`, and `quality`.
Recommended command: `target/debug/capy motion verify --manifest <motion-asset-dir>/manifest.json`
Output families: `frames/rgba/` transparent PNG sequence, `masks/` alpha masks, `atlas/walk.png` plus `atlas/walk.json`, `video/preview.webm`, `video/rgb.mp4`, `video/alpha.mp4`, and `qa/preview.html`.
Do not: move files without updating `manifest.json`; treat `video/rgb.mp4` alone as alpha-capable; discard masks or QA metrics.
Next step: open `qa/preview.html` on black, white, photo, and game-like backgrounds before approving the package.
"#;

pub(super) const GAME_ASSETS_HELP: &str = r#"
Topic: capy game-assets agent

Use when: AI needs to create, rebuild, preview, or verify a compact 2D game asset pack from image generation and slicing.
Required parameters: `sample` needs `--out`; `build` and `verify` need `--pack <pack.json>`.
Recommended commands:
1. `target/debug/capy game-assets doctor`
2. `target/debug/capy game-assets sample --preset forest-action-rpg-compact --out target/capy-game-assets-sample --overwrite`
3. `target/debug/capy game-assets verify --pack target/capy-game-assets-sample/pack.json`
4. Open `target/capy-game-assets-sample/preview/index.html` or the desktop Game Assets tab.
Outputs: `pack.json`, `prompts/`, `raw/`, `transparent/`, `frames/`, `spritesheets/`, `qa/contact-sheet.png`, `preview/index.html`, and `report.json`.
Do not: use `--live` for smoke tests; edit frame paths by hand without rerunning `build`; claim asset quality from manifest existence alone.
Next step: save command JSON and the contact sheet into version evidence.
"#;

pub(super) const GAME_ASSETS_LIVE_HELP: &str = r#"
Topic: capy game-assets live

Use when: the user explicitly approved provider spend for a real image-generated sample pack.
Required parameters: `sample --live --max-live-calls <n> --out <dir>`; the compact preset currently needs 8 calls.
Recommended commands:
1. `target/debug/capy image balance`
2. `target/debug/capy game-assets sample --preset forest-action-rpg-compact --live --max-live-calls 8 --out target/capy-game-assets-live --overwrite`
3. `target/debug/capy game-assets verify --pack target/capy-game-assets-live/pack.json`
Do not: omit `--max-live-calls`; run live generation in project gates; log provider secrets or raw credentials; accept the pack without opening the QA contact sheet.
Next step: if live generation fails or quality is not acceptable, fall back to the no-spend sample and record the failure in evidence.
"#;

pub(super) const GAME_ASSETS_MANIFEST_HELP: &str = r#"
Topic: capy game-assets manifest

Use when: AI needs to inspect or patch a generated game asset pack.
Required file: `pack.json` with schema `capy.game_assets.pack.v1`.
Important fields: `assets[]`, `assets[].prompt_path`, `assets[].raw_path`, `assets[].transparent_path`, `assets[].actions[].source_path`, `frame_paths[]`, `spritesheet_path`, and `outputs`.
Recommended command: `target/debug/capy game-assets verify --pack <pack.json>`
Do not: point paths outside the pack directory; leave missing preview/contact-sheet/report outputs; reduce the compact sample below 5 assets or 16 frames.
Next step: rerun `target/debug/capy game-assets build --pack <pack.json>` after manifest or source image changes.
"#;
