#![deny(unsafe_code)]

use axum::{Json, Router};
use chrono::Utc;

use crate::models::{HealthResponse, InfoResponse};

/// Returns the router for this module.
pub fn routes() -> Router {
    Router::new()
        .route("/health", axum::routing::get(get_health))
        .route("/health/ready", axum::routing::get(get_ready))
        .route("/api/v1/info", axum::routing::get(get_info))
}

async fn get_health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_owned(),
        timestamp: Utc::now(),
    })
}

async fn get_ready() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ready".to_owned(),
        timestamp: Utc::now(),
    })
}

async fn get_info() -> Json<InfoResponse> {
    Json(InfoResponse {
        name: "aether-server".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}
