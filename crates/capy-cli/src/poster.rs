use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy poster export --input <poster.json> --out <dir>` to turn capy.poster.document.v1 into SVG/PNG/PDF/PPTX/JSON files.
  Required params: export needs --input and --out.
  Defaults: --formats svg,png,pdf,pptx,json and --page all.
  Pitfalls: component layers need a static svg template; runtime-only components preview in the app but fail static export.
  Evidence: save stdout JSON plus output manifest.json under the version evidence assets."
)]
pub struct PosterArgs {
    #[command(subcommand)]
    command: PosterCommand,
}

#[derive(Debug, Subcommand)]
enum PosterCommand {
    #[command(about = "Export capy.poster.document.v1 to real delivery files")]
    Export(PosterExportArgs),
}

#[derive(Debug, Args)]
pub struct PosterExportArgs {
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = "svg,png,pdf,pptx,json")]
    formats: String,
    #[arg(long, default_value = "all")]
    page: String,
}

pub fn handle(args: PosterArgs) -> Result<(), String> {
    match args.command {
        PosterCommand::Export(args) => export(args),
    }
}

fn export(args: PosterExportArgs) -> Result<(), String> {
    let document = capy_poster::read_document_v1(&args.input).map_err(|err| err.to_string())?;
    let formats = parse_formats(&args.formats)?;
    let report = capy_poster::export_document(capy_poster::ExportRequest {
        document,
        out_dir: args.out,
        formats,
        page: (args.page != "all").then_some(args.page),
    })
    .map_err(|err| err.to_string())?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
    );
    Ok(())
}

pub(crate) fn parse_formats(value: &str) -> Result<Vec<capy_poster::ExportFormat>, String> {
    value
        .split(',')
        .map(capy_poster::ExportFormat::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}
