//! Scale Command
//!
//! Scale actor instances up or down.

use clap::Args;
use thiserror::Error;

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
}

/// Scale command errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to scale actor: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ScaleFailed(String),

    #[error("Actor not found: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ActorNotFound(String),

    #[error("Invalid replica count: {0}")]
    InvalidReplicaCount(String),

    #[error("Connection to daemon failed: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ConnectionFailed(String),
}

/// Execute the scale command
pub async fn execute(args: ScaleArgs) -> Result<(), Error> {
    println!(
        "Scaling actor '{}' to {} replica(s)...",
        args.actor, args.replicas
    );
    println!();

    // Validate replica count
    if args.replicas == 0 {
        return Err(Error::InvalidReplicaCount(
            "Replicas must be greater than 0".to_string(),
        ));
    }

    // Check for min/max constraints
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

    Err(Error::ConnectionFailed(
        "Not connected to Aether runtime".to_string(),
    ))
}
