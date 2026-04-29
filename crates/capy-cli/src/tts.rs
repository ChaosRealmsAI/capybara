use clap::Args;

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy tts --help` as the index and `capy tts help <topic>` for full workflows.
  Common commands: doctor, init, synth, batch, voices, preview, play, concat.
  Required params: synth needs text or --file; batch needs JSON input and -d <out-dir>.
  Pitfalls: Edge ignores emotion/SSML; use punctuation for pauses; use init/doctor for alignment.
  Help topics: `capy tts help agent`, `capy tts help karaoke`, `capy tts help batch`."
)]
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
