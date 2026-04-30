use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy component validate --path fixtures/components` to check capy.component.v1 packages.
  Required params: validate/inspect need --path. --path may be a components root or one component package dir.
  Output: JSON report with schema capy.component.validation.v1.
  Pitfalls: first version accepts only trusted local single-file runtime modules; remote/untrusted sandboxing is a later version.
  Next topic: `capy help component`."
)]
pub struct ComponentArgs {
    #[command(subcommand)]
    command: ComponentCommand,
}

#[derive(Debug, Subcommand)]
enum ComponentCommand {
    #[command(about = "Validate a components root or one capy.component.v1 package")]
    Validate(ComponentPathArgs),
    #[command(about = "Inspect one capy.component.v1 package")]
    Inspect(ComponentPathArgs),
}

#[derive(Debug, Args)]
pub struct ComponentPathArgs {
    #[arg(long)]
    path: PathBuf,
}

pub fn handle(args: ComponentArgs) -> Result<(), String> {
    let report = match args.command {
        ComponentCommand::Validate(args) => validate(args.path),
        ComponentCommand::Inspect(args) => capy_components::inspect_component(&args.path),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
    );
    if report.ok {
        Ok(())
    } else {
        Err("component validation failed".to_string())
    }
}

fn validate(path: PathBuf) -> capy_components::ComponentValidationReport {
    if path.join("component.json").is_file() || path.is_file() {
        capy_components::inspect_component(&path)
    } else {
        capy_components::validate_components_root(&path)
    }
}
