//! Status Command
//!
//! Display status of actors and the system from the Aether HTTP API.

use clap::Args;
use std::time::Duration;
use thiserror::Error;

use super::DEFAULT_DASHBOARD_ADDR;

/// Status command arguments
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Actor name to check (optional)
    #[arg(short, long)]
    pub actor: Option<String>,

    /// Output format
    #[arg(short, long, default_value = "table")]
    pub format: String,

    /// Watch mode (continuously update)
    #[arg(short, long)]
    pub watch: bool,

    /// Dashboard API address
    #[arg(long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,
}

/// Status command errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to get status: {0}")]
    GetStatus(String),

    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
}

struct ApiClient {
    client: reqwest::Client,
    base_url: String,
}

impl ApiClient {
    fn new(api_addr: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: api_addr.trim_end_matches('/').to_string(),
        }
    }

    async fn fetch_actors(&self) -> Result<Vec<serde_json::Value>, Error> {
        let resp = self
            .client
            .get(format!("{}/api/v1/actors", self.base_url))
            .send()
            .await
            .map_err(|e| Error::GetStatus(format!("Failed to fetch actors: {e}")))?;

        if !resp.status().is_success() {
            return Err(Error::GetStatus(format!(
                "Server returned {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| Error::GetStatus(format!("Failed to parse actors response: {e}")))
    }

    async fn fetch_cluster_status(&self) -> Result<serde_json::Value, Error> {
        let resp = self
            .client
            .get(format!("{}/api/v1/cluster/status", self.base_url))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => r
                .json()
                .await
                .map_err(|e| Error::GetStatus(format!("Failed to parse cluster status: {e}"))),
            Ok(r) => Err(Error::GetStatus(format!(
                "Cluster status returned {}",
                r.status()
            ))),
            Err(e) => Err(Error::GetStatus(format!(
                "Failed to reach cluster endpoint: {e}"
            ))),
        }
    }
}

/// Execute the status command
pub async fn execute(args: StatusArgs) -> Result<(), Error> {
    let client = ApiClient::new(&args.api_addr);

    if args.watch {
        loop {
            print_status(&client, &args).await?;
            println!("\nWatching for changes... (Press Ctrl+C to stop)\n");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    } else {
        print_status(&client, &args).await?;
    }

    Ok(())
}

async fn print_status(client: &ApiClient, args: &StatusArgs) -> Result<(), Error> {
    match args.format.as_str() {
        "json" => print_json_status(client, args).await,
        _ => print_table_status(client, args).await,
    }
}

async fn print_json_status(client: &ApiClient, args: &StatusArgs) -> Result<(), Error> {
    if let Some(actor_name) = &args.actor {
        let actors = client.fetch_actors().await?;
        let actor = actors
            .iter()
            .find(|a| {
                a.get("name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|n| n == actor_name)
            })
            .ok_or_else(|| Error::GetStatus(format!("Actor '{actor_name}' not found")))?;

        println!(
            "{}",
            serde_json::to_string_pretty(actor).unwrap_or_default()
        );
    } else {
        let actors = client.fetch_actors().await?;
        let cluster = client.fetch_cluster_status().await;

        let mut status = serde_json::json!({
            "runtime": {
                "version": env!("CARGO_PKG_VERSION"),
            },
            "actors": {
                "total": actors.len(),
                "running": actors.iter().filter(|a| {
                    a.get("state").and_then(|v| v.as_str()) == Some("running")
                }).count(),
            },
        });

        if let Ok(cluster_val) = cluster {
            status["cluster"] = cluster_val;
        }

        println!(
            "{}",
            serde_json::to_string_pretty(&status).unwrap_or_default()
        );
    }

    Ok(())
}

async fn print_table_status(client: &ApiClient, args: &StatusArgs) -> Result<(), Error> {
    println!("+=============================================================+");
    println!("|                    AETHER RUNTIME STATUS                      |");
    println!("+=============================================================+");
    println!();

    if let Some(actor_name) = &args.actor {
        let actors = client.fetch_actors().await?;
        let actor = actors
            .iter()
            .find(|a| {
                a.get("name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|n| n == actor_name)
            })
            .ok_or_else(|| Error::GetStatus(format!("Actor '{actor_name}' not found")))?;

        println!("Actor: {}", actor_name);
        println!(
            "|-- Status:     {}",
            actor
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );
        println!(
            "|-- ID:         {}",
            actor
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );
        println!(
            "|-- Messages:   {}",
            actor.get("messages").and_then(|v| v.as_u64()).unwrap_or(0)
        );
        println!(
            "|-- Cold starts: {}",
            actor
                .get("cold_starts")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        );
        println!(
            "|-- Errors:     {}",
            actor.get("errors").and_then(|v| v.as_u64()).unwrap_or(0)
        );
    } else {
        let actors = client.fetch_actors().await?;
        let running = actors
            .iter()
            .filter(|a| {
                a.get("state")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s == "running")
            })
            .count();
        let total = actors.len();

        println!("Runtime Overview");
        println!("|-- Version:    {}", env!("CARGO_PKG_VERSION"));
        println!("|-- Server:     {}", client.base_url);
        println!("|-- Status:     healthy");
        println!();
        println!("Actors");
        println!("|-- Total:      {}", total);
        println!("|-- Running:    {}", running);
        println!("|-- Stopped:    {}", total - running);
        println!();

        if !actors.is_empty() {
            println!(
                "{:<30} {:<12} {:<10} {:<10}",
                "NAME", "STATE", "MESSAGES", "ERRORS"
            );
            println!("{}", "-".repeat(62));
            for actor in &actors {
                let name = actor
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let state = actor
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let msgs = actor.get("messages").and_then(|v| v.as_u64()).unwrap_or(0);
                let errors = actor.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
                println!("{:<30} {:<12} {:<10} {:<10}", name, state, msgs, errors);
            }
            println!();
        }

        match client.fetch_cluster_status().await {
            Ok(cluster) => {
                println!("Cluster");
                println!(
                    "|-- Status:     {}",
                    cluster
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
                println!(
                    "|-- Nodes:      {}",
                    cluster.get("nodes").and_then(|v| v.as_u64()).unwrap_or(1)
                );
            }
            Err(_) => {
                println!("Cluster");
                println!("|-- Status:     endpoint not available");
            }
        }
    }

    println!();
    println!("+=============================================================+");

    Ok(())
}
