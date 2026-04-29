# Capybara TTS

`capy-tts` is the library behind `capy tts`. It provides Capybara-owned TTS
handlers for synthesis, batch synthesis, quick playback, voice preview,
configuration, concatenation, word-level timelines, SRT, and karaoke HTML.

The public surface is the root CLI:

```bash
capy tts --help
capy tts doctor
capy tts init --language zh
capy tts synth "这是一段，配好标点，的，中文 TTS 演示。" -o demo.mp3
capy tts batch jobs.json -d out --dry-run
capy tts voices --lang zh
```

Primary backend modes:

- `edge`: default no-spend draft backend.
- `volcengine`: paid production backend; requires provider credentials.

Generated default bundle:

- `<stem>.mp3`
- `<stem>.timeline.json`
- `<stem>.srt`
- `<stem>.karaoke.html`

Configuration lives under the Capybara app config directory at
`capybara/tts/config.toml`. The local audio cache is `.capy-tts-cache/` under
the selected output directory.

Karaoke timing uses a local whisperX forced-alignment runtime. `capy tts doctor`
checks Python packages and cached wav2vec2 alignment models without downloading.
`capy tts init` installs missing Python dependencies and alignment models into a
Capybara-managed runtime when the system Python is not already ready.
