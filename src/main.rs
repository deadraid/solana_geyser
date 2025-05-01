use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod config;
mod geyser;
mod solana;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Path to the YAML configuration file
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Connect to Shyft Geyser and send a transfer for every new block
    Subscribe {
        /// Send transfer every N-th finalized block (default 1 => every block)
        #[arg(long, default_value_t = 1)]
        every: u64,

        /// Maximum number of concurrent transfers in flight
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let cfg = config::read_config(cli.config)?;

    match cli.command {
        Command::Subscribe { every, concurrency } => {
            geyser::subscribe_and_transfer(cfg, every, concurrency).await?
        }
    }

    Ok(())
}
