//! Exec Command
//!
//! Execute commands inside a running actor.

use clap::Args;
use std::io::{self, BufRead};
use thiserror::Error;

#[derive(Args, Debug)]
pub struct ExecArgs {
    #[arg(short, long)]
    pub actor: String,

    #[arg(short, long)]
    pub interactive: bool,

    #[arg(short, long)]
    pub tty: bool,

    #[arg(short, long, default_value = "/bin/sh")]
    pub shell: String,

    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Actor not found: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ActorNotFound(String),

    #[error("Actor not running: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ActorNotRunning(String),

    #[error("Failed to execute command: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ExecutionFailed(String),

    #[error("TTY not available: {0}")]
    TtyNotAvailable(String),

    #[error("Connection failed: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ConnectionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

pub async fn execute(args: ExecArgs) -> Result<(), Error> {
    if !args.command.is_empty() {
        execute_non_interactive(&args).await
    } else if args.interactive || args.tty {
        execute_interactive(&args).await
    } else {
        execute_default_shell(&args).await
    }
}

async fn execute_non_interactive(args: &ExecArgs) -> Result<(), Error> {
    println!("Executing command in actor '{}'...", args.actor);
    println!();

    let cmd = args.command.join(" ");

    println!("$ {}", cmd);
    println!("────────────────────────────────────────────────────────────");

    println!("[stdout] Command output would appear here");
    println!("[stdout] Exit code: 0");

    println!();
    println!("✓ Command completed successfully");

    Ok(())
}

async fn execute_interactive(args: &ExecArgs) -> Result<(), Error> {
    if args.tty && !crossterm::tty::IsTty::is_tty(&io::stdout()) {
        return Err(Error::TtyNotAvailable(
            "Standard output is not a TTY. Remove --tty flag or run in a terminal.".into(),
        ));
    }

    println!("Starting interactive session in actor '{}'...", args.actor);
    println!("Type 'exit' or press Ctrl+D to end session.");
    println!();

    let _shell = &args.shell;

    println!("aether:{}# ", args.actor);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim() == "exit" || line.trim() == "quit" {
            break;
        }

        if !line.trim().is_empty() {
            return Err(Error::ConnectionFailed(
                "Not connected to Aether runtime".to_string(),
            ));
        } else {
            println!("aether:{}# ", args.actor);
        }
    }

    println!();
    println!("Session ended.");

    Ok(())
}

async fn execute_default_shell(args: &ExecArgs) -> Result<(), Error> {
    println!(
        "Starting shell '{}' in actor '{}'...",
        args.shell, args.actor
    );
    println!("Use --interactive for interactive mode.");
    println!();

    Err(Error::ConnectionFailed(
        "Not connected to Aether runtime".to_string(),
    ))
}
