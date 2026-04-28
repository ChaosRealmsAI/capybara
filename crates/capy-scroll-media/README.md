# Capy Scroll Media

`capy-scroll-media` turns one source MP4 into a static scroll-driven media package.
The package can be dropped into any static HTML site that serves MP4 byte ranges.

This is the formal implementation behind:

```text
capy media scroll-pack
capy media serve
capy media inspect
```

## Input

The main input is one video file:

```bash
cargo run -p capy-cli -- media scroll-pack \
  --input crates/capy-scroll-media/examples/inputs/card-pan-2s.mp4 \
  --out crates/capy-scroll-media/examples/outputs/card-pan-2s \
  --name card-pan-2s \
  --poster-width 320 \
  --default 180:28 \
  --fallback 120:32 \
  --hq 240:26 \
  --verify \
  --overwrite
```

Production defaults are higher quality:

```text
--default 720:23
--fallback 720:27
--hq 1080:24
--poster-width 1280
```

The examples use smaller presets so the fixture files can stay in git.

## Output

Each output folder contains a complete static package:

```text
manifest.json
demo.html
scroll-hq.html
raw-quality.html
poster-320.jpg
scrub-120-crf32-allkey.mp4
scrub-180-crf28-allkey.mp4
scrub-240-crf26-allkey.mp4
runtime/scroll-video.css
runtime/scroll-video.js
evidence/metrics.json
```

Use `scroll-hq.html` for the no-copy, no-overlay quality check page.
Use `demo.html` when you want a page that explains the interaction.
Use `raw-quality.html` when debugging only the encoded video quality.

## Rebuild The Examples

The committed examples are small and deterministic. Rebuild them with:

```bash
crates/capy-scroll-media/examples/build-examples.sh
```

The script creates three small input MP4 files under `examples/inputs/` and
three matching output packages under `examples/outputs/`.

## Serve A Package

Always serve scroll packages over HTTP. The video clips require Range requests.

```bash
cargo run -p capy-cli -- media serve \
  --root crates/capy-scroll-media/examples/outputs/card-pan-2s \
  --port 5202
```

Then open:

```text
http://127.0.0.1:5202/scroll-hq.html
```

Quick Range check:

```bash
curl -r 1000-1999 -I \
  http://127.0.0.1:5202/scrub-240-crf26-allkey.mp4
```

Expected response:

```text
HTTP/1.1 206 Partial Content
```

## Embed In Another HTML Page

Copy the whole output folder or publish it unchanged. The runtime expects paths
from `manifest.json`.

```html
<link rel="stylesheet" href="./runtime/scroll-video.css" />
<section class="capy-scroll-story">
  <div data-capy-scroll-video data-manifest="./manifest.json" data-clip="hq"></div>
</section>
<script src="./runtime/scroll-video.js"></script>
```

`data-clip` accepts:

```text
default
fallback
hq
```

It can also be a direct MP4 path for custom delivery.

## Future AI Checklist

When changing this package:

1. Keep `scroll-hq.html` in the generated output.
2. Keep all scrub clips all-keyframe (`-g 1`, `keyint_min 1`).
3. Keep HTTP Range support in `capy media serve`.
4. Rebuild examples with `crates/capy-scroll-media/examples/build-examples.sh`.
5. Run `scripts/check-project.sh`.
6. Browser-check at least one `scroll-hq.html` page before claiming a web/media change is complete.
