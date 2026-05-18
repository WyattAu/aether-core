//! Dashboard REST API v2
//!
//! Provides a dedicated set of endpoints under `/api/dashboard/` for
//! the web dashboard skeleton. These endpoints are designed to be
//! consumed by the single-page HTML dashboard.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET    | /api/dashboard/overview | Cluster status, actor counts, health |
//! | GET    | /api/dashboard/actors   | List all actors with status |
//! | GET    | /api/dashboard/metrics  | Prometheus-compatible metrics |
//! | GET    | /api/dashboard/topology | Mesh network graph as JSON |

use axum::{Router, extract::State, response::Json, routing::get};
use std::sync::Arc;

use crate::dashboard::handlers::DashboardState;
use crate::dashboard::{DashboardActor, DashboardOverview, DashboardTopology, MetricsResponse};

/// Build the dashboard API router.
pub fn dashboard_router() -> Router<Arc<DashboardState>> {
    Router::new()
        .route("/overview", get(overview))
        .route("/actors", get(actors))
        .route("/metrics", get(metrics))
        .route("/topology", get(topology))
}

async fn overview(State(state): State<Arc<DashboardState>>) -> Json<DashboardOverview> {
    let obs = &state.observability;
    let m = obs.metrics();
    let h = obs.health();

    if h.needs_check() {
        h.run_checks();
    }

    Json(DashboardOverview {
        status: h.overall_status().to_string(),
        version: crate::VERSION.to_string(),
        uptime_secs: obs.uptime_secs(),
        actors_running: m.actors_running(),
        messages_total: m.messages_total(),
        nodes_count: 0,
        connections_count: 0,
    })
}

async fn actors(State(state): State<Arc<DashboardState>>) -> Json<Vec<DashboardActor>> {
    let metrics = state.observability.metrics();
    let guard = metrics.actor_metrics();

    let list: Vec<DashboardActor> = guard
        .iter()
        .map(|(name, m)| DashboardActor {
            id: format!("{name}-{}", uuid::Uuid::new_v4()),
            name: name.clone(),
            state: "running".to_string(),
            messages: m.messages,
            errors: m.errors,
            cold_starts: m.cold_starts,
            last_cold_start_us: m.last_cold_start_us,
        })
        .collect();

    Json(list)
}

async fn metrics(State(state): State<Arc<DashboardState>>) -> Json<MetricsResponse> {
    let prometheus = state.observability.metrics().export_prometheus();
    Json(MetricsResponse { prometheus })
}

async fn topology() -> Json<DashboardTopology> {
    Json(DashboardTopology {
        local_node_id: format!("node-{}", uuid::Uuid::new_v4()),
        nodes: Vec::new(),
        connections: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::ws::WebSocketBroadcaster;
    use crate::observability::Observability;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tokio::sync::broadcast;
    use tower::ServiceExt;

    fn make_test_state() -> Arc<DashboardState> {
        let (shutdown_tx, _) = broadcast::channel(16);
        let broadcaster = WebSocketBroadcaster::new(shutdown_tx.clone());
        let observability = Arc::new(Observability::new());
        Arc::new(DashboardState {
            observability,
            broadcaster,
            shutdown: shutdown_tx,
        })
    }

    #[tokio::test]
    async fn overview_returns_200() {
        let state = make_test_state();
        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/overview")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn overview_contains_expected_fields() {
        let state = make_test_state();
        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/overview")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("body");

        let val: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert!(val["status"].is_string());
        let status = val["status"].as_str().expect("status");
        assert!(status == "healthy" || status == "degraded");
        assert!(val["version"].is_string());
        assert!(val["uptime_secs"].is_number());
        assert!(val["actors_running"].is_number());
        assert!(val["messages_total"].is_number());
    }

    #[tokio::test]
    async fn actors_returns_200() {
        let state = make_test_state();
        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/actors")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn actors_returns_array() {
        let state = make_test_state();
        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/actors")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("body");

        let val: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert!(val.is_array());
    }

    #[tokio::test]
    async fn metrics_returns_200() {
        let state = make_test_state();
        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_contains_prometheus_field() {
        let state = make_test_state();
        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("body");

        let val: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert!(val["prometheus"].is_string());
    }

    #[tokio::test]
    async fn topology_returns_200() {
        let state = make_test_state();
        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/topology")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn topology_contains_mesh_fields() {
        let state = make_test_state();
        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/topology")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("body");

        let val: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert!(val["local_node_id"].is_string());
        assert!(val["nodes"].is_array());
        assert!(val["connections"].is_array());
    }

    #[tokio::test]
    async fn overview_with_actor_metrics() {
        let state = make_test_state();
        state.observability.record_actor_start("test-actor", 42);
        state.observability.record_message_processed(100);

        let app = dashboard_router().with_state(state);

        let req = Request::builder()
            .uri("/overview")
            .body(Body::empty())
            .expect("build request");

        let resp = app.oneshot(req).await.expect("response");
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("body");

        let val: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(val["actors_running"], 1);
        assert_eq!(val["messages_total"], 1);
    }
}
