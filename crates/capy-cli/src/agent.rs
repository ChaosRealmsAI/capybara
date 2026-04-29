use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(
    disable_help_subcommand = true,
    after_help = "AI quick start:
  Use `capy agent --help` as the index and `capy agent help doctor` for the full runtime check workflow.
  Common command: `capy agent doctor`.
  Required params: none.
  Pitfalls: check runtime availability before starting long chat runs.
  Help topics: `capy agent help doctor`."
)]
pub struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    #[command(about = "Check Claude and Codex runtime availability")]
    Doctor,
    #[command(about = "Show self-contained AI help topics for agent runtime")]
    Help(AgentHelpArgs),
}

#[derive(Debug, Args)]
struct AgentHelpArgs {
    #[arg(value_name = "TOPIC")]
    topic: Option<String>,
}

pub fn handle(args: AgentArgs) -> Result<(), String> {
    match args.command {
        AgentCommand::Doctor => {
            println!("{}", capy_shell::agent::doctor());
            Ok(())
        }
        AgentCommand::Help(args) => crate::help_topics::print_agent_topic(args.topic.as_deref()),
    }
}
