use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

#[derive(Debug, Args)]
pub struct NextFrameArgs {
    #[command(subcommand)]
    command: NextFrameCommand,
}

#[derive(Debug, Subcommand)]
enum NextFrameCommand {
    #[command(about = "Check NextFrame binary adapter availability")]
    Doctor(NextFrameDoctorArgs),
    #[command(about = "Compose Poster JSON into a NextFrame composition project")]
    ComposePoster(NextFrameComposePosterArgs),
}

#[derive(Debug, Args)]
struct NextFrameDoctorArgs {
    #[arg(long)]
    nf: Option<PathBuf>,
    #[arg(long, alias = "nf-recorder")]
    recorder: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct NextFrameComposePosterArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    project: Option<String>,
    #[arg(long)]
    composition: Option<String>,
    #[arg(long, default_value_t = 1000)]
    duration_ms: u64,
}

pub fn handle(args: NextFrameArgs) -> Result<(), String> {
    match args.command {
        NextFrameCommand::Doctor(args) => doctor(args),
        NextFrameCommand::ComposePoster(args) => compose_poster(args),
    }
}

fn doctor(args: NextFrameDoctorArgs) -> Result<(), String> {
    let report = capy_nextframe::doctor(capy_nextframe::NextFrameConfig {
        nf_bin: args.nf,
        recorder_bin: args.recorder,
        home: args.home,
    });
    print_json(&report)
}

fn compose_poster(args: NextFrameComposePosterArgs) -> Result<(), String> {
    let request = capy_nextframe::ComposePosterRequest {
        poster_path: args.input,
        project_slug: args.project,
        composition_id: args.composition,
        output_dir: args.out,
        duration_ms: args.duration_ms,
    };
    match capy_nextframe::compose_poster(request) {
        Ok(report) => print_json(&report),
        Err(err) => {
            print_json(&capy_nextframe::compose::failure(err))?;
            std::process::exit(1);
        }
    }
}

fn print_json<T: Serialize>(data: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}
