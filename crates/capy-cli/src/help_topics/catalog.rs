use super::HelpTopic;
use super::docs::*;
use super::media_docs::*;

pub(super) const CAPY_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "desktop",
        aliases: &["verify", "window"],
        summary: "Open, inspect, capture, and verify the desktop shell.",
        body: DESKTOP_HELP,
    },
    HelpTopic {
        name: "canvas",
        aliases: &["canvas-agent"],
        summary: "Operate live canvas state and AI-readable context.",
        body: CANVAS_HELP,
    },
    HelpTopic {
        name: "chat",
        aliases: &["conversation"],
        summary: "Manage persistent Claude/Codex conversations.",
        body: CHAT_HELP,
    },
    HelpTopic {
        name: "agent",
        aliases: &["runtime"],
        summary: "Inspect local Claude/Codex runtime availability.",
        body: AGENT_HELP,
    },
    HelpTopic {
        name: "image",
        aliases: &["generate-image"],
        summary: "Generate images through project-owned adapters.",
        body: IMAGE_HELP,
    },
    HelpTopic {
        name: "image-cutout",
        aliases: &["cutout-ready"],
        summary: "Generate source images suitable for alpha cutout.",
        body: IMAGE_CUTOUT_HELP,
    },
    HelpTopic {
        name: "cutout",
        aliases: &["alpha-cutout"],
        summary: "Initialize and run transparent PNG cutout with Focus.",
        body: CUTOUT_HELP,
    },
    HelpTopic {
        name: "tts",
        aliases: &["voice", "audio"],
        summary: "Generate speech, subtitles, timelines, and karaoke HTML.",
        body: TTS_HELP,
    },
    HelpTopic {
        name: "tts-karaoke",
        aliases: &["karaoke"],
        summary: "Generate and inspect word-synced TTS karaoke HTML.",
        body: TTS_KARAOKE_HELP,
    },
    HelpTopic {
        name: "tts-batch",
        aliases: &["batch-tts"],
        summary: "Batch synthesize long scripts or multiple voices.",
        body: TTS_BATCH_HELP,
    },
    HelpTopic {
        name: "clips",
        aliases: &["video-cut", "videocut"],
        summary: "Download, transcribe, align, cut, and preview clips.",
        body: CLIPS_HELP,
    },
    HelpTopic {
        name: "media",
        aliases: &["scroll-media"],
        summary: "Package scroll-driven video HTML assets.",
        body: MEDIA_SCROLL_HELP,
    },
    HelpTopic {
        name: "timeline",
        aliases: &["poster-export", "recorder"],
        summary: "Compose, compile, export, and verify Timeline projects.",
        body: TIMELINE_HELP,
    },
];

pub(super) const IMAGE_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "agent",
        aliases: &["workflow"],
        summary: "Use capy image safely from an AI agent.",
        body: IMAGE_HELP,
    },
    HelpTopic {
        name: "cutout-ready",
        aliases: &["cutout", "alpha-source"],
        summary: "Prompt rules for images passed to capy cutout.",
        body: IMAGE_CUTOUT_HELP,
    },
];

pub(super) const CUTOUT_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "agent",
        aliases: &["workflow", "run"],
        summary: "Use capy cutout safely from an AI agent.",
        body: CUTOUT_HELP,
    },
    HelpTopic {
        name: "manifest",
        aliases: &["batch"],
        summary: "Batch manifest shape for multiple cutout inputs.",
        body: CUTOUT_MANIFEST_HELP,
    },
];

pub(super) const CANVAS_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "agent",
        aliases: &["workflow"],
        summary: "Operate live canvas nodes and state.",
        body: CANVAS_HELP,
    },
    HelpTopic {
        name: "context",
        aliases: &["selection"],
        summary: "Export selected-image or region context packets.",
        body: CANVAS_CONTEXT_HELP,
    },
    HelpTopic {
        name: "images",
        aliases: &["generate-image", "insert-image"],
        summary: "Insert or generate images on the live canvas.",
        body: CANVAS_IMAGES_HELP,
    },
];

pub(super) const CHAT_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "agent",
        aliases: &["workflow"],
        summary: "Create, send, inspect, and export conversations.",
        body: CHAT_HELP,
    },
    HelpTopic {
        name: "canvas-tools",
        aliases: &["capy-canvas-tools"],
        summary: "Inject canvas CLI instructions into agent turns.",
        body: CHAT_CANVAS_TOOLS_HELP,
    },
];

pub(super) const AGENT_TOPICS: &[HelpTopic] = &[HelpTopic {
    name: "doctor",
    aliases: &["agent", "runtime"],
    summary: "Check local Claude and Codex runtime availability.",
    body: AGENT_HELP,
}];

pub(super) const CLIPS_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "pipeline",
        aliases: &["agent", "workflow"],
        summary: "Run download, transcribe, align, cut, preview, and karaoke.",
        body: CLIPS_HELP,
    },
    HelpTopic {
        name: "youtube",
        aliases: &["real-download"],
        summary: "Acceptance path for real YouTube download and clip cutting.",
        body: CLIPS_YOUTUBE_HELP,
    },
];

pub(super) const MEDIA_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "scroll-pack",
        aliases: &["agent", "single-video"],
        summary: "Package one MP4 into a scroll media HTML bundle.",
        body: MEDIA_SCROLL_HELP,
    },
    HelpTopic {
        name: "story-pack",
        aliases: &["multi-video"],
        summary: "Package a multi-video story manifest into a landing page.",
        body: MEDIA_STORY_HELP,
    },
];

pub(super) const TIMELINE_TOPICS: &[HelpTopic] = &[
    HelpTopic {
        name: "poster-export",
        aliases: &["agent", "workflow"],
        summary: "Compose Poster JSON and produce Timeline evidence/export.",
        body: TIMELINE_HELP,
    },
    HelpTopic {
        name: "live",
        aliases: &["attach", "preview"],
        summary: "Attach a Timeline composition to a live canvas node.",
        body: TIMELINE_LIVE_HELP,
    },
];
