//! Dashboard Command
//!
//! Launch the web dashboard for monitoring and managing Aether.

use aether_core::dashboard::{DashboardConfig, DashboardServer};
use aether_core::observability::Observability;
use clap::Args;
use std::net::SocketAddr;
use std::sync::Arc;
use thiserror::Error;

/// Dashboard command arguments
#[derive(Args, Debug)]
pub struct DashboardArgs {
    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Open browser automatically
    #[arg(short, long)]
    pub open: bool,
}

/// Dashboard command errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to bind to address: {0}")]
    BindFailed(String),

    #[error("Failed to start server: {0}")]
    ServerFailed(String),

    #[error("Failed to open browser: {0}")]
    BrowserFailed(String),
}

pub async fn execute(args: DashboardArgs) -> Result<(), Error> {
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .map_err(|e: std::net::AddrParseError| Error::BindFailed(e.to_string()))?;

    let url = format!("http://{}", addr);

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    AETHER DASHBOARD                           ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!();
    println!("  Dashboard URL: {}", url);
    println!();
    println!("  API Endpoints:");
    println!("    GET /api/v1/status     - Runtime status");
    println!("    GET /api/v1/actors     - List all actors");
    println!("    GET /api/v1/actors/:id - Get actor details");
    println!("    GET /api/v1/metrics     - Prometheus metrics");
    println!("    GET /api/v1/health      - Health check results");
    println!("    GET /api/v1/mesh        - Mesh topology");
    println!("    GET /api/v1/traces      - Recent traces");
    println!("    GET /api/v1/openapi.json - OpenAPI specification");
    println!("    GET /ws                - WebSocket for real-time updates");
    println!("    GET /healthz           - Kubernetes health probe");
    println!("    GET /readyz            - Kubernetes readiness probe");
    println!();
    println!("  Press Ctrl+C to stop the server");
    println!();
    println!("╚═══════════════════════════════════════════════════════════════╝");

    if args.open {
        if let Err(e) = open_browser(&url) {
            eprintln!("Warning: Could not open browser: {}", e);
        }
    }

    // Create observability layer
    let observability = Arc::new(Observability::new());

    // Create and start the dashboard server
    let config = DashboardConfig::new(addr);
    let server = DashboardServer::new(config, observability);

    server
        .serve()
        .await
        .map_err(|e| Error::ServerFailed(e.to_string()))
}

fn open_browser(url: &str) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| Error::BrowserFailed(e.to_string()))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| Error::BrowserFailed(e.to_string()))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .map_err(|e| Error::BrowserFailed(e.to_string()))?;
    }

    Ok(())
}
