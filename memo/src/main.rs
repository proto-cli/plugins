use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(text: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), text.style(Theme::HEADER))
}
fn success(msg: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), msg)
}
fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}
fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}
fn divider() -> String {
    "─".repeat(40).dimmed().to_string()
}

#[derive(Parser)]
#[command(name = "memo", about = "Location-aware scratchpad memos")]
struct Cli {
    #[command(subcommand)]
    action: MemoAction,
}

#[derive(Subcommand, Debug, Clone)]
enum MemoAction {
    #[command(about = "Show all memos")]
    List,
    #[command(about = "Add a new memo")]
    Add {
        #[arg(required = true, value_name = "TEXT")]
        text: String,
    },
    #[command(about = "Clear all memos")]
    Clear,
}

fn main() {
    let cli = Cli::parse();
    match cli.action {
        MemoAction::List => list(),
        MemoAction::Add { ref text } => add(text),
        MemoAction::Clear => clear(),
    }
}

fn memo_path() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_default().join(".proto")
}

fn list() {
    let path = memo_path();
    if !path.exists() {
        println!("{} No memos here yet.", "  ".dimmed());
        println!("  {}", "memo add \"your note here\"".style(Theme::MUTED));
        return;
    }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    if content.trim().is_empty() {
        println!("{} No memos.", "  ".dimmed());
        return;
    }

    println!(
        "{} {}",
        "◆".style(Theme::ACCENT),
        format!("Memos — {}", path.to_string_lossy()).style(Theme::HEADER)
    );
    println!("{}", divider());
    println!("{}", content);
    println!("{}", divider());
}

fn add(text: &str) {
    let path = memo_path();
    let now = chrono_now();

    let entry = format!("[{}] {}\n", now, text);

    let existing = if path.exists() {
        std::fs::read_to_string(&path).unwrap_or_default()
    } else {
        String::new()
    };

    std::fs::write(&path, format!("{}{}", existing, entry)).unwrap();
    println!(
        "{} {}",
        "✦".style(Theme::SUCCESS),
        text.style(Theme::ACCENT)
    );
    println!(
        "  {} ({})",
        "in".dimmed(),
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    );
}

fn clear() {
    use dialoguer::Confirm;
    let path = memo_path();
    if !path.exists() {
        println!("{} No memos to clear.", "  ".dimmed());
        return;
    }

    let confirm = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Delete all memos in this directory?")
        .default(false)
        .interact()
        .unwrap_or(false);

    if confirm {
        std::fs::remove_file(&path).unwrap();
        println!("{} Memos cleared.", success(""));
    } else {
        println!("{}", "Aborted.".style(Theme::MUTED));
    }
}

fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let s = dur.as_secs() as i64;
    let _days = s / 86400;
    let rem = s % 86400;
    let h = rem / 3600;
    let mi = (rem % 3600) / 60;
    format!("{:02}:{:02}", h, mi)
}
