use clap::Parser;
use owo_colors::OwoColorize;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
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
fn divider() -> String {
    "─".repeat(40).dimmed().to_string()
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

const RECORD_TYPES: &[&str] = &["A", "AAAA", "CNAME", "MX", "TXT", "NS"];

#[derive(Parser)]
#[command(name = "dns", about = "DNS lookup for domains")]
struct Cli {
    /// Domain to look up
    domain: String,
}

fn main() {
    let cli = Cli::parse();
    run(&cli.domain);
}

fn run(domain: &str) {
    if !which("dig") {
        eprintln!(
            "{} dig required. Install bind-tools (e.g. {}).",
            error(""),
            "sudo pacman -S bind-tools".dimmed()
        );
        return;
    }

    println!("{}", header(format!("DNS Lookup: {}", domain).as_str()));
    println!("{}", divider());

    let mut found = false;
    for rt in RECORD_TYPES {
        let out = run_command_output("dig", &["+short", domain, rt]).unwrap_or_default();
        let values: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();

        if values.is_empty() {
            continue;
        }
        found = true;

        let label = format!("{:5}", rt);
        for (i, v) in values.iter().enumerate() {
            if i == 0 {
                println!(
                    "  {} {}",
                    label.style(Theme::ACCENT).bold(),
                    v
                );
            } else {
                println!("  {} {}", " ".repeat(5), v);
            }
        }
    }

    println!();
    if found {
        println!(
            "  {} All records above resolved for {}",
            success(""),
            domain
        );
    } else {
        println!(
            "  {} No records found — domain may not exist or has no records.",
            warn("")
        );
    }
}
