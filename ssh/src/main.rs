use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process::Command;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn warn(s: &str) -> String { format!("{} {}", "⚠".style(Theme::WARN), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(50).dimmed().to_string() }

#[derive(Debug, Clone)]
struct Host {
    name: String,
    hostname: String,
    user: String,
    port: u16,
    identity: String,
    jump: String,
    options: Vec<(String, String)>,
}

fn ssh_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".ssh/config")
}

fn parse_hosts() -> Vec<Host> {
    let content = std::fs::read_to_string(ssh_config_path()).unwrap_or_default();
    let mut hosts = Vec::new();
    let mut cur: Option<Host> = None;

    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        if let Some(rest) = t.strip_prefix("Host ") {
            if let Some(h) = cur.take() { hosts.push(h); }
            let names: Vec<String> = rest
                .split_whitespace()
                .filter(|s| !s.starts_with('*') && !s.starts_with('!'))
                .map(|s| s.to_string())
                .collect();
            if !names.is_empty() {
                cur = Some(Host {
                    name: names.join(", "),
                    hostname: String::new(),
                    user: String::new(),
                    port: 22,
                    identity: String::new(),
                    jump: String::new(),
                    options: Vec::new(),
                });
            }
        } else if let Some(h) = cur.as_mut() {
            let kv: Vec<&str> = t.splitn(2, char::is_whitespace).collect();
            if kv.len() == 2 {
                let key = kv[0].to_lowercase();
                let val = kv[1].trim().to_string();
                match key.as_str() {
                    "hostname" => h.hostname = val,
                    "user" => h.user = val,
                    "port" => h.port = val.parse().unwrap_or(22),
                    "identityfile" => h.identity = val,
                    "proxyjump" => h.jump = val,
                    _ => h.options.push((key, val)),
                }
            }
        }
    }
    if let Some(h) = cur.take() { hosts.push(h); }
    hosts
}

fn resolve_host<'a>(hosts: &'a [Host], query: &str) -> Option<&'a Host> {
    hosts
        .iter()
        .find(|h| h.name.split(", ").any(|n| n == query))
        .or_else(|| hosts.iter().find(|h| h.name.contains(query)))
}

fn connect(hosts: &[Host], query: &str) {
    let Some(h) = resolve_host(hosts, query) else {
        eprintln!("  {} Unknown host: {}", error(""), query);
        std::process::exit(1);
    };
    let target = if h.hostname.is_empty() { h.name.split(", ").next().unwrap().to_string() } else { h.hostname.clone() };
    let mut args: Vec<String> = Vec::new();
    if !h.user.is_empty() { args.push("-l".into()); args.push(h.user.clone()); }
    if h.port != 22 { args.push("-p".into()); args.push(h.port.to_string()); }
    args.push(target.clone());
    let port_txt = if h.port != 22 { format!(":{}", h.port) } else { String::new() };
    println!(
        "  {} connecting to {} ({}{}{})…",
        "🔌".dimmed(),
        h.name.split(", ").next().unwrap_or(&target).style(Theme::ACCENT),
        if h.user.is_empty() { String::new() } else { format!("{}@", h.user) },
        target.style(Theme::VALUE),
        port_txt.style(Theme::WARN),
    );
    let _ = Command::new("ssh").args(&args).status();
}

fn list_hosts(hosts: &[Host], pattern: Option<&str>) {
    println!("{} {}", header("SSH Hosts"), muted(&format!("({} total)", hosts.len())));
    println!("{}", divider());
    let filtered: Vec<&Host> = hosts
        .iter()
        .filter(|h| {
            pattern.map(|p| {
                h.name.to_lowercase().contains(&p.to_lowercase())
                    || h.hostname.to_lowercase().contains(&p.to_lowercase())
            }).unwrap_or(true)
        })
        .collect();

    if filtered.is_empty() {
        println!("  {} No hosts matched", muted(""));
        println!("  {}", "proto ssh add".style(Theme::ACCENT));
        return;
    }

    for (i, h) in filtered.iter().enumerate() {
        let display = h.name.split(", ").next().unwrap_or(&h.name).to_string();
        let meta = left(String::new(), [
            if h.hostname.is_empty() { None } else { Some(h.hostname.clone()) },
            if h.user.is_empty() { None } else { Some(format!("user {}", h.user)) },
            if h.port != 22 { Some(format!("port {}", h.port)) } else { None },
            if h.jump.is_empty() { None } else { Some(format!("via {}", h.jump)) },
        ]);
        println!(
            "  {} {} {}",
            format!("{:>3}.", i + 1).style(Theme::MUTED),
            display.style(Theme::ACCENT).bold(),
            meta.dimmed()
        );
    }
    println!();
    println!("  {} {} {} {}", "→".dimmed(), "proto ssh <name>".style(Theme::MUTED), "connect".dimmed(), "◈".dimmed());
}

fn left(_pad: String, parts: [Option<String>; 4]) -> String {
    parts
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
}

fn add_host() {
    use dialoguer::{Confirm, Input};
    println!("{}", header("Add SSH Host"));
    println!("{}", divider());

    let name: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Alias (Host)")
        .interact_text()
        .unwrap();
    if name.is_empty() || name.contains(' ') {
        eprintln!("  {} Host must be a single word", error(""));
        return;
    }
    let hostname: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Hostname (IP or domain)")
        .interact_text()
        .unwrap();
    let user: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("User (optional)")
        .allow_empty(true)
        .default("".into())
        .interact_text()
        .unwrap();
    let port: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Port")
        .default("22".into())
        .interact_text()
        .unwrap();
    let identity: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("IdentityFile (optional)")
        .allow_empty(true)
        .default("".into())
        .interact_text()
        .unwrap();

    println!("\n  {} {}", "◆".style(Theme::ACCENT), name.style(Theme::ACCENT).bold());
    println!("    {} {}", "host".dimmed(), hostname.style(Theme::VALUE));
    if !user.is_empty() { println!("    {} {}", "user".dimmed(), user.dimmed()); }
    if !identity.is_empty() { println!("    {} {}", "identity".dimmed(), identity.dimmed()); }

    let save = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Append to ~/.ssh/config?")
        .default(true)
        .interact()
        .unwrap_or(true);

    if !save {
        return;
    }

    let path = ssh_config_path();
    let mut block = String::new();
    block.push_str(&format!("\nHost {}\n", name));
    block.push_str(&format!("    HostName {}\n", hostname));
    if !user.is_empty() { block.push_str(&format!("    User {}\n", user)); }
    let port: u16 = port.parse().unwrap_or(22);
    if port != 22 { block.push_str(&format!("    Port {}\n", port)); }
    if !identity.is_empty() { block.push_str(&format!("    IdentityFile {}\n", identity)); }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.to_lowercase().contains(&format!("host {}\n", name.to_lowercase())) {
        eprintln!("  {} Host '{}' already exists", warn(""), name);
        return;
    }
    std::fs::write(&path, format!("{}{}", existing, block)).ok();
    println!("  {} saved → {}", success(""), path.display().to_string().dimmed());
}

fn show_info(hosts: &[Host], query: &str) {
    let Some(h) = resolve_host(hosts, query) else {
        eprintln!("  {} Unknown host: {}", error(""), query);
        std::process::exit(1);
    };
    println!("{}", header(&format!("SSH: {}", h.name.split(", ").next().unwrap_or(&h.name))));
    println!("{}", divider());
    let rows: Vec<(String, String)> = vec![
        ("Host".into(), h.name.clone()),
        ("HostName".into(), h.hostname.clone()),
        ("User".into(), h.user.clone()),
        ("Port".into(), h.port.to_string()),
        ("IdentityFile".into(), h.identity.clone()),
        ("ProxyJump".into(), h.jump.clone()),
    ];
    for (k, v) in rows {
        if !v.is_empty() {
            println!("  {} {}", format!("{:>12}:", k).style(Theme::MUTED), v.style(Theme::VALUE));
        }
    }
    if !h.options.is_empty() {
        println!();
        println!("  {} {}", "Options".style(Theme::MUTED), "".to_string().dimmed());
        for (k, v) in &h.options {
            println!("  {} {} = {}", "·".dimmed(), k.style(Theme::ACCENT), v.dimmed());
        }
    }
}

fn print_help() {
    println!("{} SSH Host Manager", header("proto"));
    println!("{}", divider());
    println!();
    println!("  USAGE:");
    println!("    proto ssh list [pattern]      List hosts from ~/.ssh/config");
    println!("    proto ssh <name> [cmd]        Connect to a host (optionally run a command)");
    println!("    proto ssh info <name>         Show full host config");
    println!("    proto ssh add                 Add a new host interactively");
    println!("    proto ssh --help              Show this help");
    println!();
    println!("  EXAMPLES:");
    println!("    proto ssh list prod");
    println!("    proto ssh web01");
    println!("    proto ssh add");
    println!("    proto ssh info db-primary");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hosts = parse_hosts();

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return;
    }

    match args[0].as_str() {
        "list" => list_hosts(&hosts, args.get(1).map(|s| s.as_str())),
        "add" => add_host(),
        "info" => {
            if let Some(q) = args.get(1) { show_info(&hosts, q); }
            else { eprintln!("  {} Usage: ssh info <name>", error("")); }
        }
        "help" => print_help(),
        other => {
            // If it matches a known host → connect; else treat as raw host
            if resolve_host(&hosts, other).is_some() {
                connect(&hosts, other);
            } else {
                // Could be a raw ssh target like user@host or an unknown alias
                let mut cmd = Command::new("ssh");
                for a in args.iter().skip(1) { cmd.arg(a); }
                cmd.arg(other);
                let _ = cmd.status();
            }
        }
    }
}