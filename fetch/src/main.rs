use owo_colors::{AnsiColors, DynColors, OwoColorize, Style};
use std::process::Command;

fn run_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn file_first_line(path: &str, strip: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(strip) {
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn distro() -> String {
    if cfg!(target_os = "macos") {
        let v = run_output("sw_vers", &["-productVersion"]);
        return if v.is_empty() { "macOS".into() } else { format!("macOS {}", v) };
    }
    if cfg!(target_os = "windows") {
        return std::env::var("OS").unwrap_or_else(|_| "Windows".into());
    }
    file_first_line("/etc/os-release", "PRETTY_NAME=")
        .or_else(|| file_first_line("/etc/os-release", "NAME="))
        .or_else(|| file_first_line("/etc/lsb-release", "DISTRIB_DESCRIPTION="))
        .unwrap_or_else(|| "Linux".to_string())
}

fn distro_id() -> String {
    file_first_line("/etc/os-release", "ID=").unwrap_or_default()
}

fn kernel() -> String {
    run_output("uname", &["-r"])
}

fn hostname() -> String {
    let h = run_output("hostname", &[]);
    if h.is_empty() {
        std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into())
    } else {
        h
    }
}

fn uptime() -> String {
    if let Ok(content) = std::fs::read_to_string("/proc/uptime") {
        if let Some(secs) = content
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok().map(|s| s as u64))
        {
            let mut parts = Vec::new();
            if secs / 86400 > 0 { parts.push(format!("{}d", secs / 86400)); }
            let h = (secs % 86400) / 3600;
            if h > 0 { parts.push(format!("{}h", h)); }
            let m = (secs % 3600) / 60;
            if m > 0 { parts.push(format!("{}m", m)); }
            let s = secs % 60;
            if s > 0 && parts.is_empty() { parts.push(format!("{}s", s)); }
            if parts.is_empty() { parts.push("just now".into()); }
            return parts.join(" ");
        }
    }
    "unknown".to_string()
}

fn shell() -> String {
    std::env::var("SHELL")
        .unwrap_or_default()
        .split('/')
        .next_back()
        .filter(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn de_wm() -> String {
    for var in ["XDG_CURRENT_DESKTOP", "DESKTOP_SESSION", "XDG_SESSION_DESKTOP"] {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() { return val.to_lowercase(); }
        }
    }
    for var in ["DISPLAY", "WAYLAND_DISPLAY"] {
        if std::env::var(var).is_ok() { return "X / Wayland".to_string(); }
    }
    "tty".to_string()
}

fn terminal() -> String {
    std::env::var("TERM").unwrap_or_else(|_| "unknown".to_string())
}

fn gpu() -> String {
    let mm = run_output("lspci", &["-mm"]);
    let mut gpu = String::new();
    for l in mm.lines() {
        if l.contains("VGA") || l.contains("3D controller") || l.contains("Display controller") {
            let quoted: Vec<&str> = l.split('"').collect();
            let odd: Vec<&str> = quoted.iter().skip(1).step_by(2).map(|s| *s).collect();
            gpu = odd
                .get(2)
                .or_else(|| odd.get(1))
                .unwrap_or(&"")
                .to_string();
            break;
        }
    }
    if gpu.is_empty() { "unknown".to_string() } else { gpu }
}

fn package_counts() -> String {
    let mut counts = Vec::new();
    let checks: [(&str, &[&str]); 6] = [
        ("pacman", &["-Q"]),
        ("dpkg-query", &["-f", "${Package}\n", "-W"]),
        ("rpm", &["-qa"]),
        ("apk", &["info"]),
        ("brew", &["list", "--formula"]),
        ("xbps-query", &["-l"]),
    ];
    for (bin, args) in checks {
        if let Ok(out) = Command::new(bin).args(args).output() {
            let n = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count();
            if n > 0 {
                counts.push(format!("{} ({})", n, bin.split('-').next().unwrap_or(bin)));
            }
        }
    }
    if counts.is_empty() {
        "? (no pm detected)".to_string()
    } else {
        counts.join(", ")
    }
}

fn logo_for(distro_id: &str) -> Vec<&'static str> {
    match distro_id {
        "arch" | "cachyos" | "endeavouros" | "archarm" | "manjaro" => vec![
            "      /\\        ",
            "     /  \\       ",
            "    /    \\      ",
            "   /  __  \\     ",
            "  /  /  \\  \\    ",
            " /_/    \\_\\    ",
        ],
        "ubuntu" | "pop" | "linuxmint" => vec![
            "       __       ",
            "      (--       ",
            "     (  -)      ",
            "    (___)(      ",
            "      ())       ",
            "      (())      ",
        ],
        "debian" => vec![
            "     ______     ",
            "    /  __  \\    ",
            "   /  /  \\  \\   ",
            "  |   \\__/  |   ",
            "  \\        /    ",
            "   \\______/     ",
        ],
        "fedora" => vec![
            "      ______    ",
            "     /      \\   ",
            "    |  (  )  |  ",
            "     \\  __  /   ",
            "      \\____/    ",
            "               ",
        ],
        "nixos" => vec![
            "       /\\      ",
            "      /  \\     ",
            "     /████\\    ",
            "    ████████   ",
            "     \\████/    ",
            "      \\  /     ",
            "       \\/      ",
        ],
        _ => vec![
            "     ⣀⡀         ",
            " ⢠⣤⡀⣾⣿⣿⠀⣤⣤⡄ ",
            " ⢿⣿⡇⠘⠛⠁⢸⣿⣿⠃ ",
            " ⠈⣉⣤⣾⣿⣿⡆⠉⣴⣶⣶ ",
            " ⣾⣿⣿⣿⣿⣿⣿⡀⠻⠟⠃ ",
            " ⠙⠛⠻⢿⣿⣿⣿⡇      ",
            "     ⠈⠙⠋⠁      ",
        ],
    }
}

fn color_block() {
    let cols = [
        AnsiColors::Black,
        AnsiColors::Red,
        AnsiColors::Green,
        AnsiColors::Yellow,
        AnsiColors::Blue,
        AnsiColors::Magenta,
        AnsiColors::Cyan,
        AnsiColors::White,
    ];
    let normal: Vec<String> = cols
        .iter()
        .map(|c| "████".color(DynColors::Ansi(*c)).to_string())
        .collect();
    let bright: Vec<String> = cols
        .iter()
        .map(|c| {
            format!(
                "{}",
                "████"
                    .color(DynColors::Ansi(bright(*c)))
                    .bold()
            )
        })
        .collect();
    println!();
    println!("  {}", normal.join(" "));
    println!("  {}", bright.join(" "));
    println!();
}

fn bright(c: AnsiColors) -> AnsiColors {
    use AnsiColors::*;
    match c {
        Black => AnsiColors::BrightBlack,
        Red => AnsiColors::BrightRed,
        Green => AnsiColors::BrightGreen,
        Yellow => AnsiColors::BrightYellow,
        Blue => AnsiColors::BrightBlue,
        Magenta => AnsiColors::BrightMagenta,
        Cyan => AnsiColors::BrightCyan,
        White => AnsiColors::BrightWhite,
        _ => c,
    }
}

fn info_lines() -> Vec<String> {
    let mut lines: Vec<(String, String)> = Vec::new();
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    lines.push(("Host".into(), format!("{}@{}", user, hostname())));
    lines.push(("OS".into(), distro()));
    lines.push(("Kernel".into(), kernel()));
    lines.push(("Uptime".into(), uptime()));
    lines.push(("Shell".into(), shell()));
    lines.push(("DE/WM".into(), de_wm()));
    lines.push(("Terminal".into(), terminal()));

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    if let Some(cpu) = sys.cpus().first() {
        lines.push(("CPU".into(), format!("{} ({})", cpu.brand().trim(), sys.cpus().len())));
    }
    lines.push((
        "Memory".into(),
        format!(
            "{} / {}",
            format_bytes(sys.used_memory()),
            format_bytes(sys.total_memory())
        ),
    ));
    if let Some(disk) = sysinfo::Disks::new_with_refreshed_list()
        .iter()
        .find(|d| d.mount_point().to_str() == Some("/"))
    {
        let used = disk.total_space() - disk.available_space();
        lines.push((
            "Disk (/)".into(),
            format!("{} / {}", format_bytes(used), format_bytes(disk.total_space())),
        ));
    }

    let g = gpu();
    if g != "unknown" {
        lines.push(("GPU".into(), g));
    }

    if !cfg!(target_os = "windows") {
        lines.push(("Packages".into(), package_counts()));
    }

    let width = lines.iter().map(|(k, _)| k.len()).max().unwrap_or(6) + 1;
    lines
        .into_iter()
        .map(|(k, v)| format!("{:>width$} {}", k, v))
        .collect()
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut i = 0;
    while size >= 1024.0 && i < UNITS.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", size, UNITS[i])
}

fn render(logo_color: Style) {
    let logo = logo_for(&distro_id());
    let styled: Vec<String> = logo.iter().map(|l| l.style(logo_color).to_string()).collect();
    let logo_w = styled.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 2;
    let info = info_lines();

    for (i, l) in styled.iter().enumerate() {
        let pad: String = " ".repeat(logo_w.saturating_sub(l.chars().count()));
        let right = info.get(i).map(|s| s.as_str()).unwrap_or("");
        println!("{}{}{}", l, pad, right);
    }
    for line in info.iter().skip(styled.len()) {
        println!("{}{}", " ".repeat(logo_w), line);
    }
}

fn print_help() {
    println!("proto fetch — neofetch-style system info");
    println!();
    println!("  USAGE:");
    println!("    proto fetch                Show system info (like neofetch)");
    println!("    proto fetch --color <c>    Logo color: cyan, blue, green, magenta,");
    println!("                               red, yellow, white (default cyan)");
    println!("    proto fetch --help         Show this help");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        print_help();
        return;
    }

    let mut color = "cyan".to_string();
    if let Some(i) = args.iter().position(|a| a == "--color") {
        if let Some(v) = args.get(i + 1) {
            color = v.to_lowercase();
        }
    }

    let logo_style = match color.as_str() {
        "blue" => Style::new().blue(),
        "green" => Style::new().green(),
        "magenta" => Style::new().magenta(),
        "red" => Style::new().red(),
        "yellow" => Style::new().yellow(),
        "white" => Style::new().white(),
        _ => Style::new().cyan(),
    };

    println!();
    render(logo_style);
    color_block();
}