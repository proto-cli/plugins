pub use owo_colors;
pub use owo_colors::OwoColorize;
pub use indicatif;
pub use dirs;
use std::path::PathBuf;
use std::process::Command;

// ─── Theme ───────────────────────────────────────────────────────────────────

pub struct Theme;

impl Theme {
    pub const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    pub const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    pub const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    pub const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    pub const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    pub const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    pub const BOLD: owo_colors::Style = owo_colors::Style::new().bold();
    pub const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
    pub const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

// ─── Formatters ──────────────────────────────────────────────────────────────

pub fn header(text: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), text.style(Theme::HEADER))
}

pub fn success(msg: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), msg)
}

pub fn warn(msg: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), msg)
}

pub fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}

pub fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}

pub fn divider() -> String {
    "─".repeat(40).dimmed().to_string()
}

pub fn label_value(label: &str, value: &str) -> String {
    format!(
        "  {} {}",
        format!("{:>14}:", label).style(Theme::LABEL),
        value.style(Theme::VALUE)
    )
}

pub fn section(title: &str) -> String {
    format!(
        "\n{}\n{}\n",
        title.style(Theme::HEADER).bold().to_string(),
        "─".repeat(title.len()).dimmed().to_string()
    )
}

// ─── Spinner ─────────────────────────────────────────────────────────────────

pub struct Spinner {
    spinner: indicatif::ProgressBar,
}

impl Spinner {
    pub fn new(msg: &str) -> Self {
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

    pub fn update(&self, msg: &str) {
        self.spinner.set_message(msg.to_string());
    }

    pub fn done(&self, msg: &str) {
        self.spinner.finish_with_message(msg.to_string());
    }

    pub fn fail(&self, msg: &str) {
        self.spinner.finish_with_message(
            format!("{} {}", "✗".style(Theme::ERROR), msg.style(Theme::ERROR))
        );
    }
}

// ─── System Helpers ──────────────────────────────────────────────────────────

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
        .join("proto")
}

pub fn plugins_dir() -> PathBuf {
    config_dir().join("plugins")
}

pub fn which(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn run_command(program: &str, args: &[&str]) -> std::io::Result<std::process::ExitStatus> {
    Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
}

pub fn run_command_output(program: &str, args: &[&str]) -> std::io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn run_command_output_err(program: &str, args: &[&str]) -> std::io::Result<String> {
    let output = Command::new(program).args(args).output()?;
    Ok(String::from_utf8_lossy(&output.stderr).trim().to_string())
}

pub fn format_size(bytes: u64) -> String {
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

pub fn get_arch() -> String {
    std::env::consts::ARCH.to_string()
}

pub fn get_os() -> String {
    std::env::consts::OS.to_string()
}

pub fn get_platform() -> &'static str {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    match (arch, os) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        _ => "x86_64-unknown-linux-gnu",
    }
}

pub fn is_ci() -> bool {
    std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok()
}

pub fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
}

pub fn adaptive_divider() -> String {
    let width = terminal_width().saturating_sub(2);
    "─".repeat(width).dimmed().to_string()
}
