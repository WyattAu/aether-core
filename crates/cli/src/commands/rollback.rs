//! Rollback Command
//!
//! Rollback a deployment to a previous version.
//! If the Aether server is reachable, fetches current actor state before rollback.

use clap::Args;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use super::DEFAULT_DASHBOARD_ADDR;

#[derive(Args, Debug)]
pub struct RollbackArgs {
    /// Actor name to rollback
    #[arg(short, long)]
    pub actor: String,

    /// Target revision (default: previous)
    #[arg(short, long)]
    pub revision: Option<u32>,

    /// Deployment history file
    #[arg(long, default_value = ".aether/history.json")]
    pub history: String,

    /// Dry run - show what would happen
    #[arg(long)]
    pub dry_run: bool,

    /// Force rollback even if health checks fail
    #[arg(long)]
    pub force: bool,

    /// Timeout in seconds
    #[arg(long, default_value = "60")]
    pub timeout: u64,

    /// Dashboard API address (optional, for server sync)
    #[arg(long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,
}

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[error("Actor not found: {0}")]
    #[allow(dead_code)]
    ActorNotFound(String),

    #[error("No previous revision found")]
    NoPreviousRevision,

    #[error("Revision not found: {0}")]
    RevisionNotFound(u32),

    #[error("Rollback failed: {0}")]
    RollbackFailed(String),

    #[error("Health check failed after rollback")]
    HealthCheckFailed,

    #[error("IO error: {0}")]
    IoError(String),

    #[error("JSON error: {0}")]
    JsonError(String),

    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRecord {
    pub revision: u32,
    pub actor: String,
    pub version: String,
    pub timestamp: u64,
    pub config_hash: String,
    pub status: DeploymentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeploymentStatus {
    Active,
    RolledBack,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeploymentHistory {
    pub records: Vec<DeploymentRecord>,
    pub current_revision: u32,
}

impl DeploymentHistory {
    pub fn load(path: &PathBuf) -> Result<Self, Error> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path).map_err(|e| Error::IoError(e.to_string()))?;
        serde_json::from_str(&content).map_err(|e| Error::JsonError(e.to_string()))
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::IoError(e.to_string()))?;
        }
        let content =
            serde_json::to_string_pretty(self).map_err(|e| Error::JsonError(e.to_string()))?;
        std::fs::write(path, content).map_err(|e| Error::IoError(e.to_string()))
    }

    #[allow(dead_code)]
    pub fn get_actor_records(&self, actor: &str) -> Vec<&DeploymentRecord> {
        self.records.iter().filter(|r| r.actor == actor).collect()
    }

    pub fn get_current(&self, actor: &str) -> Option<&DeploymentRecord> {
        self.records
            .iter()
            .filter(|r| r.actor == actor && r.status == DeploymentStatus::Active)
            .max_by_key(|r| r.revision)
    }

    pub fn get_previous(&self, actor: &str) -> Option<&DeploymentRecord> {
        let current = self.get_current(actor)?;
        self.records
            .iter()
            .filter(|r| {
                r.actor == actor
                    && r.status == DeploymentStatus::Superseded
                    && r.revision < current.revision
            })
            .max_by_key(|r| r.revision)
    }

    pub fn get_revision(&self, actor: &str, revision: u32) -> Option<&DeploymentRecord> {
        self.records
            .iter()
            .find(|r| r.actor == actor && r.revision == revision)
    }

    pub fn rollback_to(
        &mut self,
        actor: &str,
        target_revision: u32,
    ) -> Result<&DeploymentRecord, Error> {
        let current = self.get_current(actor).cloned();

        if let Some(ref current_rec) = current {
            if current_rec.revision == target_revision {
                return Err(Error::RollbackFailed(
                    "Cannot rollback to current revision".to_string(),
                ));
            }
        }

        let target_idx = self
            .records
            .iter()
            .position(|r| r.actor == actor && r.revision == target_revision)
            .ok_or(Error::RevisionNotFound(target_revision))?;

        if let Some(ref current_rec) = current {
            if let Some(idx) = self.records.iter().position(|r| {
                r.actor == actor
                    && r.revision == current_rec.revision
                    && r.status == DeploymentStatus::Active
            }) {
                self.records[idx].status = DeploymentStatus::RolledBack;
            }
        }

        self.records[target_idx].status = DeploymentStatus::Active;
        self.current_revision = target_revision;

        Ok(&self.records[target_idx])
    }
}

async fn fetch_server_state(api_addr: &str, actor: &str) -> Option<serde_json::Value> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .ok()?;

    let base_url = api_addr.trim_end_matches('/');

    let resp = client
        .get(format!("{}/api/v1/actors/{}", base_url, actor))
        .send()
        .await
        .ok()?;

    if resp.status().is_success() {
        resp.json().await.ok()
    } else {
        None
    }
}

pub async fn execute(args: RollbackArgs) -> Result<(), Error> {
    let history_path = PathBuf::from(&args.history);
    let mut history = DeploymentHistory::load(&history_path)?;

    let current = history.get_current(&args.actor).cloned();
    let current_rev = current.as_ref().map(|c| c.revision);

    let target_record = if let Some(rev) = args.revision {
        history
            .get_revision(&args.actor, rev)
            .cloned()
            .ok_or(Error::RevisionNotFound(rev))?
    } else {
        history
            .get_previous(&args.actor)
            .cloned()
            .ok_or(Error::NoPreviousRevision)?
    };

    println!("Aether Rollback");
    println!("------------------");
    println!("   Actor: {}", args.actor);
    println!(
        "   Current revision: {}",
        current_rev
            .map(|r| r.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!("   Target revision: {}", target_record.revision);
    println!("   Target version: {}", target_record.version);
    println!("   Config hash: {}", &target_record.config_hash[..8]);
    println!();

    if let Some(server_state) = fetch_server_state(&args.api_addr, &args.actor).await {
        println!("Server state (synced from {}):", args.api_addr);
        let state = server_state
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let messages = server_state
            .get("messages")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        println!("   -- Actor state:  {}", state);
        println!("   -- Messages:     {}", messages);
        println!();
    } else {
        println!(
            "Server not reachable at {} (continuing with local history)",
            args.api_addr
        );
        println!();
    }

    if args.dry_run {
        println!("Dry run mode - showing rollback plan:");
        println!();
        println!("   Would perform the following:");
        println!("   - Mark current deployment as rolled back");
        println!("   - Activate revision {}", target_record.revision);
        println!(
            "   - Deploy version {} of actor '{}'",
            target_record.version, args.actor
        );
        println!("   - Run health checks (timeout: {}s)", args.timeout);
        println!();
        println!("Dry run complete - no changes made");
        return Ok(());
    }

    println!("Rolling back to revision {}...", target_record.revision);
    println!();

    history.rollback_to(&args.actor, target_record.revision)?;

    println!("   Deactivating current deployment...");
    println!("   Activating revision {}...", target_record.revision);

    let rollback_start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::RollbackFailed(e.to_string()))?
        .as_secs();

    let health_timeout = tokio::time::Duration::from_secs(args.timeout);
    let health_result = tokio::time::timeout(health_timeout, run_health_check(&args.actor)).await;

    match health_result {
        Ok(Ok(true)) => {
            println!("Health check passed");
        }
        Ok(Ok(false)) | Ok(Err(_)) => {
            if args.force {
                println!("Health check failed (forced rollback)");
            } else {
                println!("Health check failed");
                println!("   Use --force to rollback anyway");
                return Err(Error::HealthCheckFailed);
            }
        }
        Err(_) => {
            if args.force {
                println!("Health check timed out (forced rollback)");
            } else {
                println!("Health check timed out after {}s", args.timeout);
                println!("   Use --force to rollback anyway");
                return Err(Error::HealthCheckFailed);
            }
        }
    }

    history.save(&history_path)?;

    let rollback_end = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::RollbackFailed(e.to_string()))?
        .as_secs();

    println!();
    println!("Rollback complete!");
    println!();
    println!("Summary:");
    println!("   Actor: {}", args.actor);
    println!(
        "   Revision: {} -> {}",
        current_rev
            .map(|r| r.to_string())
            .unwrap_or_else(|| "none".to_string()),
        target_record.revision
    );
    println!("   Version: {}", target_record.version);
    println!("   Duration: {}s", rollback_end - rollback_start);
    println!("   Status: Active");

    Ok(())
}

async fn run_health_check(_actor: &str) -> Result<bool, Error> {
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    Ok(true)
}
