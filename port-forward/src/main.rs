use clap::Parser;
use owo_colors::OwoColorize;
use std::net::{SocketAddr, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
fn label_value(label: &str, value: &str) -> String { format!("{} {}", format!("{:>14}:", label).style(Theme::LABEL), value.style(Theme::VALUE)) }
fn which(binary: &str) -> bool {
    Command::new("which").arg(binary).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

#[derive(Parser)]
#[command(name = "port-forward", about = "SSH port forwarding with auto-retry")]
struct Cli {
    spec: String,
    #[arg(long, default_value = "0", help = "Max retries (0 = unlimited)")]
    retries: usize,
    #[arg(long, default_value = "5", help = "Health check interval in seconds")]
    interval: u64,
}

fn main() {
    let cli = Cli::parse();
    let (local, ssh_target, remote) = match parse_spec(&cli.spec) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{} {} Use: {}", error(""), e, "port-forward 8080:user@host:5432".style(Theme::ACCENT));
            return;
        }
    };
    if !which("ssh") {
        eprintln!("{} ssh not found on PATH.", error(""));
        return;
    }
    println!("{}", header("SSH Port Forward"));
    println!("{}", divider());
    println!("  {}", label_value("Forward", &format!("127.0.0.1:{} → {}:{}", local, ssh_target, remote)));
    if cli.retries == 0 {
        println!("  {}", label_value("Retries", "unlimited (press Ctrl+C to stop)"));
    } else {
        println!("  {}", label_value("Retries", &cli.retries.to_string()));
    }
    let local = local as u16;
    if port_open(local) {
        println!("  {} Local port {} is already in use.", warn(""), local);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let mon = Arc::clone(&stop);
    let health = std::thread::spawn(move || {
        let mut last = false;
        loop {
            let ok = port_open(local);
            if ok != last {
                if ok {
                    println!("  {} Forward is UP ({}:{} reachable)", "▲".style(Theme::SUCCESS), "127.0.0.1", local);
                } else {
                    println!("  {} Forward is DOWN ({}:{})", "▼".style(Theme::ERROR), "127.0.0.1", local);
                }
                last = ok;
            }
            if mon.load(Ordering::Relaxed) { break; }
            std::thread::sleep(Duration::from_secs(cli.interval.max(1)));
        }
    });
    let mut attempt: usize = 0;
    let mut quick_exits = 0usize;
    loop {
        if attempt > 0 {
            if cli.retries != 0 && attempt > cli.retries {
                println!("  {} Max retries reached, giving up.", error(""));
                break;
            }
            println!("  {} Connection dropped, reconnecting in 3s... (attempt {}{})", "↻".style(Theme::WARN), attempt, if cli.retries == 0 { String::new() } else { format!("/{}", cli.retries) });
            std::thread::sleep(Duration::from_secs(3));
        }
        attempt += 1;
        let mut child = match spawn_ssh(local, &ssh_target, remote) {
            Ok(c) => c,
            Err(e) => { eprintln!("  {} Failed to launch ssh: {}", error(""), e); break; }
        };
        let started = Instant::now();
        let mut alive = true;
        while alive {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let quick = started.elapsed() < Duration::from_secs(10);
                    if quick { quick_exits += 1; } else { quick_exits = 0; }
                    if quick_exits >= 3 {
                        println!("  {} ssh keeps exiting immediately — check your auth key or host.", warn(""));
                    }
                    if status.code() == Some(255) {
                        println!("  {} ssh error (code 255).", error(""));
                    }
                    alive = false;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(500)),
                Err(_) => { alive = false; }
            }
        }
        let _ = child.kill();
    }
    stop.store(true, Ordering::Relaxed);
    let _ = health.join();
}

fn parse_spec(spec: &str) -> Result<(u16, &str, u16), String> {
    let (local, rest) = spec.split_once(':').ok_or_else(|| format!("Invalid spec '{}'", spec))?;
    let (host, remote) = rest.rsplit_once(':').ok_or_else(|| format!("Invalid spec '{}'", spec))?;
    if host.is_empty() || remote.is_empty() { return Err(format!("Invalid spec '{}'", spec)); }
    let l: u16 = local.parse().map_err(|_| format!("Invalid local port '{}'", local))?;
    let r: u16 = remote.parse().map_err(|_| format!("Invalid remote port '{}'", remote))?;
    Ok((l, host, r))
}

fn spawn_ssh(local: u16, ssh_target: &str, remote: u16) -> std::io::Result<Child> {
    let listener = format!("{}:127.0.0.1:{}", local, remote);
    Command::new("ssh").args(["-N", "-T", "-o", "ServerAliveInterval=15", "-o", "ServerAliveCountMax=3", "-o", "ExitOnForwardFailure=yes", "-o", "ConnectTimeout=10", "-L", &listener, ssh_target]).stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn()
}

fn port_open(port: u16) -> bool {
    let addr: SocketAddr = match format!("127.0.0.1:{}", port).parse() { Ok(a) => a, Err(_) => return false };
    TcpStream::connect_timeout(&addr, Duration::from_millis(800)).is_ok()
}
