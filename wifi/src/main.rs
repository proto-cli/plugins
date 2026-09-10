use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::process::Command;

struct Theme;
impl Theme {
    const HEADER: Style = Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD);
    const ACCENT: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    const MUTED: Style = Style::new().fg(Color::DarkGray);
    const SUCCESS: Style = Style::new().fg(Color::Green);
    const ERROR: Style = Style::new().fg(Color::Red);
    const WARN: Style = Style::new().fg(Color::Yellow);
    const VALUE: Style = Style::new().fg(Color::White);
    const SELECTED: Style = Style::new().fg(Color::Black).bg(Color::Cyan);
    const CONNECTED: Style = Style::new().fg(Color::Green).add_modifier(Modifier::BOLD);
}

#[derive(Debug, Clone)]
struct WifiNetwork {
    ssid: String,
    signal: i32,
    security: String,
    connected: bool,
}

enum ActivePanel {
    Networks,
    Saved,
}

struct App {
    networks: Vec<WifiNetwork>,
    saved: Vec<String>,
    net_state: ListState,
    saved_state: ListState,
    active: ActivePanel,
    status_msg: String,
    scanning: bool,
}

impl App {
    fn new() -> Self {
        Self {
            networks: Vec::new(),
            saved: Vec::new(),
            net_state: ListState::default(),
            saved_state: ListState::default(),
            active: ActivePanel::Networks,
            status_msg: String::new(),
            scanning: false,
        }
    }
}

fn nmcli_available() -> bool {
    Command::new("which")
        .arg("nmcli")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn scan_networks() -> Vec<WifiNetwork> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "SSID,SIGNAL,SECURITY,ACTIVE", "dev", "wifi", "list", "--rescan", "yes"])
        .output();
    let text = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return Vec::new(),
    };
    let mut networks = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() >= 3 {
            let ssid = parts[0].to_string();
            if ssid.is_empty() || seen.contains(&ssid) { continue; }
            seen.insert(ssid.clone());
            let signal: i32 = parts[1].parse().unwrap_or(0);
            let security = parts[2..].join(":");
            let connected = parts.len() > 3 && parts[parts.len()-1] == "yes";
            networks.push(WifiNetwork { ssid, signal, security, connected });
        }
    }
    networks.sort_by(|a, b| b.signal.cmp(&a.signal));
    networks
}

fn get_saved() -> Vec<String> {
    let output = Command::new("nmcli")
        .args(["-t", "-f", "NAME", "connection", "show"])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn connect(ssid: &str) -> Result<String, String> {
    let out = Command::new("nmcli")
        .args(["dev", "wifi", "connect", ssid])
        .output()
        .map_err(|e| format!("Failed: {}", e))?;
    if out.status.success() {
        Ok(format!("Connected to {}", ssid))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

fn disconnect() -> Result<String, String> {
    let out = Command::new("nmcli")
        .args(["dev", "disconnect", "wlan0"])
        .output()
        .map_err(|e| format!("Failed: {}", e))?;
    if out.status.success() {
        Ok("Disconnected".into())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

fn signal_bar(signal: i32) -> &'static str {
    if signal >= 80 { "▂▄▆█" }
    else if signal >= 60 { "▂▄▆░" }
    else if signal >= 40 { "▂▄░░" }
    else { "▂░░░" }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(f.area());

    let main = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[0]);

    render_networks(f, app, main[0]);
    render_saved(f, app, main[1]);
    render_status(f, app, chunks[1]);
}

fn render_networks(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app.networks.iter().enumerate().map(|(i, net)| {
        let style = if app.net_state.selected() == Some(i) {
            Theme::SELECTED
        } else if net.connected {
            Theme::CONNECTED
        } else {
            Theme::VALUE
        };
        let conn = if net.connected { " ●" } else { "  " };
        ListItem::new(Line::from(vec![
            Span::styled(format!(" {} {} ", conn, net.ssid), style),
            Span::styled(signal_bar(net.signal), Theme::SUCCESS),
            Span::styled(format!(" {}%", net.signal), Theme::MUTED),
            Span::styled(format!(" [{}]", net.security), Theme::MUTED),
        ]))
    }).collect();

    let title = if app.scanning { " Scanning... " } else { " Networks " };
    let block = Block::default()
        .title(Span::styled(title, Theme::ACCENT))
        .borders(Borders::ALL)
        .border_style(Theme::ACCENT);

    let list = List::new(items).block(block).highlight_style(Theme::SELECTED).highlight_symbol("▶ ");
    let mut state = app.net_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_saved(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app.saved.iter().enumerate().map(|(i, name)| {
        let style = if app.saved_state.selected() == Some(i) {
            Theme::SELECTED
        } else {
            Theme::VALUE
        };
        ListItem::new(Line::from(Span::styled(format!(" {} ", name), style)))
    }).collect();

    let block = Block::default()
        .title(Span::styled(" Saved ", Theme::ACCENT))
        .borders(Borders::ALL)
        .border_style(Theme::MUTED);

    let list = List::new(items).block(block).highlight_style(Theme::SELECTED).highlight_symbol("▶ ");
    let mut state = app.saved_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let msg = if app.status_msg.is_empty() {
        Line::from(vec![
            Span::styled(" r scan ", Theme::SELECTED),
            Span::styled(" Enter connect ", Theme::SUCCESS),
            Span::styled(" d disconnect ", Theme::WARN),
            Span::styled(" Tab switch ", Theme::MUTED),
            Span::styled(" q quit ", Theme::MUTED),
        ])
    } else {
        Line::from(Span::styled(&app.status_msg, Theme::SUCCESS))
    };
    let block = Block::default().borders(Borders::ALL).border_style(Theme::MUTED);
    let para = Paragraph::new(msg).block(block);
    f.render_widget(para, area);
}

fn main() -> std::io::Result<()> {
    if !nmcli_available() {
        eprintln!("✗ nmcli not found. Install NetworkManager.");
        std::process::exit(1);
    }

    let mut app = App::new();
    app.networks = scan_networks();
    app.saved = get_saved();
    if !app.networks.is_empty() {
        app.net_state.select(Some(0));
    }
    if !app.saved.is_empty() {
        app.saved_state.select(Some(0));
    }

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => {
                        app.active = match &app.active {
                            ActivePanel::Networks => ActivePanel::Saved,
                            ActivePanel::Saved => ActivePanel::Networks,
                        };
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        match &app.active {
                            ActivePanel::Networks => {
                                let i = app.net_state.selected().unwrap_or(0).saturating_sub(1);
                                app.net_state.select(Some(i));
                            }
                            ActivePanel::Saved => {
                                let i = app.saved_state.selected().unwrap_or(0).saturating_sub(1);
                                app.saved_state.select(Some(i));
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        match &app.active {
                            ActivePanel::Networks => {
                                let i = (app.net_state.selected().unwrap_or(0) + 1)
                                    .min(app.networks.len().saturating_sub(1));
                                app.net_state.select(Some(i));
                            }
                            ActivePanel::Saved => {
                                let i = (app.saved_state.selected().unwrap_or(0) + 1)
                                    .min(app.saved.len().saturating_sub(1));
                                app.saved_state.select(Some(i));
                            }
                        }
                    }
                    KeyCode::Enter => {
                        match &app.active {
                            ActivePanel::Networks => {
                                if let Some(i) = app.net_state.selected() {
                                    let ssid = app.networks[i].ssid.clone();
                                    app.status_msg = format!("Connecting to {}...", ssid);
                                    terminal.draw(|f| ui(f, &mut app))?;
                                    match connect(&ssid) {
                                        Ok(msg) => {
                                            app.status_msg = msg;
                                            app.networks = scan_networks();
                                            app.saved = get_saved();
                                        }
                                        Err(e) => app.status_msg = format!("✗ {}", e),
                                    }
                                }
                            }
                            ActivePanel::Saved => {
                                if let Some(i) = app.saved_state.selected() {
                                    let ssid = app.saved[i].clone();
                                    app.status_msg = format!("Connecting to {}...", ssid);
                                    terminal.draw(|f| ui(f, &mut app))?;
                                    match connect(&ssid) {
                                        Ok(msg) => {
                                            app.status_msg = msg;
                                            app.networks = scan_networks();
                                        }
                                        Err(e) => app.status_msg = format!("✗ {}", e),
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        match disconnect() {
                            Ok(msg) => {
                                app.status_msg = msg;
                                app.networks = scan_networks();
                            }
                            Err(e) => app.status_msg = format!("✗ {}", e),
                        }
                    }
                    KeyCode::Char('r') => {
                        app.scanning = true;
                        terminal.draw(|f| ui(f, &mut app))?;
                        app.networks = scan_networks();
                        app.saved = get_saved();
                        app.scanning = false;
                        app.status_msg = format!("Found {} networks", app.networks.len());
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
