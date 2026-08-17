use clap::Parser;
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
#[command(name = "clean-cache", about = "Clean package manager and tool caches")]
struct Cli {
    /// Run in serve mode
    #[arg(long)]
    serve: bool,
    /// Port for serve mode
    #[arg(long, default_value = "9103")]
    port: u16,
}

fn main() {
    let cli = Cli::parse();
    run(cli.serve, cli.port);
}

struct CacheTarget {
    label: &'static str,
    path: Option<PathBuf>,
    needs_sudo: bool,
    cmd: Option<(&'static str, Vec<String>)>,
}

struct ScanEntry {
    target: CacheTarget,
    size: u64,
}

fn run(serve: bool, port: u16) {
    if serve {
        serve_scan(port);
        return;
    }
    interactive();
}

fn cache_targets() -> Vec<CacheTarget> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    vec![
        CacheTarget { label: "npm cache", path: Some(home.join(".npm/_cacache")), needs_sudo: false, cmd: None },
        CacheTarget { label: "pip cache", path: Some(home.join(".cache/pip")), needs_sudo: false, cmd: None },
        CacheTarget { label: "uv cache", path: Some(home.join(".cache/uv")), needs_sudo: false, cmd: None },
        CacheTarget { label: "bun cache", path: Some(home.join(".bun/install/cache")), needs_sudo: false, cmd: None },
        CacheTarget { label: "yarn cache", path: Some(home.join(".cache/yarn")), needs_sudo: false, cmd: None },
        CacheTarget { label: "cargo registry cache", path: Some(home.join(".cargo/registry/cache")), needs_sudo: false, cmd: None },
        CacheTarget { label: "cargo registry src", path: Some(home.join(".cargo/registry/src")), needs_sudo: false, cmd: None },
        CacheTarget { label: "go build cache", path: Some(home.join(".cache/go-build")), needs_sudo: false, cmd: None },
        CacheTarget { label: "gradle cache", path: Some(home.join(".gradle/caches")), needs_sudo: false, cmd: None },
        CacheTarget { label: "yay cache (AUR build)", path: Some(home.join(".cache/yay")), needs_sudo: false, cmd: None },
        CacheTarget { label: "paru cache (AUR build)", path: Some(home.join(".cache/paru")), needs_sudo: false, cmd: None },
        CacheTarget { label: "pacman pkg cache", path: Some(PathBuf::from("/var/cache/pacman/pkg")), needs_sudo: true, cmd: None },
        CacheTarget { label: "apt archives", path: Some(PathBuf::from("/var/cache/apt/archives")), needs_sudo: true, cmd: None },
        CacheTarget { label: "dnf cache", path: Some(PathBuf::from("/var/cache/dnf")), needs_sudo: true, cmd: None },
        CacheTarget { label: "docker builder cache", path: None, needs_sudo: false, cmd: Some(("docker", vec!["builder".into(), "prune".into(), "-f".into()])) },
    ]
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let out = Command::new("du")
        .args(["-sb", path.to_str().unwrap_or("")])
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        }
        Err(_) => 0,
    }
}

fn docker_builder_size() -> u64 {
    let out = Command::new("docker")
        .args(["system", "df", "--format", "{{.Size}}"])
        .output();
    let text = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return 0,
    };
    text.lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() { None } else { Some(t) }
        })
        .next_back()
        .map(parse_docker_size)
        .unwrap_or(0)
}

fn parse_docker_size(s: &str) -> u64 {
    let s = s.trim();
    let (num, mult) = if let Some(v) = s.strip_suffix("GB") {
        (v, 1_000_000_000u64)
    } else if let Some(v) = s.strip_suffix("MB") {
        (v, 1_000_000)
    } else if let Some(v) = s.strip_suffix("KB") {
        (v, 1_000)
    } else if let Some(v) = s.strip_suffix("B") {
        (v, 1)
    } else {
        (s, 1)
    };
    num.trim().parse::<f64>().unwrap_or(0.0) as u64 * mult
}

fn scan() -> Vec<ScanEntry> {
    cache_targets()
        .into_iter()
        .map(|target| {
            let size = if target.cmd.is_some() {
                docker_builder_size()
            } else if let Some(p) = &target.path {
                dir_size(p)
            } else {
                0
            };
            ScanEntry { target, size }
        })
        .collect()
}

fn disk_free() -> u64 {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let home = home.to_string_lossy().to_string();
    let out = Command::new("df")
        .args(["-B1", "--output=avail", &home])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next_back()
                .and_then(|l| l.trim().parse().ok())
                .unwrap_or(0)
        }
        _ => 0,
    }
}

fn interactive() {
    println!("{}", header("Cache Cleaner"));
    println!("{}", divider());

    let entries = scan();
    let total: u64 = entries.iter().map(|e| e.size).sum();
    let free_before = disk_free();

    println!();
    println!(
        "  {}",
        label_value("Disk free (before)", &format_size(free_before))
    );
    println!("  {}", label_value("Total cache", &format_size(total)));
    println!();
    println!(
        "  {:>18}  {:<24} {}",
        "SIZE".style(Theme::LABEL),
        "CACHE".style(Theme::LABEL),
        "PATH".style(Theme::LABEL),
    );

    let mut options: Vec<String> = Vec::new();
    let mut non_empty: Vec<usize> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if e.size == 0 {
            continue;
        }
        non_empty.push(i);
        let path = e
            .target
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "docker builder".into());
        let sudo = if e.target.needs_sudo { " (sudo)" } else { "" };
        println!(
            "  {:>18}  {:<24} {}",
            format_size(e.size).style(Theme::VALUE),
            format!("{}{}", e.target.label, sudo).dimmed(),
            path.dimmed()
        );
        options.push(format!(
            "{}  {}",
            e.target.label,
            format_size(e.size)
        ));
    }
    println!();

    if non_empty.is_empty() {
        println!("{} No caches found to clean.", success(""));
        return;
    }

    let selected =
        dialoguer::MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Select caches to clean (space to toggle)")
            .items(&options)
            .interact()
            .unwrap_or_default();

    if selected.is_empty() {
        println!("{} Nothing selected.", muted(""));
        return;
    }

    let chosen: Vec<&ScanEntry> = selected.iter().map(|&i| &entries[non_empty[i]]).collect();
    let reclaim: u64 = chosen.iter().map(|e| e.size).sum();
    println!();
    for c in &chosen {
        println!(
            "  {} {}  ({} → {})",
            warn(""),
            c.target.label,
            format_size(c.size),
            "will be freed".dimmed()
        );
    }
    println!("\n  {}", label_value("Total", &format_size(reclaim)));

    let confirm =
        dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Proceed with cleanup?")
            .default(false)
            .interact()
            .unwrap_or(false);
    if !confirm {
        println!("{} Aborted.", muted(""));
        return;
    }

    for c in &chosen {
        let ok = clean_target(&c.target);
        if ok {
            println!("  {} Cleaned {}", success(""), c.target.label);
        } else {
            println!("  {} Failed to clean {}", error(""), c.target.label);
        }
    }

    let free_after = disk_free();
    println!();
    println!("{}", divider());
    println!(
        "  {}",
        label_value("Disk free (before)", &format_size(free_before)),
    );
    println!(
        "  {}",
        label_value("Disk free (after)", &format_size(free_after)),
    );
    println!(
        "  {}",
        label_value(
            "Recovered",
            &format_size(free_after.saturating_sub(free_before))
        ),
    );
}

fn clean_target(target: &CacheTarget) -> bool {
    if let Some((cmd, args)) = &target.cmd {
        return Command::new(cmd)
            .args(args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    let path = match &target.path {
        Some(p) => p,
        None => return false,
    };
    let mut full: Vec<String> = Vec::new();
    if target.needs_sudo {
        full.push("sudo".to_string());
    }
    full.push("rm".to_string());
    full.push("-rf".to_string());
    full.push(path.to_string_lossy().to_string());
    let status = Command::new(&full[0]).args(&full[1..]).status();
    status.map(|s| s.success()).unwrap_or(false)
}

fn serve_scan(port: u16) {
    let entries = scan();
    let total: u64 = entries.iter().map(|e| e.size).sum();
    println!(
        "  {} Cache scan sent to port {}. Total: {}.",
        success(""),
        port,
        format_size(total)
    );
}
