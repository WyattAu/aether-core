//! Top Command
//!
//! Terminal-based real-time dashboard for monitoring Aether actors.
//! Connects to the Aether dashboard HTTP API to fetch live runtime data.

use clap::Args;
use std::io;
use std::time::Duration;
use thiserror::Error;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Tabs},
};

use serde::Deserialize;

const DEFAULT_DASHBOARD_ADDR: &str = "http://127.0.0.1:8080";

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
    #[allow(dead_code)]
    stopped: u32,
    cpu_total: u16,
    memory_total_mb: u64,
    memory_available_mb: u64,
    uptime_secs: u64,
}

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
}

struct App {
    actors: Vec<ActorInfo>,
    metrics: SystemMetrics,
    selected_tab: usize,
    tabs: Vec<&'static str>,
    filter: Option<String>,
    sort_field: String,
    should_quit: bool,
    connected: bool,
    dashboard: DashboardClient,
}

impl App {
    fn new(args: &TopArgs) -> Self {
        let dashboard = DashboardClient::new(&args.api_addr);
        Self {
            actors: Vec::new(),
            metrics: SystemMetrics::empty(),
            selected_tab: 0,
            tabs: vec!["Actors", "Resources", "Mesh", "Logs"],
            filter: args.filter.clone(),
            sort_field: args.sort.clone(),
            should_quit: false,
            connected: false,
            dashboard,
        }
    }

    async fn update(&mut self) {
        self.connected = self.dashboard.check_connection().await;

        if !self.connected {
            self.actors.clear();
            self.metrics = SystemMetrics::empty();
            return;
        }

        if let Some(status) = self.dashboard.fetch_status().await {
            self.metrics = SystemMetrics {
                total_actors: status.actors_running as u32,
                running: status.actors_running as u32,
                pending: 0,
                stopped: 0,
                cpu_total: 0,
                memory_total_mb: 0,
                memory_available_mb: 0,
                uptime_secs: status.uptime_secs as u64,
            };
        }

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
            KeyCode::Up | KeyCode::Char('k') => {}
            KeyCode::Down | KeyCode::Char('j') => {}
            _ => {}
        }
    }
}

impl SystemMetrics {
    fn empty() -> Self {
        Self {
            total_actors: 0,
            running: 0,
            pending: 0,
            stopped: 0,
            cpu_total: 0,
            memory_total_mb: 0,
            memory_available_mb: 0,
            uptime_secs: 0,
        }
    }
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
        {
            if let Event::Key(key) = event::read().map_err(|e| Error::Terminal(e.to_string()))? {
                app.handle_key(key.code);
            }
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
        .map(|a| {
            let status_color = match a.status.as_str() {
                "running" => Color::Green,
                "pending" => Color::Yellow,
                "stopped" => Color::Red,
                _ => Color::Gray,
            };
            let cells = [
                Cell::from(a.name.as_str()).style(Style::default().fg(Color::White)),
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
            Row::new(cells).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(20),
        Constraint::Length(10),
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
        .column_spacing(2);
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
            Span::styled("8", Style::default().fg(Color::Cyan)),
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

    let content = vec![
        Line::from(vec![
            Span::styled(" Nodes: ", Style::default().fg(Color::DarkGray)),
            Span::styled("1", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(" Connections: ", Style::default().fg(Color::DarkGray)),
            Span::styled("0", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
            Span::styled("local only", Style::default().fg(Color::Green)),
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

    let content = vec![Line::from(vec![
        Span::styled(" Connected. Use ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "aether logs",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " for live log streaming.",
            Style::default().fg(Color::DarkGray),
        ),
    ])];

    let panel = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Logs (live) ")
            .title_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(panel, area);
}
