use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    #[command(about = "Check Claude and Codex runtime availability")]
    Doctor,
}

pub fn handle(args: AgentArgs) -> Result<(), String> {
    match args.command {
        AgentCommand::Doctor => {
            println!("{}", capy_shell::agent::doctor());
            Ok(())
        }
    }
}
