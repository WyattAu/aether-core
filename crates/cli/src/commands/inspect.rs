//! Inspect Command
//!
//! Inspect actor memory, state, and metadata via the Aether HTTP API.

use clap::Args;
use clap::Subcommand;
use std::time::Duration;
use thiserror::Error;

use super::DEFAULT_DASHBOARD_ADDR;

#[derive(Args, Debug)]
pub struct InspectArgs {
    #[command(subcommand)]
    pub command: InspectCommand,

    /// Dashboard API address
    #[arg(long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,
}

#[derive(Subcommand, Debug)]
pub enum InspectCommand {
    Memory(MemoryArgs),
    State(StateArgs),
    Stack(StackArgs),
    Metadata(MetadataArgs),
    All(AllArgs),
}

#[derive(Args, Debug)]
pub struct MemoryArgs {
    #[arg(short, long)]
    pub actor: String,

    #[arg(short, long)]
    pub format: Option<String>,

    #[arg(short, long, default_value = "64")]
    pub bytes: usize,

    #[arg(short, long)]
    pub offset: Option<usize>,
}

#[derive(Args, Debug)]
pub struct StateArgs {
    #[arg(short, long)]
    pub actor: String,

    #[arg(short, long)]
    pub format: Option<String>,

    #[arg(short, long)]
    pub key: Option<String>,
}

#[derive(Args, Debug)]
pub struct StackArgs {
    #[arg(short, long)]
    pub actor: String,

    #[arg(short, long, default_value = "10")]
    pub depth: usize,
}

#[derive(Args, Debug)]
pub struct MetadataArgs {
    #[arg(short, long)]
    pub actor: String,

    #[arg(short, long)]
    pub format: Option<String>,
}

#[derive(Args, Debug)]
pub struct AllArgs {
    #[arg(short, long)]
    pub actor: String,

    #[arg(short, long)]
    pub format: Option<String>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Actor not found: {0}")]
    ActorNotFound(String),

    #[error("Failed to inspect actor: {0}")]
    InspectFailed(String),

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

    async fn fetch_actor(&self, actor: &str) -> Result<serde_json::Value, Error> {
        let resp = self
            .client
            .get(format!("{}/api/v1/actors/{}", self.base_url, actor))
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::ActorNotFound(actor.to_string()));
        }
        if !resp.status().is_success() {
            return Err(Error::InspectFailed(format!(
                "Server returned {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| Error::InspectFailed(format!("Failed to parse actor response: {e}")))
    }

    async fn fetch_state(
        &self,
        actor: &str,
        key: Option<&str>,
    ) -> Result<serde_json::Value, Error> {
        let url = match key {
            Some(k) => format!("{}/api/v1/state/{}/{}", self.base_url, actor, k),
            None => format!("{}/api/v1/state/{}", self.base_url, actor),
        };

        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(Error::InspectFailed(format!(
                "Server returned {} for state query",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| Error::InspectFailed(format!("Failed to parse state response: {e}")))
    }
}

pub async fn execute(args: InspectArgs) -> Result<(), Error> {
    let api = ApiClient::new(&args.api_addr);
    match args.command {
        InspectCommand::Memory(m) => inspect_memory(&api, &m).await,
        InspectCommand::State(s) => inspect_state(&api, &s).await,
        InspectCommand::Stack(s) => inspect_stack(&s).await,
        InspectCommand::Metadata(m) => inspect_metadata(&api, &m).await,
        InspectCommand::All(a) => inspect_all(&api, &a).await,
    }
}

async fn inspect_memory(api: &ApiClient, args: &MemoryArgs) -> Result<(), Error> {
    let actor = api.fetch_actor(&args.actor).await?;
    let format = args.format.as_deref().unwrap_or("hex");

    println!("Memory info for actor: {}", args.actor);
    println!("----------------------------------------------------------------");
    println!();

    if format == "json" {
        let mut mem = serde_json::json!({
            "actor": args.actor,
            "state": actor.get("state").and_then(|v| v.as_str()).unwrap_or("unknown"),
            "messages_processed": actor.get("messages").and_then(|v| v.as_u64()).unwrap_or(0),
            "cold_starts": actor.get("cold_starts").and_then(|v| v.as_u64()).unwrap_or(0),
        });
        if let Some(last_start) = actor.get("last_cold_start_us") {
            mem["last_cold_start_us"] = last_start.clone();
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&mem).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!("Actor Metadata (memory-related)");
        println!(
            "|-- State:              {}",
            actor
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );
        println!(
            "|-- Messages processed: {}",
            actor.get("messages").and_then(|v| v.as_u64()).unwrap_or(0)
        );
        println!(
            "|-- Cold starts:        {}",
            actor
                .get("cold_starts")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        );
        println!(
            "|-- Last cold start:    {} us",
            actor
                .get("last_cold_start_us")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        );
        println!();
        println!("NOTE: Detailed WASM linear memory dumps require the debugging");
        println!("      extensions to be enabled on the server.");
    }

    println!();
    Ok(())
}

async fn inspect_state(api: &ApiClient, args: &StateArgs) -> Result<(), Error> {
    let format = args.format.as_deref().unwrap_or("table");

    println!("Actor state for: {}", args.actor);
    println!("----------------------------------------------------------------");
    println!();

    let state = api.fetch_state(&args.actor, args.key.as_deref()).await?;

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&state).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        if args.key.is_some() {
            println!("Key value:");
            println!(
                "{}",
                serde_json::to_string_pretty(&state).unwrap_or_else(|_| state.to_string())
            );
        } else {
            match state.as_object() {
                Some(map) if !map.is_empty() => {
                    println!("State keys: {}", map.len());
                    println!();
                    for (key, value) in map {
                        let val_str = match value {
                            serde_json::Value::String(s) => format!("\"{}\"", s),
                            other => other.to_string(),
                        };
                        println!("|-- {}: {}", key, val_str);
                    }
                }
                _ => {
                    println!("State:");
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&state).unwrap_or_else(|_| state.to_string())
                    );
                }
            }
        }
    }

    println!();
    Ok(())
}

async fn inspect_stack(args: &StackArgs) -> Result<(), Error> {
    println!("Call stack for actor: {}", args.actor);
    println!("----------------------------------------------------------------");
    println!();

    println!("Stack traces require WASM debugging extensions to be enabled on the server.");
    println!("This feature is not yet available via the HTTP API.");
    println!();
    println!("To obtain stack traces:");
    println!("  1. Enable the `debug` feature on the aether-server");
    println!("  2. Use the WASM debugging port (usually 9229)");
    println!("  3. Connect with a DWARF-compatible debugger");
    println!();
    println!("Requested depth: {} frames", args.depth);

    Ok(())
}

async fn inspect_metadata(api: &ApiClient, args: &MetadataArgs) -> Result<(), Error> {
    let format = args.format.as_deref().unwrap_or("table");

    println!("Metadata for actor: {}", args.actor);
    println!("----------------------------------------------------------------");
    println!();

    let meta = api.fetch_actor(&args.actor).await?;

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&meta).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!("Identity");
        println!(
            "|-- ID:       {}",
            meta.get("id").and_then(|v| v.as_str()).unwrap_or("unknown")
        );
        println!(
            "|-- Name:     {}",
            meta.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&args.actor)
        );
        println!(
            "|-- State:    {}",
            meta.get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );
        println!();
        println!("Runtime");
        println!("|-- Version:  {}", env!("CARGO_PKG_VERSION"));
        println!(
            "|-- Messages: {}",
            meta.get("messages").and_then(|v| v.as_u64()).unwrap_or(0)
        );
        println!(
            "|-- Errors:   {}",
            meta.get("errors").and_then(|v| v.as_u64()).unwrap_or(0)
        );
        println!(
            "|-- Cold starts: {}",
            meta.get("cold_starts")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
        );
    }

    println!();
    Ok(())
}

async fn inspect_all(api: &ApiClient, args: &AllArgs) -> Result<(), Error> {
    let format = args.format.as_deref().unwrap_or("table");

    println!("+=============================================================+");
    println!("|              FULL ACTOR INSPECTION: {:<22}|", args.actor);
    println!("+=============================================================+");
    println!();

    let actor = api.fetch_actor(&args.actor).await;

    match actor {
        Ok(meta) => {
            if format == "json" {
                let mut full = serde_json::json!({
                    "actor": args.actor,
                    "metadata": meta,
                });

                if let Ok(state) = api.fetch_state(&args.actor, None).await {
                    full["state"] = state;
                }

                println!(
                    "{}",
                    serde_json::to_string_pretty(&full).unwrap_or_else(|_| "{}".to_string())
                );
            } else {
                println!("Metadata");
                println!(
                    "|-- ID:          {}",
                    meta.get("id").and_then(|v| v.as_str()).unwrap_or("unknown")
                );
                println!(
                    "|-- Name:        {}",
                    meta.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&args.actor)
                );
                println!(
                    "|-- State:       {}",
                    meta.get("state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
                println!(
                    "|-- Messages:    {}",
                    meta.get("messages").and_then(|v| v.as_u64()).unwrap_or(0)
                );
                println!(
                    "|-- Cold starts: {}",
                    meta.get("cold_starts")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0)
                );
                println!(
                    "|-- Errors:      {}",
                    meta.get("errors").and_then(|v| v.as_u64()).unwrap_or(0)
                );
                println!();

                match api.fetch_state(&args.actor, None).await {
                    Ok(state) => {
                        println!("State");
                        if let Some(map) = state.as_object() {
                            for (key, value) in map {
                                let val_str = match value {
                                    serde_json::Value::String(s) => format!("\"{}\"", s),
                                    other => other.to_string(),
                                };
                                println!("|-- {}: {}", key, val_str);
                            }
                        } else {
                            println!("|-- {}", state);
                        }
                        println!();
                    }
                    Err(e) => {
                        println!("State");
                        println!("|-- (unavailable: {})", e);
                        println!();
                    }
                }

                println!("Call Stack");
                println!("|-- (requires WASM debugging extensions)");
                println!();
            }
        }
        Err(e) => {
            println!("Failed to fetch actor data: {}", e);
            println!();
        }
    }

    println!("+=============================================================+");
    Ok(())
}
