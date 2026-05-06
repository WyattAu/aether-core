//! Observability Command
//!
//! Manage observability backends: push metrics, ship logs, check connectivity.

use clap::{Args, Subcommand};
use thiserror::Error;

#[derive(Args, Debug)]
pub struct ObservabilityArgs {
    #[command(subcommand)]
    pub command: ObservabilityCommand,
}

#[derive(Subcommand, Debug)]
pub enum ObservabilityCommand {
    /// One-shot push metrics to VictoriaMetrics
    PushMetrics,

    /// One-shot ship logs to VictoriaLogs/Loki
    PushLogs,

    /// Show current observability config and connectivity
    Status,
}

/// Observability command errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Metrics push failed: {0}")]
    PushMetrics(String),

    #[error("Log shipping failed: {0}")]
    PushLogs(String),

    #[error("Connectivity check failed: {0}")]
    #[allow(dead_code)]
    Connectivity(String),

    #[error("Config not found: {0}")]
    #[allow(dead_code)]
    ConfigNotFound(String),
}

/// Execute the observability command
pub async fn execute(args: ObservabilityArgs) -> Result<(), Error> {
    match args.command {
        ObservabilityCommand::PushMetrics => push_metrics().await,
        ObservabilityCommand::PushLogs => push_logs().await,
        ObservabilityCommand::Status => status().await,
    }
}

async fn push_metrics() -> Result<(), Error> {
    use aether_core::observability::{MetricsCollector, VictoriaMetricsConfig, VictoriaMetricsPusher};
    use std::time::Duration;

    let url = std::env::var("VICTORIAMETRICS_URL")
        .unwrap_or_else(|_| "http://localhost:8428/api/v1/write".to_string());

    let config = VictoriaMetricsConfig {
        endpoint: url.clone(),
        push_interval: Duration::from_secs(15),
        extra_labels: vec![],
    };

    let pusher =
        VictoriaMetricsPusher::new(config).map_err(|e| Error::PushMetrics(e.to_string()))?;

    let metrics = MetricsCollector::new();
    let data = metrics.export_prometheus();

    pusher.push(&data).await.map_err(|e| Error::PushMetrics(e.to_string()))?;

    println!("Metrics pushed to {}", url);
    Ok(())
}

async fn push_logs() -> Result<(), Error> {
    let vl_url = std::env::var("VICTORIALOGS_URL").ok();
    let loki_url = std::env::var("LOKI_URL").ok();

    if vl_url.is_none() && loki_url.is_none() {
        return Err(Error::PushLogs(
            "No log endpoints configured. Set VICTORIALOGS_URL or LOKI_URL.".to_string(),
        ));
    }

    if let Some(url) = vl_url {
        use aether_core::observability::{VictoriaLogsConfig, VictoriaLogsShipper};

        let config = VictoriaLogsConfig {
            endpoint: url.clone(),
            extra_labels: vec![],
            batch_size: 1000,
        };

        let shipper =
            VictoriaLogsShipper::new(config).map_err(|e| Error::PushLogs(e.to_string()))?;

        shipper
            .ship(&[])
            .await
            .map_err(|e| Error::PushLogs(e.to_string()))?;

        println!("Logs shipped to VictoriaLogs at {}", url);
    }

    if let Some(url) = loki_url {
        use aether_core::observability::{LokiConfig, LokiPusher, LogEntryStream, LogStream};
        use std::collections::HashMap;

        let tenant_id = std::env::var("LOKI_TENANT_ID").unwrap_or_default();

        let config = LokiConfig {
            endpoint: url.clone(),
            tenant_id: tenant_id.clone(),
            extra_labels: vec![("job".to_string(), "aether".to_string())],
        };

        let pusher =
            LokiPusher::new(config).map_err(|e| Error::PushLogs(e.to_string()))?;

        let streams = vec![LogStream {
            streams: vec![LogEntryStream {
                stream: HashMap::new(),
                values: vec![],
            }],
        }];

        pusher.push(&streams).await.map_err(|e| Error::PushLogs(e.to_string()))?;

        println!("Logs pushed to Loki at {}", url);
    }

    Ok(())
}

async fn check_reachable(name: &'static str, url: &str) -> (&'static str, String, bool) {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let url_owned = url.to_string();
    match client {
        Ok(c) => match c.head(url).send().await {
            Ok(resp) => (name, url_owned, resp.status().is_success()),
            Err(_) => (name, url_owned, false),
        },
        Err(_) => (name, url_owned, false),
    }
}

async fn status() -> Result<(), Error> {
    use aether_core::config::AetherConfig;

    let vm_url = std::env::var("VICTORIAMETRICS_URL")
        .unwrap_or_else(|_| "http://localhost:8428".to_string());
    let vl_url = std::env::var("VICTORIALOGS_URL")
        .unwrap_or_else(|_| "http://localhost:9428".to_string());
    let loki_url = std::env::var("LOKI_URL")
        .unwrap_or_else(|_| "http://localhost:3100".to_string());

    let (vm_status, vl_status, loki_status) = tokio::join!(
        check_reachable("VictoriaMetrics", &vm_url),
        check_reachable("VictoriaLogs", &vl_url),
        check_reachable("Loki", &loki_url),
    );

    let checks = [vm_status, vl_status, loki_status];

    println!("┌──────────────────────────────────────────────────────┐");
    println!("│            OBSERVABILITY STATUS                       │");
    println!("├──────────────────────┬───────────────────────────────┤");
    println!("│ Backend              │ Status                        │");
    println!("├──────────────────────┼───────────────────────────────┤");

    for (name, _url, reachable) in &checks {
        let status_str = if *reachable { "reachable" } else { "unreachable" };
        println!("│ {:<20} │ {:<29} │", name, status_str);
    }

    println!("├──────────────────────┴───────────────────────────────┤");
    println!("│ Configuration                                         │");
    println!("├──────────────────────┬───────────────────────────────┤");
    println!("│ VictoriaMetrics URL  │ {}", &vm_url[..vm_url.len().min(29)]);
    println!("│ VictoriaLogs URL     │ {}", &vl_url[..vl_url.len().min(29)]);
    println!("│ Loki URL             │ {}", &loki_url[..loki_url.len().min(29)]);
    println!("└──────────────────────┴───────────────────────────────┘");

    let config_path = "aether.toml";
    match AetherConfig::from_file(config_path).await {
        Ok(config) => {
            if let Some(obs) = &config.observability {
                println!();
                println!("Config from aether.toml:");
                println!("  metrics_push_enabled:    {}", obs.metrics_push_enabled);
                println!("  log_shipping_enabled:    {}", obs.log_shipping_enabled);
            } else {
                println!();
                println!("No [observability] section found in aether.toml");
            }
        }
        Err(_) => {
            println!();
            println!("Could not load aether.toml (using env vars)");
        }
    }

    let all_ok = checks.iter().all(|(_, _, ok)| *ok);
    if !all_ok {
        println!();
        println!("Some backends are unreachable.");
    }

    Ok(())
}
