#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
EXAMPLES="$ROOT/crates/capy-scroll-media/examples"
INPUTS="$EXAMPLES/inputs"
OUTPUTS="$EXAMPLES/outputs"

mkdir -p "$INPUTS" "$OUTPUTS"

make_clip() {
  local filter="$1"
  local output="$2"
  ffmpeg -nostdin -y -loglevel error \
    -f lavfi -i "$filter" \
    -an \
    -c:v libx264 \
    -preset veryfast \
    -crf 30 \
    -pix_fmt yuv420p \
    -movflags +faststart \
    "$output"
}

make_clip \
  "color=c=0xf8fafc:s=320x180:r=24:d=2,drawbox=x='24+88*t':y=42:w=78:h=78:color=0x0f172a@1:t=fill,drawbox=x='216-54*t':y=72:w=40:h=40:color=0xe11d48@1:t=fill" \
  "$INPUTS/card-pan-2s.mp4"

make_clip \
  "color=c=0x111827:s=320x180:r=24:d=2,drawbox=x='24+132*t':y=0:w=72:h=180:color=0x22c55e@1:t=fill,drawbox=x='148+38*sin(6.28318*t)':y=54:w=72:h=72:color=0xf8fafc@1:t=fill" \
  "$INPUTS/color-wipe-2s.mp4"

make_clip \
  "color=c=0xffffff:s=180x320:r=24:d=2,drawbox=x=42:y='24+112*t':w=96:h=96:color=0x2563eb@1:t=fill,drawbox=x=68:y='222-68*t':w=44:h=44:color=0xf97316@1:t=fill" \
  "$INPUTS/vertical-reveal-2s.mp4"

build_pack() {
  local name="$1"
  local output_dir="$OUTPUTS/$name"
  cargo run -p capy-cli -- media scroll-pack \
    --input "$INPUTS/$name.mp4" \
    --out "$output_dir" \
    --name "$name" \
    --poster-width 320 \
    --default 180:28 \
    --fallback 120:32 \
    --hq 240:26 \
    --verify \
    --overwrite >/dev/null
  METRICS_PATH="$output_dir/evidence/metrics.json" EXAMPLE_NAME="$name" node <<'NODE'
const fs = require("fs");
const path = require("path");
const metricsPath = process.env.METRICS_PATH;
const name = process.env.EXAMPLE_NAME;
const data = JSON.parse(fs.readFileSync(metricsPath, "utf8"));
const outputDir = `crates/capy-scroll-media/examples/outputs/${name}`;
data.input = `crates/capy-scroll-media/examples/inputs/${name}.mp4`;
data.output_dir = outputDir;
data.manifest_path = `${outputDir}/manifest.json`;
if (data.verification && Array.isArray(data.verification.clips)) {
  for (const clip of data.verification.clips) {
    clip.path = `${outputDir}/${path.basename(clip.path)}`;
  }
}
fs.writeFileSync(metricsPath, `${JSON.stringify(data, null, 2)}\n`);
NODE
}

(
  cd "$ROOT"
  build_pack card-pan-2s
  build_pack color-wipe-2s
  build_pack vertical-reveal-2s
)

find "$EXAMPLES" -maxdepth 3 -type f | sort
