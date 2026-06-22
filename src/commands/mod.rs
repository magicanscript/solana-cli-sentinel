use anyhow::Result;
use clap::Subcommand;

use crate::config::Config;

pub mod status;
pub mod watch;

#[derive(Subcommand)]
pub enum Commands {
    /// Single probe of both nodes: prints slot, RTT, and status to stdout.
    /// Exits with code 1 if a problem is detected.
    Status,

    /// Starts the monitoring daemon: polls nodes every N seconds,
    /// generates an LLM alert on a problem and sends it to Telegram.
    Watch,
}

/// Command dispatcher. Receives the parsed command and configuration,
/// calls the corresponding handler.
pub async fn execute(cmd: Commands, cfg: Config) -> Result<()> {
    match cmd {
        Commands::Status => status::run(cfg).await,
        Commands::Watch => watch::run(cfg).await,
    }
}
