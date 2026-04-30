pub(super) const TTS_HELP: &str = r#"
Topic: capy tts

Use when: AI needs speech audio plus word timing, SRT, and karaoke HTML.
Required parameters: `synth` needs text or `--file`; `batch` needs JSON input and `-d <out-dir>`; `doctor/init` inspect alignment readiness.
Recommended commands:
1. `target/debug/capy tts doctor`
2. `target/debug/capy tts init --dry-run`
3. `target/debug/capy tts voices --lang zh`
4. `target/debug/capy tts synth "这是一段，配好标点，的，中文演示。" -o target/tts/demo.mp3`
Do not: expect Edge to honor emotion/SSML; feed huge text as one job; skip Chinese punctuation.
Next step: open `<stem>.karaoke.html` or inspect `<stem>.timeline.json`.
"#;

pub(super) const TTS_KARAOKE_HELP: &str = r#"
Topic: capy tts karaoke

Use when: output needs synchronized visible text or timing for video composition.
Required parameters: run `synth` or `batch` without `--no-sub`; ensure `capy tts doctor` passes or run `init`.
Recommended command: `target/debug/capy tts synth "训练中的一切，都变成了三维体验。" -o target/tts/demo.mp3`
Do not: pass `--no-sub`; trust timing without opening karaoke HTML once.
Next step: use `.timeline.json` programmatically or `.karaoke.html` for visual QA.
"#;

pub(super) const TTS_BATCH_HELP: &str = r#"
Topic: capy tts batch

Use when: long scripts need paragraph splitting or many voices/languages are required.
Required parameters: JSON array with at least `text`; use `-d <out-dir>`.
Recommended JSON: `[{"text":"第一段。","filename":"p1"},{"text":"第二段。","filename":"p2","voice":"zh-CN-YunxiNeural"}]`
Recommended command: `target/debug/capy tts batch jobs.json -d target/tts`
Do not: batch paid backend jobs without checking cost; skip `--dry-run` for new manifests.
Next step: read `manifest.json`; use `capy tts concat` if one audio file is required.
"#;

pub(super) const CLIPS_HELP: &str = r#"
Topic: capy clips pipeline

Use when: AI needs to download a source video, transcribe, align words, cut clips, and build preview/karaoke HTML.
Required parameters: `download --url --out-dir`; `transcribe --video --out-dir`; `align --video --srt-path --out-dir`; `cut --video --sentences-path --plan-path --out-dir`.
Recommended commands:
1. `target/debug/capy clips doctor`
2. `target/debug/capy clips download --url <youtube_url> --out-dir target/clips/source`
3. `target/debug/capy clips transcribe --video target/clips/source/source.mp4 --out-dir target/clips/transcribe --model large-v3 --language en`
4. `target/debug/capy clips align --video target/clips/source/source.mp4 --srt-path <subtitles.srt> --out-dir target/clips/align`
5. `target/debug/capy clips cut --video target/clips/source/source.mp4 --sentences-path <sentences.json> --plan-path <plan.json> --out-dir target/clips/cut`
Do not: call local fixtures final download acceptance; skip doctor; guess sentence IDs.
Next step: run preview or karaoke and save the cut report.
"#;

pub(super) const CLIPS_YOUTUBE_HELP: &str = r#"
Topic: capy clips youtube

Use when: PM asks to prove download and cutting against a real YouTube video.
Required parameters: public YouTube URL, stable output dirs, and a cut plan referencing real sentence IDs.
Recommended start: `target/debug/capy clips download --url <youtube_url> --out-dir target/clips/source --format-height 1080`
Do not: reuse an old video when a fresh sample is requested; call `file://` a download test; leave evidence as terminal text only.
Next step: transcribe, align, cut, then open preview/karaoke HTML.
"#;

pub(super) const MEDIA_SCROLL_HELP: &str = r#"
Topic: capy media scroll-pack

Use when: one MP4 should become a scroll-driven HTML media package.
Required parameters: `--input <mp4> --out <dir>`; optional `--name`; use `--dry-run` for planning and `--verify` for keyframe checks.
Recommended commands:
1. `target/debug/capy media scroll-pack --input <mp4> --out target/scroll --name demo --dry-run`
2. `target/debug/capy media scroll-pack --input <mp4> --out target/scroll --name demo --verify --overwrite`
3. `target/debug/capy media serve --root target/scroll/demo`
4. `target/debug/capy media inspect --manifest target/scroll/demo/manifest.json`
Do not: pass both `--emit-html` and `--emit-composition`; claim browser delivery without HTTP verification.
Next step: open served HTML and verify playback/scrub behavior.
"#;

pub(super) const MEDIA_STORY_HELP: &str = r#"
Topic: capy media story-pack

Use when: multiple videos should become one scroll story landing page.
Required parameters: `--manifest <json> --out <dir>`; manifest defines chapters and source videos.
Recommended command: `target/debug/capy media story-pack --manifest <story.json> --out target/story --dry-run`
Do not: use story-pack for one clip when scroll-pack is enough; skip manifest inspection.
Next step: serve output with `capy media serve` and inspect manifest.
"#;

pub(super) const TIMELINE_HELP: &str = r#"
Topic: capy timeline poster-export

Use when: Poster JSON should become a Timeline composition, snapshot, MP4, or evidence report.
Required parameters: `compose-poster --input <poster.json>`; later steps use `--composition <composition.json>`.
Recommended commands:
1. `target/debug/capy timeline doctor`
2. `target/debug/capy timeline compose-poster --input fixtures/poster/v0.1/sample-poster.json --out target/capy-timeline/sample`
3. `target/debug/capy timeline validate --composition target/capy-timeline/sample/composition.json`
4. `target/debug/capy timeline compile --composition target/capy-timeline/sample/composition.json`
5. `target/debug/capy timeline verify-export --composition target/capy-timeline/sample/composition.json`
Do not: export before validate/compile; change brand tokens without `rebuild`; use Timeline for raw scroll video packaging.
Next step: for live canvas preview, read `capy timeline help live`.
"#;

pub(super) const TIMELINE_LIVE_HELP: &str = r#"
Topic: capy timeline live

Use when: a Timeline composition should attach to a live canvas node, or a clip-first composition JSON should open in the desktop video editor.
Required parameters: `attach --canvas-node <id> --composition <composition.json>` for canvas preview; `open --composition <composition.json>` for the video editor tab.
Recommended commands:
1. `target/debug/capy timeline attach --canvas-node <id> --composition <composition.json>`
2. `target/debug/capy timeline state --canvas-node <id>`
3. `target/debug/capy timeline open --canvas-node <id>`
4. `target/debug/capy timeline open --composition fixtures/timeline/video-editing/compositions/main.json`
5. `target/debug/capy timeline export --composition fixtures/timeline/video-editing-4k/compositions/main.json --kind mp4 --resolution 4k --fps 30 --parallel 2 --profile final --strict-recorder --out spec/versions/v0.25-video-editing-tab/evidence/assets/video-editing-4k-30s.mp4`
Do not: attach to guessed ids; open before preview-ready; pass a track-only JSON when the editor expects full composition JSON; accept embedded fallback for PM-facing 4K proof.
Next step: capture the desktop editor tab, preview iframe, and export status as evidence.
"#;
