//! Logs Command
//!
//! Stream logs from running actors with support for multiple log sources.
//! Connects to the Aether dashboard HTTP API or WebSocket endpoint.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use clap::Args;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc;
use tokio_tungstenite::{WebSocketStream, connect_async};

const DEFAULT_DASHBOARD_ADDR: &str = "http://127.0.0.1:8080";

/// Logs command arguments
#[derive(Args, Debug)]
pub struct LogsArgs {
    /// Actor name to get logs from
    #[arg(short, long)]
    pub actor: Option<String>,

    /// Follow log output
    #[arg(short, long)]
    pub follow: bool,

    /// Number of lines to show
    #[arg(short, long, default_value = "100")]
    pub lines: usize,

    /// Filter by log level
    #[arg(short, long)]
    pub level: Option<String>,

    /// Output format (text, json, pretty)
    #[arg(short, long, default_value = "text")]
    pub format: String,

    /// Log file path to tail
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// WebSocket URL to connect to
    #[arg(short, long)]
    pub websocket: Option<String>,

    /// Tracing filter string
    #[arg(short, long)]
    pub tracing_filter: Option<String>,

    /// Dashboard API address
    #[arg(long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,
}

/// Log source configuration
#[derive(Debug, Clone)]
pub enum LogSource {
    /// Tail a log file
    File { path: PathBuf },
    /// Connect to WebSocket endpoint
    WebSocket { url: String },
    /// Subscribe to tracing subscriber (via dashboard API)
    Tracing { filter: String },
    /// Read from actor via dashboard API
    Actor { actor_id: String },
}

/// Log entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub actor: Option<String>,
    pub message: String,
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            actor: None,
            message: message.into(),
            fields: HashMap::new(),
        }
    }

    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    #[allow(dead_code)]
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TRACE" => Some(LogLevel::Trace),
            "DEBUG" => Some(LogLevel::Debug),
            "INFO" => Some(LogLevel::Info),
            "WARN" | "WARNING" => Some(LogLevel::Warn),
            "ERROR" | "ERR" => Some(LogLevel::Error),
            _ => None,
        }
    }
}

/// Logs command errors
#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[error("Failed to stream logs: {0}")]
    #[allow(dead_code)]
    StreamFailed(String),

    #[error("Actor not found: {0}")]
    #[allow(dead_code)]
    ActorNotFound(String),

    #[error("Invalid log level: {0}")]
    InvalidLevel(String),

    #[error("File error: {0}")]
    #[allow(dead_code)]
    FileError(String),

    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Channel error: {0}")]
    #[allow(dead_code)]
    ChannelError(String),

    #[error("No running Aether host found. Start one with: aether run")]
    HostNotFound,

    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
}

/// Trait for log streaming implementations
#[async_trait]
pub trait LogStreamer: Send {
    async fn next(&mut self) -> Option<LogEntry>;
    fn close(&mut self);
}

/// File log streamer - tails log files
pub struct FileLogStreamer {
    receiver: Option<mpsc::Receiver<LogEntry>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    #[allow(dead_code)]
    path: PathBuf,
}

impl FileLogStreamer {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, Error> {
        let path = path.into();
        let (entry_tx, entry_rx) = mpsc::channel(256);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        let file_path = path.clone();
        let handle = tokio::spawn(async move {
            let file = match tokio::fs::File::open(&file_path).await {
                Ok(f) => f,
                Err(e) => {
                    let _ = entry_tx
                        .send(LogEntry::new(
                            LogLevel::Error,
                            format!("Failed to open file: {}", e),
                        ))
                        .await;
                    return;
                }
            };

            let reader = BufReader::new(file);
            let mut lines = reader.lines();

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                    line = lines.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                if let Some(entry) = parse_log_line(&line) {
                                    if entry_tx.send(entry).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            }
                            Err(e) => {
                                let _ = entry_tx.send(LogEntry::new(LogLevel::Error, format!("Read error: {}", e))).await;
                                break;
                            }
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {
            let _ = handle.await;
        });

        Ok(Self {
            receiver: Some(entry_rx),
            shutdown_tx: Some(shutdown_tx),
            path,
        })
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[async_trait]
impl LogStreamer for FileLogStreamer {
    async fn next(&mut self) -> Option<LogEntry> {
        if let Some(ref mut rx) = self.receiver {
            rx.recv().await
        } else {
            None
        }
    }

    fn close(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
        self.receiver.take();
    }
}

/// WebSocket log streamer
pub struct WebSocketLogStreamer {
    ws: Option<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    #[allow(dead_code)]
    url: String,
}

impl WebSocketLogStreamer {
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| Error::WebSocketError(format!("Connection failed: {}", e)))?;

        Ok(Self {
            ws: Some(ws_stream),
            url: url.to_string(),
        })
    }

    #[allow(dead_code)]
    pub fn url(&self) -> &str {
        &self.url
    }
}

#[async_trait]
impl LogStreamer for WebSocketLogStreamer {
    async fn next(&mut self) -> Option<LogEntry> {
        if let Some(ref mut ws) = self.ws {
            loop {
                match ws.next().await {
                    Some(Ok(msg)) => {
                        if msg.is_text() || msg.is_binary() {
                            let text = if msg.is_text() {
                                msg.to_text().unwrap_or("").to_string()
                            } else {
                                let data = msg.into_data();
                                String::from_utf8_lossy(&data).to_string()
                            };

                            let text: &str = &text;

                            if let Ok(entry) = serde_json::from_str::<LogEntry>(text) {
                                return Some(entry);
                            } else if let Some(entry) = parse_log_line(text) {
                                return Some(entry);
                            }
                        }
                    }
                    Some(Err(_)) | None => {
                        return None;
                    }
                }
            }
        } else {
            None
        }
    }

    fn close(&mut self) {
        if let Some(ws) = self.ws.take() {
            tokio::spawn(async move {
                let mut ws = ws;
                let _ = ws.close(None).await;
            });
        }
    }
}

/// Dashboard API log streamer - connects to the Aether dashboard WebSocket endpoint
/// for real-time log streaming.
pub struct DashboardLogStreamer {
    receiver: Option<mpsc::Receiver<LogEntry>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl DashboardLogStreamer {
    pub async fn new(api_addr: &str, actor_filter: Option<&str>) -> Result<Self, Error> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let base_url = api_addr.trim_end_matches('/');

        let resp = client
            .get(format!("{}/api/v1/status", base_url))
            .send()
            .await
            .map_err(|_| Error::HostNotFound)?;

        if !resp.status().is_success() {
            return Err(Error::HostNotFound);
        }

        let ws_url = base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://");

        let (ws_stream, _) = connect_async(format!("{}/ws", ws_url))
            .await
            .map_err(|e| Error::WebSocketError(format!("WebSocket connection failed: {}", e)))?;

        let (entry_tx, entry_rx) = mpsc::channel(256);
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let actor_filter = actor_filter.map(|s| s.to_string());

        let handle = tokio::spawn(async move {
            let mut ws = ws_stream;

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                    msg = ws.next() => {
                        match msg {
                            Some(Ok(m)) => {
                                if m.is_text() || m.is_binary() {
                                    let text = if m.is_text() {
                                        m.to_text().unwrap_or("").to_string()
                                    } else {
                                        let data = m.into_data();
                                        String::from_utf8_lossy(&data).to_string()
                                    };

                                    if let Ok(entry) = serde_json::from_str::<LogEntry>(&text) {
                                        let send = actor_filter.as_ref().is_none_or(|filter| {
                                            entry.actor.as_deref() == Some(filter.as_str())
                                        });
                                        if send && entry_tx.send(entry).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            Some(Err(_)) | None => {
                                let _ = entry_tx.send(LogEntry::new(
                                    LogLevel::Warn,
                                    "Dashboard WebSocket connection closed",
                                )).await;
                                break;
                            }
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {
            let _ = handle.await;
        });

        Ok(Self {
            receiver: Some(entry_rx),
            shutdown_tx: Some(shutdown_tx),
        })
    }
}

#[async_trait]
impl LogStreamer for DashboardLogStreamer {
    async fn next(&mut self) -> Option<LogEntry> {
        if let Some(ref mut rx) = self.receiver {
            rx.recv().await
        } else {
            None
        }
    }

    fn close(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
        self.receiver.take();
    }
}

/// Dashboard API recent-logs fetcher (non-streaming mode).
struct DashboardLogFetcher {
    api_addr: String,
    actor_filter: Option<String>,
}

impl DashboardLogFetcher {
    fn new(api_addr: &str, actor_filter: Option<&str>) -> Self {
        Self {
            api_addr: api_addr.trim_end_matches('/').to_string(),
            actor_filter: actor_filter.map(|s| s.to_string()),
        }
    }

    async fn fetch_recent(&self, _limit: usize) -> Result<Vec<LogEntry>, Error> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let resp = client
            .get(format!("{}/api/v1/status", self.api_addr))
            .send()
            .await
            .map_err(|_| Error::HostNotFound)?;

        if !resp.status().is_success() {
            return Err(Error::HostNotFound);
        }

        let traces_resp = client
            .get(format!("{}/api/v1/traces", self.api_addr))
            .send()
            .await;

        let mut entries = Vec::new();

        if let Ok(resp) = traces_resp {
            if resp.status().is_success() {
                if let Ok(traces) = resp.json::<Vec<serde_json::Value>>().await {
                    for trace in &traces {
                        let operation = trace
                            .get("operation")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let duration = trace
                            .get("duration_us")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);

                        let actor = trace
                            .get("operation")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.split("::").next())
                            .map(|s| s.to_string());

                        if let Some(ref filter) = self.actor_filter {
                            if actor.as_deref() != Some(filter.as_str()) {
                                continue;
                            }
                        }

                        entries.push(
                            LogEntry::new(
                                LogLevel::Info,
                                format!("{} ({}us)", operation, duration),
                            )
                            .with_actor(actor.unwrap_or_else(|| "system".to_string())),
                        );
                    }
                }
            }
        }

        Ok(entries)
    }
}

fn determine_log_source(args: &LogsArgs) -> LogSource {
    if let Some(ref path) = args.file {
        LogSource::File { path: path.clone() }
    } else if let Some(ref url) = args.websocket {
        LogSource::WebSocket { url: url.clone() }
    } else if let Some(ref filter) = args.tracing_filter {
        LogSource::Tracing {
            filter: filter.clone(),
        }
    } else if let Some(ref actor) = args.actor {
        LogSource::Actor {
            actor_id: actor.clone(),
        }
    } else {
        LogSource::Tracing {
            filter: "info".to_string(),
        }
    }
}

fn parse_level_filter(level: Option<&str>) -> Result<Option<LogLevel>, Error> {
    match level {
        Some(s) => LogLevel::from_str(s)
            .map(Some)
            .ok_or_else(|| Error::InvalidLevel(s.to_string())),
        None => Ok(None),
    }
}

async fn create_streamer(source: LogSource, api_addr: &str) -> Result<Box<dyn LogStreamer>, Error> {
    match source {
        LogSource::File { path } => {
            let streamer = FileLogStreamer::new(path)?;
            Ok(Box::new(streamer))
        }
        LogSource::WebSocket { url } => {
            let streamer = WebSocketLogStreamer::connect(&url).await?;
            Ok(Box::new(streamer))
        }
        LogSource::Tracing { filter: _ } => {
            let streamer = DashboardLogStreamer::new(api_addr, None).await?;
            Ok(Box::new(streamer))
        }
        LogSource::Actor { actor_id } => {
            let streamer = DashboardLogStreamer::new(api_addr, Some(&actor_id)).await?;
            Ok(Box::new(streamer))
        }
    }
}

fn should_display(
    entry: &LogEntry,
    level_filter: &Option<LogLevel>,
    actor_filter: &Option<String>,
) -> bool {
    if let Some(filter) = level_filter {
        if entry.level < *filter {
            return false;
        }
    }

    if let Some(actor) = actor_filter {
        if entry.actor.as_deref() != Some(actor.as_str()) {
            return false;
        }
    }

    true
}

fn print_entry(entry: &LogEntry, format: &str) {
    match format {
        "json" => print_json(entry),
        "pretty" => print_pretty(entry),
        _ => print_text(entry),
    }
}

fn print_json(entry: &LogEntry) {
    match serde_json::to_string(entry) {
        Ok(json) => println!("{}", json),
        Err(_) => println!("{{\"error\": \"serialization failed\"}}"),
    }
}

fn print_text(entry: &LogEntry) {
    let actor = entry.actor.as_deref().unwrap_or("system");
    println!(
        "[{}] {:5} {} - {}",
        entry.timestamp.format("%H:%M:%S%.3f"),
        entry.level.as_str(),
        actor,
        entry.message
    );

    for (key, value) in &entry.fields {
        println!("    {}: {}", key, value);
    }
}

fn print_pretty(entry: &LogEntry) {
    let level_style = match entry.level {
        LogLevel::Error => "\x1b[31m",
        LogLevel::Warn => "\x1b[33m",
        LogLevel::Info => "\x1b[32m",
        LogLevel::Debug => "\x1b[36m",
        LogLevel::Trace => "\x1b[90m",
    };

    let actor = entry.actor.as_deref().unwrap_or("system");
    let actor_style = "\x1b[35m";
    let reset = "\x1b[0m";
    let dim = "\x1b[2m";

    println!(
        "{}{}{}{:5}{} {}{}{}{} - {}{}",
        dim,
        entry.timestamp.format("%H:%M:%S%.3f"),
        reset,
        level_style,
        entry.level.as_str(),
        reset,
        actor_style,
        actor,
        reset,
        level_style,
        entry.message
    );

    for (key, value) in &entry.fields {
        println!("  {}{}:{} {}", dim, key, reset, value);
    }
}

fn parse_log_line(line: &str) -> Option<LogEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    if line.starts_with('{') {
        if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
            return Some(entry);
        }
    }

    let remaining = line;

    let (timestamp, remaining) = if remaining.starts_with('[') {
        if let Some(end) = remaining.find(']') {
            let ts_str = &remaining[1..end];
            let ts = chrono::NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d %H:%M:%S%.f")
                .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
                .ok();
            (ts, remaining[end + 1..].trim_start())
        } else {
            (None, remaining)
        }
    } else {
        (None, remaining)
    };

    let parts: Vec<&str> = remaining.splitn(2, ' ').collect();
    if parts.is_empty() {
        return Some(LogEntry::new(LogLevel::Info, line));
    }

    let level = LogLevel::from_str(parts[0]);
    if level.is_none() {
        return Some(LogEntry::new(LogLevel::Info, line));
    }
    let level = level?;

    let remaining = if parts.len() > 1 { parts[1] } else { "" };

    let (actor, message) = if let Some(dash_pos) = remaining.find(" - ") {
        let actor_str = remaining[..dash_pos].trim();
        let msg = remaining[dash_pos + 3..].trim();
        (
            if actor_str.is_empty() {
                None
            } else {
                Some(actor_str.to_string())
            },
            msg,
        )
    } else {
        (None, remaining)
    };

    let mut entry = LogEntry::new(level, message);
    if let Some(ts) = timestamp {
        entry.timestamp = ts;
    }
    entry.actor = actor;

    Some(entry)
}

pub async fn execute(args: LogsArgs) -> Result<(), Error> {
    let level_filter = parse_level_filter(args.level.as_deref())?;
    let source = determine_log_source(&args);

    println!(
        "Streaming logs for: {}",
        match &source {
            LogSource::File { path } => format!("file://{}", path.display()),
            LogSource::WebSocket { url } => format!("ws://{}", url),
            LogSource::Tracing { filter } => format!("tracing:{}", filter),
            LogSource::Actor { actor_id } => format!("actor:{}", actor_id),
        }
    );
    println!("Press Ctrl+C to stop.");
    println!();

    if args.follow {
        stream_logs(&args, source, level_filter, &args.api_addr).await
    } else {
        print_recent_logs(&args, level_filter, &args.api_addr).await
    }
}

async fn stream_logs(
    args: &LogsArgs,
    source: LogSource,
    level_filter: Option<LogLevel>,
    api_addr: &str,
) -> Result<(), Error> {
    let mut streamer = create_streamer(source, api_addr).await?;
    let mut sigint = signal(SignalKind::interrupt())?;

    loop {
        tokio::select! {
            entry = streamer.next() => {
                match entry {
                    Some(entry) => {
                        if should_display(&entry, &level_filter, &args.actor) {
                            print_entry(&entry, &args.format);
                        }
                    }
                    None => {
                        println!("Log stream ended.");
                        break;
                    }
                }
            }
            _ = sigint.recv() => {
                println!("\nStopped streaming logs.");
                streamer.close();
                break;
            }
        }
    }

    Ok(())
}

async fn print_recent_logs(
    args: &LogsArgs,
    level_filter: Option<LogLevel>,
    api_addr: &str,
) -> Result<(), Error> {
    println!("Showing recent log entries...");
    println!();

    let fetcher = match &args.file {
        Some(path) => {
            let mut streamer = FileLogStreamer::new(path.clone())?;
            let mut entries = Vec::new();
            for _ in 0..args.lines {
                match streamer.next().await {
                    Some(entry) => entries.push(entry),
                    None => break,
                }
            }
            let filtered: Vec<_> = entries
                .into_iter()
                .filter(|e| should_display(e, &level_filter, &args.actor))
                .collect();
            for entry in filtered {
                print_entry(&entry, &args.format);
            }
            return Ok(());
        }
        None => {
            let actor = args.actor.as_deref();
            DashboardLogFetcher::new(api_addr, actor)
        }
    };

    let entries = fetcher.fetch_recent(args.lines).await?;

    let filtered: Vec<_> = entries
        .into_iter()
        .filter(|e| should_display(e, &level_filter, &args.actor))
        .collect();

    let count = args.lines.min(filtered.len());
    for entry in filtered.iter().take(count) {
        print_entry(entry, &args.format);
    }

    if filtered.is_empty() {
        println!("No log entries found.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Error > LogLevel::Warn);
        assert!(LogLevel::Warn > LogLevel::Info);
        assert!(LogLevel::Info > LogLevel::Debug);
        assert!(LogLevel::Debug > LogLevel::Trace);
    }

    #[test]
    fn test_log_level_from_str() {
        assert_eq!(LogLevel::from_str("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str("WARN"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str("WARNING"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str("invalid"), None);
    }

    #[test]
    fn test_log_entry_builder() {
        let entry = LogEntry::new(LogLevel::Info, "Test message")
            .with_actor("test-actor")
            .with_field("key", "value");

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "Test message");
        assert_eq!(entry.actor, Some("test-actor".to_string()));
        assert_eq!(entry.fields.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_should_display_level_filter() {
        let entry = LogEntry::new(LogLevel::Debug, "test");

        assert!(should_display(&entry, &Some(LogLevel::Debug), &None));
        assert!(!should_display(&entry, &Some(LogLevel::Info), &None));
        assert!(should_display(&entry, &Some(LogLevel::Trace), &None));
    }

    #[test]
    fn test_should_display_actor_filter() {
        let entry = LogEntry::new(LogLevel::Info, "test").with_actor("worker-1");

        assert!(should_display(&entry, &None, &Some("worker-1".to_string())));
        assert!(!should_display(
            &entry,
            &None,
            &Some("worker-2".to_string())
        ));
    }

    #[test]
    fn test_parse_level_filter() {
        assert!(parse_level_filter(Some("INFO")).is_ok());
        assert!(parse_level_filter(Some("invalid")).is_err());
        assert!(parse_level_filter(None).is_ok());
    }

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry::new(LogLevel::Info, "Test message").with_actor("test");

        let json = serde_json::to_string(&entry).unwrap_or_else(|_| "{}".to_string());
        assert!(json.contains("\"level\":\"Info\""));
        assert!(json.contains("\"message\":\"Test message\""));
        assert!(json.contains("\"actor\":\"test\""));
    }

    #[test]
    fn test_determine_log_source_file() {
        let args = LogsArgs {
            actor: None,
            follow: false,
            lines: 100,
            level: None,
            format: "text".to_string(),
            file: Some(PathBuf::from("/var/log/test.log")),
            websocket: None,
            tracing_filter: None,
            api_addr: DEFAULT_DASHBOARD_ADDR.to_string(),
        };
        match determine_log_source(&args) {
            LogSource::File { .. } => {}
            other => panic!("Expected File, got {:?}", other),
        }
    }

    #[test]
    fn test_determine_log_source_actor() {
        let args = LogsArgs {
            actor: Some("my-actor".to_string()),
            follow: false,
            lines: 100,
            level: None,
            format: "text".to_string(),
            file: None,
            websocket: None,
            tracing_filter: None,
            api_addr: DEFAULT_DASHBOARD_ADDR.to_string(),
        };
        match determine_log_source(&args) {
            LogSource::Actor { actor_id } => assert_eq!(actor_id, "my-actor"),
            other => panic!("Expected Actor, got {:?}", other),
        }
    }

    #[test]
    fn test_determine_log_source_default_tracing() {
        let args = LogsArgs {
            actor: None,
            follow: false,
            lines: 100,
            level: None,
            format: "text".to_string(),
            file: None,
            websocket: None,
            tracing_filter: None,
            api_addr: DEFAULT_DASHBOARD_ADDR.to_string(),
        };
        match determine_log_source(&args) {
            LogSource::Tracing { .. } => {}
            other => panic!("Expected Tracing, got {:?}", other),
        }
    }
}
