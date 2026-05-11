//! Run a local Aether server.

use clap::Args;
use std::net::SocketAddr;
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
}

impl RunCommand {
    /// Execute the run command.
    pub async fn execute(&self) -> Result<(), Error> {
        let addr = format!("{}:{}", self.host, self.port);
        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|e| Error::InvalidAddress(format!("{}: {}", addr, e)))?;

        println!("aether run v{}", env!("CARGO_PKG_VERSION"));
        println!("binding to {socket_addr}");
        println!("server crate: available");
        println!("wasm engine: available (feature-gated)");
        println!();
        println!("NOTE: Full server startup will be available in v2.2.0");
        println!("      Use `cargo run --bin aether-server` for the standalone server.");

        Ok(())
    }
}
