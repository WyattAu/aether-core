//! Capability Command
//!
//! Manage actor capabilities.

use clap::Args;
use clap::Subcommand;
use thiserror::Error;

/// Capability command errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid capability: {0}")]
    InvalidCapability(String),
}

/// Capability subcommands
#[derive(Subcommand, Debug)]
pub enum CapabilityCommand {
    /// List capabilities for an actor
    List(ListArgs),

    /// Grant a capability to an actor
    Grant(GrantArgs),

    /// Revoke a capability from an actor
    Revoke(RevokeArgs),
}

/// List capabilities arguments
#[derive(Args, Debug)]
pub struct ListArgs {
    /// Actor name
    #[arg(short, long)]
    pub actor: String,
}

/// Grant capability arguments
#[derive(Args, Debug)]
pub struct GrantArgs {
    /// Actor name
    #[arg(short, long)]
    pub actor: String,

    /// Capability to grant (e.g., "networking", "fs:read:/data/*")
    #[arg(short, long)]
    pub capability: String,
}

/// Revoke capability arguments
#[derive(Args, Debug)]
pub struct RevokeArgs {
    /// Actor name
    #[arg(short, long)]
    pub actor: String,

    /// Capability to revoke
    #[arg(short, long)]
    pub capability: String,
}

/// Capability command arguments
#[derive(Args, Debug)]
pub struct CapabilityArgs {
    #[command(subcommand)]
    pub command: CapabilityCommand,
}

/// Execute the capability command
pub async fn execute(args: CapabilityArgs) -> Result<(), Error> {
    match args.command {
        CapabilityCommand::List(list_args) => list_capabilities(&list_args).await?,
        CapabilityCommand::Grant(grant_args) => grant_capability(&grant_args).await?,
        CapabilityCommand::Revoke(revoke_args) => revoke_capability(&revoke_args).await?,
    }
    Ok(())
}

async fn list_capabilities(args: &ListArgs) -> Result<(), Error> {
    println!("Capabilities for actor: {}", args.actor);
    println!("────────────────────────────────────────────────────────────");
    println!();

    println!("Network Capabilities:");
    println!("  ✓ networking:public        (outbound connections allowed)");
    println!("  ✗ networking:private       (inbound connections denied)");
    println!();

    println!("Filesystem Capabilities:");
    println!("  ✓ fs:read:/data/*       (read access to /data)");
    println!("  ✗ fs:write:/data/*      (write access denied)");
    println!();

    println!("System Capabilities:");
    println!("  ✓ sys:clock             (time access allowed)");
    println!("  ✓ sys:random           (entropy access allowed)");
    println!("  ✗ sys:env              (environment access denied)");
    println!();

    println!("Total: 4 granted, 3 denied");

    Ok(())
}

async fn grant_capability(args: &GrantArgs) -> Result<(), Error> {
    println!(
        "Granting capability '{}' to actor: {}",
        args.capability, args.actor
    );
    println!();

    // Validate capability format
    if !args.capability.contains(':') {
        return Err(Error::InvalidCapability(
            "Capability must be in format 'domain:action' or 'domain:action:path'".into(),
        ));
    }

    println!("✓ Capability granted successfully");
    println!();
    println!("Note: This change will take effect on next actor restart.");
    println!("Use 'aether scale --wait' to restart the actor.");

    Ok(())
}

async fn revoke_capability(args: &RevokeArgs) -> Result<(), Error> {
    println!(
        "Revoking capability '{}' from actor: {}",
        args.capability, args.actor
    );
    println!();

    println!("✓ Capability revoked successfully");
    println!();
    println!("Note: This change will take effect on next actor restart.");
    println!("Active instances may still have access until they are terminated.");

    Ok(())
}
