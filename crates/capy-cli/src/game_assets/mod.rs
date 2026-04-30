mod model;
mod render;

use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

use model::{GameAssetPack, normalize_rel, sample_pack, source_jobs};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy game-assets sample --preset forest-action-rpg-compact --out target/capy-game-assets-sample --overwrite` to create a no-spend asset pack.
  Then run `capy game-assets verify --pack target/capy-game-assets-sample/pack.json`.
  Required params: sample needs --out; build/verify need --pack.
  Pitfalls: sample is no-spend unless --live is passed; --live is capped by --max-live-calls and should only be used after reading `capy game-assets help live`.
  Help topics: `capy game-assets help agent`, `capy game-assets help live`, `capy game-assets help manifest`."
)]
pub struct GameAssetsArgs {
    #[command(subcommand)]
    command: GameAssetsCommand,
}

#[derive(Debug, Subcommand)]
enum GameAssetsCommand {
    #[command(about = "Check local game asset workflow readiness without spending credits")]
    Doctor,
    #[command(about = "Generate a compact 2D game asset sample pack")]
    Sample(SampleArgs),
    #[command(about = "Build frames, spritesheets, preview, and QA outputs from a pack")]
    Build(PackPathArgs),
    #[command(about = "Verify pack manifest and generated outputs")]
    Verify(PackPathArgs),
    #[command(about = "Show self-contained game asset help topics")]
    Help(HelpArgs),
}

#[derive(Debug, Args)]
struct HelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args)]
struct PackPathArgs {
    #[arg(long)]
    pack: PathBuf,
}

#[derive(Debug, Args)]
struct SampleArgs {
    #[arg(long, value_enum, default_value = "forest-action-rpg-compact")]
    preset: GameAssetPreset,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    overwrite: bool,
    #[arg(
        long,
        help = "Use live image generation. Omitted by default to avoid spend."
    )]
    live: bool,
    #[arg(long, default_value_t = 8)]
    max_live_calls: u32,
}

#[derive(Debug, Clone, ValueEnum)]
enum GameAssetPreset {
    #[value(name = "forest-action-rpg-compact")]
    ForestActionRpgCompact,
}

pub fn handle(args: GameAssetsArgs) -> Result<(), String> {
    match args.command {
        GameAssetsCommand::Doctor => crate::print_json(&doctor_report()),
        GameAssetsCommand::Sample(args) => {
            create_sample(args).and_then(|data| crate::print_json(&data))
        }
        GameAssetsCommand::Build(args) => {
            build_existing_pack(args).and_then(|data| crate::print_json(&data))
        }
        GameAssetsCommand::Verify(args) => {
            verify_existing_pack(args).and_then(|data| crate::print_json(&data))
        }
        GameAssetsCommand::Help(args) => {
            crate::help_topics::print_game_assets_topic(args.topic.as_deref())
        }
    }
}

fn doctor_report() -> Value {
    let image_provider = capy_image_gen::doctor(capy_image_gen::ImageProviderId::ApimartGptImage2);
    json!({
        "ok": true,
        "schema": "capy.game_assets.doctor.v1",
        "commands": ["doctor", "sample", "build", "verify", "help"],
        "default_mode": "fixture-no-spend",
        "live_generation_available": image_provider.ok,
        "provider": image_provider,
        "required_dirs": ["prompts", "raw", "transparent", "frames", "spritesheets", "qa", "preview"]
    })
}

fn create_sample(args: SampleArgs) -> Result<Value, String> {
    validate_sample_args(&args)?;
    let root = args.out;
    prepare_output_dir(&root, args.overwrite)?;
    let mut pack = sample_pack(args.live, args.max_live_calls);
    render::ensure_pack_dirs(&root)?;
    for job in source_jobs() {
        render::write_prompt(&root, &job)?;
        if args.live {
            generate_live_source(&root, &job)?;
        } else {
            render::write_fixture_source(&root, &job)?;
        }
    }
    render::build_pack_outputs(&mut pack, &root)?;
    let pack_path = render::write_pack_json(&pack, &root)?;
    let verify = render::verify_pack(&pack, &root);
    Ok(json!({
        "ok": true,
        "schema": "capy.game_assets.sample.v1",
        "preset": args.preset.as_str(),
        "mode": pack.mode,
        "pack_path": normalize_rel(&pack_path),
        "preview_html": normalize_rel(root.join(&pack.outputs.preview_html)),
        "contact_sheet": normalize_rel(root.join(&pack.outputs.contact_sheet)),
        "asset_count": pack.assets.len(),
        "frame_count": pack.frame_count(),
        "spritesheet_count": pack.spritesheet_count(),
        "live_call_count": if args.live { source_jobs().len() } else { 0 },
        "verify": verify
    }))
}

fn build_existing_pack(args: PackPathArgs) -> Result<Value, String> {
    let pack_path = args.pack;
    let root = pack_root(&pack_path)?;
    let mut pack = read_pack(&pack_path)?;
    render::ensure_pack_dirs(&root)?;
    render::build_pack_outputs(&mut pack, &root)?;
    render::write_pack_json(&pack, &root)?;
    let verify = render::verify_pack(&pack, &root);
    Ok(json!({
        "ok": verify.get("verdict").and_then(Value::as_str) == Some("passed"),
        "schema": "capy.game_assets.build.v1",
        "pack_path": normalize_rel(&pack_path),
        "asset_count": pack.assets.len(),
        "frame_count": pack.frame_count(),
        "spritesheet_count": pack.spritesheet_count(),
        "verify": verify
    }))
}

fn verify_existing_pack(args: PackPathArgs) -> Result<Value, String> {
    let pack_path = args.pack;
    let root = pack_root(&pack_path)?;
    let pack = read_pack(&pack_path)?;
    Ok(render::verify_pack(&pack, &root))
}

fn validate_sample_args(args: &SampleArgs) -> Result<(), String> {
    if args.live && args.max_live_calls == 0 {
        return Err("--live requires --max-live-calls greater than 0".to_string());
    }
    if args.live && source_jobs().len() as u32 > args.max_live_calls {
        return Err(format!(
            "--live would make {} provider calls, above --max-live-calls={}",
            source_jobs().len(),
            args.max_live_calls
        ));
    }
    Ok(())
}

fn prepare_output_dir(root: &Path, overwrite: bool) -> Result<(), String> {
    if root.exists() {
        if !overwrite {
            return Err(format!(
                "{} already exists; pass --overwrite or choose another --out",
                root.display()
            ));
        }
        fs::remove_dir_all(root)
            .map_err(|err| format!("remove {} failed: {err}", root.display()))?;
    }
    fs::create_dir_all(root).map_err(|err| format!("create {} failed: {err}", root.display()))
}

fn generate_live_source(root: &Path, job: &model::SourceJob) -> Result<(), String> {
    let output_dir = root.join("raw");
    let response = capy_image_gen::generate_image(capy_image_gen::GenerateImageRequest {
        provider: capy_image_gen::ImageProviderId::ApimartGptImage2,
        mode: capy_image_gen::ImageGenerateMode::Generate,
        prompt: Some(job.prompt.to_string()),
        size: if job.visual.frames > 1 { "16:9" } else { "1:1" }.to_string(),
        resolution: "1k".to_string(),
        refs: Vec::new(),
        output_dir: Some(output_dir),
        name: Some(job.slug.to_string()),
        download: true,
        task_id: None,
        cutout_ready: true,
    })
    .map_err(|err| err.to_string())?;
    let downloaded = capy_image_gen::find_downloaded_image_path(&response).ok_or_else(|| {
        format!(
            "provider did not return a downloaded image path for {}",
            job.slug
        )
    })?;
    let target = root.join(job.output_path);
    if downloaded != target {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
        }
        fs::copy(&downloaded, &target).map_err(|err| {
            format!(
                "copy live source {} to {} failed: {err}",
                downloaded.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}

fn read_pack(path: &Path) -> Result<GameAssetPack, String> {
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("parse {} failed: {err}", path.display()))
}

fn pack_root(pack_path: &Path) -> Result<PathBuf, String> {
    pack_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{} has no parent directory", pack_path.display()))
}

impl GameAssetPreset {
    fn as_str(&self) -> &'static str {
        match self {
            GameAssetPreset::ForestActionRpgCompact => "forest-action-rpg-compact",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_sample_creates_verifiable_pack() -> Result<(), String> {
        let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
        let out = dir.path().join("sample");
        let response = create_sample(SampleArgs {
            preset: GameAssetPreset::ForestActionRpgCompact,
            out: out.clone(),
            overwrite: false,
            live: false,
            max_live_calls: 8,
        })?;
        assert_eq!(response.get("ok").and_then(Value::as_bool), Some(true));
        let verify = verify_existing_pack(PackPathArgs {
            pack: out.join("pack.json"),
        })?;
        assert_eq!(
            verify.get("verdict").and_then(Value::as_str),
            Some("passed")
        );
        assert_eq!(verify.get("frame_count").and_then(Value::as_u64), Some(16));
        Ok(())
    }

    #[test]
    fn live_generation_respects_call_cap() {
        let result = validate_sample_args(&SampleArgs {
            preset: GameAssetPreset::ForestActionRpgCompact,
            out: PathBuf::from("target/unused"),
            overwrite: false,
            live: true,
            max_live_calls: 2,
        });
        assert!(result.is_err());
    }
}
