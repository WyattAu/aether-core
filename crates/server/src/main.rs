//! Aether Platform server binary.

#![deny(unsafe_code)]

use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use aether_server::AppState;
use aether_server::config::ServerConfig;
use aether_server::routes;

#[derive(Parser, Debug)]
#[command(name = "aether-server", about = "Aether Platform Server")]
struct Cli {
    #[arg(long, default_value_t = 8080)]
    port: u16,

    #[arg(long)]
    config_path: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let config = ServerConfig::from_env();

    let port = cli.port;
    let log_level = config.log_level.as_str();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)),
        )
        .with_target(false)
        .init();

    tracing::info!("aether-server v{} starting", env!("CARGO_PKG_VERSION"));
    tracing::info!("http port: {port}");

    let state = Arc::new(AppState::new());
    tracing::info!(
        "wasm execution: {}",
        if state.wasm_engine.is_available() {
            "available"
        } else {
            "disabled"
        }
    );

    let app = routes::actors::routes()
        .merge(routes::state::routes())
        .merge(routes::health::routes::<Arc<AppState>>())
        .merge(routes::events::routes())
        .merge(routes::cluster::routes())
        .with_state(state)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("listening on {addr}");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind to {addr}: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!("server error: {e}");
        std::process::exit(1);
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| {
                tracing::error!("failed to listen for ctrl+c: {e}");
            })
            .ok();
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                tracing::error!("failed to install SIGTERM handler: {e}");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received, stopping...");
}
