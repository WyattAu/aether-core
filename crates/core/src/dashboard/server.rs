//! HTTP Server Implementation
//!
//! Axum-based HTTP server with WebSocket and CORS support.

use crate::Result;
use crate::dashboard::handlers::{DashboardState, create_router};
use crate::dashboard::ws::WebSocketBroadcaster;
use crate::observability::Observability;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Configuration for the dashboard HTTP server.
#[derive(Debug, Clone)]
pub struct DashboardConfig {
    /// Address to bind the server to
    pub bind_addr: SocketAddr,
    /// Whether to enable CORS headers
    pub enable_cors: bool,
    /// Optional directory for static files
    pub static_dir: Option<String>,
    /// WebSocket heartbeat interval in seconds
    pub ws_heartbeat_interval_secs: u64,
    /// Metrics update interval in seconds
    pub metrics_update_interval_secs: u64,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            // Use infallible SocketAddr construction for default
            bind_addr: SocketAddr::from(([0, 0, 0, 0], 8080)),
            enable_cors: true,
            static_dir: None,
            ws_heartbeat_interval_secs: 30,
            metrics_update_interval_secs: 5,
        }
    }
}

impl DashboardConfig {
    /// Creates a new config with the specified bind address.
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            ..Default::default()
        }
    }

    /// Sets the static files directory.
    pub fn with_static_dir(mut self, dir: impl Into<String>) -> Self {
        self.static_dir = Some(dir.into());
        self
    }

    /// Enables or disables CORS.
    pub fn with_cors(mut self, enable: bool) -> Self {
        self.enable_cors = enable;
        self
    }
}

/// Dashboard HTTP server with WebSocket support.
pub struct DashboardServer {
    config: DashboardConfig,
    state: Arc<DashboardState>,
    shutdown_tx: broadcast::Sender<()>,
}

impl DashboardServer {
    /// Creates a new dashboard server with the given configuration.
    pub fn new(config: DashboardConfig, observability: Arc<Observability>) -> Self {
        let (shutdown_tx, _) = broadcast::channel(16);
        let broadcaster = WebSocketBroadcaster::new(shutdown_tx.clone());

        let state = Arc::new(DashboardState {
            observability,
            broadcaster,
            shutdown: shutdown_tx.clone(),
        });

        Self {
            config,
            state,
            shutdown_tx,
        }
    }

    /// Starts the HTTP server and blocks until shutdown.
    pub async fn serve(self) -> Result<()> {
        let mut cors_layer = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        if !self.config.enable_cors {
            cors_layer = CorsLayer::new();
        }

        let app = create_router(self.state.clone())
            .layer(TraceLayer::new_for_http())
            .layer(cors_layer);

        let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;
        let addr = listener.local_addr()?;

        tracing::info!("Dashboard server listening on {}", addr);
        tracing::info!("API endpoints available at http://{}/api/v1/", addr);
        tracing::info!("WebSocket available at ws://{}/ws", addr);
        tracing::info!("OpenAPI docs at http://{}/api/v1/openapi.json", addr);

        let shutdown_rx = self.shutdown_tx.subscribe();

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal(shutdown_rx))
            .await?;

        tracing::info!("Dashboard server shutdown complete");
        Ok(())
    }

    /// Initiates a graceful shutdown of the server.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

async fn shutdown_signal(mut rx: broadcast::Receiver<()>) {
    let ctrl_c = async {
        if signal::ctrl_c().await.is_err() {
            tracing::warn!(
                "Failed to install Ctrl+C handler, graceful shutdown via Ctrl+C unavailable"
            );
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => {
                tracing::warn!("Failed to install terminate signal handler");
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    let broadcast = async {
        let _ = rx.recv().await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
        _ = broadcast => {},
    }

    tracing::info!("Shutdown signal received");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = DashboardConfig::default();
        assert!(config.enable_cors);
        assert_eq!(config.bind_addr.port(), 8080);
    }

    #[test]
    fn test_config_builder() {
        let config = DashboardConfig::new(SocketAddr::from(([127, 0, 0, 1], 9090)))
            .with_static_dir("/var/www")
            .with_cors(false);

        assert_eq!(config.bind_addr.port(), 9090);
        assert!(!config.enable_cors);
        assert_eq!(config.static_dir, Some("/var/www".to_string()));
    }
}
