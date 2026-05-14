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
use aether_server::auth::AuthConfigBuilder;
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

    #[cfg(feature = "wasm")]
    let state = {
        use aether_core::actor::SchedulerConfig;
        let config = SchedulerConfig::default();
        let scheduler = Arc::new(aether_core::actor::ActorScheduler::new(config));
        if let Err(e) = scheduler.start() {
            tracing::error!("failed to start actor scheduler: {e}");
            std::process::exit(1);
        }
        Arc::new(AppState::with_core_backend(scheduler))
    };

    #[cfg(not(feature = "wasm"))]
    let state = Arc::new(AppState::new());

    tracing::info!(
        "wasm execution: {}",
        if state.wasm_engine.is_available() {
            "available"
        } else {
            "disabled"
        }
    );

    #[cfg(feature = "wasm")]
    tracing::info!("actor backend: aether-core ActorScheduler");

    #[cfg(not(feature = "wasm"))]
    tracing::info!("actor backend: in-memory");

    // Build authentication configuration from environment variables.
    let auth_config = build_auth_config(&config);

    if auth_config.api_keys.read().map_or(true, |s| s.is_empty()) {
        #[cfg(feature = "jwt")]
        if auth_config.jwt.is_none() {
            tracing::warn!(
                "no authentication configured (no API keys or JWT secret); set AETHER_API_KEYS or AETHER_JWT_SECRET"
            );
        }
        #[cfg(not(feature = "jwt"))]
        tracing::warn!("no authentication configured (no API keys); set AETHER_API_KEYS");
    } else {
        tracing::info!("authentication: API keys configured");
    }

    let require_auth = config.require_auth;
    let auth_config = Arc::new(auth_config);
    let app = routes::actors::routes()
        .merge(routes::state::routes())
        .merge(routes::health::routes::<Arc<AppState>>())
        .merge(routes::events::routes())
        .merge(routes::cluster::routes())
        .with_state(state)
        .route_layer(axum::middleware::from_fn(
            move |request: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| {
                let auth_config = auth_config.clone();
                async move {
                    if require_auth {
                        aether_server::auth::require_auth_inner(
                            (*auth_config).clone(),
                            request,
                            next,
                        )
                        .await
                    } else {
                        aether_server::auth::optional_auth_inner(
                            (*auth_config).clone(),
                            request,
                            next,
                        )
                        .await
                    }
                }
            },
        ))
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

/// Build authentication configuration from server config.
///
/// Parses `AETHER_API_KEYS` (comma-separated `key=principal` pairs) and
/// `AETHER_JWT_SECRET` environment variables into an [`AuthConfig`].
fn build_auth_config(config: &ServerConfig) -> aether_server::auth::AuthConfig {
    let mut builder = AuthConfigBuilder::new();

    // Parse API keys from environment.
    // Format: "secret-key=admin,read-only-key=viewer"
    if let Some(ref keys_str) = config.api_keys {
        for entry in keys_str.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if let Some((key, principal)) = entry.split_once('=') {
                builder = builder.api_key(key.trim(), principal.trim());
            } else {
                tracing::warn!(
                    "skipping malformed API key entry (expected key=principal): {}",
                    entry
                );
            }
        }
    }

    // Configure JWT if a secret is provided.
    #[cfg(feature = "jwt")]
    if let Some(ref secret) = config.jwt_secret {
        builder = builder.jwt_secret(secret.clone());
    }

    #[cfg(not(feature = "jwt"))]
    if config.jwt_secret.is_some() {
        tracing::warn!(
            "AETHER_JWT_SECRET is set but the 'jwt' feature is not enabled; JWT auth will not be available"
        );
    }

    builder.build()
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
