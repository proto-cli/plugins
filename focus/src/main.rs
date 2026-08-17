use owo_colors::OwoColorize;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut work = 25u64;
    let mut short_break = 5u64;
    let mut long_break = 15u64;
    let mut cycles = 4u64;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--work" => {
                if let Some(v) = args.get(i + 1) {
                    work = v.parse().unwrap_or(work);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--short" => {
                if let Some(v) = args.get(i + 1) {
                    short_break = v.parse().unwrap_or(short_break);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--long" => {
                if let Some(v) = args.get(i + 1) {
                    long_break = v.parse().unwrap_or(long_break);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--cycles" => {
                if let Some(v) = args.get(i + 1) {
                    cycles = v.parse().unwrap_or(cycles);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    println!("{}", header("Focus Timer"));
    println!(
        "  {} {} min work / {} min break / {} min long break / {} cycles",
        muted(""),
        work.style(Theme::VALUE),
        short_break.style(Theme::VALUE),
        long_break.style(Theme::VALUE),
        cycles.style(Theme::VALUE),
    );
    println!("{}", divider());
    println!("  Press Ctrl+C to stop\n");

    for cycle in 1..=cycles {
        run_phase("WORK", work * 60, Theme::LABEL);
        if cycle == cycles {
            break;
        }
        if cycle % 4 == 0 {
            run_phase("LONG BREAK", long_break * 60, Theme::SUCCESS);
        } else {
            run_phase("BREAK", short_break * 60, Theme::SUCCESS);
        }
    }

    println!("\n  {} All cycles complete!", success(""));
}

fn run_phase(label: &str, total_secs: u64, color: owo_colors::Style) {
    let start = Instant::now();
    let end = start + Duration::from_secs(total_secs);

    print!(
        "  {} {} | remaining: ",
        label.style(color),
        "█".repeat(20).style(Theme::MUTED),
    );
    io::stdout().flush().unwrap();

    loop {
        let now = Instant::now();
        if now >= end {
            break;
        }
        let remaining = end.duration_since(now).as_secs();
        let elapsed = now.duration_since(start).as_secs();
        let mins = remaining / 60;
        let secs = remaining % 60;
        let pct = elapsed as f64 / total_secs as f64;
        let filled = (pct * 20.0) as usize;
        let bar: String = (0..20)
            .map(|i| if i < filled { '█' } else { '░' })
            .collect();

        print!(
            "\r  {} {} | {:02}:{:02}",
            label.style(color),
            bar.style(if filled > 15 {
                Theme::ERROR
            } else {
                Theme::VALUE
            }),
            mins,
            secs,
        );
        io::stdout().flush().unwrap();
        thread::sleep(Duration::from_millis(250));
    }
    println!(
        "\r  {} {} | done!{}\n",
        label.style(color),
        "████████████████████".style(Theme::SUCCESS),
        " ".repeat(10)
    );
    print!("\x07"); // bell
    io::stdout().flush().unwrap();
}
