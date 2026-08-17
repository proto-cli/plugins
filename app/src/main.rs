use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
}

fn header(s: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER))
}
fn success(s: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), s)
}
fn error(s: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), s)
}
fn warn(s: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), s)
}
fn muted(s: &str) -> String {
    format!("{}", s.style(Theme::MUTED))
}
fn divider() -> String {
    "─".repeat(50).dimmed().to_string()
}
fn label_value(label: &str, value: &str) -> String {
    format!("  {}: {}", label.style(Theme::MUTED), value.style(Theme::ACCENT))
}

fn which(binary: &str) -> bool {
    std::process::Command::new("which")
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
fn run_command_output(program: &str, args: &[&str]) -> std::io::Result<String> {
    let output = std::process::Command::new(program).args(args).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct Spinner {
    spinner: indicatif::ProgressBar,
}
impl Spinner {
    fn new(msg: &str) -> Self {
        let sp = indicatif::ProgressBar::new_spinner()
            .with_message(msg.to_string())
            .with_style(
                indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg}")
                    .unwrap()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { spinner: sp }
    }
    fn update(&self, msg: &str) {
        self.spinner.set_message(msg.to_string());
    }
    fn done(&self, msg: &str) {
        self.spinner.finish_with_message(msg.to_string());
    }
    fn fail(&self, msg: &str) {
        self.spinner.finish_with_message(format!("{} {}", "✗".style(Theme::ERROR), msg.style(Theme::ERROR)));
    }
}

#[derive(Parser)]
#[command(name = "app", about = "Application utilities")]
struct Cli {
    #[command(subcommand)]
    action: AppAction,
}

#[derive(Subcommand, Debug, Clone)]
enum AppAction {
    #[command(about = "Scan directory for missing deps, .env gaps, and port conflicts")]
    Doctor,
    #[command(name = "port", about = "Port management")]
    Port {
        #[command(subcommand)]
        action: PortAction,
    },
    #[command(about = "Purge build artifacts and reclaim disk space")]
    Nuke {
        #[arg(long, help = "Skip confirmation prompts")]
        skip: bool,
    },
    #[command(about = "Snapshot git state before risky changes")]
    Snap {
        #[command(subcommand)]
        action: SnapAction,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum PortAction {
    #[command(about = "Find and optionally kill what's running on a port")]
    Release {
        #[arg(required = true, value_name = "PORT")]
        port: u16,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum SnapAction {
    #[command(about = "Create a snapshot of current git state")]
    Create {
        #[arg(required = true, value_name = "NAME")]
        name: String,
    },
    #[command(about = "Restore a snapshot")]
    Restore {
        #[arg(required = true, value_name = "NAME")]
        name: String,
    },
    #[command(about = "View snapshot contents")]
    View {
        #[arg(value_name = "NAME")]
        name: Option<String>,
    },
    #[command(about = "Delete a snapshot")]
    Delete {
        #[arg(required = true, value_name = "NAME")]
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match &cli.action {
        AppAction::Doctor => doctor(),
        AppAction::Port { action } => match action {
            PortAction::Release { port } => port_release(*port),
        },
        AppAction::Nuke { skip } => nuke(*skip),
        AppAction::Snap { action } => snap(action),
    }
}

fn doctor() {
    use dialoguer::Confirm;
    println!("{} {}\n", "◆".style(Theme::ACCENT), "App Doctor".style(Theme::HEADER));

    let mut issues = 0u32;
    let mut fixes = Vec::new();

    println!("{}", "Dependencies".style(Theme::HEADER));
    println!("{}", divider());
    let tools = &[
        ("node", "Node.js runtime", "curl -fsSL https://deb.nodesource.com/setup_lts | sudo bash - && sudo apt install nodejs"),
        ("npm", "npm package manager", "comes with Node.js"),
        ("python3", "Python 3", "sudo apt install python3"),
        ("pip3", "pip package manager", "sudo apt install python3-pip"),
        ("cargo", "Rust toolchain", "curl --proto '=https' -sSf https://sh.rustup.rs | sh"),
        ("docker", "Docker", "curl -fsSL https://get.docker.com | sh"),
        ("git", "Git", "sudo apt install git"),
        ("lsof", "port scanner", "sudo apt install lsof"),
    ];

    for (bin, desc, fix) in tools {
        if which(bin) {
            println!("  {} {}", "✔".green(), bin.style(Theme::ACCENT));
        } else {
            issues += 1;
            println!("  {} {} — {}", "✗".red(), bin.style(Theme::ACCENT), desc.dimmed());
            fixes.push((bin, fix));
        }
    }

    println!("\n{}", ".env audit".style(Theme::HEADER));
    println!("{}", divider());
    read_env_issues(&mut issues);

    println!("\n{}", "Ports".style(Theme::HEADER));
    println!("{}", divider());
    read_port_issues(&mut issues);

    println!("\n{}", divider());
    println!(
        "{} {} issue(s) found.",
        if issues > 0 { "⚠".yellow().to_string() } else { "✔".green().to_string() },
        issues
    );

    if !fixes.is_empty() {
        println!("\n{}", "Fix suggestions:".style(Theme::HEADER));
        for (bin, fix) in &fixes {
            println!("  {} {}", "◆".style(Theme::ACCENT), bin.style(Theme::ACCENT));
            println!("    {}", fix.dimmed());
        }
        println!();
        let auto = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Try to auto-fix missing dependencies?")
            .default(false)
            .interact()
            .unwrap_or(false);

        if auto {
            for (bin, fix) in fixes {
                if bin == "python3" || bin == "pip3" || bin == "git" || bin == "lsof" {
                    let sp = Spinner::new(&format!("Installing {}...", bin));
                    let status = std::process::Command::new("sudo")
                        .args([
                            "apt", "install", "-y",
                            if *bin == "python3" {
                                "python3"
                            } else if *bin == "pip3" {
                                "python3-pip"
                            } else if *bin == "lsof" {
                                "lsof"
                            } else {
                                bin
                            },
                        ])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if status {
                        sp.done(&format!("Installed {}", bin));
                    } else {
                        sp.fail(&format!("Could not install {}", bin));
                    }
                } else {
                    println!(
                        "  {} Install {} manually: {}",
                        "◆".style(Theme::MUTED),
                        bin,
                        fix.dimmed()
                    );
                }
            }
        }
    }
}

fn read_env_issues(issues: &mut u32) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let example = cwd.join(".env.example");
    let dotenv = cwd.join(".env");

    if !example.exists() {
        println!("  {} No .env.example found — skipping.", "?".dimmed());
        return;
    }
    println!("  {} .env.example found", "✔".green());

    if !dotenv.exists() {
        println!("  {} .env is missing — create from .env.example", "✗".red());
        *issues += 1;
        return;
    }

    let ex_keys = read_env_keys(&example);
    let env_keys = read_env_keys(&dotenv);
    let mut missing = Vec::new();

    for k in &ex_keys {
        if !env_keys.contains(k) && !k.is_empty() {
            missing.push(k.clone());
        }
    }

    if missing.is_empty() {
        println!("  {} All {} keys present in .env", "✔".green(), ex_keys.len());
    } else {
        *issues += missing.len() as u32;
        for k in missing {
            println!("  {} Missing key: {}", "✗".red(), k.style(Theme::ACCENT));
        }
    }
}

fn read_env_keys(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() || t.starts_with('#') {
                None
            } else {
                t.split('=').next().map(|s| s.trim().to_string())
            }
        })
        .collect()
}

fn read_port_issues(issues: &mut u32) {
    let check_ports = &[3000u16, 8080, 5432, 6379, 8000, 5000, 4200, 9090, 27017, 3306];
    for port in check_ports {
        let result = check_port(*port);
        if !result.is_empty() {
            *issues += 1;
            println!("  {} :{} — {}", "⚠".yellow(), port, result.style(Theme::WARN));
        }
    }
    if *issues == 0 || !check_ports.iter().any(|p| !check_port(*p).is_empty()) {
        println!("  {} No common ports occupied", "✔".green());
    }
}

fn check_port(port: u16) -> String {
    if let Ok(out) = run_command_output("lsof", &["-i", &format!(":{}", port), "-t", "-n", "-P"]) {
        let trimmed = out.trim();
        if !trimmed.is_empty() {
            if let Ok(name) = run_command_output("lsof", &["-i", &format!(":{}", port), "-n", "-P"]) {
                let lines: Vec<&str> = name.lines().skip(1).collect();
                let first = lines
                    .first()
                    .map(|l| {
                        let parts: Vec<&str> = l.split_whitespace().collect();
                        parts.first().unwrap_or(&"?").to_string()
                    })
                    .unwrap_or_else(|| "?".into());
                return format!("{} (PID {})", first, trimmed.lines().next().unwrap_or("?"));
            }
        }
    }
    String::new()
}

fn port_release(port: u16) {
    use dialoguer::Confirm;

    println!("{} Port {}", "◆".style(Theme::ACCENT), port.to_string().style(Theme::ACCENT));
    println!("{}", divider());

    let info = if let Ok(out) = run_command_output("lsof", &["-i", &format!(":{}", port), "-n", "-P"]) {
        out
    } else if let Ok(out) = run_command_output("ss", &["-tlnp", &format!("sport = :{}", port)]) {
        out
    } else {
        eprintln!("{} Cannot scan port {} (need lsof or ss)", error(""), port);
        return;
    };

    if info.trim().is_empty() || info.trim() == "COMMAND" {
        println!("{} Nothing is listening on port {}", success(""), port);
        return;
    }

    println!("{}", info);

    let pids: Vec<String> = info
        .lines()
        .skip(1)
        .filter_map(|l| {
            let parts: Vec<&str> = l.split_whitespace().collect();
            if parts.len() >= 2 {
                Some(parts[1].to_string())
            } else {
                None
            }
        })
        .collect();

    if pids.is_empty() {
        println!("{} Could not identify process.", warn(""));
        return;
    }

    for pid in pids {
        let confirm = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(&format!("Kill PID {}? (SIGTERM)", pid))
            .default(false)
            .interact()
            .unwrap_or(false);

        if confirm {
            nix_kill(&pid);
        }

        let confirm2 = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(&format!("Force kill PID {}? (SIGKILL)", pid))
            .default(false)
            .interact()
            .unwrap_or(false);

        if confirm2 {
            nix_force_kill(&pid);
        }
    }
}

fn nix_kill(pid: &str) {
    let _ = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    println!("{} Sent SIGTERM to PID {}", success(""), pid);
}

fn nix_force_kill(pid: &str) {
    let _ = std::process::Command::new("kill")
        .arg("-KILL")
        .arg(pid)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    println!("{} Sent SIGKILL to PID {}", success(""), pid);
}

fn nuke(skip: bool) {
    use dialoguer::Confirm;

    println!("{} {}\n", "◆".style(Theme::ACCENT), "App Nuke".style(Theme::HEADER));

    let sp = Spinner::new("Scanning for artifacts...");
    let cwd = std::env::current_dir().unwrap_or_default();
    let hits = find_artifacts(&cwd, 4);
    sp.done(&format!("Found {} artifact(s)", hits.len()));

    if hits.is_empty() {
        println!("{} Nothing to clean.", success(""));
        return;
    }

    let total_bytes: u64 = hits.iter().map(|(_, s, _)| *s).sum();
    println!("\n{}", divider());
    for (path, size, kind) in &hits {
        println!(
            "  {} {} {}",
            "▸".style(Theme::ACCENT),
            format_size(*size).style(Theme::ACCENT),
            format!("{}  ({})", path, kind).dimmed()
        );
    }
    println!("{}", divider());
    println!("\n{}", label_value("Total to free", &format_size(total_bytes)));

    if !skip {
        let confirm = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(&format!(
                "Delete {} artifact(s) and reclaim {}?",
                hits.len(),
                format_size(total_bytes)
            ))
            .default(false)
            .interact()
            .unwrap_or(false);

        if !confirm {
            println!("{}", "Aborted.".style(Theme::MUTED));
            return;
        }
    }

    let mut freed = 0u64;
    for (path, size, _) in &hits {
        let p = std::path::Path::new(path);
        if p.exists() {
            let _ = std::fs::remove_dir_all(p).or_else(|_| std::fs::remove_file(p));
            freed += size;
            println!("  {} {}", "✗".dimmed(), path.dimmed());
        }
    }

    println!(
        "\n{} {}",
        success(""),
        format!("Freed {}", format_size(freed))
            .style(Theme::ACCENT)
            .bold()
    );
}

fn find_artifacts(root: &std::path::Path, depth: u32) -> Vec<(String, u64, String)> {
    let mut results = Vec::new();
    if depth == 0 || !root.is_dir() {
        return results;
    }

    let patterns: &[(&str, &str)] = &[
        ("node_modules", "npm"),
        ("target", "rust"),
        (".cache", "cache"),
        ("build", "build"),
        ("dist", "build"),
        ("__pycache__", "python"),
        (".next", "next.js"),
        (".nuxt", "nuxt"),
        ("vendor", "composer (check size)"),
    ];

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();

            let mut matched = false;
            for (pat, kind) in patterns {
                if name == *pat && path.is_dir() {
                    let size = dir_size(&path);
                    if size > 0 {
                        results.push((path.to_string_lossy().to_string(), size, kind.to_string()));
                    }
                    matched = true;
                    break;
                }
            }

            if name.ends_with(".pyc") && path.is_file() {
                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                if size > 0 {
                    results.push((path.to_string_lossy().to_string(), size, "python".into()));
                }
                matched = true;
            }

            if !matched && path.is_dir() && !name.starts_with('.') {
                results.extend(find_artifacts(&path, depth - 1));
            }
        }
    }

    results
}

fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else {
                total += p.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

fn format_size(bytes: u64) -> String {
    const U: &[&str] = &["B", "KB", "MB", "GB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v < 10.0 {
        format!("{:.1} {}", v, U[i])
    } else {
        format!("{:.0} {}", v, U[i])
    }
}

fn snap(action: &SnapAction) {
    match action {
        SnapAction::Create { name } => snap_create(name),
        SnapAction::Restore { name } => snap_restore(name),
        SnapAction::View { name } => snap_view(name.as_deref()),
        SnapAction::Delete { name } => snap_delete(name),
    }
}

fn snaps_dir() -> std::path::PathBuf {
    dirs::data_local_dir()
        .unwrap_or_default()
        .join("proto/snaps")
}

fn snap_create(name: &str) {
    if !which("git") {
        eprintln!("{} Git required.", error(""));
        return;
    }

    let dir = snaps_dir().join(name);
    if dir.exists() {
        eprintln!("{} Snapshot '{}' already exists.", error(""), name);
        return;
    }
    std::fs::create_dir_all(&dir).unwrap();

    let sp = Spinner::new(&format!("Creating snapshot '{}'...", name));

    let diff = run_command_output("git", &["diff", "--stat"]).unwrap_or_default();
    let full_diff = run_command_output("git", &["diff"]).unwrap_or_default();
    let untracked = run_command_output("git", &["ls-files", "--others", "--exclude-standard"]).unwrap_or_default();
    let status = run_command_output("git", &["status", "--porcelain"]).unwrap_or_default();

    let info = format!(
        "snapshot: {}\ncreated: {}\n\ngit status:\n{}\n\nFiles changed:\n{}",
        name,
        chrono_now(),
        status,
        if diff.is_empty() {
            "(working tree clean)"
        } else {
            &diff
        }
    );
    std::fs::write(dir.join("info.txt"), &info).unwrap();
    std::fs::write(dir.join("diff.patch"), &full_diff).unwrap();

    if !untracked.is_empty() {
        std::fs::write(dir.join("untracked.txt"), &untracked).unwrap();
        let stg = std::path::PathBuf::from(&format!("/tmp/proto_snap_{}", name));
        let _ = std::fs::create_dir_all(&stg);
        for f in untracked.lines() {
            let src = std::env::current_dir().unwrap_or_default().join(f);
            let dst = stg.join(f);
            if let Some(p) = dst.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            let _ = std::fs::copy(&src, &dst);
        }
        let _ = std::process::Command::new("tar")
            .args([
                "-czf",
                &format!("/tmp/proto_snap_{}.tar.gz", name),
                "-C",
                "/tmp",
                &format!("proto_snap_{}", name),
            ])
            .status();
        let _ = std::fs::remove_dir_all(&stg);
        if std::path::Path::new(&format!("/tmp/proto_snap_{}.tar.gz", name)).exists() {
            let _ = std::fs::rename(
                format!("/tmp/proto_snap_{}.tar.gz", name),
                dir.join("untracked.tar.gz"),
            );
        }
    }

    sp.done(&format!("Snapshot '{}' saved", name));
    println!("  {}", label_value("Snapshots", &dir.to_string_lossy()));
}

fn snap_view(name: Option<&str>) {
    let dir = snaps_dir();
    if let Some(n) = name {
        let snap_dir = dir.join(n);
        if !snap_dir.exists() {
            eprintln!("{} Snapshot '{}' not found.", error(""), n);
            return;
        }
        let info = std::fs::read_to_string(snap_dir.join("info.txt")).unwrap_or_default();
        println!("{}", info);
        let diff = std::fs::read_to_string(snap_dir.join("diff.patch")).unwrap_or_default();
        if !diff.is_empty() {
            println!("{}", divider());
            println!(
                "{}",
                diff.lines().take(50).collect::<Vec<_>>().join("\n")
            );
        }
    } else {
        println!(
            "\n{}",
            "Snapshots".style(Theme::HEADER)
        );
        println!("{}", divider());
        if !dir.exists() || std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0) == 0 {
            println!("  {}", "No snapshots yet.".dimmed());
            return;
        }
        for entry in std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()) {
            let n = entry.file_name().to_string_lossy().to_string();
            let info = std::fs::read_to_string(entry.path().join("info.txt")).unwrap_or_default();
            let first = info.lines().next().unwrap_or("");
            println!("  {} {}", "▸".style(Theme::ACCENT), n.style(Theme::ACCENT));
            println!("    {}", first.dimmed());
        }
    }
}

fn snap_restore(name: &str) {
    use dialoguer::Confirm;
    let dir = snaps_dir().join(name);
    if !dir.exists() {
        eprintln!("{} Snapshot '{}' not found.", error(""), name);
        return;
    }

    let info = std::fs::read_to_string(dir.join("info.txt")).unwrap_or_default();
    println!("{}", info);
    println!("{}", divider());

    let confirm = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt(&format!(
            "Restore snapshot '{}'? This will overwrite working changes.",
            name
        ))
        .default(false)
        .interact()
        .unwrap_or(false);

    if !confirm {
        println!("{}", "Aborted.".style(Theme::MUTED));
        return;
    }

    let sp = Spinner::new(&format!("Restoring '{}'...", name));

    let patch = dir.join("diff.patch");
    if patch.exists() {
        let _ = std::process::Command::new("git")
            .args(["apply", "--reject", &patch.to_string_lossy()])
            .status();
    }

    let tgz = dir.join("untracked.tar.gz");
    if tgz.exists() {
        let _ = std::process::Command::new("tar")
            .args(["-xzf", &tgz.to_string_lossy(), "-C", "."])
            .status();
    }

    sp.done(&format!("Snapshot '{}' restored", name));
}

fn snap_delete(name: &str) {
    use dialoguer::Confirm;
    let dir = snaps_dir().join(name);
    if !dir.exists() {
        eprintln!("{} Snapshot '{}' not found.", error(""), name);
        return;
    }

    let confirm = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt(&format!("Delete snapshot '{}' permanently?", name))
        .default(false)
        .interact()
        .unwrap_or(false);

    if !confirm {
        println!("{}", "Aborted.".style(Theme::MUTED));
        return;
    }

    let _ = std::fs::remove_dir_all(&dir);
    println!("{} Snapshot '{}' deleted.", success(""), name);
}

fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let s = dur.as_secs() as i64;
    let days = s / 86400;
    let rem = s % 86400;
    let h = rem / 3600;
    let mi = (rem % 3600) / 60;
    let sec = rem % 60;
    let (y, mo, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, mi, sec)
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as i64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as i64;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
