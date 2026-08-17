use clap::Parser;
use owo_colors::OwoColorize;
use std::path::PathBuf;

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

#[derive(Parser)]
#[command(name = "battery", about = "Battery health diagnostics")]
struct Cli {
    /// Run in serve mode (prints JSON to stdout)
    #[arg(long)]
    serve: bool,
    /// Monitoring interval in seconds (serve mode)
    #[arg(long, default_value = "5")]
    interval: u64,
    /// Port for serve mode
    #[arg(long, default_value = "9100")]
    port: u16,
}

fn main() {
    let cli = Cli::parse();
    run(cli.serve, cli.interval, cli.port);
}

fn run(serve: bool, interval: u64, port: u16) {
    let battery = match Battery::detect() {
        Some(b) => b,
        None => {
            eprintln!("{} No battery found on this system.", error(""));
            return;
        }
    };

    if serve {
        serve_mode(battery, interval, port);
        return;
    }

    let info = battery.snapshot();
    print_report(&info);
}

fn serve_mode(battery: Battery, interval: u64, _port: u16) {
    println!();
    println!(
        "  {} Monitoring battery every {}s. Ctrl+C to stop.",
        "◉".style(Theme::ACCENT),
        interval
    );

    loop {
        let info = battery.snapshot();
        print_snapshot_line(&info);
        std::thread::sleep(std::time::Duration::from_secs(interval));
    }
}

fn print_snapshot_line(info: &BatteryInfo) {
    println!(
        "  {}  {}%  {}  {:.1} W  health {:.1}%  {}",
        info.status.style(Theme::ACCENT).bold(),
        info.capacity.to_string().style(Theme::VALUE),
        format!("{}/{}", info.full_short(), info.design_short()).dimmed(),
        info.watts,
        info.health_percent,
        format!("{} cycles", info.cycles.unwrap_or(0)).dimmed(),
    );
}

fn print_report(info: &BatteryInfo) {
    println!("{}", header("Battery Health"));
    println!("{}", divider());

    println!("  {}", label_value("Model", &info.model));
    println!("  {}", label_value("Status", &info.status));
    println!(
        "  {}",
        label_value("Health", &format!("{:.1}%", info.health_percent))
    );
    println!(
        "  {}",
        label_value("Charge", &format!("{}%", info.capacity))
    );
    println!(
        "  {}",
        label_value("Capacity (current)", &info.current_capacity_str().to_string())
    );
    println!(
        "  {}",
        label_value("Capacity (design)", &info.design_capacity_str().to_string())
    );
    println!(
        "  {}",
        label_value(
            "Cycles",
            &info.cycles
                .map(|c| c.to_string())
                .unwrap_or_else(|| "n/a".into())
        )
    );
    println!(
        "  {}",
        label_value(
            "Power",
            &format!("{:.2} W {}", info.watts, info.status.to_lowercase())
        )
    );

    println!();
    if info.health_percent < 80.0 {
        println!(
            "  {} Battery health is degraded — consider a replacement.",
            warn("")
        );
    } else if info.health_percent < 60.0 {
        println!("  {} Battery health is critical.", error(""));
    } else {
        println!("  {} Battery health looks good.", success(""));
    }
}

struct Battery {
    path: PathBuf,
    name: String,
}

struct BatteryInfo {
    model: String,
    status: String,
    capacity: u32,
    full: Option<u64>,
    full_design: Option<u64>,
    cycles: Option<u64>,
    watts: f64,
    health_percent: f64,
}

impl Battery {
    fn detect() -> Option<Battery> {
        let dir = PathBuf::from("/sys/class/power_supply");
        let entries = std::fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("BAT") {
                return Some(Battery {
                    path: entry.path(),
                    name,
                });
            }
        }
        None
    }

    fn read_int(&self, field: &str) -> Option<u64> {
        let content = std::fs::read_to_string(self.path.join(field)).ok()?;
        content.trim().parse().ok()
    }

    fn read_str(&self, field: &str) -> Option<String> {
        std::fs::read_to_string(self.path.join(field))
            .ok()
            .map(|s| s.trim().to_string())
    }

    fn snapshot(&self) -> BatteryInfo {
        let model = self
            .read_str("model_name")
            .unwrap_or_else(|| self.name.clone());
        let status = self
            .read_str("status")
            .unwrap_or_else(|| "Unknown".into());
        let capacity = self.read_int("capacity").unwrap_or(0) as u32;

        let full = self
            .read_int("energy_full")
            .or_else(|| self.read_int("charge_full"));
        let full_design = self
            .read_int("energy_full_design")
            .or_else(|| self.read_int("charge_full_design"));
        let cycles = self.read_int("cycle_count");

        let watts = if let Some(p) = self.read_int("power_now") {
            p as f64 / 1_000_000.0
        } else if let (Some(v), Some(c)) =
            (self.read_int("voltage_now"), self.read_int("current_now"))
        {
            v as f64 * c as f64 / 1_000_000_000_000.0
        } else {
            0.0
        };

        let health_percent = match (full, full_design) {
            (Some(f), Some(fd)) if fd > 0 => f as f64 / fd as f64 * 100.0,
            _ => 100.0,
        };

        BatteryInfo {
            model,
            status,
            capacity,
            full,
            full_design,
            cycles,
            watts,
            health_percent,
        }
    }
}

impl BatteryInfo {
    fn full_short(&self) -> String {
        format_wh(self.full)
    }
    fn design_short(&self) -> String {
        format_wh(self.full_design)
    }
    fn current_capacity_str(&self) -> String {
        format_wh(self.full)
    }
    fn design_capacity_str(&self) -> String {
        format_wh(self.full_design)
    }
}

fn format_wh(val: Option<u64>) -> String {
    match val {
        Some(v) => {
            if v > 1_000_000 {
                format!("{:.1} Wh", v as f64 / 1_000_000.0)
            } else {
                format!("{:.1} Wh", v as f64 / 1_000.0)
            }
        }
        None => "n/a".to_string(),
    }
}
