use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::Repository;
use notify::{Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    const LIVE: Style = Style::new().fg(Color::Green).add_modifier(Modifier::BOLD);
}

#[derive(Debug, Clone)]
struct FileEntry {
    path: PathBuf,
    rel: String,
    modified: bool,
}

#[derive(Debug, Clone)]
struct LogEntry {
    hash: String,
    short_hash: String,
    author: String,
    date: String,
    summary: String,
}

enum ActivePane {
    Files,
    History,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ViewMode {
    Log,
    Diff,
}

struct App {
    files: Vec<FileEntry>,
    file_state: ListState,
    history: Vec<LogEntry>,
    history_scroll: usize,
    active: ActivePane,
    view_mode: ViewMode,
    watch_events: Vec<String>,
    watching: bool,
    repo_root: PathBuf,
    diff_output: String,
}

impl App {
    fn new(repo_root: PathBuf) -> Self {
        let files = list_tracked_files(&repo_root);
        let mut state = ListState::default();
        if !files.is_empty() {
            state.select(Some(0));
        }
        Self {
            files,
            file_state: state,
            history: Vec::new(),
            history_scroll: 0,
            active: ActivePane::Files,
            view_mode: ViewMode::Log,
            watch_events: Vec::new(),
            watching: false,
            repo_root,
            diff_output: String::new(),
        }
    }

    fn load_history(&mut self) {
        if let Some(idx) = self.file_state.selected() {
            if let Some(file) = self.files.get(idx) {
                self.history = git_log_for_file(&self.repo_root, &file.path);
                self.history_scroll = 0;
                self.diff_output.clear();
            }
        }
    }

    fn load_diff(&mut self, commit_idx: usize) {
        if let Some(entry) = self.history.get(commit_idx) {
            self.diff_output = git_diff_for_commit(&self.repo_root, &entry.hash, &self.files[self.file_state.selected().unwrap()].rel);
            self.history_scroll = 0;
        }
    }
}

fn list_tracked_files(repo_root: &Path) -> Vec<FileEntry> {
    let output = std::process::Command::new("git")
        .args(["ls-files"])
        .current_dir(repo_root)
        .output();
    let text = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return Vec::new(),
    };
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| FileEntry {
            path: repo_root.join(l),
            rel: l.to_string(),
            modified: false,
        })
        .collect()
}

fn git_log_for_file(repo_root: &Path, file: &Path) -> Vec<LogEntry> {
    let rel = file.strip_prefix(repo_root).unwrap_or(file);
    let output = std::process::Command::new("git")
        .args([
            "log", "--format=%H|%h|%an|%ar|%s", "-30", "--", rel.to_str().unwrap_or(""),
        ])
        .current_dir(repo_root)
        .output();
    let text = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return Vec::new(),
    };
    text.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(5, '|');
            Some(LogEntry {
                hash: parts.next()?.to_string(),
                short_hash: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                summary: parts.next()?.to_string(),
            })
        })
        .collect()
}

fn git_diff_for_commit(repo_root: &Path, commit: &str, file: &str) -> String {
    let output = std::process::Command::new("git")
        .args(["diff", &format!("{}~1", commit), commit, "--", file])
        .current_dir(repo_root)
        .output();
    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            if stdout.is_empty() && stderr.contains("unknown revision") {
                // First commit — use show instead
                let out2 = std::process::Command::new("git")
                    .args(["show", "--format=", "--stat", commit, "--", file])
                    .current_dir(repo_root)
                    .output();
                match out2 {
                    Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                    Err(_) => "Could not generate diff".to_string(),
                }
            } else {
                stdout.to_string()
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}

fn git_diff_stats(repo_root: &Path) -> BTreeMap<String, char> {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root)
        .output();
    let text = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return BTreeMap::new(),
    };
    let mut map = BTreeMap::new();
    for line in text.lines() {
        if line.len() >= 3 {
            let status = line.as_bytes()[1] as char;
            let path = line[3..].trim();
            map.insert(path.to_string(), status);
        }
    }
    map
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    let main = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(chunks[0]);

    render_file_list(f, app, main[0]);
    render_history_pane(f, app, main[1]);
    render_status_bar(f, app, chunks[1]);
}

fn render_file_list(f: &mut Frame, app: &mut App, area: Rect) {
    let git_status = git_diff_stats(&app.repo_root);

    let items: Vec<ListItem> = app
        .files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let is_selected = app.file_state.selected() == Some(i);
            let git_char = git_status.get(&file.rel).map(|c| match c {
                'M' => (" ●", Theme::WARN),
                'A' => (" +", Theme::SUCCESS),
                'D' => (" −", Theme::ERROR),
                '?' => (" ?", Theme::MUTED),
                _ => (" ?", Theme::MUTED),
            });

            let mut spans = vec![Span::styled(
                format!(" {} ", file.rel),
                if is_selected { Theme::SELECTED } else { Theme::VALUE },
            )];

            if let Some((ch, style)) = git_char {
                spans.push(Span::styled(ch, style));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            format!(" Files ({}) ", app.files.len()),
            Theme::ACCENT,
        ))
        .borders(Borders::ALL)
        .border_style(Theme::ACCENT);

    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::SELECTED)
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.file_state);
}

fn render_history_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(area);

    let title = match app.view_mode {
        ViewMode::Log => format!(
            " History — {} ",
            app.files
                .get(app.file_state.selected().unwrap_or(0))
                .map(|f| f.rel.as_str())
                .unwrap_or("")
        ),
        ViewMode::Diff => format!(
            " Diff — {} ",
            app.files
                .get(app.file_state.selected().unwrap_or(0))
                .map(|f| f.rel.as_str())
                .unwrap_or("")
        ),
    };

    match app.view_mode {
        ViewMode::Log => {
            let items: Vec<ListItem> = app
                .history
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let style = if i == app.history_scroll {
                        Theme::SELECTED
                    } else {
                        Theme::VALUE
                    };
                    ListItem::new(Line::from(vec![
                        Span::styled(format!(" {} ", entry.short_hash), Theme::ACCENT),
                        Span::styled(format!("{:<20}", entry.author), Theme::MUTED),
                        Span::styled(format!("{:<14}", entry.date), Theme::MUTED),
                        Span::styled(&entry.summary, style),
                    ]))
                })
                .collect();

            let block = Block::default()
                .title(Span::styled(title, Theme::ACCENT))
                .borders(Borders::ALL)
                .border_style(Theme::ACCENT);

            let list = List::new(items)
                .block(block)
                .highlight_style(Theme::SELECTED)
                .highlight_symbol("▶ ");

            let mut state = ListState::default();
            state.select(Some(app.history_scroll));
            f.render_stateful_widget(list, chunks[0], &mut state);
        }
        ViewMode::Diff => {
            let lines: Vec<Line> = app
                .diff_output
                .lines()
                .map(|l| {
                    let style = if l.starts_with('+') && !l.starts_with("+++") {
                        Theme::SUCCESS
                    } else if l.starts_with('-') && !l.starts_with("---") {
                        Theme::ERROR
                    } else if l.starts_with("@@") {
                        Theme::WARN
                    } else {
                        Theme::VALUE
                    };
                    Line::from(Span::styled(l.to_string(), style))
                })
                .collect();

            let block = Block::default()
                .title(Span::styled(title, Theme::ACCENT))
                .borders(Borders::ALL)
                .border_style(Theme::ACCENT);

            let para = Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false })
                .scroll((app.history_scroll as u16, 0));

            f.render_widget(para, chunks[0]);
        }
    }
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mode = match &app.active {
        ActivePane::Files => "FILES",
        ActivePane::History => "HISTORY",
    };
    let view = match app.view_mode {
        ViewMode::Log => "log",
        ViewMode::Diff => "diff",
    };
    let watch = if app.watching { "● LIVE" } else { "○ off" };
    let watch_style = if app.watching { Theme::LIVE } else { Theme::MUTED };

    let status = Line::from(vec![
        Span::styled(
            format!(" {} ", mode),
            Theme::SELECTED,
        ),
        Span::styled(
            format!(" view:{} ", view),
            Theme::MUTED,
        ),
        Span::styled(
            format!(" {} ", watch),
            watch_style,
        ),
        Span::styled("  ← → tabs │ d diff │ w watch │ q quit", Theme::MUTED),
    ]);

    let block = Block::default().borders(Borders::ALL).border_style(Theme::MUTED);
    let para = Paragraph::new(status).block(block);
    f.render_widget(para, area);
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let watch_mode = args.iter().any(|a| a == "--watch" || a == "-w");
    let target = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Find repo root
    let repo_root = if let Ok(repo) = Repository::discover(&target) {
        repo.workdir()
            .unwrap_or(Path::new("."))
            .to_path_buf()
    } else {
        eprintln!("✗ Not inside a git repository");
        std::process::exit(1);
    };

    let mut app = App::new(repo_root.clone());
    if !app.files.is_empty() {
        app.load_history();
    }

    // File watcher
    let live_events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = live_events.clone();
    let watch_path = repo_root.clone();

    let mut _watcher: Option<RecommendedWatcher> = None;
    if watch_mode {
        let mut watcher = notify::recommended_watcher(move |res: Result<NotifyEvent, _>| {
            if let Ok(evt) = res {
                for path in &evt.paths {
                    if let Some(name) = path.file_name() {
                    let mut events = events_clone.lock().unwrap();
                    events.push(format!(
                        "{} {}",
                        chrono::Local::now().format("%H:%M:%S"),
                        path.display()
                    ));
                    let len = events.len();
                    if len > 50 {
                        events.drain(0..len - 50);
                    }
                    }
                }
            }
        })
        .expect("Failed to create file watcher");

        watcher
            .watch(&watch_path, RecursiveMode::Recursive)
            .expect("Failed to start watching");
        app.watching = true;
        _watcher = Some(watcher);
    }

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
    terminal.draw(|f| {
        // Update live events
        {
            let evts = live_events.lock().unwrap();
            app.watch_events = evts.clone();
        }
        ui(f, &mut app);
    })?;

    let timeout = tick_rate.saturating_sub(last_tick.elapsed());
    if event::poll(timeout)? {
        if let Event::Key(key) = event::read()? {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                break;
            }
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Tab => {
                    app.active = match &app.active {
                        ActivePane::Files => ActivePane::History,
                        ActivePane::History => ActivePane::Files,
                    };
                }
                KeyCode::Up | KeyCode::Char('k') => match &app.active {
                    ActivePane::Files => {
                        let i = app.file_state.selected().unwrap_or(0).saturating_sub(1);
                        app.file_state.select(Some(i));
                        app.load_history();
                    }
                    ActivePane::History => {
                        app.history_scroll = app.history_scroll.saturating_sub(1);
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => match &app.active {
                    ActivePane::Files => {
                        let i = (app.file_state.selected().unwrap_or(0) + 1)
                            .min(app.files.len().saturating_sub(1));
                        app.file_state.select(Some(i));
                        app.load_history();
                    }
                    ActivePane::History => {
                        let max = match app.view_mode {
                            ViewMode::Log => app.history.len().saturating_sub(1),
                            ViewMode::Diff => {
                                let h = app.diff_output.lines().count();
                                if h > 5 { h - 5 } else { 0 }
                            }
                        };
                        app.history_scroll = (app.history_scroll + 1).min(max);
                    }
                },
                KeyCode::Char('d') => {
                    if app.view_mode == ViewMode::Log {
                        app.view_mode = ViewMode::Diff;
                        app.load_diff(app.history_scroll);
                    } else {
                        app.view_mode = ViewMode::Log;
                    }
                }
                KeyCode::Enter => {
                    if app.view_mode == ViewMode::Log && !app.history.is_empty() {
                        app.load_diff(app.history_scroll);
                        app.view_mode = ViewMode::Diff;
                    }
                }
                KeyCode::Esc => {
                    app.view_mode = ViewMode::Log;
                }
                _ => {}
            }
        }
    }

    if last_tick.elapsed() >= tick_rate {
        last_tick = Instant::now();
    }
}

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
