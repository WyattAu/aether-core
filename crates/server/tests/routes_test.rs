#![allow(missing_docs)]
#![deny(unsafe_code)]

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use aether_server::routes::{actors, cluster, events, health, state as state_routes};
use aether_server::state::AppState;

fn make_app(shared: Arc<AppState>) -> Router {
    Router::new()
        .merge(health::routes())
        .merge(actors::routes())
        .merge(cluster::routes())
        .merge(state_routes::routes())
        .merge(events::routes())
        .with_state(shared)
}

async fn dispatch(shared: Arc<AppState>, req: Request<Body>) -> (StatusCode, Value) {
    let resp = make_app(shared)
        .oneshot(req)
        .await
        .expect("test precondition: request dispatch failed");
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .expect("test precondition: body collect failed")
        .to_bytes();
    let body: Value = if bytes.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(v) => v,
            Err(_) => Value::String(format!("<non-json: {} bytes>", bytes.len())),
        }
    };
    (status, body)
}

fn fresh() -> Arc<AppState> {
    Arc::new(AppState::new())
}

fn json_req(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&body).expect("test precondition: serialize"),
        ))
        .expect("test precondition: request build")
}

fn empty_req(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("test precondition: request build")
}

// ---------------------------------------------------------------------------
// Health routes (stateless — each call gets its own state)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_returns_ok() {
    let (status, body) = dispatch(fresh(), empty_req("GET", "/health")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["timestamp"].is_string());
}

#[tokio::test]
async fn health_ready_returns_ready() {
    let (status, body) = dispatch(fresh(), empty_req("GET", "/health/ready")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn info_returns_server_identity() {
    let (status, body) = dispatch(fresh(), empty_req("GET", "/api/v1/info")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "aether-server");
    assert!(body["version"].is_string());
}

// ---------------------------------------------------------------------------
// Actor routes — register
// ---------------------------------------------------------------------------

#[tokio::test]
async fn register_actor_success() {
    let s = fresh();
    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "name": "test-actor",
                "actor_type": "echo",
                "version": "1.0.0"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["actor_id"].is_string());
    assert_eq!(body["status"], "created");
}

#[tokio::test]
async fn register_actor_with_preassigned_id() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id,
                "name": "preassigned",
                "actor_type": "worker"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["actor_id"], id);
}

#[tokio::test]
async fn register_actor_with_metadata() {
    let s = fresh();
    let (status, _body) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "name": "meta-actor",
                "actor_type": "svc",
                "metadata": {"env": "test", "replicas": 3}
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn register_duplicate_actor_fails() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();

    let (status, _) = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "dup",
                "actor_type": "svc"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status2, body2) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id,
                "name": "dup",
                "actor_type": "svc"
            }),
        ),
    )
    .await;
    assert_eq!(status2, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body2["error"], "internal_error");
}

#[tokio::test]
async fn register_actor_empty_body_returns_error() {
    let s = fresh();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/actors")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .expect("test precondition");
    let (status, _body) = dispatch(s, req).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ---------------------------------------------------------------------------
// Actor routes — list / get / deregister
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_actors_empty() {
    let s = fresh();
    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/actors")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.is_empty()));
}

#[tokio::test]
async fn list_actors_after_register() {
    let s = fresh();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "name": "a", "actor_type": "t"
            }),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "name": "b", "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/actors")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.len() >= 2));
}

#[tokio::test]
async fn get_actor_by_id() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "findable",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", &format!("/api/v1/actors/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["actor_id"], id);
    assert_eq!(body["name"], "findable");
}

#[tokio::test]
async fn get_actor_not_found() {
    let s = fresh();
    let (status, body) = dispatch(
        s,
        empty_req("GET", "/api/v1/actors/00000000-0000-0000-0000-000000000000"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn deregister_actor_success() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "temporary",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s.clone(),
        empty_req("DELETE", &format!("/api/v1/actors/{id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "deregistered");

    let (status2, _) = dispatch(s, empty_req("GET", &format!("/api/v1/actors/{id}"))).await;
    assert_eq!(status2, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn deregister_actor_not_found() {
    let s = fresh();
    let (status, _) = dispatch(
        s,
        empty_req(
            "DELETE",
            "/api/v1/actors/00000000-0000-0000-0000-000000000000",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Actor routes — messages / heartbeat
// ---------------------------------------------------------------------------

#[tokio::test]
async fn send_message_to_existing_actor() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "mbox",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            &format!("/api/v1/actors/{id}/messages"),
            json!({
                "payload": {"msg": "hello"},
                "content_type": "application/json"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "delivered");
    assert!(body["message_id"].is_string());
}

#[tokio::test]
async fn send_message_to_nonexistent_actor() {
    let s = fresh();
    let (status, _) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/actors/00000000-0000-0000-0000-000000000000/messages",
            json!({"payload": "hi"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_inbox_empty() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "empty-inbox",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s,
        empty_req("GET", &format!("/api/v1/actors/{id}/messages")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.is_empty()));
}

#[tokio::test]
async fn get_inbox_after_message() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "inbox-test",
                "actor_type": "t"
            }),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            &format!("/api/v1/actors/{id}/messages"),
            json!({"payload": "first"}),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            &format!("/api/v1/actors/{id}/messages"),
            json!({"payload": "second"}),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s,
        empty_req("GET", &format!("/api/v1/actors/{id}/messages")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.len() >= 2));
}

#[tokio::test]
async fn heartbeat_existing_actor() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "hb-test",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s,
        empty_req("POST", &format!("/api/v1/actors/{id}/heartbeat")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "running");
    assert!(body["last_heartbeat"].is_string());
}

#[tokio::test]
async fn heartbeat_nonexistent_actor() {
    let s = fresh();
    let (status, _) = dispatch(
        s,
        empty_req(
            "POST",
            "/api/v1/actors/00000000-0000-0000-0000-000000000000/heartbeat",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Cluster routes — nodes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_nodes_empty() {
    let s = fresh();
    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/cluster/nodes")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.is_empty()));
}

#[tokio::test]
async fn join_cluster_success() {
    let s = fresh();
    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/cluster/join",
            json!({
                "node_id": "node-1",
                "address": "127.0.0.1:8080"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["node_id"], "node-1");
    assert_eq!(body["status"], "joined");
}

#[tokio::test]
async fn join_cluster_auto_generates_id() {
    let s = fresh();
    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/cluster/join",
            json!({
                "address": "10.0.0.1:9090"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["node_id"].is_string());
    assert_ne!(body["node_id"], "");
}

#[tokio::test]
async fn get_node_success() {
    let s = fresh();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/cluster/join",
            json!({
                "node_id": "node-get",
                "address": "1.2.3.4:5"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/cluster/nodes/node-get")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["node_id"], "node-get");
    assert_eq!(body["address"], "1.2.3.4:5");
}

#[tokio::test]
async fn get_node_not_found() {
    let s = fresh();
    let (status, _) = dispatch(s, empty_req("GET", "/api/v1/cluster/nodes/nonexistent")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn remove_node_success() {
    let s = fresh();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/cluster/join",
            json!({
                "node_id": "node-rm",
                "address": "1.1.1.1:1"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s.clone(),
        empty_req("DELETE", "/api/v1/cluster/nodes/node-rm"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "removed");

    let (status2, _) = dispatch(s, empty_req("GET", "/api/v1/cluster/nodes/node-rm")).await;
    assert_eq!(status2, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn remove_node_not_found() {
    let s = fresh();
    let (status, _) = dispatch(s, empty_req("DELETE", "/api/v1/cluster/nodes/nope")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn leave_cluster_success() {
    let s = fresh();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/cluster/join",
            json!({
                "node_id": "node-leave",
                "address": "a"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/cluster/leave",
            json!({
                "node_id": "node-leave"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "left");
}

#[tokio::test]
async fn leave_cluster_missing_node_id() {
    let s = fresh();
    let (status, body) = dispatch(s, json_req("POST", "/api/v1/cluster/leave", json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");
}

#[tokio::test]
async fn leave_cluster_node_not_found() {
    let s = fresh();
    let (status, _) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/cluster/leave",
            json!({
                "node_id": "ghost"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cluster_status() {
    let s = fresh();
    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/cluster/status")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["node_count"].is_number());
    assert!(body["actor_count"].is_number());
    assert!(body["uptime_seconds"].is_number());
}

#[tokio::test]
async fn cluster_status_after_join() {
    let s = fresh();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/cluster/join",
            json!({
                "node_id": "n1",
                "address": "a"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/cluster/status")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["node_count"], 1);
}

// ---------------------------------------------------------------------------
// State routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_state_success() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "state-actor",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s,
        json_req("PUT", &format!("/api/v1/state/{id}/counter"), json!(42)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["actor_id"], id);
    assert_eq!(body["key"], "counter");
    assert!(body["version"].is_number());
}

#[tokio::test]
async fn get_state_success() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "state-get",
                "actor_type": "t"
            }),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req("PUT", &format!("/api/v1/state/{id}/color"), json!("blue")),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", &format!("/api/v1/state/{id}/color"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["value"], "blue");
    assert!(body["version"].is_number());
}

#[tokio::test]
async fn get_state_key_not_found() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "state-missing-key",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, _) = dispatch(
        s,
        empty_req("GET", &format!("/api/v1/state/{id}/nonexistent")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_state_actor_not_found() {
    let s = fresh();
    let (status, _) = dispatch(
        s,
        empty_req(
            "GET",
            "/api/v1/state/00000000-0000-0000-0000-000000000000/k",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_state_success() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "state-del",
                "actor_type": "t"
            }),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req("PUT", &format!("/api/v1/state/{id}/temp"), json!(true)),
    )
    .await;

    let (status, body) =
        dispatch(s, empty_req("DELETE", &format!("/api/v1/state/{id}/temp"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "deleted");
}

#[tokio::test]
async fn put_state_actor_not_found() {
    let s = fresh();
    let (status, _) = dispatch(
        s,
        json_req(
            "PUT",
            "/api/v1/state/00000000-0000-0000-0000-000000000000/k",
            json!("v"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_all_state_success() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "state-all",
                "actor_type": "t"
            }),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req("PUT", &format!("/api/v1/state/{id}/x"), json!(1)),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req("PUT", &format!("/api/v1/state/{id}/y"), json!(2)),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", &format!("/api/v1/state/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_object());
    assert_eq!(body["x"], 1);
    assert_eq!(body["y"], 2);
}

#[tokio::test]
async fn get_all_state_actor_not_found() {
    let s = fresh();
    let (status, _) = dispatch(
        s,
        empty_req("GET", "/api/v1/state/00000000-0000-0000-0000-000000000000"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Event routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn publish_event_success() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "evt-actor",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            &format!("/api/v1/events/{id}"),
            json!({
                "event_type": "user.created",
                "payload": {"user": "alice"}
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["actor_id"], id);
    assert!(body["event_id"].is_string());
    assert!(body["sequence"].is_number());
    assert!(body["timestamp"].is_string());
}

#[tokio::test]
async fn publish_event_actor_not_found() {
    let s = fresh();
    let (status, _) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/events/00000000-0000-0000-0000-000000000000",
            json!({
                "event_type": "fail"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn publish_event_default_type() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "evt-default",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            &format!("/api/v1/events/{id}"),
            json!({
                "payload": "data"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["event_id"].is_string());
    assert!(body["sequence"].is_number());
}

#[tokio::test]
async fn get_events_empty() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let (status, body) = dispatch(s, empty_req("GET", &format!("/api/v1/events/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.is_empty()));
}

#[tokio::test]
async fn get_events_after_publish() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "evt-list",
                "actor_type": "t"
            }),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            &format!("/api/v1/events/{id}"),
            json!({
                "event_type": "e1",
                "payload": "a"
            }),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            &format!("/api/v1/events/{id}"),
            json!({
                "event_type": "e2",
                "payload": "b"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", &format!("/api/v1/events/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.len() >= 2));
}

// ---------------------------------------------------------------------------
// Pub/sub routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_topic_success() {
    let s = fresh();
    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/pubsub/orders/subscribe",
            json!({
                "subscriber_id": "sub-1"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["topic"], "orders");
    assert_eq!(body["status"], "subscribed");
    assert_eq!(body["subscriber_id"], "sub-1");
}

#[tokio::test]
async fn subscribe_topic_auto_id() {
    let s = fresh();
    let (status, body) = dispatch(
        s,
        json_req("POST", "/api/v1/pubsub/alerts/subscribe", json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["subscriber_id"].is_string());
}

#[tokio::test]
async fn publish_topic_message_success() {
    let s = fresh();
    let (status, body) = dispatch(
        s,
        json_req(
            "POST",
            "/api/v1/pubsub/updates/publish",
            json!({
                "topic": "updates",
                "payload": {"msg": "new update"},
                "publisher_id": null,
                "timestamp": "2026-01-01T00:00:00Z"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "published");
    assert!(body["message_id"].is_string());
}

#[tokio::test]
async fn get_topic_messages_empty() {
    let s = fresh();
    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/pubsub/empty")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.is_empty()));
}

#[tokio::test]
async fn get_topic_messages_after_publish() {
    let s = fresh();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/pubsub/news/publish",
            json!({
                "topic": "news",
                "payload": "breaking",
                "publisher_id": null,
                "timestamp": "2026-01-01T00:00:00Z"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/pubsub/news")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| a.len() >= 1));
}

#[tokio::test]
async fn pubsub_publish_and_retrieve_multiple() {
    let s = fresh();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/pubsub/chat/publish",
            json!({
                "topic": "chat",
                "payload": "hello",
                "publisher_id": null,
                "timestamp": "2026-01-01T00:00:00Z"
            }),
        ),
    )
    .await;
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/pubsub/chat/publish",
            json!({
                "topic": "chat",
                "payload": "world",
                "publisher_id": null,
                "timestamp": "2026-01-01T00:00:00Z"
            }),
        ),
    )
    .await;

    let (status, body) = dispatch(s, empty_req("GET", "/api/v1/pubsub/chat")).await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().expect("test precondition: array");
    assert!(arr.len() >= 2);
}

// ---------------------------------------------------------------------------
// CRUD flow — register -> get -> send message -> get events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_crud_flow() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();

    let (status, body) = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "flow-actor",
                "actor_type": "pipeline",
                "version": "2.0.0"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["actor_id"], id);

    let (status, body) =
        dispatch(s.clone(), empty_req("GET", &format!("/api/v1/actors/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "flow-actor");

    let (status, body) = dispatch(
        s.clone(),
        json_req(
            "POST",
            &format!("/api/v1/actors/{id}/messages"),
            json!({
                "payload": {"action": "process"},
                "content_type": "application/json"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "delivered");

    let (status, body) = dispatch(
        s.clone(),
        empty_req("GET", &format!("/api/v1/actors/{id}/messages")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.as_array().map_or(false, |a| !a.is_empty()));

    let (status, _body) = dispatch(
        s.clone(),
        json_req(
            "POST",
            &format!("/api/v1/events/{id}"),
            json!({
                "event_type": "processed",
                "payload": {"result": "ok"}
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) =
        dispatch(s.clone(), empty_req("GET", &format!("/api/v1/events/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    let events = body.as_array().expect("test precondition: events array");
    assert!(events.iter().any(|e| e["event_type"] == "processed"));

    let (status, _) = dispatch(
        s.clone(),
        empty_req("DELETE", &format!("/api/v1/actors/{id}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = dispatch(s, empty_req("GET", &format!("/api/v1/actors/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// 404 for unknown routes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unknown_route_returns_404() {
    let s = fresh();
    let (status, _) = dispatch(s, empty_req("GET", "/api/v1/nonexistent")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// State version increments
// ---------------------------------------------------------------------------

#[tokio::test]
async fn state_version_increments_on_overwrite() {
    let s = fresh();
    let id = uuid::Uuid::new_v4().to_string();
    let _ = dispatch(
        s.clone(),
        json_req(
            "POST",
            "/api/v1/actors",
            json!({
                "actor_id": id.clone(),
                "name": "version-actor",
                "actor_type": "t"
            }),
        ),
    )
    .await;

    let (_, b1) = dispatch(
        s.clone(),
        json_req("PUT", &format!("/api/v1/state/{id}/k"), json!(1)),
    )
    .await;
    let v1: u64 = b1["version"]
        .as_u64()
        .expect("test precondition: version is u64");

    let (_, b2) = dispatch(
        s,
        json_req("PUT", &format!("/api/v1/state/{id}/k"), json!(2)),
    )
    .await;
    let v2: u64 = b2["version"]
        .as_u64()
        .expect("test precondition: version is u64");

    assert!(v2 > v1);
}
