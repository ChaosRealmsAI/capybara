use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::model::PackagePaths;

const PROMPT_FILES: [(&str, &str); 4] = [
    ("README.md", "handoff"),
    ("process.md", "process"),
    ("qa-review.md", "qa-review"),
    ("app-integration.md", "app-integration"),
];

pub(super) fn write_package_prompt_pack(
    paths: &PackagePaths,
    source: &Path,
    manifest: Option<&Value>,
    qa_report: Option<&Value>,
) -> Result<Value, String> {
    write_prompt_pack(
        &paths.prompts_dir,
        source,
        Some(&paths.root),
        manifest,
        qa_report,
    )
}

pub(super) fn write_standalone_prompt_pack(
    out: &Path,
    source: &Path,
    package: Option<&Path>,
) -> Result<Value, String> {
    let manifest = package
        .map(|root| read_json(&root.join("manifest.json")))
        .transpose()?;
    let qa_report = package
        .map(|root| read_json(&root.join("qa/report.json")))
        .transpose()?;
    write_prompt_pack(out, source, package, manifest.as_ref(), qa_report.as_ref())
}

fn write_prompt_pack(
    out: &Path,
    source: &Path,
    package: Option<&Path>,
    manifest: Option<&Value>,
    qa_report: Option<&Value>,
) -> Result<Value, String> {
    fs::create_dir_all(out).map_err(|err| format!("create {} failed: {err}", out.display()))?;
    let context = PromptContext {
        source,
        package,
        manifest,
        qa_report,
    };
    let files = [
        ("README.md", readme(&context)),
        ("process.md", process_prompt(&context)),
        ("qa-review.md", qa_review_prompt(&context)),
        ("app-integration.md", integration_prompt(&context)),
    ];
    let mut written = Vec::new();
    for (name, body) in files {
        let path = out.join(name);
        write_text(&path, &body)?;
        written.push(path);
    }
    Ok(json!({
        "schema": "capy.motion.prompt_pack.v1",
        "source": source,
        "package": package,
        "out": out,
        "files": written,
        "topics": PROMPT_FILES
            .iter()
            .map(|(path, kind)| json!({ "path": path, "kind": kind }))
            .collect::<Vec<_>>()
    }))
}

struct PromptContext<'a> {
    source: &'a Path,
    package: Option<&'a Path>,
    manifest: Option<&'a Value>,
    qa_report: Option<&'a Value>,
}

impl PromptContext<'_> {
    fn package_text(&self) -> String {
        self.package
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not yet generated".to_string())
    }

    fn source_summary(&self) -> String {
        let Some(manifest) = self.manifest else {
            return format!("source path: {}", self.source.display());
        };
        let source = manifest.get("source").unwrap_or(&Value::Null);
        format!(
            "source path: {}\nwidth: {}\nheight: {}\nfps: {}\nframes: {}\nduration_sec: {}\nvideo_codec: {}",
            source
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_else(|| self.source.to_str().unwrap_or("unknown")),
            source.get("width").and_then(Value::as_u64).unwrap_or(0),
            source.get("height").and_then(Value::as_u64).unwrap_or(0),
            source.get("fps").and_then(Value::as_f64).unwrap_or(0.0),
            source
                .get("frame_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            source
                .get("duration_sec")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            source
                .get("video_codec")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        )
    }

    fn qa_summary(&self) -> String {
        let Some(report) = self.qa_report else {
            return "qa report: not yet generated".to_string();
        };
        let verdict = report
            .get("verdict")
            .and_then(Value::as_str)
            .unwrap_or("draft");
        let metrics = report.get("metrics").cloned().unwrap_or(Value::Null);
        let warnings = report.get("warnings").cloned().unwrap_or(Value::Null);
        format!(
            "qa verdict: {verdict}\nmetrics: {}\nwarnings: {}",
            compact_json(&metrics),
            compact_json(&warnings)
        )
    }
}

fn readme(ctx: &PromptContext<'_>) -> String {
    format!(
        r#"# Motion Asset AI Handoff

Use this prompt pack when another AI needs to reuse, verify, or integrate the generated transparent motion asset.

## Package

- Source MP4: `{}`
- Motion package: `{}`
- Primary CLI help: `target/debug/capy motion help agent`
- Manifest help: `target/debug/capy motion help manifest`

## Read Order

1. `process.md` for the exact processing flow and red lines.
2. `qa-review.md` before calling the asset app/game ready.
3. `app-integration.md` before using the outputs in a runtime.
4. `manifest.json` and `qa/report.json` for machine-readable truth.

## Non-Negotiable Rules

- Do not claim ordinary H.264 MP4 has transparency.
- Do not judge quality from one still frame.
- Do not use fixed-background or chroma-key removal as the product path.
- Do not approve the asset without multi-background motion preview evidence.
"#,
        ctx.source.display(),
        ctx.package_text()
    )
}

fn process_prompt(ctx: &PromptContext<'_>) -> String {
    format!(
        r#"# Motion Cutout Process Prompt

You are operating Capybara's animation-grade motion cutout workflow.

## Goal

Convert the source MP4 into a structured transparent motion asset package usable by APP, game, and animation surfaces.

## Source

```text
{}
```

## Required CLI Flow

```bash
target/debug/capy motion doctor
target/debug/capy motion cutout \
  --input "{}" \
  --out <motion-asset-dir> \
  --quality animation \
  --target all \
  --verify \
  --overwrite
target/debug/capy motion inspect --manifest <motion-asset-dir>/manifest.json
target/debug/capy motion verify --manifest <motion-asset-dir>/manifest.json
```

## Required Outputs

- `frames/rgba/`: transparent PNG sequence.
- `masks/`: alpha masks for QA and debugging.
- `frames/cropped/`: fixed-cell cropped subject frames.
- `atlas/walk.png` and `atlas/walk.json`: sprite atlas contract.
- `video/preview.webm`: browser preview with alpha.
- `video/rgb.mp4` and `video/alpha.mp4`: dual-stream runtime fallback.
- `qa/preview.html` and `qa/report.json`: visible and machine-readable QA.
- `prompts/`: AI handoff, QA, and integration instructions.
- `manifest.json`: single package truth source.

## Red Lines

- Never approve from one still frame.
- Never hide edge issues by previewing only on a pale background.
- Never call `video/rgb.mp4` transparent.
- Keep source RGB plus model alpha as the default strategy unless a later spec explicitly changes it.

## Current QA Context

```text
{}
```
"#,
        ctx.source_summary(),
        ctx.source.display(),
        ctx.qa_summary()
    )
}

fn qa_review_prompt(ctx: &PromptContext<'_>) -> String {
    format!(
        r#"# Motion Asset QA Review Prompt

Review the motion package as a real APP/game asset, not as a demo file.

## Package

`{}`

## Checks

1. Run `target/debug/capy motion verify --manifest <package>/manifest.json`.
2. Run `target/debug/capy motion inspect --manifest <package>/manifest.json`.
3. Serve the preview with `target/debug/capy motion preview --package <package> --port <port>`.
4. Open `http://127.0.0.1:<port>/qa/preview.html` in a browser.
5. Capture desktop and mobile screenshots.
6. Switch backgrounds: deep, white, photo-like, game-like.
7. Pause/play once and confirm the visible frame state changes.
8. Check browser console/page errors.

## Quality Criteria

- Transparent PNG frame count equals source frame count.
- Mask count equals source frame count.
- Subject remains visible on dark, white, photo-like, and game-like backgrounds.
- Edge shimmer, shoe/ground contact, crop jitter, and width variation are recorded in `qa/report.json`.
- Travel-through clips are allowed, but must not be mislabeled as seamless loops.

## Current QA Context

```text
{}
```

Approve only when the manifest, QA report, and browser evidence agree.
"#,
        ctx.package_text(),
        ctx.qa_summary()
    )
}

fn integration_prompt(ctx: &PromptContext<'_>) -> String {
    format!(
        r#"# Motion Asset App Integration Prompt

Use the generated package as a transparent motion asset.

## Package

`{}`

## Runtime Choices

- Prefer `video/preview.webm` when the target browser/runtime supports WebM alpha.
- Prefer `frames/rgba/` or `atlas/walk.png` + `atlas/walk.json` for game engines and custom renderers.
- Use `video/rgb.mp4` + `video/alpha.mp4` only when the runtime can combine RGB and alpha streams.
- Do not use `video/rgb.mp4` alone as a transparent asset.

## Anchor and Sizing

- Read fixed cell and anchor data from `atlas/walk.json`.
- Treat width variation as motion data unless QA marks it as jitter.
- Preserve the frame rate from `manifest.json` unless the target runtime explicitly retimes the animation.

## Acceptance Before Shipping

```bash
target/debug/capy motion inspect --manifest <package>/manifest.json
target/debug/capy motion verify --manifest <package>/manifest.json
```

Then open `qa/preview.html` on multiple backgrounds and confirm the animation remains readable.

## Current Source Context

```text
{}
```
"#,
        ctx.package_text(),
        ctx.source_summary()
    )
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {} failed: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_prompt_pack_writes_expected_files() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let report = write_standalone_prompt_pack(dir.path(), Path::new("/tmp/source.mp4"), None)
            .map_err(std::io::Error::other)?;

        assert_eq!(
            report.get("schema").and_then(Value::as_str),
            Some("capy.motion.prompt_pack.v1")
        );
        for (name, _) in PROMPT_FILES {
            assert!(dir.path().join(name).is_file(), "{name} should exist");
        }
        let qa = fs::read_to_string(dir.path().join("qa-review.md"))?;
        assert!(qa.contains("capy motion verify"));
        assert!(qa.contains("multiple backgrounds") || qa.contains("backgrounds"));
        Ok(())
    }
}
