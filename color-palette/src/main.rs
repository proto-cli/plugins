use owo_colors::OwoColorize;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
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

fn main() {
    println!("{}", header("Color Palette"));
    println!("{}", divider());

    println!("  {} Standard:", muted(""));
    let std = [
        ("BLACK  ", "\x1b[40m  \x1b[0m"),
        ("RED    ", "\x1b[41m  \x1b[0m"),
        ("GREEN  ", "\x1b[42m  \x1b[0m"),
        ("YELLOW ", "\x1b[43m  \x1b[0m"),
        ("BLUE   ", "\x1b[44m  \x1b[0m"),
        ("MAGENTA", "\x1b[45m  \x1b[0m"),
        ("CYAN   ", "\x1b[46m  \x1b[0m"),
        ("WHITE  ", "\x1b[47m  \x1b[0m"),
    ];
    for (name, swatch) in &std {
        print!("    {} {}", swatch, name);
    }
    println!("\n");

    println!("  {} Bright:", muted(""));
    let bright = [
        ("BLACK  ", "\x1b[100m  \x1b[0m"),
        ("RED    ", "\x1b[101m  \x1b[0m"),
        ("GREEN  ", "\x1b[102m  \x1b[0m"),
        ("YELLOW ", "\x1b[103m  \x1b[0m"),
        ("BLUE   ", "\x1b[104m  \x1b[0m"),
        ("MAGENTA", "\x1b[105m  \x1b[0m"),
        ("CYAN   ", "\x1b[106m  \x1b[0m"),
        ("WHITE  ", "\x1b[107m  \x1b[0m"),
    ];
    for (name, swatch) in &bright {
        print!("    {} {}", swatch, name);
    }
    println!("\n");

    println!("  {} 256-color cube (6x6x6):", muted(""));
    for g in 0..6 {
        print!("  ");
        for r in 0..6 {
            for b in 0..6 {
                let code = 16 + 36 * r + 6 * g + b;
                print!("\x1b[48;5;{}m  \x1b[0m", code);
            }
            print!(" ");
        }
        println!();
    }
    println!();

    println!("  {} Grayscale:", muted(""));
    print!("  ");
    for i in 232..=255 {
        print!("\x1b[48;5;{}m \x1b[0m", i);
    }
    println!("\n");

    println!("  {} Proto theme sample:", muted(""));
    println!(
        "    {}  HEADER / {}  LABEL / {}  VALUE / {}  SUCCESS / {}  WARN / {}  ERROR",
        "HEADER".style(Theme::HEADER),
        "LABEL".style(Theme::LABEL),
        "VALUE".style(Theme::VALUE),
        "SUCCESS".style(Theme::SUCCESS),
        "WARN".style(Theme::WARN),
        "ERROR".style(Theme::ERROR),
    );
    println!();
}
