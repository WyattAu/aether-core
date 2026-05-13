//! Exec Command
//!
//! Send messages to running actors via the Aether HTTP API.
//! Interactive mode requires WebSocket transport (not yet available).

use clap::Args;
use std::io;
use std::time::Duration;
use thiserror::Error;

use super::DEFAULT_DASHBOARD_ADDR;

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

    #[arg(short, long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,

    #[arg(trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Actor not found: {0}")]
    #[allow(dead_code)]
    ActorNotFound(String),

    #[error("Actor not running: {0}")]
    #[allow(dead_code)]
    ActorNotRunning(String),

    #[error("Failed to execute command: {0}")]
    ExecutionFailed(String),

    #[error("TTY not available: {0}")]
    TtyNotAvailable(String),

    #[error("Connection failed: {0}")]
    #[allow(dead_code)]
    ConnectionFailed(String),

    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),

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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let base_url = args.api_addr.trim_end_matches('/').to_string();
    let cmd_str = args.command.join(" ");

    println!("Executing command in actor '{}'...", args.actor);
    println!();
    println!("$ {}", cmd_str);
    println!("----------------------------------------------------------------");

    let body = serde_json::json!({
        "payload": cmd_str,
        "source": "cli-exec",
    });

    let resp = client
        .post(format!(
            "{}/api/v1/actors/{}/messages",
            base_url, args.actor
        ))
        .json(&body)
        .send()
        .await;

    match resp {
        Ok(response) if response.status().is_success() => {
            let result: serde_json::Value = response.json().await.unwrap_or_default();
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "OK".to_string())
            );
            println!();
            println!("Command completed successfully");
        }
        Ok(response) => {
            let status = response.status();
            let body_text = response.text().await.ok();
            let detail = body_text.as_deref().unwrap_or("unknown error");
            return Err(Error::ExecutionFailed(format!("HTTP {status}: {detail}")));
        }
        Err(e) => {
            return Err(Error::ExecutionFailed(format!("Request failed: {e}")));
        }
    }

    Ok(())
}

async fn execute_interactive(args: &ExecArgs) -> Result<(), Error> {
    if args.tty && !crossterm::tty::IsTty::is_tty(&io::stdout()) {
        return Err(Error::TtyNotAvailable(
            "Standard output is not a TTY. Remove --tty flag or run in a terminal.".into(),
        ));
    }

    println!("Interactive exec requires WebSocket transport.");
    println!();
    println!("The Aether CLI does not yet support interactive exec sessions.");
    println!("Use non-interactive mode instead:");
    println!(
        "  aether exec --actor {} -- <command> [args...]",
        args.actor
    );
    println!();
    println!("WebSocket-based interactive sessions are planned for a future release.");

    Ok(())
}

async fn execute_default_shell(args: &ExecArgs) -> Result<(), Error> {
    println!(
        "Starting shell '{}' in actor '{}'...",
        args.shell, args.actor
    );
    println!("Use --interactive for interactive mode.");
    println!(
        "Use: aether exec --actor {} -- <command> [args...]",
        args.actor
    );
    println!();
    println!(
        "NOTE: Shell sessions inside actors require WebSocket transport. \
         Use non-interactive command mode with -- to send a single command."
    );

    Ok(())
}
