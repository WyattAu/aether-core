//! Aether CLI Entry Point

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use clap::Parser;
use std::process::ExitCode;

mod commands;

/// Aether: The Post-Container Application OS
#[derive(Parser, Debug)]
#[command(name = "aether")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Available CLI subcommands
    #[command(subcommand)]
    pub command: commands::Command,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match commands::execute(cli.command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
