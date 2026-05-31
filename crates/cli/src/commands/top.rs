//! Top Command
//!
//! Terminal-based real-time dashboard for monitoring Aether actors.
//! Connects to the Aether dashboard HTTP API to fetch live runtime data.

use clap::Args;
use std::collections::VecDeque;
use std::io;
use std::time::Duration;
use thiserror::Error;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Gauge, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, Tabs, Wrap,
    },
};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;

use serde::Deserialize;

use super::DEFAULT_DASHBOARD_ADDR;

/// Top command arguments
#[derive(Args, Debug)]
pub struct TopArgs {
    /// Refresh rate in milliseconds
    #[arg(short, long, default_value = "1000")]
    pub refresh: u64,

    /// Filter by actor name pattern
    #[arg(short, long)]
    pub filter: Option<String>,

    /// Sort by field (cpu, memory, name, status)
    #[arg(long, default_value = "cpu")]
    pub sort: String,

    /// Dashboard API address
    #[arg(long, default_value = DEFAULT_DASHBOARD_ADDR)]
    pub api_addr: String,
}

/// Top command errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Terminal error: {0}")]
    Terminal(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("API request failed: {0}")]
    Api(#[from] reqwest::Error),
}

#[derive(Debug, Clone)]
struct ActorInfo {
    name: String,
    status: String,
    instances: u32,
    cpu_percent: u16,
    memory_mb: u64,
    messages_per_sec: u64,
}

#[derive(Debug, Clone)]
struct SystemMetrics {
    total_actors: u32,
    running: u32,
    pending: u32,
    _stopped: u32,
    cpu_total: u16,
    cpu_cores: u32,
    memory_total_mb: u64,
    memory_available_mb: u64,
    uptime_secs: u64,
}

#[derive(Debug, Clone, Default)]
struct MeshInfo {
    nodes: usize,
    connections: usize,
}

#[derive(Debug, Clone)]
struct LogLine {
    text: String,
    is_error: bool,
}

#[derive(Clone)]
struct DashboardClient {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Deserialize)]
struct ApiRuntimeStatus {
    uptime_secs: i64,
    actors_running: u64,
    #[allow(dead_code)]
    messages_total: u64,
    #[allow(dead_code)]
    status: String,
}

#[derive(Deserialize)]
struct ApiActorInfo {
    #[allow(dead_code)]
    id: String,
    name: String,
    state: String,
    #[allow(dead_code)]
    cold_starts: u64,
    messages: u64,
    #[allow(dead_code)]
    errors: u64,
    #[allow(dead_code)]
    last_cold_start_us: u64,
}

impl DashboardClient {
    fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn check_connection(&self) -> bool {
        self.client
            .get(format!("{}/api/v1/status", self.base_url))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    async fn fetch_status(&self) -> Option<ApiRuntimeStatus> {
        let resp = self
            .client
            .get(format!("{}/api/v1/status", self.base_url))
            .send()
            .await
            .ok()?;
        resp.json().await.ok()
    }

    async fn fetch_actors(&self) -> Option<Vec<ApiActorInfo>> {
        let resp = self
            .client
            .get(format!("{}/api/v1/actors", self.base_url))
            .send()
            .await
            .ok()?;
        resp.json().await.ok()
    }

    async fn fetch_mesh(&self) -> MeshInfo {
        let resp = match self
            .client
            .get(format!("{}/api/v1/mesh", self.base_url))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => return MeshInfo::default(),
        };

        let val: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return MeshInfo::default(),
        };

        let nodes = val.get("nodes").and_then(|n| n.as_u64()).unwrap_or(1) as usize;
        let connections = val.get("connections").and_then(|c| c.as_u64()).unwrap_or(0) as usize;

        MeshInfo { nodes, connections }
    }

    fn ws_url(&self) -> String {
        self.base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
    }

    async fn connect_logs_ws(&self, tx: mpsc::Sender<LogLine>) {
        let url = format!("{}/ws", self.ws_url());
        let ws_stream = match connect_async(&url).await {
            Ok((stream, _)) => stream,
            Err(_) => {
                let _ = tx
                    .send(LogLine {
                        text: "Failed to connect to log stream".to_string(),
                        is_error: true,
                    })
                    .await;
                return;
            }
        };

        let (_, mut read) = ws_stream.split();
        while let Some(msg) = read.next().await {
            match msg {
                Ok(m) => {
                    if m.is_text() || m.is_binary() {
                        let text = if m.is_text() {
                            m.to_text().map(|s| s.to_string()).unwrap_or_default()
                        } else {
                            String::from_utf8_lossy(&m.into_data()).to_string()
                        };

                        let is_error = text.contains("\"level\":\"Error\"")
                            || text.contains("\"level\":\"error\"")
                            || text.contains("ERROR");
                        if tx.send(LogLine { text, is_error }).await.is_err() {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }
}

struct App {
    actors: Vec<ActorInfo>,
    metrics: SystemMetrics,
    mesh: MeshInfo,
    log_lines: VecDeque<LogLine>,
    log_rx: Option<mpsc::Receiver<LogLine>>,
    log_scroll: u16,
    selected_tab: usize,
    selected_actor: usize,
    tabs: Vec<&'static str>,
    filter: Option<String>,
    sort_field: String,
    should_quit: bool,
    connected: bool,
    ws_connected: bool,
    dashboard: DashboardClient,
}

impl App {
    fn new(args: &TopArgs) -> Self {
        let dashboard = DashboardClient::new(&args.api_addr);
        Self {
            actors: Vec::new(),
            metrics: SystemMetrics::empty(),
            mesh: MeshInfo::default(),
            log_lines: VecDeque::new(),
            log_rx: None,
            log_scroll: 0,
            selected_tab: 0,
            selected_actor: 0,
            tabs: vec!["Actors", "Resources", "Mesh", "Logs"],
            filter: args.filter.clone(),
            sort_field: args.sort.clone(),
            should_quit: false,
            connected: false,
            ws_connected: false,
            dashboard,
        }
    }

    async fn update(&mut self) {
        self.connected = self.dashboard.check_connection().await;

        if !self.connected {
            self.actors.clear();
            self.metrics = SystemMetrics::empty();
            self.mesh = MeshInfo::default();
            self.ws_connected = false;
            return;
        }

        let (cpu_cores, mem_total_mb, mem_available_mb) = read_system_metrics();

        if let Some(status) = self.dashboard.fetch_status().await {
            self.metrics = SystemMetrics {
                total_actors: status.actors_running as u32,
                running: status.actors_running as u32,
                pending: 0,
                _stopped: 0,
                cpu_total: 0,
                cpu_cores,
                memory_total_mb: mem_total_mb,
                memory_available_mb: mem_available_mb,
                uptime_secs: status.uptime_secs as u64,
            };
        }

        self.mesh = self.dashboard.fetch_mesh().await;

        if let Some(api_actors) = self.dashboard.fetch_actors().await {
            self.actors = api_actors
                .into_iter()
                .map(|a| ActorInfo {
                    name: a.name,
                    status: a.state,
                    instances: 1,
                    cpu_percent: 0,
                    memory_mb: 0,
                    messages_per_sec: a.messages,
                })
                .collect();
        }

        if let Some(ref filter) = self.filter {
            self.actors.retain(|a| a.name.contains(filter));
        }

        match self.sort_field.as_str() {
            "memory" => self.actors.sort_by_key(|b| std::cmp::Reverse(b.memory_mb)),
            "name" => self.actors.sort_by(|a, b| a.name.cmp(&b.name)),
            "status" => self.actors.sort_by(|a, b| a.status.cmp(&b.status)),
            _ => self
                .actors
                .sort_by_key(|b| std::cmp::Reverse(b.cpu_percent)),
        }

        if self.selected_actor >= self.actors.len() {
            self.selected_actor = self.actors.len().saturating_sub(1);
        }

        if self.log_rx.is_none() && self.selected_tab == 3 {
            let (tx, rx) = mpsc::channel(256);
            self.log_rx = Some(rx);
            self.ws_connected = true;
            let client = self.dashboard.clone();
            tokio::spawn(async move {
                client.connect_logs_ws(tx).await;
            });
        }

        if let Some(ref mut rx) = self.log_rx {
            while let Ok(line) = rx.try_recv() {
                if self.log_lines.len() >= 100 {
                    self.log_lines.pop_front();
                }
                self.log_lines.push_back(line);
            }
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Left | KeyCode::Char('h') if self.selected_tab > 0 => {
                self.selected_tab -= 1;
            }
            KeyCode::Right | KeyCode::Char('l') if self.selected_tab < self.tabs.len() - 1 => {
                self.selected_tab += 1;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_tab == 0 && self.selected_actor > 0 {
                    self.selected_actor -= 1;
                } else if self.selected_tab == 3 && self.log_scroll > 0 {
                    self.log_scroll = self.log_scroll.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_tab == 0 && !self.actors.is_empty() {
                    if self.selected_actor < self.actors.len() - 1 {
                        self.selected_actor += 1;
                    }
                } else if self.selected_tab == 3 {
                    let max_scroll = self.log_lines.len().saturating_sub(1) as u16;
                    if self.log_scroll < max_scroll {
                        self.log_scroll += 1;
                    }
                }
            }
            _ => {}
        }
    }
}

impl SystemMetrics {
    fn empty() -> Self {
        let (cpu_cores, mem_total_mb, mem_available_mb) = read_system_metrics();
        Self {
            total_actors: 0,
            running: 0,
            pending: 0,
            _stopped: 0,
            cpu_total: 0,
            cpu_cores,
            memory_total_mb: mem_total_mb,
            memory_available_mb: mem_available_mb,
            uptime_secs: 0,
        }
    }
}

fn read_system_metrics() -> (u32, u64, u64) {
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(0);

    let (mem_total_mb, mem_available_mb) = read_linux_memory();

    (cpu_cores, mem_total_mb, mem_available_mb)
}

fn read_linux_memory() -> (u64, u64) {
    let content = match std::fs::read_to_string("/proc/meminfo") {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };

    let mut total_kb: u64 = 0;
    let mut available_kb: u64 = 0;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("MemTotal:") {
            total_kb = parse_meminfo_kb(val);
        } else if let Some(val) = line.strip_prefix("MemAvailable:") {
            available_kb = parse_meminfo_kb(val);
        }
    }

    (total_kb / 1024, available_kb / 1024)
}

fn parse_meminfo_kb(s: &str) -> u64 {
    s.split_whitespace()
        .next()
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(0)
}

pub async fn execute(args: TopArgs) -> Result<(), Error> {
    enable_raw_mode().map_err(|e| Error::Terminal(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| Error::Terminal(e.to_string()))?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| Error::Terminal(e.to_string()))?;

    let mut app = App::new(&args);
    let res = run_app(&mut terminal, &mut app, args.refresh).await;

    disable_raw_mode().map_err(|e| Error::Terminal(e.to_string()))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|e| Error::Terminal(e.to_string()))?;
    terminal
        .show_cursor()
        .map_err(|e| Error::Terminal(e.to_string()))?;

    res
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    refresh_ms: u64,
) -> Result<(), Error> {
    loop {
        terminal
            .draw(|f| ui(f, app))
            .map_err(|e| Error::Terminal(e.to_string()))?;

        if event::poll(Duration::from_millis(refresh_ms))
            .map_err(|e| Error::Terminal(e.to_string()))?
            && let Event::Key(key) = event::read().map_err(|e| Error::Terminal(e.to_string()))?
        {
            app.handle_key(key.code);
        }

        app.update().await;

        if app.should_quit {
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(f.area());

    let titles: Vec<Line> = app
        .tabs
        .iter()
        .map(|t| Line::from(vec![Span::styled(*t, Style::default().fg(Color::Green))]))
        .collect();
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if app.connected {
                    " Aether Top "
                } else {
                    " Aether Top (disconnected) "
                })
                .title_style(
                    Style::default()
                        .fg(if app.connected {
                            Color::Cyan
                        } else {
                            Color::Red
                        })
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .select(app.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    render_metrics_panel(f, app, chunks[1]);

    match app.selected_tab {
        0 => render_actors_table(f, app, chunks[2]),
        1 => render_resources_panel(f, app, chunks[2]),
        2 => render_mesh_panel(f, app, chunks[2]),
        3 => render_logs_panel(f, app, chunks[2]),
        _ => {}
    }

    let help = Paragraph::new(Line::from(vec![
        Span::styled(
            " ←/h →/l ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(" tabs  "),
        Span::styled(
            " ↑/k ↓/j ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::raw(" navigate  "),
        Span::styled(" q ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        Span::raw(" quit"),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(help, chunks[3]);
}

fn render_metrics_panel(f: &mut Frame, app: &App, area: Rect) {
    if !app.connected {
        let panel = Paragraph::new(Line::from(vec![Span::styled(
            "No running Aether host found.",
            Style::default().fg(Color::Yellow),
        )]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" System Metrics "),
        );
        f.render_widget(panel, area);
        return;
    }

    let m = &app.metrics;
    let hours = m.uptime_secs / 3600;
    let mins = (m.uptime_secs % 3600) / 60;

    let mem_percent = if m.memory_available_mb > 0 {
        (m.memory_total_mb as f64 / m.memory_available_mb as f64 * 100.0) as u16
    } else {
        0
    };

    let info = vec![
        Line::from(vec![
            Span::styled(" Uptime: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}h {}m", hours, mins),
                Style::default().fg(Color::Green),
            ),
            Span::raw("   "),
            Span::styled(" Actors: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", m.total_actors),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(" ("),
            Span::styled(
                format!("{} running", m.running),
                Style::default().fg(Color::Green),
            ),
            Span::raw(", "),
            Span::styled(
                format!("{} pending", m.pending),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(")"),
        ]),
        Line::from(vec![
            Span::styled(" CPU: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:>3}%", m.cpu_total),
                Style::default().fg(if m.cpu_total > 80 {
                    Color::Red
                } else if m.cpu_total > 50 {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Memory: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} MB / {} MB", m.memory_total_mb, m.memory_available_mb),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Memory Usage "),
        )
        .gauge_style(
            Style::default()
                .fg(if mem_percent > 80 {
                    Color::Red
                } else if mem_percent > 50 {
                    Color::Yellow
                } else {
                    Color::Green
                })
                .bg(Color::Black),
        )
        .percent(mem_percent);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(30), Constraint::Length(30)])
        .split(area);

    let metrics = Paragraph::new(info).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" System Metrics "),
    );
    f.render_widget(metrics, chunks[0]);
    f.render_widget(gauge, chunks[1]);
}

fn render_actors_table(f: &mut Frame, app: &App, area: Rect) {
    if !app.connected {
        let panel = Paragraph::new(vec![
            Line::from(Span::raw("")),
            Line::from(vec![Span::styled(
                "  No running Aether host found.",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(vec![Span::styled(
                "  Start one with: aether run",
                Style::default().fg(Color::DarkGray),
            )]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Actors ")
                .title_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(panel, area);
        return;
    }

    let header_cells = ["Name", "Status", "Instances", "CPU%", "Memory", "Msg/s"];
    let header = Row::new(header_cells)
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .actors
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let status_color = match a.status.as_str() {
                "running" => Color::Green,
                "failed" => Color::Red,
                "suspended" => Color::Yellow,
                "creating" | "stopped" => Color::Gray,
                "draining" => Color::Magenta,
                _ => Color::Gray,
            };
            let cells = [
                Cell::from(a.name.as_str()),
                Cell::from(a.status.as_str()).style(Style::default().fg(status_color)),
                Cell::from(a.instances.to_string()),
                Cell::from(format!("{:>3}%", a.cpu_percent)).style(Style::default().fg(
                    if a.cpu_percent > 80 {
                        Color::Red
                    } else if a.cpu_percent > 50 {
                        Color::Yellow
                    } else {
                        Color::Green
                    },
                )),
                Cell::from(format!("{} MB", a.memory_mb)),
                Cell::from(format!("{:>6}", a.messages_per_sec)),
            ];
            let row_style = if i == app.selected_actor {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(cells).style(row_style).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(20),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(12),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Actors ")
                .title_style(Style::default().fg(Color::Cyan)),
        )
        .column_spacing(2)
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol(">> ");
    f.render_widget(table, area);
}

fn render_resources_panel(f: &mut Frame, app: &App, area: Rect) {
    if !app.connected {
        let panel = Paragraph::new(vec![
            Line::from(Span::raw("")),
            Line::from(vec![Span::styled(
                "  No running Aether host found.",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(vec![Span::styled(
                "  Start one with: aether run",
                Style::default().fg(Color::DarkGray),
            )]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Resources ")
                .title_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(panel, area);
        return;
    }

    let m = &app.metrics;

    let content = vec![
        Line::from(vec![
            Span::styled(" CPU Cores: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", m.cpu_cores), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(" Total CPU Usage: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}%", m.cpu_total),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Memory Allocated: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} MB", m.memory_total_mb),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Memory Available: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} MB", m.memory_available_mb),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![Span::styled(
            " Network I/O",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  RX: ", Style::default().fg(Color::DarkGray)),
            Span::styled("--", Style::default().fg(Color::Green)),
            Span::raw("  TX: "),
            Span::styled("--", Style::default().fg(Color::Green)),
        ]),
    ];

    let panel = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Resources ")
            .title_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(panel, area);
}

fn render_mesh_panel(f: &mut Frame, app: &App, area: Rect) {
    if !app.connected {
        let panel = Paragraph::new(vec![
            Line::from(Span::raw("")),
            Line::from(vec![Span::styled(
                "  No running Aether host found.",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(vec![Span::styled(
                "  Start one with: aether run",
                Style::default().fg(Color::DarkGray),
            )]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Mesh Network ")
                .title_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(panel, area);
        return;
    }

    let mesh = &app.mesh;
    let status_text = if mesh.nodes <= 1 {
        "local only"
    } else {
        "clustered"
    };
    let status_color = if mesh.nodes <= 1 {
        Color::Green
    } else {
        Color::Cyan
    };

    let content = vec![
        Line::from(vec![
            Span::styled(" Nodes: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", mesh.nodes), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(" Connections: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", mesh.connections),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]),
    ];

    let panel = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Mesh Network ")
            .title_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(panel, area);
}

fn render_logs_panel(f: &mut Frame, app: &App, area: Rect) {
    if !app.connected {
        let panel = Paragraph::new(vec![
            Line::from(Span::raw("")),
            Line::from(vec![Span::styled(
                "  No running Aether host found.",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(vec![Span::styled(
                "  Use 'aether logs' to stream logs.",
                Style::default().fg(Color::DarkGray),
            )]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Logs (live) ")
                .title_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(panel, area);
        return;
    }

    if app.log_lines.is_empty() && !app.ws_connected {
        let content = vec![Line::from(vec![Span::styled(
            " Connecting to log stream...",
            Style::default().fg(Color::Yellow),
        )])];
        let panel = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Logs (live) ")
                .title_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(panel, area);
        return;
    }

    if app.log_lines.is_empty() {
        let content = vec![Line::from(vec![Span::styled(
            " Waiting for log entries...",
            Style::default().fg(Color::DarkGray),
        )])];
        let panel = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Logs (live) ")
                .title_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(panel, area);
        return;
    }

    let lines: Vec<Line> = app
        .log_lines
        .iter()
        .map(|entry| {
            let style = if entry.is_error {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(&entry.text, style))
        })
        .collect();

    let title = format!(" Logs (live) [{} lines] ", app.log_lines.len());
    let panel = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.log_scroll, 0));
    f.render_widget(panel, area);

    let total_lines = app.log_lines.len() as u16;
    if total_lines > 0 {
        let mut scrollbar_state =
            ScrollbarState::new(total_lines as usize).position(app.log_scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        f.render_stateful_widget(
            scrollbar,
            area.inner(ratatui::layout::Margin {
                vertical: 0,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}
