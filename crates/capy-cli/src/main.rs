use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};

mod agent;
mod canvas;
mod canvas_context;
mod chat;
mod chat_context;
mod clips;
mod cutout;
mod desktop_verify;
mod help_topics;
mod image;
mod ipc_client;
mod media;
mod timeline;
mod tts;

#[derive(Debug, Parser)]
#[command(
    name = "capy",
    version,
    about = "Capybara CLI for desktop control and AI-friendly verification",
    disable_help_subcommand = true,
    after_help = "AI quick start:
  capy --help is the index. Use `capy help <topic>` for self-contained workflows.
  Common checks: `capy verify`, `capy image doctor`, `capy cutout doctor`, `capy clips doctor`, `capy tts doctor`.
  Common asset flow: `capy image generate --cutout-ready ...` then `capy cutout run ...`.
  Required params: image prompts use five labeled sections; cutout run needs --input and --output.
  Pitfalls: live image/TTS provider calls may spend credits; cutout/TTS alignment may need init.
  Help topics: desktop, canvas, chat, agent, image, image-cutout, cutout, tts, tts-karaoke, tts-batch, clips, media, timeline."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Run the Capybara desktop shell")]
    Shell,
    #[command(about = "Open or focus a Capybara project window")]
    Open(OpenArgs),
    #[command(about = "List running Capybara windows")]
    Ps,
    #[command(about = "Read a UI state key from the active Capybara window")]
    State(StateArgs),
    #[command(about = "Inspect DOM state for AI verification")]
    Devtools(DevtoolsArgs),
    #[command(about = "Capture a real desktop PNG for a DOM region")]
    Screenshot(ScreenshotArgs),
    #[command(about = "Capture the native macOS window PNG")]
    Capture(CaptureArgs),
    #[command(about = "Cut generated assets into transparent PNGs with withoutbg/focus")]
    Cutout(cutout::CutoutCliArgs),
    #[command(about = "Run a lightweight Capybara runtime verification")]
    Verify(VerifyArgs),
    #[command(about = "Manage persistent Claude/Codex conversations")]
    Chat(Box<chat::ChatArgs>),
    #[command(about = "Operate the live canvas through AI-safe commands")]
    Canvas(canvas::CanvasArgs),
    #[command(about = "Run AI-usable creative generation tools")]
    Image(image::ImageArgs),
    #[command(about = "Show self-contained AI help topics")]
    Help(HelpArgs),
    #[command(about = "Package video clips for scroll-driven HTML pages")]
    Media(media::MediaArgs),
    #[command(about = "Operate Timeline composition and recorder integration")]
    Timeline(timeline::TimelineArgs),
    #[command(about = "Generate, preview, play, batch, and align TTS audio")]
    Tts(tts::TtsArgs),
    #[command(about = "Download, transcribe, align, cut, and preview video clips")]
    Clips(clips::ClipsArgs),
    #[command(about = "Inspect local agent runtimes")]
    Agent(agent::AgentArgs),
    #[command(about = "Quit the Capybara shell")]
    Quit,
}

#[derive(Debug, Args)]
struct HelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args)]
struct OpenArgs {
    #[arg(long, default_value = "demo")]
    project: String,
    #[arg(long)]
    new_window: bool,
}

#[derive(Debug, Args)]
struct StateArgs {
    #[arg(long)]
    key: String,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct DevtoolsArgs {
    #[arg(long)]
    query: Option<String>,
    #[arg(long)]
    eval: Option<String>,
    #[arg(long, default_value = "outerHTML")]
    get: String,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct ScreenshotArgs {
    #[arg(long, default_value = "full")]
    region: String,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct CaptureArgs {
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
struct VerifyArgs {
    #[arg(long)]
    window: Option<String>,
    #[arg(long, value_enum, default_value = "readiness")]
    profile: VerifyProfile,
    #[arg(long, help = "Required for --profile desktop")]
    capture_out: Option<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
enum VerifyProfile {
    Readiness,
    Desktop,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Shell => {
            capy_shell::run();
            Ok(())
        }
        Command::Open(args) => send(
            "open-window",
            json!({ "project": args.project, "new_window": args.new_window }),
        ),
        Command::Ps => send("state-query", json!({ "query": "windows" })),
        Command::State(args) => send(
            "state-query",
            json!({ "key": args.key, "window": args.window }),
        ),
        Command::Devtools(args) => {
            if let Some(script) = args.eval {
                send(
                    "devtools-eval",
                    json!({ "eval": script, "window": args.window }),
                )
            } else {
                let query = args
                    .query
                    .ok_or_else(|| "missing --query or --eval for devtools".to_string())?;
                send(
                    "devtools-query",
                    json!({ "query": query, "get": args.get, "window": args.window }),
                )
            }
        }
        Command::Screenshot(args) => send(
            "screenshot",
            json!({
                "region": args.region,
                "out": args.out.display().to_string(),
                "window": args.window
            }),
        ),
        Command::Capture(args) => send(
            "capture",
            json!({ "out": args.out.display().to_string(), "window": args.window }),
        ),
        Command::Cutout(args) => cutout::handle(args),
        Command::Verify(args) => match args.profile {
            VerifyProfile::Readiness => send(
                "state-query",
                json!({ "key": "app.ready", "window": args.window, "verify": true }),
            ),
            VerifyProfile::Desktop => desktop_verify::verify(args.window, args.capture_out),
        },
        Command::Chat(args) => chat::handle(args),
        Command::Canvas(args) => canvas::handle(args),
        Command::Image(args) => image::handle(args),
        Command::Help(args) => help_topics::print_capy_topic(args.topic.as_deref()),
        Command::Media(args) => media::handle(args),
        Command::Timeline(args) => timeline::handle(args),
        Command::Tts(args) => tts::handle(args),
        Command::Clips(args) => clips::handle(args),
        Command::Agent(args) => agent::handle(args),
        Command::Quit => send("quit", json!({})),
    }
}

pub(crate) fn send(op: &str, params: Value) -> Result<(), String> {
    let data = request_data(op, params)?;
    print_json(&data)
}

pub(crate) fn print_json(data: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}

fn request_data(op: &str, params: Value) -> Result<Value, String> {
    let request = ipc_client::request(op, params);
    let response = ipc_client::send(request)?;
    if response.ok {
        return Ok(response.data.unwrap_or(Value::Null));
    }
    Err(response
        .error
        .map(|value| value.to_string())
        .unwrap_or_else(|| "capy IPC request failed".to_string()))
}
