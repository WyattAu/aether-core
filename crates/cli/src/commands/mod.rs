//! CLI Commands
//!
//! Defines all available CLI commands for the Aether runtime.

use clap::Subcommand;
use thiserror::Error;

/// Default dashboard address for CLI commands.
pub const DEFAULT_DASHBOARD_ADDR: &str = "http://127.0.0.1:8080";

pub mod capability;
pub mod completion;
pub mod config;
pub mod dashboard;
pub mod deploy;
pub mod dev;
pub mod exec;
pub mod import;
pub mod inspect;
pub mod logs;
pub mod mesh;
pub mod observability;
pub mod rollback;
pub mod run;
pub mod scale;
pub mod status;
pub mod top;

/// Available CLI commands
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start local development environment
    Dev(dev::DevArgs),

    /// Deploy actors to cluster
    Deploy(deploy::DeployArgs),

    /// Show status of running actors
    Status(status::StatusArgs),

    /// View actor logs
    Logs(logs::LogsArgs),

    /// Scale actors up or down
    Scale(scale::ScaleArgs),

    /// Manage capabilities
    Capability(capability::CapabilityArgs),

    /// Execute commands inside a running actor
    Exec(exec::ExecArgs),

    /// Inspect actor memory and state
    Inspect(inspect::InspectArgs),

    /// Manage mesh network
    Mesh(mesh::MeshArgs),

    /// Manage configuration files
    Config(config::ConfigArgs),

    /// Import docker-compose.yml to aether.toml
    Import(import::ImportArgs),

    /// Launch web dashboard
    Dashboard(dashboard::DashboardArgs),

    /// Terminal-based real-time dashboard
    Top(top::TopArgs),

    /// Rollback a deployment to a previous version
    Rollback(rollback::RollbackArgs),

    /// Generate shell completion scripts
    Completion(completion::CompletionArgs),

    /// Manage observability backends (metrics, logs, status)
    Observability(observability::ObservabilityArgs),

    /// Start a local Aether server
    Run(run::RunCommand),
}

/// Command execution errors
#[derive(Error, Debug)]
pub enum CommandError {
    /// Dev command error
    #[error("{0}")]
    Dev(#[from] dev::Error),

    /// Deploy command error
    #[error("{0}")]
    Deploy(#[from] deploy::Error),

    /// Status command error
    #[error("{0}")]
    Status(#[from] status::Error),

    /// Logs command error
    #[error("{0}")]
    Logs(#[from] logs::Error),

    /// Scale command error
    #[error("{0}")]
    Scale(#[from] scale::Error),

    /// Capability command error
    #[error("{0}")]
    Capability(#[from] capability::Error),

    /// Exec command error
    #[error("{0}")]
    Exec(#[from] exec::Error),

    /// Inspect command error
    #[error("{0}")]
    Inspect(#[from] inspect::Error),

    /// Mesh command error
    #[error("{0}")]
    Mesh(#[from] mesh::Error),

    /// Config command error
    #[error("{0}")]
    Config(#[from] config::Error),

    /// Import command error
    #[error("{0}")]
    Import(#[from] import::Error),

    /// Dashboard command error
    #[error("{0}")]
    Dashboard(#[from] dashboard::Error),

    /// Top command error
    #[error("{0}")]
    Top(#[from] top::Error),

    /// Rollback command error
    #[error("{0}")]
    Rollback(#[from] rollback::Error),

    /// Completion command error
    #[error("{0}")]
    Completion(#[from] completion::Error),

    /// Observability command error
    #[error("{0}")]
    Observability(#[from] observability::Error),

    /// Run command error
    #[error("{0}")]
    Run(#[from] run::Error),
}

/// Execute a CLI command
pub async fn execute(command: Command) -> Result<(), CommandError> {
    match command {
        Command::Dev(args) => dev::execute(args).await?,
        Command::Deploy(args) => deploy::execute(args).await?,
        Command::Status(args) => status::execute(args).await?,
        Command::Logs(args) => logs::execute(args).await?,
        Command::Scale(args) => scale::execute(args).await?,
        Command::Capability(args) => capability::execute(args).await?,
        Command::Exec(args) => exec::execute(args).await?,
        Command::Inspect(args) => inspect::execute(args).await?,
        Command::Mesh(args) => mesh::execute(args).await?,
        Command::Config(args) => config::execute(args).await?,
        Command::Import(args) => import::execute(args).await?,
        Command::Dashboard(args) => dashboard::execute(args).await?,
        Command::Top(args) => top::execute(args).await?,
        Command::Rollback(args) => rollback::execute(args).await?,
        Command::Completion(args) => completion::execute(args)?,
        Command::Observability(args) => observability::execute(args).await?,
        Command::Run(args) => args.execute().await?,
    }
    Ok(())
}
