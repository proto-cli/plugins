use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::io::Read;
use std::time::Instant;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
}

fn header(text: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), text.style(Theme::HEADER))
}

fn divider() -> String {
    "─".repeat(40).style(Theme::MUTED).to_string()
}

fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}

fn success(msg: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), msg)
}

struct Spinner {
    pb: ProgressBar,
}

impl Spinner {
    fn new(msg: &str) -> Self {
        let pb = ProgressBar::new_spinner()
            .with_message(msg.to_string())
            .with_style(
                ProgressStyle::with_template("{spinner:.cyan} {msg}")
                    .unwrap()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { pb }
    }

    fn done(&self, msg: &str) {
        self.pb.finish_with_message(msg.to_string());
    }

    fn fail(&self, msg: &str) {
        self.pb.finish_with_message(
            format!("{} {}", "✗".style(Theme::ERROR), msg.style(Theme::ERROR)),
        );
    }
}

fn main() {
    println!("{}", header("Speedtest"));
    println!("{}", divider());

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(5))
        .timeout_read(std::time::Duration::from_secs(30))
        .build();

    println!("  {} Download test ...\n", muted(""));

    let test = |label: &str, url: &str| {
        let spin = Spinner::new(&format!("Fetching {}...", label));
        let start = Instant::now();
        let mut total = 0u64;
        let result = match agent.get(url).call() {
            Ok(resp) => {
                let mut reader = resp.into_reader();
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => total += n as u64,
                        Err(_) => break,
                    }
                }
                let elapsed = start.elapsed().as_secs_f64().max(0.1);
                let mbps = (total as f64 * 8.0) / (elapsed * 1_000_000.0);
                spin.done(&format!("{}: {:.1} Mbps", label, mbps));
                Ok((mbps, total, elapsed))
            }
            Err(e) => {
                spin.fail(&format!("{} failed: {}", label, e));
                Err(e)
            }
        };
        result
    };

    let r1 = test("25MB", "https://speed.cloudflare.com/__down?bytes=25000000");
    let _ = test("10MB", "https://speed.cloudflare.com/__down?bytes=10000000");

    println!();
    if let Ok((mbps, bytes, secs)) = r1 {
        println!(
            "  {} {:.1} Mbps  ({:.1} MB in {:.1}s)",
            success("Download:"),
            mbps,
            bytes as f64 / 1_000_000.0,
            secs
        );
    }
    println!();
}
