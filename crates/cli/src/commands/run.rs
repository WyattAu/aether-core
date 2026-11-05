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

    /// Mesh node identity. Mesh networking is enabled only when both
    /// this and `--mesh-port` are set.
    #[arg(long)]
    node_id: Option<String>,

    /// QUIC listen port for the actor mesh.
    #[arg(long)]
    mesh_port: Option<u16>,

    /// Comma-separated `host:port` addresses of existing mesh nodes to
    /// dial at startup.
    #[arg(long, value_delimiter = ',')]
    mesh_seeds: Vec<String>,
}

impl RunCommand {
    /// The argument vector handed to the spawned aether-server. Kept as a
    /// method so the flag mapping is unit-testable.
    fn server_args(&self) -> Vec<String> {
        let mut args = vec![
            "--host".to_string(),
            self.host.clone(),
            "--port".to_string(),
            self.port.to_string(),
        ];
        if let Some(id) = &self.node_id {
            args.push("--node-id".to_string());
            args.push(id.clone());
        }
        if let Some(port) = self.mesh_port {
            args.push("--mesh-port".to_string());
            args.push(port.to_string());
        }
        if !self.mesh_seeds.is_empty() {
            args.push("--mesh-seeds".to_string());
            args.push(self.mesh_seeds.join(","));
        }
        args
    }
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
                    c.args(["run", "--bin", "aether-server", "--"]);
                    c.args(self.server_args());
                    c
                } else {
                    let mut c = tokio::process::Command::new(&binary);
                    c.args(self.server_args());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_args_map_http_flags() {
        let cmd = RunCommand {
            port: 9000,
            host: "0.0.0.0".to_string(),
            node_id: None,
            mesh_port: None,
            mesh_seeds: Vec::new(),
        };
        assert_eq!(cmd.server_args(), ["--host", "0.0.0.0", "--port", "9000"]);
    }

    #[test]
    fn server_args_map_mesh_flags() {
        // The 3-node compose demo's contract: identity, mesh port, and
        // seeds flow through to the server verbatim.
        let cmd = RunCommand {
            port: 8080,
            host: "127.0.0.1".to_string(),
            node_id: Some("node-1".to_string()),
            mesh_port: Some(9101),
            mesh_seeds: vec!["10.0.0.2:9101".to_string(), "10.0.0.3:9101".to_string()],
        };
        assert_eq!(
            cmd.server_args(),
            [
                "--host",
                "127.0.0.1",
                "--port",
                "8080",
                "--node-id",
                "node-1",
                "--mesh-port",
                "9101",
                "--mesh-seeds",
                "10.0.0.2:9101,10.0.0.3:9101"
            ]
        );
    }
}
