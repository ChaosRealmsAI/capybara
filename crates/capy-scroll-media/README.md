# Capy Scroll Media

`capy-scroll-media` turns one source MP4 into a static scroll-driven media package.
The package can be dropped into any static HTML site that serves MP4 byte ranges.

This is the formal implementation behind:

```text
capy media scroll-pack
capy media story-pack
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

For a multi-video scroll story, pass a JSON manifest instead of one input video:

```bash
cargo run -p capy-cli -- media story-pack \
  --manifest crates/capy-scroll-media/examples/inputs/watch-story-dry-run.json \
  --out target/capy-watch-story \
  --default 720:23 \
  --fallback 360:30 \
  --hq 1080:24 \
  --verify \
  --overwrite
```

The story manifest shape is:

```json
{
  "schema_version": 1,
  "title": "Exploded Watch Scroll Story",
  "eyebrow": "Capybara Scroll Media",
  "summary": "One landing page coordinated by several all-keyframe MP4 clips.",
  "theme": "watch",
  "chapters": [
    {
      "id": "hero",
      "title": "Hero Position",
      "kicker": "Opening angle",
      "body": "Copy for this scroll chapter.",
      "video": "relative-or-absolute-input.mp4"
    }
  ]
}
```

`video` paths may be absolute or relative to the story manifest file.

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

`story-pack` outputs a similar static package for a full landing page:

```text
manifest.json
story.html
posters/<chapter-id>-1280.jpg
clips/<chapter-id>-720-crf23-allkey.mp4
clips/<chapter-id>-360-crf30-allkey.mp4
clips/<chapter-id>-1080-crf24-allkey.mp4
runtime/multi-video-story.css
runtime/multi-video-story.js
evidence/metrics.json
```

If a source video is lower than the requested preset height, the generated clip
keeps the source height instead of upscaling.

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
http://127.0.0.1:5202/story.html
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
5. Keep `story-pack --dry-run` in `scripts/check-project.sh`.
6. Run `scripts/check-project.sh`.
7. Browser-check at least one `scroll-hq.html` or `story.html` page before claiming a web/media change is complete.
