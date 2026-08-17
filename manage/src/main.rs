use clap::{Subcommand, Parser};
use owo_colors::OwoColorize;
use std::process::Command;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}
fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn warn(s: &str) -> String { format!("{} {}", "⚠".style(Theme::WARN), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(40).dimmed().to_string() }

struct Spinner;
impl Spinner {
    fn new(msg: &str) -> Self { print!("  ⠋ {}...", msg); std::io::Write::flush(&mut std::io::stdout()).ok(); Self }
    fn done(&self, msg: &str) { println!("\r  ✔ {}", msg); }
    fn fail(&self, msg: &str) { println!("\r  ✗ {}", msg); }
}

fn repo_dir() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let p = exe.parent()?;
    let p = p.parent()?;
    Some(p.display().to_string())
}

#[derive(Parser)]
#[command(name = "manage", about = "CLI self-management — update, uninstall, reset")]
struct Cli {
    #[command(subcommand)]
    action: ManageAction,
}

#[derive(Subcommand)]
enum ManageAction {
    Update,
    Uninstall {
        #[arg(long, help = "Also delete the cloned repository")]
        purge: bool,
    },
    Reset,
}

fn main() {
    let cli = Cli::parse();
    match cli.action {
        ManageAction::Update => update(),
        ManageAction::Uninstall { purge } => uninstall(purge),
        ManageAction::Reset => reset(),
    }
}

fn update() {
    println!("{}", header("Proto Update"));
    println!("{}", divider());
    let repo = match repo_dir() { Some(r) => r, None => { eprintln!("{} Cannot find repo directory.", error("")); return; } };
    println!("  {}\n", muted(&format!("Repo: {}", repo)));
    let sp = Spinner::new("git pull origin master...");
    match Command::new("git").args(["-C", &repo, "pull", "origin", "master"]).output() {
        Ok(o) if o.status.success() => {
            sp.done("Git pull complete");
            let stdout = String::from_utf8_lossy(&o.stdout);
            if !stdout.contains("Already up to date") { println!("  {} New commits pulled.", muted("")); }
        }
        Ok(o) => { sp.fail("Git pull failed"); eprintln!("  {}", String::from_utf8_lossy(&o.stderr)); return; }
        Err(e) => { sp.fail(&format!("Git pull error: {}", e)); return; }
    }
    let sp = Spinner::new("cargo build --release...");
    match Command::new("cargo").args(["build", "--release"]).current_dir(&repo).output() {
        Ok(o) if o.status.success() => { sp.done("Build complete"); }
        Ok(o) => { sp.fail("Build failed"); eprintln!("  {}", String::from_utf8_lossy(&o.stderr)); return; }
        Err(e) => { sp.fail(&format!("Build error: {}", e)); return; }
    }
    let current = std::env::current_exe().unwrap_or_default();
    let new_binary = format!("{}/target/release/proto", repo);
    println!("  {} Installing {} -> {}", muted(""), new_binary, current.display());
    if let Err(e) = std::fs::copy(&new_binary, &current) {
        eprintln!("  {} Failed to install: {}", error(""), e);
        return;
    }
    println!("  {} Update complete.", success(""));
}

fn uninstall(purge: bool) {
    println!("{}", header("Proto Uninstall"));
    println!("{}", divider());
    let current = std::env::current_exe().unwrap_or_default();
    println!("  Binary: {}", current.display().to_string().style(Theme::VALUE));
    let confirm = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt("Remove the proto binary?").default(false).interact().unwrap_or(false);
    if confirm {
        if let Err(e) = std::fs::remove_file(&current) { eprintln!("  {} Failed to remove binary: {}", error(""), e); }
        else { println!("  {} Binary removed.", success("")); }
    } else { println!("  {} Cancelled.", muted("")); return; }
    if purge {
        let repo = match repo_dir() { Some(r) => r, None => return };
        println!("\n  {} Purging repository at {}", warn(""), repo);
        let confirm = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt("Remove the entire proto repository?").default(false).interact().unwrap_or(false);
        if confirm {
            if let Err(e) = std::fs::remove_dir_all(&repo) { eprintln!("  {} Failed to remove repo: {}", error(""), e); }
            else { println!("  {} Repository removed.", success("")); }
        } else { println!("  {} Repo kept.", muted("")); }
    }
}

fn reset() {
    println!("{}", header("Proto Reset"));
    println!("{}", divider());
    let dirs = vec![dirs::config_dir().map(|d| d.join("proto")), dirs::data_local_dir().map(|d| d.join("proto")), dirs::home_dir().map(|d| d.join(".proto"))];
    let confirm = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt("Remove all proto config and state directories?").default(false).interact().unwrap_or(false);
    if !confirm { println!("  {} Cancelled.", muted("")); return; }
    for dir in dirs.into_iter().flatten() {
        if dir.exists() { println!("  {} Removing {}...", muted(""), dir.display()); let _ = std::fs::remove_dir_all(&dir); }
    }
    println!("  {} Config and state reset.", success(""));
}
