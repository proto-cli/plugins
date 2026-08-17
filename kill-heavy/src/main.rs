use clap::Parser;
use owo_colors::OwoColorize;
use std::process::Command;

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
fn warn(msg: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), msg)
}
fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}
fn divider() -> String {
    "─".repeat(40).dimmed().to_string()
}
fn label_value(label: &str, value: &str) -> String {
    format!(
        "{} {}",
        format!("{:>14}:", label).style(Theme::LABEL),
        value.style(Theme::VALUE)
    )
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", v, UNITS[i])
}

#[derive(Parser)]
#[command(name = "kill-heavy", about = "Find and kill heavy processes")]
struct Cli {
    /// Minimum CPU% threshold
    #[arg(long, default_value = "5.0")]
    cpu: f64,
    /// Minimum RAM MB threshold
    #[arg(long, default_value = "100")]
    mem: u64,
    /// Show all processes (not just heavy ones)
    #[arg(long)]
    all: bool,
    /// Run in serve mode
    #[arg(long)]
    serve: bool,
    /// Port for serve mode
    #[arg(long, default_value = "9102")]
    port: u16,
}

fn main() {
    let cli = Cli::parse();
    run(cli.cpu, cli.mem, cli.all, cli.serve, cli.port);
}

struct HeavyProc {
    pid: u32,
    name: String,
    cpu: f64,
    mem: f64,
    rss_kb: u64,
}

fn run(min_cpu: f64, min_mem_mb: u64, all: bool, serve: bool, port: u16) {
    if serve {
        serve_scan(min_cpu, min_mem_mb, all, port);
        return;
    }
    println!("{}", header("Heavy Process Scanner"));
    println!("{}", divider());

    let procs = match list_procs() {
        Some(p) => p,
        None => {
            eprintln!(
                "{} Could not read process list (ps required).",
                error("")
            );
            return;
        }
    };

    let self_pid = std::process::id();
    let heavy: Vec<HeavyProc> = procs
        .into_iter()
        .filter(|p| p.pid >= 100 && p.pid != self_pid)
        .filter(|p| {
            all || p.cpu >= min_cpu || (p.rss_kb as f64 / 1024.0) >= min_mem_mb as f64
        })
        .collect();

    if heavy.is_empty() {
        println!("{} No heavy processes found.", success(""));
        return;
    }

    let shown = heavy.len().min(15);
    println!(
        "  {} Top heavy processes (CPU >{}% or RAM >{}MB):\n",
        warn(""),
        min_cpu,
        min_mem_mb
    );

    use dialoguer::{Confirm, MultiSelect};
    let items: Vec<String> = heavy[..shown]
        .iter()
        .map(|p| {
            format!(
                "{}  {}  {}%  {:.1}%  {}",
                p.pid.to_string().style(Theme::ACCENT).bold(),
                p.name.style(Theme::VALUE),
                format!("{:>4}", p.cpu).dimmed(),
                p.mem,
                format_size(p.rss_kb * 1024).dimmed(),
            )
        })
        .collect();

    let selected = MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select processes to kill (space to toggle, enter to confirm)")
        .items(&items)
        .interact()
        .unwrap_or_default();

    if selected.is_empty() {
        println!("{} Nothing selected.", muted(""));
        return;
    }

    let targets: Vec<&HeavyProc> = selected.iter().map(|&i| &heavy[i]).collect();
    println!();
    for t in &targets {
        println!(
            "  {} {} (pid {})",
            "✗".style(Theme::WARN),
            t.name,
            t.pid
        );
    }

    let proceed = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Kill these processes?")
        .default(false)
        .interact()
        .unwrap_or(false);
    if !proceed {
        println!("{} Aborted.", muted(""));
        return;
    }

    let mut killed = 0;
    for t in targets {
        match kill_proc(t.pid) {
            true => {
                println!(
                    "  {} Killed {} (pid {})",
                    success(""),
                    t.name,
                    t.pid
                );
                killed += 1;
            }
            false => println!(
                "  {} Failed to kill {} (pid {})",
                error(""),
                t.name,
                t.pid
            ),
        }
    }
    println!("\n  {} {} process(es) killed.", success(""), killed);
}

fn serve_scan(min_cpu: f64, min_mem_mb: u64, all: bool, _port: u16) {
    println!();
    let procs = list_procs().unwrap_or_default();
    let self_pid = std::process::id();
    let heavy: Vec<HeavyProc> = procs
        .into_iter()
        .filter(|p| p.pid >= 100 && p.pid != self_pid)
        .filter(|p| {
            all || p.cpu >= min_cpu || (p.rss_kb as f64 / 1024.0) >= min_mem_mb as f64
        })
        .collect();

    println!(
        "  {} Heavy process scan: {} processes found.",
        success(""),
        heavy.len()
    );
}

fn list_procs() -> Option<Vec<HeavyProc>> {
    let out = Command::new("ps")
        .args([
            "-e", "--no-headers", "-o", "pid,ppid,comm,pcpu,pmem,rss", "--sort=-pcpu",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut procs = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let pid: u32 = it.next()?.parse().ok()?;
        let _ppid: u32 = it.next()?.parse().ok()?;
        let name = it.next()?.to_string();
        let cpu: f64 = it.next()?.parse().ok()?;
        let mem: f64 = it.next()?.parse().ok()?;
        let rss_kb: u64 = it.next()?.parse().ok()?;
        procs.push(HeavyProc {
            pid,
            name,
            cpu,
            mem,
            rss_kb,
        });
    }
    Some(procs)
}

fn kill_proc(pid: u32) -> bool {
    if !Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return false;
    }
    std::thread::sleep(std::time::Duration::from_millis(800));
    if std::path::Path::new(&format!("/proc/{}/", pid)).exists() {
        Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else {
        true
    }
}
