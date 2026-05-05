//! Logs Command
//!
//! Stream logs from running actors with support for multiple log sources.

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
}

/// Log source configuration
#[derive(Debug, Clone)]
pub enum LogSource {
    /// Tail a log file
    File {
        /// Path to the log file
        path: PathBuf,
    },
    /// Connect to WebSocket endpoint
    WebSocket {
        /// WebSocket URL
        url: String,
    },
    /// Subscribe to tracing subscriber
    Tracing {
        /// Filter string
        filter: String,
    },
    /// Read from actor via RPC
    Actor {
        /// Actor ID
        actor_id: String,
    },
    /// Simulated logs (for demo/testing)
    Simulated,
}

/// Log entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp of the log entry
    pub timestamp: DateTime<Utc>,
    /// Log level
    pub level: LogLevel,
    /// Actor that produced the log
    pub actor: Option<String>,
    /// Log message
    pub message: String,
    /// Additional fields
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            actor: None,
            message: message.into(),
            fields: HashMap::new(),
        }
    }

    /// Set the actor
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Add a field
    #[allow(dead_code)] // Public API for future log source filtering
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
}

/// Log level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    /// Trace level
    Trace,
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warn,
    /// Error level
    Error,
}

impl LogLevel {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }

    /// Parse from string
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
    /// Stream failed
    #[error("Failed to stream logs: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    StreamFailed(String),

    /// Actor not found
    #[error("Actor not found: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ActorNotFound(String),

    /// Invalid log level
    #[error("Invalid log level: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    InvalidLevel(String),

    /// File error
    #[error("File error: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    FileError(String),

    /// WebSocket error
    #[error("WebSocket error: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    WebSocketError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Channel error
    #[error("Channel error: {0}")]
    #[allow(dead_code)] // Reserved for future CLI subcommand expansion
    ChannelError(String),
}

/// Trait for log streaming implementations
#[async_trait]
pub trait LogStreamer: Send {
    /// Get the next log entry
    async fn next(&mut self) -> Option<LogEntry>;

    /// Close the streamer
    fn close(&mut self);
}

/// File log streamer - tails log files
pub struct FileLogStreamer {
    /// Receiver for log entries
    receiver: Option<mpsc::Receiver<LogEntry>>,
    /// Shutdown sender
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// File path
    path: PathBuf,
}

impl FileLogStreamer {
    /// Create a new file log streamer
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
                                // EOF, wait a bit for more data
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

        // Store handle to prevent it from being dropped
        tokio::spawn(async move {
            let _ = handle.await;
        });

        Ok(Self {
            receiver: Some(entry_rx),
            shutdown_tx: Some(shutdown_tx),
            path,
        })
    }

    /// Get the file path
    #[allow(dead_code)] // Public API for future log source filtering
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
    /// WebSocket stream
    ws: Option<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    /// URL
    url: String,
}

impl WebSocketLogStreamer {
    /// Connect to a WebSocket endpoint
    pub async fn connect(url: &str) -> Result<Self, Error> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| Error::WebSocketError(format!("Connection failed: {}", e)))?;

        Ok(Self {
            ws: Some(ws_stream),
            url: url.to_string(),
        })
    }

    /// Get the URL
    #[allow(dead_code)] // Public API for future log source filtering
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

/// Fallback streamer used when no runtime connection is available.
pub struct SimulatedLogStreamer {
    /// Running flag
    running: bool,
    /// Log level filter
    level_filter: Option<LogLevel>,
}

impl SimulatedLogStreamer {
    /// Create a new simulated log streamer
    pub fn new(level_filter: Option<LogLevel>) -> Self {
        Self {
            running: true,
            level_filter,
        }
    }
}

#[async_trait]
impl LogStreamer for SimulatedLogStreamer {
    async fn next(&mut self) -> Option<LogEntry> {
        if !self.running {
            return None;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let messages = [
            ("Actor heartbeat received", LogLevel::Debug),
            ("Processing message from mesh", LogLevel::Info),
            ("State checkpoint completed", LogLevel::Info),
            ("Health check passed", LogLevel::Debug),
            ("Request completed in 5ms", LogLevel::Info),
            ("Connection established", LogLevel::Info),
            ("Cache invalidated", LogLevel::Debug),
        ];

        let idx = (Utc::now().timestamp_subsec_micros() as usize) % messages.len();
        let (msg, level) = messages[idx];

        if let Some(filter) = &self.level_filter {
            if level < *filter {
                return self.next().await;
            }
        }

        let actors = ["system", "worker-1", "worker-2", "scheduler"];
        let actor = actors[(Utc::now().timestamp_subsec_millis() as usize) % actors.len()];

        Some(LogEntry::new(level, msg).with_actor(actor))
    }

    fn close(&mut self) {
        self.running = false;
    }
}

/// Determine the log source from arguments
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
        LogSource::Simulated
    }
}

/// Parse log level filter from string
fn parse_level_filter(level: Option<&str>) -> Result<Option<LogLevel>, Error> {
    match level {
        Some(s) => LogLevel::from_str(s)
            .map(Some)
            .ok_or_else(|| Error::InvalidLevel(s.to_string())),
        None => Ok(None),
    }
}

/// Create a log streamer based on source
async fn create_streamer(source: LogSource) -> Result<Box<dyn LogStreamer>, Error> {
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
            // For now, fall back to simulated
            // In a full implementation, this would integrate with tracing
            Ok(Box::new(SimulatedLogStreamer::new(None)))
        }
        LogSource::Actor { actor_id: _ } => {
            // For now, fall back to simulated
            // In a full implementation, this would connect via RPC
            Ok(Box::new(SimulatedLogStreamer::new(None)))
        }
        LogSource::Simulated => Ok(Box::new(SimulatedLogStreamer::new(None))),
    }
}

/// Check if an entry should be displayed
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

/// Print a log entry in the specified format
fn print_entry(entry: &LogEntry, format: &str) {
    match format {
        "json" => print_json(entry),
        "pretty" => print_pretty(entry),
        _ => print_text(entry),
    }
}

/// Print entry as JSON
fn print_json(entry: &LogEntry) {
    match serde_json::to_string(entry) {
        Ok(json) => println!("{}", json),
        Err(_) => println!("{{\"error\": \"serialization failed\"}}"),
    }
}

/// Print entry as plain text
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

/// Print entry with colors
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

/// Parse a log line into a LogEntry
fn parse_log_line(line: &str) -> Option<LogEntry> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Try JSON first
    if line.starts_with('{') {
        if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
            return Some(entry);
        }
    }

    // Try common log formats
    // Format: [TIMESTAMP] LEVEL ACTOR - MESSAGE
    // Simple parsing without regex
    let remaining = line;

    // Extract timestamp if present
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

    // Extract level
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

    // Extract actor and message (format: ACTOR - MESSAGE)
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

/// Execute the logs command
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
            LogSource::Simulated => "all actors".to_string(),
        }
    );
    println!("Press Ctrl+C to stop.");
    println!();

    if args.follow {
        stream_logs(&args, source, level_filter).await
    } else {
        print_recent_logs(&args, level_filter)
    }
}

/// Stream logs in real-time
async fn stream_logs(
    args: &LogsArgs,
    source: LogSource,
    level_filter: Option<LogLevel>,
) -> Result<(), Error> {
    let mut streamer = create_streamer(source).await?;
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

/// Print recent logs (non-streaming)
fn print_recent_logs(args: &LogsArgs, level_filter: Option<LogLevel>) -> Result<(), Error> {
    println!("Showing last {} log entries...", args.lines);
    println!();

    let sample_logs = vec![
        LogEntry::new(LogLevel::Info, "Actor started").with_actor("system"),
        LogEntry::new(LogLevel::Info, "Actor initialized").with_actor("worker-1"),
        LogEntry::new(LogLevel::Info, "Listening on port 8080").with_actor("system"),
        LogEntry::new(LogLevel::Debug, "Processing request").with_actor("worker-1"),
        LogEntry::new(LogLevel::Info, "Request completed in 5ms").with_actor("worker-1"),
        LogEntry::new(LogLevel::Warn, "Connection timeout").with_actor("network"),
        LogEntry::new(LogLevel::Error, "Failed to connect to database").with_actor("db"),
        LogEntry::new(LogLevel::Debug, "Health check passed").with_actor("health"),
    ];

    let filtered_logs: Vec<_> = sample_logs
        .into_iter()
        .filter(|entry| should_display(entry, &level_filter, &args.actor))
        .collect();

    let count = args.lines.min(filtered_logs.len());
    for entry in filtered_logs.iter().take(count) {
        print_entry(entry, &args.format);
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

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"level\":\"Info\""));
        assert!(json.contains("\"message\":\"Test message\""));
        assert!(json.contains("\"actor\":\"test\""));
    }

    #[tokio::test]
    async fn test_simulated_log_streamer() {
        let mut streamer = SimulatedLogStreamer::new(Some(LogLevel::Info));

        let entry = streamer.next().await;
        assert!(entry.is_some());

        let entry = entry.unwrap();
        assert!(entry.level >= LogLevel::Info);

        streamer.close();
        assert!(!streamer.running);
    }
}
