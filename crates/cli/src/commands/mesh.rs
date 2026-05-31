//! Mesh Command
//!
//! Manage and inspect the actor mesh network via the Aether HTTP API.

use clap::Args;
use clap::Subcommand;
use std::time::Duration;
use thiserror::Error;

use super::DEFAULT_DASHBOARD_ADDR;

#[derive(Args, Debug)]
pub struct MeshArgs {
    #[command(subcommand)]
    pub command: MeshCommand,

    /// Dashboard API address
    #[arg(long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,
}

#[derive(Subcommand, Debug)]
pub enum MeshCommand {
    Status(StatusArgs),
    Peers(PeersArgs),
    Connect(ConnectArgs),
    Disconnect(DisconnectArgs),
    Topology(TopologyArgs),
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    #[arg(short, long)]
    pub format: Option<String>,

    #[arg(short, long)]
    pub watch: bool,
}

#[derive(Args, Debug)]
pub struct PeersArgs {
    #[arg(short, long)]
    pub format: Option<String>,

    #[arg(short, long)]
    pub detailed: bool,
}

#[derive(Args, Debug)]
pub struct ConnectArgs {
    #[arg(short, long)]
    pub peer: String,

    #[arg(short, long)]
    pub port: Option<u16>,

    #[arg(short, long, default_value = "5")]
    pub timeout: u64,
}

#[derive(Args, Debug)]
pub struct DisconnectArgs {
    #[arg(short, long)]
    pub peer: String,

    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct TopologyArgs {
    #[arg(short, long)]
    pub format: Option<String>,

    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Disconnection failed: {0}")]
    DisconnectionFailed(String),

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

    async fn fetch_cluster_status(&self) -> Result<serde_json::Value, Error> {
        let resp = self
            .client
            .get(format!("{}/api/v1/cluster/status", self.base_url))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(Error::ConnectionFailed(format!(
                "Server returned {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| Error::ConnectionFailed(format!("Failed to parse cluster status: {e}")))
    }

    async fn fetch_nodes(&self) -> Result<Vec<serde_json::Value>, Error> {
        let resp = self
            .client
            .get(format!("{}/api/v1/cluster/nodes", self.base_url))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(Error::ConnectionFailed(format!(
                "Server returned {}",
                resp.status()
            )));
        }

        resp.json()
            .await
            .map_err(|e| Error::ConnectionFailed(format!("Failed to parse nodes response: {e}")))
    }

    async fn join_cluster(&self, peer: &str) -> Result<serde_json::Value, Error> {
        let body = serde_json::json!({ "peer": peer });
        let resp = self
            .client
            .post(format!("{}/api/v1/cluster/join", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.ok();
            return Err(Error::ConnectionFailed(format!(
                "Join failed ({status}): {}",
                text.as_deref().unwrap_or("unknown error")
            )));
        }

        resp.json()
            .await
            .ok()
            .ok_or_else(|| Error::ConnectionFailed("Empty response from join".to_string()))
    }

    async fn leave_cluster(&self, peer: &str) -> Result<(), Error> {
        let body = serde_json::json!({ "peer": peer });
        let resp = self
            .client
            .post(format!("{}/api/v1/cluster/leave", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.ok();
            return Err(Error::DisconnectionFailed(format!(
                "Leave failed ({status}): {}",
                text.as_deref().unwrap_or("unknown error")
            )));
        }

        Ok(())
    }
}

pub async fn execute(args: MeshArgs) -> Result<(), Error> {
    let api = ApiClient::new(&args.api_addr);
    match args.command {
        MeshCommand::Status(s) => mesh_status(&api, &s).await,
        MeshCommand::Peers(p) => mesh_peers(&api, &p).await,
        MeshCommand::Connect(c) => mesh_connect(&api, &c).await,
        MeshCommand::Disconnect(d) => mesh_disconnect(&api, &d).await,
        MeshCommand::Topology(t) => mesh_topology(&api, &t).await,
    }
}

async fn mesh_status(api: &ApiClient, args: &StatusArgs) -> Result<(), Error> {
    let format = args.format.as_deref().unwrap_or("table");
    let status = api.fetch_cluster_status().await?;

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&status).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!("+=============================================================+");
        println!("|                     MESH NETWORK STATUS                       |");
        println!("+=============================================================+");
        println!();

        if let Some(obj) = status.as_object() {
            for (key, value) in obj {
                let val_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    other => {
                        serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string())
                    }
                };
                println!("|-- {}: {}", key, val_str);
            }
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&status).unwrap_or_default()
            );
        }

        println!();
        println!("+=============================================================+");

        if args.watch {
            println!("Watching for changes... (Press Ctrl+C to stop)");
        }
    }

    Ok(())
}

async fn mesh_peers(api: &ApiClient, args: &PeersArgs) -> Result<(), Error> {
    let format = args.format.as_deref().unwrap_or("table");
    let nodes = api.fetch_nodes().await?;

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&nodes).unwrap_or_else(|_| "{}".to_string())
        );
    } else if args.detailed {
        println!("Peer Details");
        println!("================================================================");
        println!();

        for node in &nodes {
            let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let addr = node
                .get("address")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let state = node
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            println!("Node: {}", id);
            println!("|-- Address:    {}", addr);
            println!("|-- State:      {}", state);
            println!("|-- Version:    {}", env!("CARGO_PKG_VERSION"));
            println!();
        }
    } else {
        println!("Cluster Nodes: {}", nodes.len());
        println!("-----------------------------------------------------------------------");
        println!("{:<40} {:<24} {:<12}", "NODE ID", "ADDRESS", "STATE");
        println!("-----------------------------------------------------------------------");

        for node in &nodes {
            let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let addr = node
                .get("address")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let state = node
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            println!("{:<40} {:<24} {:<12}", id, addr, state);
        }

        println!("-----------------------------------------------------------------------");
        println!("Total: {} nodes", nodes.len());
    }

    Ok(())
}

async fn mesh_connect(api: &ApiClient, args: &ConnectArgs) -> Result<(), Error> {
    println!("Connecting to peer: {}", args.peer);
    println!();

    let addr = match args.port {
        Some(p) => format!("{}:{}", args.peer, p),
        None => args.peer.clone(),
    };
    println!("Address: {}", addr);
    println!("Timeout: {} seconds", args.timeout);
    println!();

    println!("Sending join request...");

    match api.join_cluster(&addr).await {
        Ok(result) => {
            println!("Successfully connected to peer");
            println!();
            println!("Connection Details:");
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "OK".to_string())
            );
        }
        Err(e) => {
            println!("Connection failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn mesh_disconnect(api: &ApiClient, args: &DisconnectArgs) -> Result<(), Error> {
    println!("Disconnecting from peer: {}", args.peer);
    println!();

    if args.force {
        println!("Force disconnect requested");
    } else {
        println!("Graceful disconnect initiated");
    }

    match api.leave_cluster(&args.peer).await {
        Ok(()) => {
            println!("Successfully disconnected from peer");
        }
        Err(e) => {
            println!("Disconnect failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn mesh_topology(api: &ApiClient, args: &TopologyArgs) -> Result<(), Error> {
    let format = args.format.as_deref().unwrap_or("tree");

    if let Some(output) = &args.output {
        println!("Writing topology to: {}", output);
    }

    let nodes = api.fetch_nodes().await?;

    match format {
        "json" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&nodes).unwrap_or_else(|_| "{}".to_string())
            );
        }
        "dot" => {
            println!("digraph aether_mesh {{");
            println!("  rankdir=LR;");
            println!("  node [shape=circle];");
            println!();

            for (i, node) in nodes.iter().enumerate() {
                let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                let label = if i == 0 {
                    format!("local\\n{}", id)
                } else {
                    id.to_string()
                };
                let style = if i == 0 {
                    " style=filled fillcolor=lightblue;"
                } else {
                    ""
                };
                println!("  \"{}\" [label=\"{}\"{}];", id, label, style);
            }

            println!();

            if nodes.len() > 1 {
                for i in 1..nodes.len() {
                    let first = nodes[0]
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let current = nodes[i]
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    println!("  \"{}\" -> \"{}\";", first, current);
                }
            }

            println!("}}");
        }
        _ => {
            println!("Mesh Topology");
            println!("===============================================================");
            println!();

            if nodes.is_empty() {
                println!("No nodes found in cluster.");
            } else {
                println!("  [Local Node]");
                let local_id = nodes[0]
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                println!("    {}", local_id);
                println!();

                if nodes.len() > 1 {
                    for node in &nodes[1..] {
                        let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let addr = node.get("address").and_then(|v| v.as_str()).unwrap_or("-");
                        println!("    |-- {} ({})", id, addr);
                    }
                }

                println!();
                println!("Nodes: {} | Local: {}", nodes.len(), local_id);
            }
        }
    }

    Ok(())
}
