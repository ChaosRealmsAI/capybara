use std::path::Path;

use serde::{Deserialize, Serialize};

pub(super) const PACK_SCHEMA: &str = "capy.game_assets.pack.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GameAssetPack {
    pub schema: String,
    pub id: String,
    pub title: String,
    pub preset: String,
    pub mode: String,
    pub created_by: String,
    pub source_policy: SourcePolicy,
    pub build: PackBuild,
    pub assets: Vec<GameAsset>,
    pub outputs: PackOutputs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SourcePolicy {
    pub live_generation: bool,
    pub max_live_calls: u32,
    pub neutral_background: String,
    pub cutout_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PackBuild {
    pub frame_width: u32,
    pub frame_height: u32,
    pub frame_count: u32,
    pub spritesheet_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PackOutputs {
    pub preview_html: String,
    pub contact_sheet: String,
    pub report_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct GameAsset {
    pub id: String,
    pub name: String,
    pub kind: AssetKind,
    pub prompt_path: String,
    pub raw_path: String,
    pub transparent_path: String,
    #[serde(default)]
    pub actions: Vec<AnimationAction>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum AssetKind {
    Character,
    Enemy,
    Prop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct AnimationAction {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub frame_paths: Vec<String>,
    pub spritesheet_path: String,
    pub frame_ms: u32,
}

#[derive(Debug, Clone)]
pub(super) struct SourceJob {
    pub slug: &'static str,
    pub prompt_path: &'static str,
    pub output_path: &'static str,
    pub prompt: &'static str,
    pub visual: FixtureVisual,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FixtureVisual {
    pub kind: FixtureKind,
    pub action: FixtureAction,
    pub frames: u32,
    pub transparent: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FixtureKind {
    Hero,
    Enemy,
    Blade,
    Herb,
    Chest,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FixtureAction {
    Anchor,
    Idle,
    Run,
    Attack,
    Loop,
}

impl GameAssetPack {
    pub(super) fn refresh_counts(&mut self) {
        self.build.frame_count = self.frame_count();
        self.build.spritesheet_count = self.spritesheet_count();
    }

    pub(super) fn frame_count(&self) -> u32 {
        self.assets
            .iter()
            .flat_map(|asset| asset.actions.iter())
            .map(|action| action.frame_paths.len() as u32)
            .sum()
    }

    pub(super) fn spritesheet_count(&self) -> u32 {
        self.assets
            .iter()
            .flat_map(|asset| asset.actions.iter())
            .filter(|action| !action.spritesheet_path.trim().is_empty())
            .count() as u32
    }
}

pub(super) fn normalize_rel(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

pub(super) fn sample_pack(live_generation: bool, max_live_calls: u32) -> GameAssetPack {
    let mut pack = GameAssetPack {
        schema: PACK_SCHEMA.to_string(),
        id: "forest-action-rpg-compact".to_string(),
        title: "Forest Action RPG Compact Pack".to_string(),
        preset: "forest-action-rpg-compact".to_string(),
        mode: if live_generation { "live" } else { "fixture" }.to_string(),
        created_by: "capy game-assets sample".to_string(),
        source_policy: SourcePolicy {
            live_generation,
            max_live_calls,
            neutral_background: "#E0E0E0".to_string(),
            cutout_strategy: "neutral-gray-source-to-alpha-png".to_string(),
        },
        build: PackBuild {
            frame_width: 160,
            frame_height: 160,
            frame_count: 0,
            spritesheet_count: 0,
        },
        outputs: PackOutputs {
            preview_html: "preview/index.html".to_string(),
            contact_sheet: "qa/contact-sheet.png".to_string(),
            report_json: "report.json".to_string(),
        },
        assets: vec![
            GameAsset {
                id: "mossblade-ranger".to_string(),
                name: "Mossblade Ranger".to_string(),
                kind: AssetKind::Character,
                prompt_path: "prompts/hero-anchor.txt".to_string(),
                raw_path: "raw/hero-anchor.png".to_string(),
                transparent_path: "transparent/hero-anchor.png".to_string(),
                actions: vec![
                    action("idle", "Idle", "raw/hero-idle-strip.png"),
                    action("run", "Run", "raw/hero-run-strip.png"),
                    action("attack", "Attack", "raw/hero-attack-strip.png"),
                ],
                notes: "Playable forest ranger with readable silhouette.".to_string(),
            },
            GameAsset {
                id: "bramble-sentinel".to_string(),
                name: "Bramble Sentinel".to_string(),
                kind: AssetKind::Enemy,
                prompt_path: "prompts/enemy-loop.txt".to_string(),
                raw_path: "raw/enemy-loop-strip.png".to_string(),
                transparent_path: "transparent/enemy-loop-000.png".to_string(),
                actions: vec![action("loop", "Loop", "raw/enemy-loop-strip.png")],
                notes: "Small enemy loop for idle patrol states.".to_string(),
            },
            prop(
                "forest-blade",
                "Forest Blade",
                FixtureKind::Blade,
                "prompts/forest-blade.txt",
                "raw/forest-blade.png",
                "transparent/forest-blade.png",
            ),
            prop(
                "healing-herb",
                "Healing Herb",
                FixtureKind::Herb,
                "prompts/healing-herb.txt",
                "raw/healing-herb.png",
                "transparent/healing-herb.png",
            ),
            prop(
                "rune-chest",
                "Rune Chest",
                FixtureKind::Chest,
                "prompts/rune-chest.txt",
                "raw/rune-chest.png",
                "transparent/rune-chest.png",
            ),
        ],
    };
    pack.refresh_counts();
    pack
}

fn action(id: &str, name: &str, source_path: &str) -> AnimationAction {
    AnimationAction {
        id: id.to_string(),
        name: name.to_string(),
        source_path: source_path.to_string(),
        frame_paths: (0..4)
            .map(|index| format!("frames/{id}/{id}-{index:03}.png"))
            .collect(),
        spritesheet_path: format!("spritesheets/{id}.png"),
        frame_ms: 120,
    }
}

fn prop(
    id: &str,
    name: &str,
    kind: FixtureKind,
    prompt_path: &str,
    raw_path: &str,
    transparent_path: &str,
) -> GameAsset {
    let _kind = kind;
    GameAsset {
        id: id.to_string(),
        name: name.to_string(),
        kind: AssetKind::Prop,
        prompt_path: prompt_path.to_string(),
        raw_path: raw_path.to_string(),
        transparent_path: transparent_path.to_string(),
        actions: Vec::new(),
        notes: "Transparent prop ready for placement in a 2D scene.".to_string(),
    }
}

pub(super) fn source_jobs() -> Vec<SourceJob> {
    vec![
        SourceJob {
            slug: "hero-anchor",
            prompt_path: "prompts/hero-anchor.txt",
            output_path: "raw/hero-anchor.png",
            prompt: HERO_ANCHOR_PROMPT,
            visual: FixtureVisual {
                kind: FixtureKind::Hero,
                action: FixtureAction::Anchor,
                frames: 1,
                transparent: false,
            },
        },
        SourceJob {
            slug: "hero-idle-strip",
            prompt_path: "prompts/hero-idle-strip.txt",
            output_path: "raw/hero-idle-strip.png",
            prompt: HERO_IDLE_PROMPT,
            visual: strip(FixtureKind::Hero, FixtureAction::Idle),
        },
        SourceJob {
            slug: "hero-run-strip",
            prompt_path: "prompts/hero-run-strip.txt",
            output_path: "raw/hero-run-strip.png",
            prompt: HERO_RUN_PROMPT,
            visual: strip(FixtureKind::Hero, FixtureAction::Run),
        },
        SourceJob {
            slug: "hero-attack-strip",
            prompt_path: "prompts/hero-attack-strip.txt",
            output_path: "raw/hero-attack-strip.png",
            prompt: HERO_ATTACK_PROMPT,
            visual: strip(FixtureKind::Hero, FixtureAction::Attack),
        },
        SourceJob {
            slug: "enemy-loop-strip",
            prompt_path: "prompts/enemy-loop.txt",
            output_path: "raw/enemy-loop-strip.png",
            prompt: ENEMY_LOOP_PROMPT,
            visual: strip(FixtureKind::Enemy, FixtureAction::Loop),
        },
        SourceJob {
            slug: "forest-blade",
            prompt_path: "prompts/forest-blade.txt",
            output_path: "raw/forest-blade.png",
            prompt: BLADE_PROMPT,
            visual: single(FixtureKind::Blade),
        },
        SourceJob {
            slug: "healing-herb",
            prompt_path: "prompts/healing-herb.txt",
            output_path: "raw/healing-herb.png",
            prompt: HERB_PROMPT,
            visual: single(FixtureKind::Herb),
        },
        SourceJob {
            slug: "rune-chest",
            prompt_path: "prompts/rune-chest.txt",
            output_path: "raw/rune-chest.png",
            prompt: CHEST_PROMPT,
            visual: single(FixtureKind::Chest),
        },
    ]
}

fn strip(kind: FixtureKind, action: FixtureAction) -> FixtureVisual {
    FixtureVisual {
        kind,
        action,
        frames: 4,
        transparent: false,
    }
}

fn single(kind: FixtureKind) -> FixtureVisual {
    FixtureVisual {
        kind,
        action: FixtureAction::Anchor,
        frames: 1,
        transparent: false,
    }
}

const HERO_ANCHOR_PROMPT: &str = "Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source. Subject: One single moss-green fantasy ranger game sprite centered, fully visible, uncropped, 70% frame height. Important details: Clean silhouette, clear edges, small leaf cloak, short sword, even light, orthographic 2D game asset style. Use case: Source for automated alpha cutout and transparent PNG game sprite. Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.";
const HERO_IDLE_PROMPT: &str = "Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source. Subject: One single isolated four-frame horizontal sprite strip of the same moss-green fantasy ranger, fully visible and uncropped in every frame. Important details: Idle breathing motion, clean silhouette, consistent scale, 2D game asset style, frame cells evenly spaced. Use case: Source for automated alpha cutout, slicing, and idle spritesheet output. Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.";
const HERO_RUN_PROMPT: &str = "Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source. Subject: One single isolated four-frame horizontal sprite strip of the same moss-green fantasy ranger running, fully visible and uncropped in every frame. Important details: Clear readable legs, consistent scale, clean silhouette, 2D side-view game asset style, frame cells evenly spaced. Use case: Source for automated alpha cutout, slicing, and run spritesheet output. Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.";
const HERO_ATTACK_PROMPT: &str = "Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source. Subject: One single isolated four-frame horizontal sprite strip of the same moss-green fantasy ranger sword attack, fully visible and uncropped in every frame. Important details: Anticipation, slash, follow-through, recovery, clean silhouette, consistent scale. Use case: Source for automated alpha cutout, slicing, and attack spritesheet output. Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.";
const ENEMY_LOOP_PROMPT: &str = "Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source. Subject: Four-frame horizontal sprite strip of one bramble forest sentinel enemy, fully visible and uncropped in every frame. Important details: Root legs, glowing eyes, thorn crown, idle patrol loop, clean silhouette, 2D game asset style. Use case: Source for automated alpha cutout, slicing, and enemy loop spritesheet output. Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.";
const BLADE_PROMPT: &str = "Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source. Subject: One single forest blade item centered, fully visible, uncropped, 65% frame height. Important details: Moss-green hilt, leaf guard, clean silhouette, 2D game item icon style. Use case: Source for automated alpha cutout and transparent PNG prop output. Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.";
const HERB_PROMPT: &str = "Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source. Subject: One single healing herb bundle centered, fully visible, uncropped, 65% frame height. Important details: Bright leaf cluster, tiny blue flower, clean silhouette, 2D game item icon style. Use case: Source for automated alpha cutout and transparent PNG prop output. Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.";
const CHEST_PROMPT: &str = "Scene: Flat uniform matte #E0E0E0 neutral gray background for cutout source. Subject: One single small rune chest centered, fully visible, uncropped, 65% frame height. Important details: Wooden body, green rune lock, clean silhouette, 2D game prop icon style. Use case: Source for automated alpha cutout and transparent PNG prop output. Constraints: No text, no watermark, no extra objects, no green screen, no blue screen, no cast shadow, no reflection.";
