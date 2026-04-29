use clap::Args;

#[derive(Debug, Args)]
pub struct TtsArgs {
    #[arg(long)]
    brief: bool,
    #[command(subcommand)]
    command: Option<capy_tts::cli::Command>,
}

pub fn handle(args: TtsArgs) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("create TTS runtime failed: {error}"))?;

    runtime
        .block_on(capy_tts::cli::run_command(args.command, args.brief))
        .map_err(|error| format!("{error:#}"))
}
