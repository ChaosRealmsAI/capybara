pub(super) const DEV_HELP: &str = r#"
Topic: capy dev

Use when: AI needs the internal operation index before verifying or automating Capybara.
Meaning of `[dev]`: internal AI/dev verification or automation command. It is safe to expose in `capy --help`, but it is not a PM-facing product workflow.
Registered `[dev]` commands:
1. Lifecycle: `shell`, `open`, `ps`, `quit`
2. Read/inspect: `doctor`, `state`, `devtools`, `verify`
3. Visible evidence: `screenshot`, `capture`
4. UI automation: `click`, `type`
5. Runtime inspection: `agent`
Product workflow commands without `[dev]`: `chat`, `canvas`, `image`, `cutout`, `tts`, `clips`, `media`, `timeline`.
Do not: hide these commands from help; run `click`/`type` on a user's active window without an isolated `CAPYBARA_SOCKET`; treat `[dev]` commands as a substitute for final product evidence.
Next step: read `capy help doctor`, `capy help interaction`, or `capy help desktop` for the exact workflow.
"#;

pub(super) const DOCTOR_HELP: &str = r#"
Topic: capy doctor

Use when: AI needs to decide whether Capybara is ready before running desktop, asset, agent, TTS, clips, media, or Timeline workflows.
Required parameters: none.
Recommended commands:
1. `target/debug/capy doctor`
2. Read `domain_doctors[]` in the JSON.
3. Run the domain doctor for the workflow you will perform next, for example `target/debug/capy clips doctor` or `target/debug/capy tts doctor`.
Do not: treat `capy doctor` as proof that the desktop UI is visible; use `capy verify --profile desktop --capture-out <png>` for real UI evidence.
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
Do not: claim UI verified from build/tests alone; mix sockets; run `devtools` without `--query` or `--eval`.
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
Required parameters: `send/open/events/stop/export` need `--id`; `send` also needs a prompt.
Recommended commands:
1. `target/debug/capy chat list`
2. `target/debug/capy chat new --provider codex --cwd <repo>`
3. `target/debug/capy chat send --id <id> "Summarize current state"`
4. `target/debug/capy chat events --id <id>`
5. `target/debug/capy chat export --id <id>`
Do not: create throwaway sessions when continuity matters; use `--write-code` casually; attach canvas context as prose when `--canvas-context` exists.
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

Use when: AI needs to know whether local Claude and Codex runtimes are available.
Required parameters: none.
Recommended command: `target/debug/capy agent doctor`
Do not: start a long agent run before checking runtime availability.
Next step: create a conversation with `capy chat new --provider claude|codex`.
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
Do not: call provider CLIs directly; use short unstructured prompts; run live generation unless spending credits is intended.
Next step: for alpha cutout, read `capy help image-cutout`.
"#;

pub(super) const IMAGE_CUTOUT_HELP: &str = r##"
Topic: image-cutout

Use when: generated image will be passed to `capy cutout run` or `batch`.
Required parameters: add `--cutout-ready`; prompt must include five sections plus `#E0E0E0`, `one`/`single`, `fully visible`/`uncropped`, clean edges, no extra objects, no text, no watermark, no green screen, no blue screen.
Recommended command: `target/debug/capy image generate --cutout-ready "<prompt>" --size 1:1 --resolution 1k --out target/capy-image --name object`
Prompt template:
```text
Scene: Neutral matte #E0E0E0 studio background for cutout source.
Subject: One single <object> centered, fully visible, uncropped, 70% frame height.
Important details: Clean silhouette, clear edges, soft even light, strong separation from background.
Use case: Source for automated alpha cutout and transparent PNG UI composition.
Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.
```
Do not: use green/blue screen; crop the object; add text, logos, hard shadows, reflections, or busy backgrounds.
Next step: run `capy cutout doctor`, `capy cutout init` if needed, then `capy cutout run`.
"##;

pub(super) const CUTOUT_HELP: &str = r#"
Topic: capy cutout

Use when: a local image must become a transparent PNG.
Required parameters: first setup `capy cutout init`; readiness `capy cutout doctor`; single run needs `--input --output`; recommended `--mask-out --qa-dir --report`.
Recommended command: `target/debug/capy cutout run --input <image.png> --output <cutout.png> --mask-out <mask.png> --qa-dir <qa-dir> --report <report.json>`
Do not: use old fixed-background removal; skip doctor/init; skip QA previews for PM-visible assets.
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
