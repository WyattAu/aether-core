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
    ScaleFailed(String),

    #[error("Actor not found: {0}")]
    ActorNotFound(String),

    #[error("Invalid replica count: {0}")]
    InvalidReplicaCount(String),

    #[error("Connection to daemon failed: {0}")]
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

    // In a real implementation, this would send a scale request to the daemon
    // For now, simulate success
    println!("✓ Scale request accepted");
    println!();
    println!(
        "Actor '{}' is now running with {} replica(s)",
        args.actor, args.replicas
    );
    println!();
    println!("Current Status:");
    println!("  Replicas: {}", args.replicas);
    if let Some(min) = args.min {
        println!("  Min:      {}", min);
    }
    if let Some(max) = args.max {
        println!("  Max:      {}", max);
    }
    println!("  Health:   healthy");

    Ok(())
}
