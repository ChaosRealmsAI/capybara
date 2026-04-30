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
mod component;
mod cutout;
mod desktop_verify;
mod doctor;
mod game_assets;
mod help_topics;
mod image;
mod interaction;
mod ipc_client;
mod media;
mod motion;
mod poster;
mod project;
mod project_context;
mod project_patch;
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
  Common checks: `capy doctor`, `capy verify`, `capy image doctor`, `capy cutout doctor`, `capy motion doctor`, `capy game-assets doctor`, `capy clips doctor`, `capy tts doctor`.
  Common asset flow: `capy image generate --cutout-ready ...` then `capy cutout run ...`.
  Common game asset flow: `capy game-assets sample --preset forest-action-rpg-compact --out target/capy-game-assets-sample --overwrite`, then `capy game-assets verify --pack target/capy-game-assets-sample/pack.json`.
  Common UI flow: `capy devtools --query <css>`, then `capy click --query <css>` or `capy type --query <css> --text <text>`.
  Required params: image prompts use five labeled sections; cutout run needs --input/--output; click/type need --query.
  Pitfalls: live image/TTS provider calls may spend credits; click/type need a running shell and the right CAPYBARA_SOCKET.
  Command tag: [dev] means internal AI/dev verification or automation, not a PM-facing product workflow.
  Help topics: dev, doctor, interaction, desktop, project, context, patch, canvas, chat, agent, image, image-cutout, cutout, motion, game-assets, tts, tts-karaoke, tts-batch, clips, media, poster, component, timeline."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(
        about = "[dev] Run the Capybara desktop shell",
        after_help = "AI quick start:
  Use `capy shell` only when you need a foreground shell process for local debugging.
  Required params: none.
  Pitfalls: this command runs until the shell exits; for normal verification prefer `capy open`, `capy ps`, then `capy verify`.
  Next topic: `capy help dev` or `capy help desktop`."
    )]
    Shell,
    #[command(about = "[dev] Open or focus a Capybara project window")]
    Open(OpenArgs),
    #[command(
        about = "[dev] List running Capybara windows",
        after_help = "AI quick start:
  Use `capy ps` after `capy open` to confirm the shell socket and window ids.
  Required params: none.
  Pitfalls: use the same CAPYBARA_SOCKET for shell and CLI; stale sockets may answer from another instance.
  Next topic: `capy help desktop`."
    )]
    Ps,
    #[command(about = "[dev] Read a UI state key from the active Capybara window")]
    State(StateArgs),
    #[command(about = "[dev] Inspect DOM state for AI verification")]
    Devtools(DevtoolsArgs),
    #[command(about = "[dev] Capture a built-in app-view PNG for a DOM region")]
    Screenshot(ScreenshotArgs),
    #[command(about = "[dev] Capture the Capybara-owned app-view PNG")]
    Capture(CaptureArgs),
    #[command(about = "[dev] Run a no-spend project health check")]
    Doctor(doctor::DoctorArgs),
    #[command(about = "[dev] Click a DOM element in the active Capybara window")]
    Click(interaction::ClickArgs),
    #[command(about = "[dev] Type text into an input in the active Capybara window")]
    Type(interaction::TypeArgs),
    #[command(about = "Cut generated assets into transparent PNGs with withoutbg/focus")]
    Cutout(cutout::CutoutCliArgs),
    #[command(about = "[dev] Run a lightweight Capybara runtime verification")]
    Verify(VerifyArgs),
    #[command(about = "Manage persistent Claude/Codex conversations")]
    Chat(Box<chat::ChatArgs>),
    #[command(about = "Manage file-backed Capybara project packages")]
    Project(Box<project::ProjectArgs>),
    #[command(about = "Build AI-readable context packages from project artifacts")]
    Context(Box<project_context::ContextArgs>),
    #[command(about = "Dry-run or apply exact-text patches to project artifacts")]
    Patch(Box<project_patch::PatchArgs>),
    #[command(about = "Operate the live canvas through AI-safe commands")]
    Canvas(Box<canvas::CanvasArgs>),
    #[command(about = "Run AI-usable creative generation tools")]
    Image(Box<image::ImageArgs>),
    #[command(about = "Generate, slice, preview, and verify 2D game asset packs")]
    GameAssets(Box<game_assets::GameAssetsArgs>),
    #[command(about = "Convert videos into animation-grade transparent motion assets")]
    Motion(Box<motion::MotionArgs>),
    #[command(about = "Show self-contained AI help topics")]
    Help(HelpArgs),
    #[command(about = "Package video clips for scroll-driven HTML pages")]
    Media(Box<media::MediaArgs>),
    #[command(about = "Export Poster/PPT JSON into SVG, PNG, PDF, and image-based PPTX")]
    Poster(Box<poster::PosterArgs>),
    #[command(about = "Validate and inspect reusable Capybara component packages")]
    Component(Box<component::ComponentArgs>),
    #[command(about = "Operate Timeline composition and recorder integration")]
    Timeline(Box<timeline::TimelineArgs>),
    #[command(about = "Generate, preview, play, batch, and align TTS audio")]
    Tts(Box<tts::TtsArgs>),
    #[command(about = "Download, transcribe, align, cut, and preview video clips")]
    Clips(Box<clips::ClipsArgs>),
    #[command(about = "[dev] Inspect local agent runtimes")]
    Agent(Box<agent::AgentArgs>),
    #[command(
        about = "[dev] Quit the Capybara shell",
        after_help = "AI quick start:
  Use `capy quit` to close the shell instance for the current CAPYBARA_SOCKET.
  Required params: none.
  Pitfalls: do not run it against the user's active socket unless closing the product is intended.
  Next topic: `capy help dev`."
    )]
    Quit,
}

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy help` to list self-contained help topics, then `capy help <topic>` for the exact workflow.
  Required params: none for the topic index; optional TOPIC for a specific playbook.
  Pitfalls: do not skip topic help for unfamiliar workflows; `--help` is the index, not the long operating manual.
  Next topic: start with `capy help dev`, `capy help desktop`, or the domain topic shown by `capy help`.")]
struct HelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy open --project=demo` to start or focus the default project window.
  Required params: none; optional --project names the workspace, --new-window creates another window in the same shell.
  Pitfalls: keep CAPYBARA_SOCKET consistent across open/ps/state/devtools/capture.
  Next topic: `capy help desktop`.")]
struct OpenArgs {
    #[arg(long, default_value = "demo")]
    project: String,
    #[arg(long)]
    new_window: bool,
}

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy state --key=app.ready` for readiness and known UI state probes.
  Required params: --key.
  Common keys: app.ready, canvas.ready, canvas.nodeCount, canvas.selectedNode, planner.status.
  Pitfalls: unknown keys fail; use `capy devtools --eval` for one-off runtime probes.
  Next topic: `capy help desktop` or `capy help canvas`.")]
struct StateArgs {
    #[arg(long)]
    key: String,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy devtools --query <css> --get=bounding-rect` before click/type automation.
  Required params: either --query or --eval; --get defaults to outerHTML for queries.
  Pitfalls: --eval runs JavaScript in the active window; prefer click/type/state when they express the action.
  Next topic: `capy help interaction` or `capy help desktop`.")]
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
#[command(after_help = "AI quick start:
  Use `capy screenshot --region canvas --out <png>` for cropped DOM-region evidence.
  Required params: --out.
  Regions: full, canvas, planner, topbar.
  Pitfalls: this uses Capybara-owned app-view capture after a DOM rect probe; it must not request macOS Screen Recording permission.
  Next topic: `capy help desktop`.")]
struct ScreenshotArgs {
    #[arg(long, default_value = "full")]
    region: String,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy capture --out <png>` for full Capybara app-view evidence.
  Required params: --out.
  Pitfalls: requires a running shell window; the default path must use built-in app-view capture and must not request macOS Screen Recording permission.
  Next topic: `capy help desktop`.")]
struct CaptureArgs {
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    window: Option<String>,
}

#[derive(Debug, Args)]
#[command(after_help = "AI quick start:
  Use `capy verify` for readiness and `capy verify --profile desktop --capture-out <png>` for visible desktop proof.
  Required params: none for readiness; --capture-out is required for --profile desktop.
  Pitfalls: readiness alone is not visual verification; desktop profile checks browser identity, bridge, errors, topbar, and built-in app-view capture.
  Next topic: `capy help desktop`.")]
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
        Command::Screenshot(args) => {
            let out = absolute_path(args.out)?;
            send(
                "screenshot",
                json!({
                    "region": args.region,
                    "out": out.display().to_string(),
                    "window": args.window
                }),
            )
        }
        Command::Capture(args) => {
            let out = absolute_path(args.out)?;
            send(
                "capture",
                json!({ "out": out.display().to_string(), "window": args.window }),
            )
        }
        Command::Doctor(args) => doctor::handle(args),
        Command::Click(args) => interaction::click(args),
        Command::Type(args) => interaction::type_text(args),
        Command::Cutout(args) => cutout::handle(args),
        Command::Verify(args) => match args.profile {
            VerifyProfile::Readiness => send(
                "state-query",
                json!({ "key": "app.ready", "window": args.window, "verify": true }),
            ),
            VerifyProfile::Desktop => desktop_verify::verify(args.window, args.capture_out),
        },
        Command::Chat(args) => chat::handle(args),
        Command::Project(args) => project::handle(*args),
        Command::Context(args) => project_context::handle(*args),
        Command::Patch(args) => project_patch::handle(*args),
        Command::Canvas(args) => canvas::handle(*args),
        Command::Image(args) => image::handle(*args),
        Command::GameAssets(args) => game_assets::handle(*args),
        Command::Motion(args) => motion::handle(*args),
        Command::Help(args) => help_topics::print_capy_topic(args.topic.as_deref()),
        Command::Media(args) => media::handle(*args),
        Command::Poster(args) => poster::handle(*args),
        Command::Component(args) => component::handle(*args),
        Command::Timeline(args) => timeline::handle(*args),
        Command::Tts(args) => tts::handle(*args),
        Command::Clips(args) => clips::handle(*args),
        Command::Agent(args) => agent::handle(*args),
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

fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map_err(|err| format!("read cwd failed: {err}"))
        .map(|cwd| cwd.join(path))
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
