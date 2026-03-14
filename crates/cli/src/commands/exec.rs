//! Exec Command
//!
//! Execute commands inside a running actor.

use clap::Args;
use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};
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
    ActorNotFound(String),

    #[error("Actor not running: {0}")]
    ActorNotRunning(String),

    #[error("Failed to execute command: {0}")]
    ExecutionFailed(String),

    #[error("TTY not available: {0}")]
    TtyNotAvailable(String),

    #[error("Connection failed: {0}")]
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
    if args.tty && !termion::is_tty(&io::stdout()) {
        return Err(Error::TtyNotAvailable(
            "Standard output is not a TTY. Remove --tty flag or run in a terminal.".into(),
        ));
    }

    println!("Starting interactive session in actor '{}'...", args.actor);
    println!("Type 'exit' or press Ctrl+D to end session.");
    println!();

    let shell = if args.command.is_empty() {
        &args.shell
    } else {
        &args.shell
    };

    println!("aether:{}# ", args.actor);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim() == "exit" || line.trim() == "quit" {
            break;
        }

        if !line.trim().is_empty() {
            println!("aether:{}# {}", args.actor, line);
            println!("[simulated output for: {}]", line);
            println!("aether:{}# ", args.actor);
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

    println!("Shell: {}", args.shell);
    println!("Actor: {}", args.actor);
    println!("PID:   12345 (simulated)");
    println!();
    println!("To attach interactively, run:");
    println!("  aether exec -i -a {}", args.actor);

    Ok(())
}
