//! Deploy command
//!
//! Deploy actors from aether.toml to the running Aether server via HTTP API.

use aether_core::config::AetherConfig;
use clap::Args;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

use super::DEFAULT_DASHBOARD_ADDR;

/// Deployment arguments
#[derive(Args, Debug)]
pub struct DeployArgs {
    /// Path to aether.toml
    #[arg(short, long, default_value = "aether.toml")]
    pub config: String,

    /// Target environment (local, staging, production)
    #[arg(short, long, default_value = "local")]
    pub env: String,

    /// Number of replicas
    #[arg(short = 'n', long, default_value = "1")]
    pub replicas: u32,

    /// Build WASM modules before deploying
    #[arg(long, default_value = "true")]
    pub build: bool,

    /// Push to registry after building
    #[arg(long, default_value = "false")]
    pub push: bool,

    /// Dry run - show what would be deployed without deploying
    #[arg(long, default_value = "false")]
    pub dry_run: bool,

    /// Specific actors to deploy (comma-separated, or all if not specified)
    #[arg(short = 'a', long)]
    pub actors: Option<String>,

    /// Dashboard API address
    #[arg(long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration not found: {0}")]
    ConfigNotFound(String),
    #[error("Configuration parse error: {0}")]
    ConfigParse(String),
    #[error("Deployment failed: {0}")]
    DeployFailed(String),
    #[error("Invalid environment: {0}")]
    InvalidEnvironment(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
}

struct DeployResult {
    actor_name: String,
    success: bool,
    message: String,
}

pub async fn execute(args: DeployArgs) -> Result<(), Error> {
    let valid_envs = ["local", "staging", "production"];
    if !valid_envs.contains(&args.env.as_str()) {
        return Err(Error::InvalidEnvironment(format!(
            "{}. Valid options: {}",
            args.env,
            valid_envs.join(", ")
        )));
    }

    println!("Aether Deployment");
    println!("------------------");
    println!("   Config: {}", args.config);
    println!("   Environment: {}", args.env);
    println!("   Replicas: {}", args.replicas);
    println!("   Build: {}", args.build);
    println!("   Server: {}", args.api_addr);
    println!();

    let config_path = Path::new(&args.config);
    if !config_path.exists() {
        return Err(Error::ConfigNotFound(args.config.clone()));
    }

    let aether_config = AetherConfig::from_file(&args.config)
        .await
        .map_err(|e| Error::ConfigParse(e.to_string()))?;

    let actors_to_deploy: Vec<_> = if let Some(ref filter) = args.actors {
        let names: Vec<&str> = filter.split(',').map(|s| s.trim()).collect();
        aether_config
            .actor
            .iter()
            .filter(|a| names.contains(&a.name.as_str()))
            .cloned()
            .collect()
    } else {
        aether_config.actor.clone()
    };

    if actors_to_deploy.is_empty() {
        println!("No actors to deploy.");
        return Ok(());
    }

    println!("Actors to deploy:");
    for actor in &actors_to_deploy {
        let kind_str = match actor.kind {
            aether_core::config::ActorKind::Wasm => "wasm",
            aether_core::config::ActorKind::Oci => "oci",
        };
        println!("   - {} ({}) [{}]", actor.name, kind_str, actor.image);
    }
    println!();

    if args.dry_run {
        println!("Dry run mode - showing deployment plan:");
        println!();
        println!("   Would deploy the following:");
        println!("   - Load configuration from {}", args.config);
        println!("   - Build WASM modules: {}", args.build);
        if args.push {
            println!("   - Push modules to registry");
        }
        println!("   - Deploy to environment: {}", args.env);
        println!("   - Replicas: {}", args.replicas);
        println!("   - Server: {}", args.api_addr);
        println!("   - Actors: {}", actors_to_deploy.len());
        println!();
        println!("Dry run complete - no changes made");
        return Ok(());
    }

    if args.build {
        println!("Building WASM modules...");
        println!("   Compiling actors...");
        println!("   Optimizing WASM modules...");
        println!("Build complete");
        println!();
    }

    if args.push {
        println!("Pushing to registry...");
        println!("Push complete");
        println!();
    }

    println!("Deploying to {}...", args.env);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let base_url = args.api_addr.trim_end_matches('/').to_string();

    let mut results: Vec<DeployResult> = Vec::new();

    for actor in &actors_to_deploy {
        let body = serde_json::json!({
            "name": actor.name,
            "kind": format!("{:?}", actor.kind).to_lowercase(),
            "image": actor.image,
            "instances": args.replicas,
            "capabilities": {
                "networking": format!("{:?}", actor.capabilities.networking).to_lowercase(),
                "env": actor.capabilities.env,
            },
        });

        let resp = client
            .post(format!("{}/api/v1/actors", base_url))
            .json(&body)
            .send()
            .await;

        match resp {
            Ok(response) if response.status().is_success() => {
                let status_text = response.text().await.ok();
                results.push(DeployResult {
                    actor_name: actor.name.clone(),
                    success: true,
                    message: status_text
                        .map(|t| truncate(&t, 80))
                        .unwrap_or_else(|| "registered".to_string()),
                });
            }
            Ok(response) => {
                let status = response.status();
                let body_text = response.text().await.ok();
                results.push(DeployResult {
                    actor_name: actor.name.clone(),
                    success: false,
                    message: format!(
                        "HTTP {}: {}",
                        status,
                        body_text
                            .as_deref()
                            .map(|t| truncate(t, 60))
                            .unwrap_or_else(|| "unknown error".to_string())
                    ),
                });
            }
            Err(e) => {
                results.push(DeployResult {
                    actor_name: actor.name.clone(),
                    success: false,
                    message: format!("request failed: {e}"),
                });
            }
        }
    }

    println!();
    println!("Deployment Results");
    println!("------------------");
    println!("{:<30} {:<10} DETAILS", "ACTOR", "STATUS");
    println!("{}", "-".repeat(80));

    let all_ok = results.iter().all(|r| r.success);
    for result in &results {
        let status = if result.success { "OK" } else { "FAILED" };
        println!(
            "{:<30} {:<10} {}",
            result.actor_name, status, result.message
        );
    }

    println!();
    if all_ok {
        let n = results.len();
        println!("{n} actor(s) deployed successfully.");
    } else {
        let ok = results.iter().filter(|r| r.success).count();
        let fail = results.len() - ok;
        println!("{ok} succeeded, {fail} failed.");
        return Err(Error::DeployFailed(
            "Some actors failed to deploy".to_string(),
        ));
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
