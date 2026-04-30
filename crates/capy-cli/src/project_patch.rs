use std::fs;
use std::path::PathBuf;

use capy_project::{PatchDocumentV1, ProjectPackage};
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy patch apply --project <dir> --patch <patch.json> --dry-run` before applying a source mutation.
  Required params: apply needs --project and --patch.
  Patch schema: capy.patch.v1 with replace_exact_text operations. old_text must match exactly once.
  Pitfalls: this command does not call AI and does not edit derived outputs directly; patch source artifacts and verify afterward.
  Help topic: `capy help patch`."
)]
pub struct PatchArgs {
    #[command(subcommand)]
    command: PatchCommand,
}

#[derive(Debug, Subcommand)]
enum PatchCommand {
    #[command(about = "Dry-run or apply a capy.patch.v1 document")]
    Apply(PatchApplyArgs),
}

#[derive(Debug, Args)]
struct PatchApplyArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long)]
    patch: PathBuf,
    #[arg(long)]
    dry_run: bool,
}

pub fn handle(args: PatchArgs) -> Result<(), String> {
    let PatchCommand::Apply(args) = args.command;
    let package = ProjectPackage::open(args.project).map_err(|err| err.to_string())?;
    let raw = fs::read_to_string(&args.patch)
        .map_err(|err| format!("read patch document {} failed: {err}", args.patch.display()))?;
    let patch: PatchDocumentV1 = serde_json::from_str(&raw).map_err(|err| {
        format!(
            "parse patch document {} failed: {err}",
            args.patch.display()
        )
    })?;
    let result = package
        .apply_patch(patch, Some(args.patch.display().to_string()), args.dry_run)
        .map_err(|err| err.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|err| err.to_string())?
    );
    Ok(())
}
