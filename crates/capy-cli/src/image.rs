use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy image --help` as the index and `capy image help <topic>` for full workflows.
  Common commands: `capy image providers`, `capy image doctor`, `capy image generate --dry-run ...`, `capy image balance`.
  Required params: `generate` needs one five-section prompt: Scene, Subject, Important details, Use case, Constraints.
  Pitfalls: live generation spends credits unless --dry-run or --submit-only is used; use --out and --name when later steps need a file.
  Help topics: `capy image help agent`, `capy image help cutout-ready`."
)]
pub struct ImageArgs {
    #[command(subcommand)]
    command: ImageCommand,
}

#[derive(Debug, Subcommand)]
enum ImageCommand {
    #[command(about = "List image generation provider options")]
    Providers,
    #[command(about = "Check image generation provider readiness without spending credits")]
    Doctor(ImageProviderArgs),
    #[command(about = "Generate, submit, resume, or dry-run an image request")]
    Generate(ImageGenerateArgs),
    #[command(about = "Check image provider balance")]
    Balance(ImageProviderArgs),
    #[command(about = "Show self-contained AI help topics for image generation")]
    Help(ImageHelpArgs),
}

#[derive(Debug, Args)]
struct ImageHelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args)]
struct ImageProviderArgs {
    #[arg(long, value_enum, default_value = "apimart-gpt-image-2")]
    provider: ImageProviderArg,
}

#[derive(Debug, Args)]
struct ImageGenerateArgs {
    #[arg(long, value_enum, default_value = "apimart-gpt-image-2")]
    provider: ImageProviderArg,
    #[arg(long, default_value = "1:1")]
    size: String,
    #[arg(long, alias = "aspect-ratio")]
    aspect_ratio: Option<String>,
    #[arg(long, default_value = "1k")]
    resolution: String,
    #[arg(long = "ref")]
    refs: Vec<String>,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    submit_only: bool,
    #[arg(long)]
    resume: Option<String>,
    #[arg(long)]
    no_download: bool,
    #[arg(
        long,
        help = "Require prompt terms for images that will be passed to capy cutout"
    )]
    cutout_ready: bool,
    #[arg()]
    prompt: Vec<String>,
}

#[derive(Debug, Clone, ValueEnum)]
enum ImageProviderArg {
    #[value(name = "apimart-gpt-image-2")]
    ApimartGptImage2,
}

impl ImageProviderArg {
    fn id(&self) -> capy_image_gen::ImageProviderId {
        match self {
            ImageProviderArg::ApimartGptImage2 => capy_image_gen::ImageProviderId::ApimartGptImage2,
        }
    }
}

pub fn handle(args: ImageArgs) -> Result<(), String> {
    let data = match args.command {
        ImageCommand::Providers => json!({
            "ok": true,
            "providers": capy_image_gen::providers()
        }),
        ImageCommand::Doctor(args) => {
            serde_json::to_value(capy_image_gen::doctor(args.provider.id()))
                .map_err(|err| err.to_string())?
        }
        ImageCommand::Balance(args) => {
            capy_image_gen::balance(args.provider.id()).map_err(|err| err.to_string())?
        }
        ImageCommand::Generate(args) => {
            let request = image_generate_request(args)?;
            capy_image_gen::generate_image(request).map_err(|err| err.to_string())?
        }
        ImageCommand::Help(args) => {
            return crate::help_topics::print_image_topic(args.topic.as_deref());
        }
    };
    print_json(&data)
}

fn image_generate_request(
    args: ImageGenerateArgs,
) -> Result<capy_image_gen::GenerateImageRequest, String> {
    if args.dry_run && args.submit_only {
        return Err("--dry-run and --submit-only cannot be used together".to_string());
    }
    if args.resume.is_some() && (args.dry_run || args.submit_only || !args.prompt.is_empty()) {
        return Err(
            "--resume cannot be combined with prompt, --dry-run, or --submit-only".to_string(),
        );
    }
    let mode = if args.resume.is_some() {
        capy_image_gen::ImageGenerateMode::Resume
    } else if args.dry_run {
        capy_image_gen::ImageGenerateMode::DryRun
    } else if args.submit_only {
        capy_image_gen::ImageGenerateMode::SubmitOnly
    } else {
        capy_image_gen::ImageGenerateMode::Generate
    };
    let prompt = if args.prompt.is_empty() {
        None
    } else {
        Some(args.prompt.join(" "))
    };
    Ok(capy_image_gen::GenerateImageRequest {
        provider: args.provider.id(),
        mode,
        prompt,
        size: args.aspect_ratio.unwrap_or(args.size),
        resolution: args.resolution,
        refs: args.refs,
        output_dir: args.out,
        name: args.name,
        download: !args.no_download,
        task_id: args.resume,
        cutout_ready: args.cutout_ready,
    })
}

fn print_json(data: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}
