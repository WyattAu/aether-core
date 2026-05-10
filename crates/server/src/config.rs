#![deny(unsafe_code)]

use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
/// Server configuration loaded from environment variables.
pub struct ServerConfig {
    #[serde(default = "default_http_port")]
    /// HTTP listen port.
    pub http_port: u16,

    #[serde(default = "default_grpc_port")]
    /// gRPC listen port.
    pub grpc_port: u16,

    #[serde(default)]
    /// Whether cluster mode is enabled.
    pub cluster_enabled: bool,

    /// JWT secret for authentication.
    pub jwt_secret: Option<String>,

    /// PostgreSQL connection URL.
    pub postgres_url: Option<String>,

    /// Redis connection URL.
    pub redis_url: Option<String>,

    #[serde(default = "default_log_level")]
    /// Log level filter string.
    pub log_level: String,
}

fn default_http_port() -> u16 {
    8080
}

fn default_grpc_port() -> u16 {
    50051
}

fn default_log_level() -> String {
    "info".to_owned()
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            http_port: default_http_port(),
            grpc_port: default_grpc_port(),
            cluster_enabled: false,
            jwt_secret: None,
            postgres_url: None,
            redis_url: None,
            log_level: default_log_level(),
        }
    }
}

impl ServerConfig {
    /// Creates a new `ServerConfig` from environment variables.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(port) = env::var("AETHER_HTTP_PORT") {
            if let Ok(p) = port.parse() {
                config.http_port = p;
            }
        }

        if let Ok(port) = env::var("AETHER_GRPC_PORT") {
            if let Ok(p) = port.parse() {
                config.grpc_port = p;
            }
        }

        if let Ok(val) = env::var("AETHER_CLUSTER_ENABLED") {
            config.cluster_enabled = val == "true" || val == "1";
        }

        config.jwt_secret = env::var("AETHER_JWT_SECRET").ok();
        config.postgres_url = env::var("AETHER_POSTGRES_URL").ok();
        config.redis_url = env::var("AETHER_REDIS_URL").ok();

        if let Ok(level) = env::var("AETHER_LOG_LEVEL") {
            config.log_level = level;
        }

        config
    }
}
