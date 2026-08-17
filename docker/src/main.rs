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
fn label_value(label: &str, value: &str) -> String { format!("  {}: {}", label.style(Theme::MUTED), value.style(Theme::ACCENT)) }
fn which(binary: &str) -> bool { Command::new("which").arg(binary).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false) }

#[derive(Parser)]
#[command(name = "docker", about = "Docker management")]
struct Cli {
    #[command(subcommand)]
    action: DockerAction,
}

#[derive(Subcommand)]
enum DockerAction {
    Containers,
    PruneSafe,
}

struct Container { id: String, image: String, status: String, names: String }

fn main() {
    let cli = Cli::parse();
    if !which("docker") { eprintln!("{} docker required.", error("")); return; }
    match cli.action {
        DockerAction::Containers => interactive(),
        DockerAction::PruneSafe => prune_safe(),
    }
}

fn list_containers(all: bool) -> Option<Vec<Container>> {
    let mut cmd = Command::new("docker"); cmd.args(["ps"]);
    if all { cmd.arg("-a"); }
    let out = cmd.args(["--format", "{{.ID}}|{{.Image}}|{{.Status}}|{{.Names}}"]).output().ok()?;
    if !out.status.success() { return None; }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut list = Vec::new();
    for line in text.lines() {
        let mut it = line.splitn(4, '|');
        let (Some(id), Some(image), Some(status), Some(names)) = (it.next(), it.next(), it.next(), it.next()) else { continue; };
        list.push(Container { id: id.to_string(), image: image.to_string(), status: status.to_string(), names: names.to_string() });
    }
    Some(list)
}

fn interactive() {
    println!("{}", header("Docker Containers")); println!("{}", divider());
    loop {
        let list = match list_containers(true) { Some(l) => l, None => { eprintln!("{} Could not list containers.", error("")); return; } };
        let mut options: Vec<String> = Vec::new(); options.push("↻ Refresh".to_string());
        for c in &list {
            let status = if c.status.contains("Up") { c.status.style(Theme::SUCCESS).to_string() } else if c.status.contains("Exited") { c.status.style(Theme::MUTED).to_string() } else { c.status.style(Theme::WARN).to_string() };
            options.push(format!("{}  {}  [{}]", c.names.style(Theme::VALUE), c.image.dimmed(), status));
        }
        options.push("Done".to_string());
        let idx = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt("Select a container").items(&options).default(0).interact().unwrap_or(options.len() - 1);
        if idx == 0 { continue; } if idx >= options.len() - 1 { break; }
        act_on(&list[idx - 1]);
    }
}

fn act_on(c: &Container) {
    let actions = vec!["start", "stop", "restart", "logs", "inspect", "remove", "back"];
    let idx = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt(format!("{} ({})", c.names, c.id.chars().take(12).collect::<String>())).items(&actions).default(6).interact().unwrap_or(6);
    match actions[idx] {
        "start" => run_docker(&["start", &c.id]),
        "stop" => run_docker(&["stop", &c.id]),
        "restart" => run_docker(&["restart", &c.id]),
        "remove" => { if dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt(format!("Remove container {}?", c.names)).default(false).interact().unwrap_or(false) { run_docker(&["rm", "-f", &c.id]); } }
        "logs" => { println!(); let _ = Command::new("docker").args(["logs", "-f", "--tail", "40", &c.id]).status(); }
        "inspect" => { println!(); if let Ok(o) = Command::new("docker").args(["inspect", &c.id]).output() { println!("{}", String::from_utf8_lossy(&o.stdout)); } pause(); }
        _ => {}
    }
}

fn run_docker(args: &[&str]) { if let Err(e) = Command::new("docker").args(args).status() { eprintln!("  {} {}", error(""), e); } }
fn pause() { use std::io::Write; print!("  {} Press Enter to continue...", "▼".style(Theme::MUTED)); let _ = std::io::stdout().flush(); let mut buf = String::new(); let _ = std::io::stdin().read_line(&mut buf); }

fn git_branch() -> Option<String> {
    let out = Command::new("git").args(["branch", "--show-current"]).output().ok()?;
    if !out.status.success() { return None; }
    let b = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if b.is_empty() { None } else { Some(b) }
}

fn system_df() -> String { Command::new("docker").args(["system", "df"]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default() }

fn prune_safe() {
    println!("{}", header("Docker Prune (safe)")); println!("{}", divider());
    println!("  {} Cleaning dangling images, stopped containers, and unused volumes.\n", muted(""));
    let branch = git_branch();
    match &branch { Some(b) => println!("  {}", label_value("Protecting branch", b)), None => println!("  {} Not in a git repo — only truly dangling objects will be removed.", warn("")) }
    println!(); println!("  {} BEFORE:\n{}", "◉".style(Theme::ACCENT), indent_df(&system_df()));
    let images: Vec<String> = docker_ids(&["images", "-q", "-f", "dangling=true"]);
    let containers: Vec<String> = stopped_containers(&branch);
    let volumes: Vec<String> = docker_ids(&["volume", "ls", "-q", "-f", "dangling=true"]);
    let count = images.len() + containers.len() + volumes.len();
    if count == 0 { println!("{} Nothing to prune.", success("")); return; }
    println!("\n  {} will remove:\n   • {} dangling image(s)\n   • {} stopped container(s)\n   • {} unused volume(s)", warn(""), images.len(), containers.len(), volumes.len());
    if !dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt("Proceed with prune-safe?").default(false).interact().unwrap_or(false) { println!("{} Aborted.", muted("")); return; }
    let mut removed = 0;
    for id in &images { if docker_rm(&["image", "rm", id]) { removed += 1; } }
    for c in &containers { if docker_rm(&["rm", "-f", c]) { removed += 1; } }
    for v in &volumes { if docker_rm(&["volume", "rm", v]) { removed += 1; } }
    println!(); println!("  {} AFTER:\n{}", "◉".style(Theme::ACCENT), indent_df(&system_df()));
    println!("\n  {} Removed {} object(s).", success(""), removed);
}

fn docker_ids(args: &[&str]) -> Vec<String> { match Command::new("docker").args(args).output() { Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(), _ => Vec::new() } }
fn docker_rm(args: &[&str]) -> bool { Command::new("docker").args(args).output().map(|o| o.status.success()).unwrap_or(false) }

fn stopped_containers(branch: &Option<String>) -> Vec<String> {
    let out = Command::new("docker").args(["ps", "-a", "-f", "status=exited", "--format", "{{.ID}}|{{.Image}}|{{.Names}}"]).output();
    let text = match out { Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(), _ => return Vec::new() };
    let mut keep = Vec::new();
    for line in text.lines() {
        let mut it = line.splitn(3, '|');
        let (Some(id), Some(image), Some(names)) = (it.next(), it.next(), it.next()) else { continue; };
        let protected = match branch { Some(b) => names.contains(b) || image.contains(b), None => false };
        if !protected { keep.push(id.to_string()); }
    }
    keep
}

fn indent_df(df: &str) -> String { df.lines().map(|l| format!("    {}", l)).collect::<Vec<_>>().join("\n") }
