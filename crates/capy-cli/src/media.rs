use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy media --help` as the index and `capy media help <topic>` for full workflows.
  Common commands: scroll-pack, story-pack, serve, inspect.
  Required params: scroll-pack needs --input/--out; story-pack needs --manifest/--out; serve needs --root.
  Pitfalls: do not pass --emit-html and --emit-composition together; browser verification should use HTTP.
  Help topics: `capy media help scroll-pack`, `capy media help story-pack`."
)]
pub struct MediaArgs {
    #[command(subcommand)]
    command: MediaCommand,
}

#[derive(Debug, Subcommand)]
enum MediaCommand {
    #[command(about = "Turn one video into an HTML-ready scroll media package")]
    ScrollPack(MediaScrollPackArgs),
    #[command(about = "Turn a multi-video story manifest into a scroll landing page package")]
    StoryPack(MediaStoryPackArgs),
    #[command(about = "Serve a scroll media package with HTTP Range support")]
    Serve(MediaServeArgs),
    #[command(about = "Inspect a scroll media manifest")]
    Inspect(MediaInspectArgs),
    #[command(about = "Show self-contained AI help topics for media packaging")]
    Help(MediaHelpArgs),
}

#[derive(Debug, Args)]
struct MediaHelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args)]
struct MediaScrollPackArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    emit_composition: bool,
    #[arg(long)]
    emit_html: bool,
    #[arg(long, default_value_t = 1280)]
    poster_width: u32,
    #[arg(long = "default", default_value = "720:23")]
    default_preset: String,
    #[arg(long, default_value = "720:27")]
    fallback: String,
    #[arg(long, default_value = "1080:24")]
    hq: String,
    #[arg(long)]
    verify: bool,
    #[arg(long)]
    overwrite: bool,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct MediaStoryPackArgs {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    emit_composition: bool,
    #[arg(long)]
    emit_html: bool,
    #[arg(long, default_value_t = 1280)]
    poster_width: u32,
    #[arg(long = "default", default_value = "720:23")]
    default_preset: String,
    #[arg(long, default_value = "360:30")]
    fallback: String,
    #[arg(long, default_value = "1080:24")]
    hq: String,
    #[arg(long)]
    verify: bool,
    #[arg(long)]
    overwrite: bool,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct MediaServeArgs {
    #[arg(long)]
    root: PathBuf,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 5202)]
    port: u16,
}

#[derive(Debug, Args)]
struct MediaInspectArgs {
    #[arg(long)]
    manifest: PathBuf,
}

pub fn handle(args: MediaArgs) -> Result<(), String> {
    match args.command {
        MediaCommand::ScrollPack(args) => {
            reject_mixed_emit_flags(args.emit_html, args.emit_composition)?;
            if args.emit_composition {
                let request = scroll_composition_request(args)?;
                let report = capy_scroll_media::emit_scroll_composition(request)
                    .map_err(|err| err.to_string())?;
                print_json(&report)
            } else {
                let request = scroll_pack_request(args)?;
                let report =
                    capy_scroll_media::scroll_pack(request).map_err(|err| err.to_string())?;
                print_json(&report)
            }
        }
        MediaCommand::StoryPack(args) => {
            reject_mixed_emit_flags(args.emit_html, args.emit_composition)?;
            if args.emit_composition {
                let request = story_composition_request(args);
                let report = capy_scroll_media::emit_story_composition(request)
                    .map_err(|err| err.to_string())?;
                print_json(&report)
            } else {
                let request = story_pack_request(args)?;
                let report =
                    capy_scroll_media::story_pack(request).map_err(|err| err.to_string())?;
                print_json(&report)
            }
        }
        MediaCommand::Serve(args) => {
            capy_scroll_media::serve_static(capy_scroll_media::ServeOptions {
                root: args.root,
                host: args.host,
                port: args.port,
            })
            .map_err(|err| err.to_string())
        }
        MediaCommand::Inspect(args) => {
            let manifest = capy_scroll_media::inspect_manifest(&args.manifest)
                .map_err(|err| err.to_string())?;
            print_json(&manifest)
        }
        MediaCommand::Help(args) => crate::help_topics::print_media_topic(args.topic.as_deref()),
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn scroll_pack_request(
    args: MediaScrollPackArgs,
) -> Result<capy_scroll_media::ScrollPackRequest, String> {
    let name = args
        .name
        .unwrap_or_else(|| default_name(&args.input, "scroll-pack"));
    Ok(capy_scroll_media::ScrollPackRequest {
        input: args.input,
        out_dir: args.out,
        name,
        poster_width: args.poster_width,
        default_preset: capy_scroll_media::ClipPreset::parse(
            capy_scroll_media::ClipRole::Default,
            &args.default_preset,
        )
        .map_err(|err| err.to_string())?,
        fallback_preset: capy_scroll_media::ClipPreset::parse(
            capy_scroll_media::ClipRole::Fallback,
            &args.fallback,
        )
        .map_err(|err| err.to_string())?,
        hq_preset: capy_scroll_media::ClipPreset::parse(capy_scroll_media::ClipRole::Hq, &args.hq)
            .map_err(|err| err.to_string())?,
        verify: args.verify,
        overwrite: args.overwrite,
        dry_run: args.dry_run,
    })
}

fn scroll_composition_request(
    args: MediaScrollPackArgs,
) -> Result<capy_scroll_media::ScrollCompositionRequest, String> {
    let name = args
        .name
        .unwrap_or_else(|| default_name(&args.input, "scroll-pack"));
    Ok(capy_scroll_media::ScrollCompositionRequest {
        name,
        input: args.input,
        out_dir: args.out,
        overwrite: args.overwrite,
    })
}

fn story_pack_request(
    args: MediaStoryPackArgs,
) -> Result<capy_scroll_media::StoryPackRequest, String> {
    Ok(capy_scroll_media::StoryPackRequest {
        manifest: args.manifest,
        out_dir: args.out,
        poster_width: args.poster_width,
        default_preset: capy_scroll_media::ClipPreset::parse(
            capy_scroll_media::ClipRole::Default,
            &args.default_preset,
        )
        .map_err(|err| err.to_string())?,
        fallback_preset: capy_scroll_media::ClipPreset::parse(
            capy_scroll_media::ClipRole::Fallback,
            &args.fallback,
        )
        .map_err(|err| err.to_string())?,
        hq_preset: capy_scroll_media::ClipPreset::parse(capy_scroll_media::ClipRole::Hq, &args.hq)
            .map_err(|err| err.to_string())?,
        verify: args.verify,
        overwrite: args.overwrite,
        dry_run: args.dry_run,
    })
}

fn story_composition_request(
    args: MediaStoryPackArgs,
) -> capy_scroll_media::StoryCompositionRequest {
    capy_scroll_media::StoryCompositionRequest {
        manifest: args.manifest,
        out_dir: args.out,
        overwrite: args.overwrite,
    }
}

fn reject_mixed_emit_flags(emit_html: bool, emit_composition: bool) -> Result<(), String> {
    if emit_html && emit_composition {
        return Err("--emit-html and --emit-composition are mutually exclusive".to_string());
    }
    Ok(())
}

fn default_name(input: &std::path::Path, fallback: &str) -> String {
    input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}
