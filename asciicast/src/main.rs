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
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(text: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), text.style(Theme::HEADER))
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

fn which(binary: &str) -> bool {
    Command::new("which")
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[derive(Parser)]
#[command(name = "asciicast", about = "Terminal session recording")]
struct Cli {
    /// Output file path
    #[arg(short = 'o')]
    output: Option<String>,
    /// Command to record
    cmd: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    run(cli.output, cli.cmd);
}

fn run(output: Option<String>, cmd: Vec<String>) {
    println!("{}", header("Asciicast"));
    println!("{}", divider());

    let asciinema = which("asciinema");
    let script = which("script");

    if asciinema {
        println!("  {} Using asciinema ...\n", muted(""));
        let mut args: Vec<&str> = vec!["rec"];
        if let Some(ref out) = output {
            args.push(out);
        }
        if cmd.is_empty() {
            let status = Command::new("asciinema").args(&args).status();
            match status {
                Ok(s) if s.success() => {}
                _ => eprintln!("  {} asciinema exited with error.", error("")),
            }
        } else {
            let mut args: Vec<String> = vec!["rec".to_string()];
            if let Some(ref out) = output {
                args.push(out.clone());
            }
            args.push("--command".to_string());
            args.push(cmd.join(" "));
            let status = Command::new("asciinema").args(&args).status();
            match status {
                Ok(s) if s.success() => {}
                _ => eprintln!("  {} asciinema exited with error.", error("")),
            }
        }
    } else if script {
        let out = output.unwrap_or_else(|| "recording.cast".to_string());
        println!(
            "  {} Using script (no asciinema found). Output: {}\n",
            muted(""),
            out.style(Theme::VALUE)
        );
        if cmd.is_empty() {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let _ = Command::new("script")
                .args(["-q", &out, "-c", &shell])
                .status();
        } else {
            let _ = Command::new("script")
                .args(["-q", &out, "-c", &cmd.join(" ")])
                .status();
        }
    } else {
        println!(
            "  {} Install {} or use:\n",
            warn("asciinema not found."),
            "asciinema".style(Theme::VALUE)
        );
        println!("    pacman -S asciinema");
        println!("    brew install asciinema\n");
        println!("  Then re-run: asciicast");
    }
}
