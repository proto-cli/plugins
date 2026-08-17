use clap::{Subcommand, Parser};
use owo_colors::OwoColorize;
use regex::Regex;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
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
}

#[derive(Parser)]
#[command(name = "secret", about = "Scan and mask leaked secrets in shell history")]
struct Cli {
    #[command(subcommand)]
    action: SecretAction,
}

#[derive(Subcommand)]
enum SecretAction {
    Mask {
        #[arg(long, value_name = "PATH")]
        file: Option<String>,
        #[arg(long, help = "Alert only, don't rewrite any files")]
        dry_run: bool,
    },
}

struct Finding { path: String, line: usize, kind: &'static str, snippet: String }
struct SecretRule { name: &'static str, re: &'static str }

const RULES: &[SecretRule] = &[
    SecretRule { name: "AWS Access Key", re: r"AKIA[0-9A-Z]{16}" },
    SecretRule { name: "GitHub Token", re: r"gh[pousr]_[0-9A-Za-z]{36,}" },
    SecretRule { name: "GitHub PAT", re: r"github_pat_[0-9A-Za-z_]{20,}" },
    SecretRule { name: "OpenAI Key", re: r"sk-(?:proj-)?[A-Za-z0-9_-]{20,}" },
    SecretRule { name: "Google API Key", re: r"AIza[0-9A-Za-z_-]{35}" },
    SecretRule { name: "Slack Token", re: r"xox[baprs]-[0-9A-Za-z-]{10,}" },
    SecretRule { name: "Stripe Key", re: r"(?:sk|rk)_live_[0-9A-Za-z]{20,}" },
    SecretRule { name: "JWT", re: r"eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{5,}" },
    SecretRule { name: "Private Key", re: r"-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----" },
    SecretRule { name: "Secret Assignment", re: r#"(?i)(?:password|passwd|token|secret|api[_-]?key|auth|bearer)\s*[:=]\s*['"]?[A-Za-z0-9_./+=-]{8,}"# },
];

fn main() {
    let cli = Cli::parse();
    match cli.action {
        SecretAction::Mask { file, dry_run } => mask(file.as_deref(), dry_run),
    }
}

fn default_targets() -> Vec<std::path::PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut targets = vec![home.join(".bash_history"), home.join(".zsh_history"), home.join(".config/fish/fish_history")];
    if let Ok(dir) = std::env::var("XDG_STATE_HOME") { let dir = std::path::PathBuf::from(dir); targets.push(dir.join("zsh/history")); targets.push(dir.join("fish/history")); }
    else { targets.push(home.join(".local/state/zsh/history")); targets.push(home.join(".local/state/fish/history")); }
    targets.into_iter().filter(|p| p.exists()).collect()
}

fn collect_files(target: &str) -> Vec<std::path::PathBuf> {
    let p = std::path::PathBuf::from(target);
    if p.is_dir() { let mut files = Vec::new(); let mut stack = vec![p]; while let Some(dir) = stack.pop() { if let Ok(rd) = std::fs::read_dir(&dir) { for entry in rd.flatten() { let path = entry.path(); if path.is_dir() { stack.push(path); } else if path.extension().map(|e| e != "pyc").unwrap_or(true) { files.push(path); } } } } files.sort(); files }
    else if p.is_file() { vec![p] } else { Vec::new() }
}

fn scan_file(path: &std::path::Path) -> Vec<Finding> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    if content.is_empty() { return Vec::new(); }
    let path_str = path.to_string_lossy().to_string();
    let mut findings = Vec::new();
    let compiled: Vec<(Regex, &str)> = RULES.iter().filter_map(|r| Regex::new(r.re).ok().map(|re| (re, r.name))).collect();
    for (line_idx, line) in content.lines().enumerate() {
        for (re, name) in &compiled {
            for m in re.find_iter(line) {
                let hit = m.as_str();
                if hit.len() < 8 && *name != "Private Key" { continue; }
                let snippet = mask_snippet(line, m.start(), m.end());
                findings.push(Finding { path: path_str.clone(), line: line_idx + 1, kind: name, snippet });
            }
        }
    }
    findings
}

fn mask_snippet(line: &str, start: usize, end: usize) -> String {
    let s = start.saturating_sub(12); let e = (end + 12).min(line.len());
    let mut out = String::new();
    if s > 0 { out.push_str("…"); }
    out.push_str(&line[s..start]); out.push_str("***"); out.push_str(&line[end..e]);
    if e < line.len() { out.push_str("…"); }
    out
}

fn mask_file(path: &std::path::Path) {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let compiled: Vec<Regex> = RULES.iter().filter_map(|r| Regex::new(r.re).ok()).collect();
    let mut masked = content.clone();
    for re in &compiled { masked = re.replace_all(&masked, "***REDACTED***").to_string(); }
    if masked != content { std::fs::write(path, masked).ok(); }
}

fn mask(file: Option<&str>, dry_run: bool) {
    println!("{} {}", "◆".style(Theme::ACCENT), "Secret Scanner".style(Theme::HEADER));
    println!("{}", divider());
    let targets: Vec<std::path::PathBuf> = match file { Some(t) => { let files = collect_files(t); if files.is_empty() { eprintln!("{} Path not found: {}", error(""), t); return; } files } None => default_targets() };
    if targets.is_empty() { println!("{} No shell history or log files found.", warn("")); return; }
    let sp = Spinner::new(&format!("Scanning {} file(s)...", targets.len()));
    let mut findings = Vec::new();
    for t in &targets { findings.extend(scan_file(t)); }
    sp.done(&format!("{} potential secret(s) found", findings.len()));
    if findings.is_empty() { println!("{} {}", "✔".style(Theme::SUCCESS), "No leaked secrets detected.".style(Theme::MUTED)); return; }
    let mut by_file: std::collections::HashMap<String, Vec<&Finding>> = std::collections::HashMap::new();
    for f in &findings { by_file.entry(f.path.clone()).or_default().push(f); }
    for (path, list) in &by_file {
        println!(); println!("  {}", path.style(Theme::ACCENT).bold());
        println!("  {}", "─".repeat(path.len().min(60)).dimmed());
        for f in list { println!("  {:>4} {}", f.line.to_string().style(Theme::MUTED), f.kind.style(Theme::WARN)); println!("      {}", f.snippet.style(Theme::MUTED)); }
    }
    println!(); println!("{} {} file(s) affected.", warn(""), by_file.len());
    if dry_run { println!("{} Dry run — no files modified.", success("")); return; }
    for (path, list) in &by_file {
        let p = std::path::PathBuf::from(path);
        let prompt = format!("Mask {} secret(s) in {}?", list.len(), path.split('/').last().unwrap_or(path));
        if dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default()).with_prompt(prompt).default(false).interact().unwrap_or(false) {
            mask_file(&p);
            println!("{} {} — {} masked", "✔".style(Theme::SUCCESS), p.to_string_lossy().dimmed(), "***REDACTED***".style(Theme::ACCENT));
        }
    }
    println!(); println!("  Rotate any exposed credentials — masking history doesn't revoke them.");
}
