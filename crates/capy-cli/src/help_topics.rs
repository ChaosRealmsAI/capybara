pub fn print_capy_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, CAPY_TOPICS, "capy help")
}

pub fn print_image_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, IMAGE_TOPICS, "capy image help")
}

pub fn print_cutout_topic(topic: Option<&str>) -> Result<(), String> {
    print_topic(topic, CUTOUT_TOPICS, "capy cutout help")
}

fn print_topic(topic: Option<&str>, topics: &[HelpTopic], command: &str) -> Result<(), String> {
    let Some(topic) = topic else {
        println!("{}", topic_index(topics, command));
        return Ok(());
    };
    let normalized = normalize_topic(topic);
    if let Some(help) = topics.iter().find(|item| {
        item.name == normalized || item.aliases.iter().any(|alias| *alias == normalized)
    }) {
        println!("{}", help.body.trim());
        return Ok(());
    }
    Err(format!(
        "unknown help topic `{topic}`. Available topics: {}",
        topics
            .iter()
            .map(|item| item.name)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn topic_index(topics: &[HelpTopic], command: &str) -> String {
    let mut lines = vec!["Available self-contained help topics:".to_string()];
    for topic in topics {
        lines.push(format!("  {:<20} {}", topic.name, topic.summary));
    }
    lines.push(String::new());
    lines.push(format!("Run `{command} <topic>`."));
    lines.join("\n")
}

fn normalize_topic(topic: &str) -> String {
    topic.trim().to_ascii_lowercase().replace('_', "-")
}

struct HelpTopic {
    name: &'static str,
    aliases: &'static [&'static str],
    summary: &'static str,
    body: &'static str,
}

const CAPY_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "image",
        aliases: &["image-agent", "generate-image"],
        summary: "Generate images through project-owned provider adapters.",
        body: r#"
Topic: image

Use when:
- An AI or PM needs a local, machine-readable image generation workflow.
- You need no-spend checks before a live provider call.

Required parameters:
- `capy image generate` requires a five-section prompt:
  `Scene: ... Subject: ... Important details: ... Use case: ... Constraints: ...`
- Live generation spends provider credits unless you pass `--dry-run` or `--submit-only`.

Recommended commands:
1. `target/debug/capy image providers`
2. `target/debug/capy image doctor`
3. `target/debug/capy image generate --dry-run "<five-section prompt>" --size 1:1 --resolution 1k`
4. `target/debug/capy image balance`
5. `target/debug/capy image generate "<five-section prompt>" --size 1:1 --resolution 1k --out <dir> --name <slug>`

Example:
`target/debug/capy image generate --dry-run "Scene: Warm studio. Subject: One ceramic cup centered. Important details: Product photo, soft light. Use case: Hero card. Constraints: No text, no watermark."`

Do not:
- Do not call the external provider CLI directly; use `capy image`.
- Do not use a short unstructured prompt; it will fail validation.
- Do not run live generation unless spending provider credits is intended.

Next step:
- If the image will be passed to `capy cutout`, read `capy help image-cutout`.
"#,
    },
    HelpTopic {
        name: "image-cutout",
        aliases: &["cutout-image", "cutout-ready"],
        summary: "Generate source images that are suitable for alpha cutout.",
        body: IMAGE_CUTOUT_READY_HELP,
    },
    HelpTopic {
        name: "cutout",
        aliases: &["cutout-agent", "alpha-cutout"],
        summary: "Initialize and run transparent PNG cutout with Focus.",
        body: CUTOUT_AGENT_HELP,
    },
];

const IMAGE_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "agent",
        aliases: &["workflow", "generate"],
        summary: "Use capy image safely from an AI agent.",
        body: r#"
Topic: capy image agent

Use when:
- You need to generate, submit, resume, or dry-run an image request.
- You need JSON output that a later AI step can parse.

Required parameters:
- `generate` requires one five-section prompt:
  `Scene: ... Subject: ... Important details: ... Use case: ... Constraints: ...`
- `--size` defaults to `1:1`; set it explicitly for deliverables.
- `--resolution` defaults to `1k`; use `2k` or `4k` only when needed.
- `--out` and `--name` are required if later steps need a stable file path.

Recommended commands:
1. `target/debug/capy image providers`
2. `target/debug/capy image doctor`
3. `target/debug/capy image generate --dry-run "<five-section prompt>" --size 1:1 --resolution 1k --out <dir> --name <slug>`
4. `target/debug/capy image balance`
5. `target/debug/capy image generate "<five-section prompt>" --size 1:1 --resolution 1k --out <dir> --name <slug>`

Example:
`target/debug/capy image generate --dry-run "Scene: Warm studio. Subject: One ceramic cup centered. Important details: Product photo, soft light. Use case: Hero card. Constraints: No text, no watermark." --out target/capy-image --name cup`

Do not:
- Do not skip `doctor` when setting up a new machine.
- Do not use live generation for smoke tests; use `--dry-run`.
- Do not omit `--out` when the next step must read the image.

Next step:
- For cutout-bound images, run `capy image help cutout-ready`.
"#,
    },
    HelpTopic {
        name: "cutout-ready",
        aliases: &["cutout", "alpha-source"],
        summary: "Prompt rules for images that will be passed to capy cutout.",
        body: IMAGE_CUTOUT_READY_HELP,
    },
];

const CUTOUT_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "agent",
        aliases: &["workflow", "run"],
        summary: "Use capy cutout safely from an AI agent.",
        body: CUTOUT_AGENT_HELP,
    },
    HelpTopic {
        name: "manifest",
        aliases: &["batch"],
        summary: "Batch manifest shape for multiple cutout inputs.",
        body: r#"
Topic: capy cutout manifest

Use when:
- You need to cut multiple generated assets in one command.
- You want outputs, masks, QA previews, per-item reports, and one summary JSON.

Required parameters:
- `--manifest <json>`: JSON file with an `items` array.
- `--out-dir <dir>`: output root. The CLI writes `outputs/`, `masks/`, `qa/`, and `reports/`.

Recommended manifest shape:
```json
{
  "items": [
    {
      "id": "hero-object",
      "input": "target/generated/hero-object.png",
      "output": "hero-object.png",
      "mask": "hero-object-mask.png"
    }
  ]
}
```

Recommended command:
`target/debug/capy cutout batch --manifest <manifest.json> --out-dir <out-dir> --report <summary.json>`

Do not:
- Do not put directories in item `input`; each item must point to one image file.
- Do not assume transparent output quality without checking `qa/qa-white.png` and `qa/qa-black.png`.

Next step:
- Read the summary JSON and open the QA previews before using the assets in HTML.
"#,
    },
];

const IMAGE_CUTOUT_READY_HELP: &str = r##"
Topic: image-cutout

Use when:
- The generated image will be passed to `capy cutout run` or `capy cutout batch`.
- You need a foreground object that can become a clean transparent PNG.

Required parameters:
- Add `--cutout-ready` to `capy image generate` or `capy canvas generate-image`.
- Prompt must still use the five sections:
  `Scene: ... Subject: ... Important details: ... Use case: ... Constraints: ...`
- Prompt must include:
  `#E0E0E0`, `one` or `single`, `fully visible` or `uncropped`,
  `clean silhouette` or `clear edges`, `no extra objects`, `no text`,
  `no watermark`, `no green screen`, and `no blue screen`.

Recommended command:
`target/debug/capy image generate --cutout-ready "<prompt>" --size 1:1 --resolution 1k --out target/capy-image --name object`

Prompt template:
```text
Scene: Neutral matte #E0E0E0 studio background for cutout source.
Subject: One single <object> centered, fully visible, uncropped, 70% frame height.
Important details: Clean silhouette, clear edges, soft even light, strong separation from background.
Use case: Source for automated alpha cutout and transparent PNG UI composition.
Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.
```

Do not:
- Do not use green screen or blue screen; those colors contaminate edges.
- Do not ask for multiple objects unless the whole group is the intended foreground.
- Do not crop the object at the frame edge.
- Do not include hard shadows, reflections, text, logos, watermark, or busy backgrounds.

Next step:
1. Run `target/debug/capy cutout doctor`.
2. If not ready, run `target/debug/capy cutout init`.
3. Run `target/debug/capy cutout run --input <image.png> --output <cutout.png> --mask-out <mask.png> --qa-dir <qa-dir> --report <report.json>`.
"##;

const CUTOUT_AGENT_HELP: &str = r#"
Topic: capy cutout agent

Use when:
- You have a local image and need a true transparent PNG.
- The image is a generated asset or product object intended for HTML composition.

Required parameters:
- First-time setup: `target/debug/capy cutout init`.
- Readiness check: `target/debug/capy cutout doctor`.
- Single image: `--input <image> --output <png>`.
- Optional but recommended: `--mask-out <png> --qa-dir <dir> --report <json>`.
- Batch mode: `--manifest <json> --out-dir <dir>`.

Recommended commands:
1. `target/debug/capy cutout doctor`
2. `target/debug/capy cutout init`
3. `target/debug/capy cutout run --input <image.png> --output <cutout.png> --mask-out <mask.png> --qa-dir <qa-dir> --report <report.json>`
4. `sips -g hasAlpha <cutout.png>`

Example:
`target/debug/capy cutout run --input target/capy-image/object.png --output target/capy-image/object-cutout.png --mask-out target/capy-image/object-mask.png --qa-dir target/capy-image/object-qa --report target/capy-image/object-cutout.json`

Do not:
- Do not use old fixed-background color removal; the product path is Focus.
- Do not assume the model exists; `doctor` must pass or `init` must be run.
- Do not skip QA previews for PM-visible or HTML-composited assets.
- Do not feed hard-to-cut images when you can regenerate with `capy image --cutout-ready`.

Next step:
- Open `qa-white.png` and `qa-black.png` to inspect edge quality.
- Use the transparent PNG only after the report says `ok: true` and `sips -g hasAlpha` says `yes`.
"#;
