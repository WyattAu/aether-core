//! Run a local Aether server.
//!
//! Attempts to find and launch the aether-server binary. Falls back to
//! a helpful message if the binary is not found.

use clap::Args;
use std::net::SocketAddr;
use std::process::Stdio;
use thiserror::Error;

/// Start a local Aether server.
#[derive(Args, Debug)]
pub struct RunCommand {
    /// Port to listen on.
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// Bind address.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Failed to start server: {0}")]
    StartFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl RunCommand {
    /// Execute the run command.
    pub async fn execute(&self) -> Result<(), Error> {
        let addr = format!("{}:{}", self.host, self.port);
        let _socket_addr: SocketAddr = addr
            .parse()
            .map_err(|e| Error::InvalidAddress(format!("{}: {}", addr, e)))?;

        println!("aether run v{}", env!("CARGO_PKG_VERSION"));
        println!("binding to {addr}");
        println!();

        let candidates = ["aether-server".to_string(), "cargo".to_string()];

        let mut found_binary = None;

        for candidate in &candidates {
            let result = tokio::process::Command::new(candidate)
                .arg("--help")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;

            if let Ok(status) = result {
                if candidate == "cargo" {
                    let cargo_result = tokio::process::Command::new("cargo")
                        .args(["run", "--bin", "aether-server", "--", "--help"])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .await;

                    if cargo_result.is_ok() {
                        found_binary = Some("cargo run --bin aether-server".to_string());
                        break;
                    }
                } else {
                    if status.success() {
                        found_binary = Some(candidate.clone());
                        break;
                    }
                }
            }
        }

        match found_binary {
            Some(binary) => {
                println!("Found server: {}", binary);
                println!();
                println!("Starting server...");

                let mut cmd = if binary == "cargo run --bin aether-server" {
                    let mut c = tokio::process::Command::new("cargo");
                    c.args([
                        "run",
                        "--bin",
                        "aether-server",
                        "--",
                        "--host",
                        &self.host,
                        "--port",
                        &self.port.to_string(),
                    ]);
                    c
                } else {
                    let mut c = tokio::process::Command::new(&binary);
                    c.args(["--host", &self.host, "--port", &self.port.to_string()]);
                    c
                };

                cmd.stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .stdin(Stdio::inherit());

                let status = cmd
                    .status()
                    .await
                    .map_err(|e| Error::StartFailed(format!("Failed to spawn server: {e}")))?;

                if !status.success() {
                    return Err(Error::StartFailed(format!(
                        "Server exited with code: {}",
                        status
                            .code()
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    )));
                }
            }
            None => {
                println!("Could not find aether-server binary.");
                println!();
                println!("To install and run the server:");
                println!("  1. Build from source:  cargo run --bin aether-server");
                println!("  2. Install:            cargo install --path crates/server");
                println!();
                println!("Make sure you are in the aether-core workspace root.");
            }
        }

        Ok(())
    }
}
