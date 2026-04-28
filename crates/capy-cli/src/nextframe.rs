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

pub fn handle(args: NextFrameArgs) -> Result<(), String> {
    match args.command {
        NextFrameCommand::Doctor(args) => doctor(args),
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

fn print_json<T: Serialize>(data: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(data).map_err(|err| err.to_string())?
    );
    Ok(())
}
