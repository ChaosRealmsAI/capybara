# Watchmaker Scroll Story

Static reference implementation for an OpenAI-style luxury watchmaker landing page:
sticky watch stage, transparent mechanical layers, scroll-linked assembly, and
alternating editorial chapters.

## Run

```bash
cd examples/watchmaker-scroll-story
./fetch-reference-assets.sh
python3 -m http.server 5264 --bind 127.0.0.1
curl -I http://127.0.0.1:5264/index.html
```

Open:

```text
http://127.0.0.1:5264/index.html
```

## Verify

With the server still running:

```bash
CAPY_WATCHMAKER_URL=http://127.0.0.1:5264/index.html \
CAPY_WATCHMAKER_EVIDENCE_DIR=/Users/Zhuanz/workspace/capybara/spec/versions/v0.10-watchmaker-story/evidence/assets \
  ./verify.mjs
```

The verifier writes desktop/mobile screenshots and JSON state evidence. It uses
the local Playwright runtime; set `PLAYWRIGHT_MODULE=/path/to/playwright` if
Playwright is not installed in a standard global location.

## Asset Contract

The page expects these transparent watch assets:

```text
assets/watch/hero-tilted.png
assets/watch/base-plate.png
assets/watch/gear-train.png
assets/watch/tourbillon.png
assets/watch/moonphase.png
assets/watch/case-ring.png
assets/watch/hands.png
```

`fetch-reference-assets.sh` downloads the public OpenAI demo assets for local
private visual comparison only. Do not treat those downloaded PNGs as owned
production assets. For production, replace the files with Capybara-owned
generated transparent layers that keep the same filenames and dimensions.

## Implementation Notes

- No React, bundler, WebGL, or video dependency.
- Scroll work is limited to `requestAnimationFrame`, `transform`, and `opacity`.
- Each mechanical layer is driven by its corresponding chapter's viewport
  position, which keeps the copy and assembly state synchronized.
- `window.__watchmakerState` exposes the active chapter, smoothed progress, and
  layer transform state for browser verification.
- The layout includes a reduced-motion path and a mobile sticky-stage layout.
