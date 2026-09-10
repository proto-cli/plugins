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
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
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
    const PAIRED: Style = Style::new().fg(Color::Green);
    const UNPAIRED: Style = Style::new().fg(Color::DarkGray);
}

#[derive(Debug, Clone)]
struct BtDevice {
    name: String,
    mac: String,
    paired: bool,
    connected: bool,
    trusted: bool,
}

enum ActivePanel {
    Devices,
    Actions,
}

struct App {
    devices: Vec<BtDevice>,
    dev_state: ListState,
    action_state: ListState,
    active: ActivePanel,
    status_msg: String,
    scanning: bool,
}

impl App {
    fn new() -> Self {
        Self {
            devices: Vec::new(),
            dev_state: ListState::default(),
            action_state: ListState::default(),
            active: ActivePanel::Devices,
            status_msg: String::new(),
            scanning: false,
        }
    }
}

fn btctl(args: &[&str]) -> String {
    let out = Command::new("bluetoothctl")
        .args(args)
        .output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}

fn btctl_bool(args: &[&str]) -> bool {
    Command::new("bluetoothctl")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn is_powered() -> bool {
    let out = btctl(&["show"]);
    out.contains("Powered: yes")
}

fn toggle_power(on: bool) {
    if on {
        btctl(&["power", "on"]);
    } else {
        btctl(&["power", "off"]);
    }
}

fn scan_devices() -> Vec<BtDevice> {
    btctl(&["--timeout", "10", "devices", "Paired"]);
    btctl(&["--timeout", "10", "devices", "Scanned"]);
    let out = btctl(&["devices"]);
    let mut devices = Vec::new();
    for line in out.lines() {
        if line.starts_with("Device ") {
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() >= 3 {
                let mac = parts[1].to_string();
                let name = parts[2].to_string();
                let info = btctl(&["info", &mac]);
                let paired = info.contains("Paired: yes");
                let connected = info.contains("Connected: yes");
                let trusted = info.contains("Trusted: yes");
                devices.push(BtDevice { name, mac, paired, connected, trusted });
            }
        }
    }
    devices.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.paired.cmp(&a.paired)));
    devices
}

fn pair_device(mac: &str) -> String {
    if btctl_bool(&["pair", mac]) {
        "Paired successfully".into()
    } else {
        "Pairing failed".into()
    }
}

fn connect_device(mac: &str) -> String {
    if btctl_bool(&["connect", mac]) {
        "Connected".into()
    } else {
        "Connection failed".into()
    }
}

fn disconnect_device(mac: &str) -> String {
    if btctl_bool(&["disconnect", mac]) {
        "Disconnected".into()
    } else {
        "Disconnect failed".into()
    }
}

fn remove_device(mac: &str) -> String {
    if btctl_bool(&["remove", mac]) {
        "Device removed".into()
    } else {
        "Remove failed".into()
    }
}

fn trusted_icon(dev: &BtDevice) -> &'static str {
    if dev.connected { "●" }
    else if dev.paired { "○" }
    else { "·" }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(f.area());

    let main = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(chunks[0]);

    render_devices(f, app, main[0]);
    render_actions(f, app, main[1]);
    render_status(f, app, chunks[1]);
}

fn render_devices(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app.devices.iter().enumerate().map(|(i, dev)| {
        let icon = trusted_icon(dev);
        let style = if app.dev_state.selected() == Some(i) {
            Theme::SELECTED
        } else if dev.connected {
            Theme::PAIRED
        } else if dev.paired {
            Theme::VALUE
        } else {
            Theme::UNPAIRED
        };
        let status = if dev.connected { " connected" } else if dev.paired { " paired" } else { "" };
        ListItem::new(Line::from(vec![
            Span::styled(format!(" {} {} ", icon, dev.name), style),
            Span::styled(&dev.mac, Theme::MUTED),
            Span::styled(status, if dev.connected { Theme::SUCCESS } else { Theme::MUTED }),
        ]))
    }).collect();

    let power = if is_powered() { "ON" } else { "OFF" };
    let power_style = if is_powered() { Theme::SUCCESS } else { Theme::ERROR };

    let block = Block::default()
        .title(Span::styled(
            format!(" Bluetooth [power: {}] ", power),
            if is_powered() { Theme::SUCCESS } else { Theme::ERROR },
        ))
        .borders(Borders::ALL)
        .border_style(Theme::ACCENT);

    let list = List::new(items).block(block).highlight_style(Theme::SELECTED).highlight_symbol("▶ ");
    let mut state = app.dev_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_actions(f: &mut Frame, app: &mut App, area: Rect) {
    let actions = vec![
        "Connect",
        "Disconnect",
        "Pair",
        "Trust",
        "Remove",
        "Toggle Power",
        "Scan",
    ];

    let items: Vec<ListItem> = actions.iter().enumerate().map(|(i, action)| {
        let style = if app.action_state.selected() == Some(i) {
            Theme::SELECTED
        } else {
            Theme::VALUE
        };
        ListItem::new(Line::from(Span::styled(format!(" {} ", action), style)))
    }).collect();

    let block = Block::default()
        .title(Span::styled(" Actions ", Theme::ACCENT))
        .borders(Borders::ALL)
        .border_style(Theme::MUTED);

    let list = List::new(items).block(block).highlight_style(Theme::SELECTED).highlight_symbol("▶ ");
    let mut state = app.action_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let msg = if app.status_msg.is_empty() {
        Line::from(vec![
            Span::styled(" r scan ", Theme::SELECTED),
            Span::styled(" Enter action ", Theme::SUCCESS),
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

fn do_action(app: &mut App) {
    let action_idx = match app.action_state.selected() {
        Some(i) => i,
        None => return,
    };
    let dev_idx = match app.dev_state.selected() {
        Some(i) => i,
        None => { app.status_msg = "Select a device first".into(); return; }
    };
    let mac = app.devices[dev_idx].mac.clone();

    let result = match action_idx {
        0 => connect_device(&mac),
        1 => disconnect_device(&mac),
        2 => pair_device(&mac),
        3 => {
            btctl_bool(&["trust", &mac]);
            "Trusted".into()
        }
        4 => remove_device(&mac),
        5 => {
            let powered = is_powered();
            toggle_power(!powered);
            format!("Power {}", if powered { "OFF" } else { "ON" })
        }
        6 => {
            app.scanning = true;
            "Scanning...".into()
        }
        _ => return,
    };

    app.status_msg = result;

    if action_idx == 6 {
        app.devices = scan_devices();
        app.scanning = false;
        app.status_msg = format!("Found {} devices", app.devices.len());
    }
    if action_idx == 0 || action_idx == 1 || action_idx == 4 {
        app.devices = scan_devices();
    }
}

fn main() -> std::io::Result<()> {
    if !Command::new("which")
        .arg("bluetoothctl")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        eprintln!("✗ bluetoothctl not found. Install BlueZ.");
        std::process::exit(1);
    }

    let mut app = App::new();
    app.devices = scan_devices();
    if !app.devices.is_empty() {
        app.dev_state.select(Some(0));
    }
    app.action_state.select(Some(0));

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
                            ActivePanel::Devices => ActivePanel::Actions,
                            ActivePanel::Actions => ActivePanel::Devices,
                        };
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        match &app.active {
                            ActivePanel::Devices => {
                                let i = app.dev_state.selected().unwrap_or(0).saturating_sub(1);
                                app.dev_state.select(Some(i));
                            }
                            ActivePanel::Actions => {
                                let i = app.action_state.selected().unwrap_or(0).saturating_sub(1);
                                app.action_state.select(Some(i));
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        match &app.active {
                            ActivePanel::Devices => {
                                let i = (app.dev_state.selected().unwrap_or(0) + 1)
                                    .min(app.devices.len().saturating_sub(1));
                                app.dev_state.select(Some(i));
                            }
                            ActivePanel::Actions => {
                                let i = (app.action_state.selected().unwrap_or(0) + 1).min(6);
                                app.action_state.select(Some(i));
                            }
                        }
                    }
                    KeyCode::Enter => {
                        do_action(&mut app);
                    }
                    KeyCode::Char('r') => {
                        app.scanning = true;
                        terminal.draw(|f| ui(f, &mut app))?;
                        app.devices = scan_devices();
                        app.scanning = false;
                        app.status_msg = format!("Found {} devices", app.devices.len());
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
