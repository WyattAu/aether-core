//! Scale Command
//!
//! View and manage actor scaling via the Aether HTTP API.
//! Scaling in the actor model is managed through WASM fuel quotas.

use clap::Args;
use std::time::Duration;
use thiserror::Error;

use super::DEFAULT_DASHBOARD_ADDR;

/// Scale command arguments
#[derive(Args, Debug)]
pub struct ScaleArgs {
    /// Actor name to scale
    #[arg(short, long)]
    pub actor: String,

    /// Number of instances
    #[arg(short, long, default_value = "1")]
    pub replicas: u32,

    /// Minimum instances
    #[arg(short, long)]
    pub min: Option<u32>,

    /// Maximum instances
    #[arg(short, long)]
    pub max: Option<u32>,

    /// Dashboard API address
    #[arg(long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,
}

/// Scale command errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to scale actor: {0}")]
    #[allow(dead_code)]
    ScaleFailed(String),

    #[error("Actor not found: {0}")]
    ActorNotFound(String),

    #[error("Invalid replica count: {0}")]
    InvalidReplicaCount(String),

    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
}

/// Execute the scale command
pub async fn execute(args: ScaleArgs) -> Result<(), Error> {
    if args.replicas == 0 {
        return Err(Error::InvalidReplicaCount(
            "Replicas must be greater than 0".to_string(),
        ));
    }

    if let Some(min) = args.min {
        if args.replicas < min {
            return Err(Error::InvalidReplicaCount(format!(
                "Replicas ({}) is less than minimum ({})",
                args.replicas, min
            )));
        }
    }

    if let Some(max) = args.max {
        if args.replicas > max {
            return Err(Error::InvalidReplicaCount(format!(
                "Replicas ({}) exceeds maximum ({})",
                args.replicas, max
            )));
        }
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base_url = args.api_addr.trim_end_matches('/').to_string();

    let resp = client
        .get(format!("{}/api/v1/actors/{}", base_url, args.actor))
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::ActorNotFound(args.actor.clone()));
    }

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(Error::ScaleFailed(format!("Server returned {status}")));
    }

    let actor_info: serde_json::Value = resp.json().await?;

    println!("Actor: {}", args.actor);
    println!("-------");
    println!(
        "  State:    {}",
        actor_info
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    );
    println!(
        "  ID:       {}",
        actor_info
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
    );
    println!(
        "  Messages: {}",
        actor_info
            .get("messages")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    );
    println!();

    println!("Desired replicas: {}", args.replicas);
    println!();
    println!(
        "NOTE: Scaling in the Aether actor model is managed via WASM fuel quotas, \
         not traditional replica counts. The actor system auto-scales based on message \
         throughput and fuel allocation."
    );
    println!();
    println!(
        "To adjust resource allocation, modify the actor's fuel quota in aether.toml \
             or use the actor system's runtime configuration API."
    );

    Ok(())
}
