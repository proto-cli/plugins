use owo_colors::{AnsiColors, OwoColorize, Style};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_CONFIG: &str = include_str!("../arch.jsonc");

fn run_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn hostname() -> String {
    run_output(
        "hostname",
        &[],
    ).trim()
        .trim_end_matches('\n')
        .to_string()
}

fn distro() -> String {
    let os = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let mut pretty = String::new();
    for line in os.lines() {
        if let Some(v) = line.strip_prefix("PRETTY_NAME=") {
            pretty = v.trim_matches('"').to_string();
        }
    }
    pretty
}

fn distro_id() -> String {
    let os = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let mut id = String::new();
    for line in os.lines() {
        if let Some(v) = line.strip_prefix("ID=") {
            id = v.trim().to_string();
        }
    }
    id
}

fn kernel() -> String {
    run_output("uname", &["-r"])
}

fn uptime() -> String {
    run_output("uptime", &["-p"])
        .trim_start_matches("up ")
        .to_string()
}

fn shell() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| {
            s.rsplit('/')
                .next()
                .map(|n| n.to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn de() -> String {
    let cur = std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default();
    cur.to_lowercase()
}

fn wm() -> String {
    std::env::var("XDG_SESSION_TYPE")
        .unwrap_or_default()
        .to_lowercase()
}

fn terminal() -> String {
    std::env::var("TERM").unwrap_or_else(|_| "unknown".into())
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
    if gpu.is_empty() {
        "unknown".to_string()
    } else {
        gpu
    }
}

fn package_counts() -> String {
    let mut parts: Vec<String> = Vec::new();
    for (cmd, label) in [
        ("pacman", "pacman"),
        ("dpkg", "dpkg"),
        ("rpm", "rpm"),
        ("apk", "apk"),
        ("brew", "brew"),
        ("xbps-query", "xbps"),
    ] {
        let n: usize = match cmd {
            "pacman" => run_output("pacman", &["-Qq"])
                .lines()
                .count(),
            "dpkg" => run_output("dpkg", &["--list"])
                .lines()
                .filter(|l| l.starts_with("ii"))
                .count(),
            "rpm" => run_output("rpm", &["-qa"]).lines().count(),
            "apk" => run_output("apk", &["info"]).lines().count(),
            "brew" => run_output("brew", &["list", "--formula"]).lines().count(),
            "xbps-query" => run_output("xbps-query", &["-l"]).lines().count(),
            _ => 0,
        };
        if n > 0 {
            parts.push(format!("{} ({})", n, label));
        }
    }
    if parts.is_empty() {
        "unknown".to_string()
    } else {
        parts.join(", ")
    }
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

// ---------------------------------------------------------------- config --

#[derive(Deserialize, Clone)]
#[serde(default)]
struct Config {
    logo: Logo,
    display: Display,
    modules: Modules,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            logo: Logo::default(),
            display: Display::default(),
            modules: Modules::default(),
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
struct Logo {
    source: String,
    color: Option<LogoColor>,
    padding: Padding,
}

impl Default for Logo {
    fn default() -> Self {
        Self {
            source: "auto".into(),
            color: Some(LogoColor::Accent("cyan".into())),
            padding: Padding::default(),
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum LogoColor {
    Accent(String),
    Map(BTreeMap<String, String>),
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct Padding {
    left: usize,
    top: usize,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
struct Display {
    separator: String,
    color: DisplayColor,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            separator: ":".into(),
            color: DisplayColor::default(),
        }
    }
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct DisplayColor {
    label: String,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
struct Modules {
    list: Vec<Module>,
}

impl Default for Modules {
    fn default() -> Self {
        Self {
            list: vec![
                Module {
                    mtype: Some("title".into()),
                    format: Some("{user-name}{at}{host-name}".into()),
                    ..Module::default()
                },
                Module { mtype: Some("os".into()), ..Module::default() },
                Module { mtype: Some("host".into()), ..Module::default() },
                Module { mtype: Some("kernel".into()), ..Module::default() },
                Module { mtype: Some("uptime".into()), ..Module::default() },
                Module { mtype: Some("shell".into()), ..Module::default() },
                Module { mtype: Some("de".into()), ..Module::default() },
                Module { mtype: Some("wm".into()), ..Module::default() },
                Module { mtype: Some("terminal".into()), ..Module::default() },
                Module { mtype: Some("cpu".into()), ..Module::default() },
                Module { mtype: Some("gpu".into()), ..Module::default() },
                Module { mtype: Some("memory".into()), ..Module::default() },
                Module { mtype: Some("disk".into()), ..Module::default() },
                Module { mtype: Some("packages".into()), ..Module::default() },
                Module { mtype: Some("colors".into()), ..Module::default() },
            ],
        }
    }
}

#[derive(Deserialize, Clone, Default)]
#[serde(default)]
struct Module {
    #[serde(rename = "type")]
    mtype: Option<String>,
    format: Option<String>,
    label: Option<String>,
    mount: Option<String>,
}

fn parse_jsonc(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_str = false;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
                i += 1;
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '/' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
            }
            ',' if {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                j < chars.len() && (chars[j] == '}' || chars[j] == ']')
            } => {
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn load_config(path: &PathBuf) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("unable to read {}: {}", path.display(), e))?;
    let cleaned = parse_jsonc(&text);
    serde_json::from_str(&cleaned)
        .map_err(|e| format!("invalid config {}: {}", path.display(), e))
}

fn resolve_config(custom: Option<&str>) -> Result<PathBuf, String> {
    if let Some(p) = custom {
        let pb = PathBuf::from(p);
        if !pb.exists() {
            return Err(format!("config not found: {}", p));
        }
        return Ok(pb);
    }

    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".config")
        });
    let dir = base.join("proto").join("fetch");
    let path = dir.join("arch.jsonc");

    if !path.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("unable to create {}: {}", dir.display(), e))?;
        fs::write(&path, DEFAULT_CONFIG)
            .map_err(|e| format!("unable to write {}: {}", path.display(), e))?;
        eprintln!("installed default config to {}", path.display());
    }
    Ok(path)
}

fn style_for(name: &str) -> Option<Style> {
    match name.to_lowercase().as_str() {
        "black" => Some(Style::new().black()),
        "red" => Some(Style::new().red()),
        "green" => Some(Style::new().green()),
        "yellow" => Some(Style::new().yellow()),
        "blue" => Some(Style::new().blue()),
        "magenta" => Some(Style::new().magenta()),
        "cyan" => Some(Style::new().cyan()),
        "white" => Some(Style::new().white()),
        _ => None,
    }
}

fn logo_color(cfg: &Config, cli_color: Option<&str>) -> Style {
    if let Some(c) = cli_color {
        return style_for(c).unwrap_or_else(|| Style::new().cyan());
    }
    match &cfg.logo.color {
        Some(LogoColor::Accent(a)) => style_for(a).unwrap_or_else(|| Style::new().cyan()),
        Some(LogoColor::Map(map)) => map
            .values()
            .next()
            .and_then(|v| style_for(v))
            .unwrap_or_else(|| Style::new().cyan()),
        None => Style::new().cyan(),
    }
}

fn logo_lines(cfg: &Config) -> Vec<&'static str> {
    let s = cfg.logo.source.to_lowercase();
    match s.as_str() {
        "" | "none" | "off" | "false" | "disabled" => vec![],
        "auto" => logo_for(&distro_id()),
        other => logo_for(other),
    }
}

fn template(fmt: &str, vals: &[(&str, &str)]) -> String {
    let mut out = fmt.to_string();
    for (k, v) in vals {
        out = out.replace(&format!("{{{}}}", k), v);
    }
    out
}

fn module_value(m: &Module, sys: &mut sysinfo::System) -> Option<(String, String)> {
    let ty = m.mtype.as_deref().unwrap_or("");
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    let host = hostname();
    let shell_path = std::env::var("SHELL").unwrap_or_default();

    let (label, value, placeholders): (&str, String, Vec<(&str, String)>) = match ty {
        "title" => {
            let fmt = m.format.as_deref().unwrap_or("{user-name}@{host-name}");
            let out = template(fmt, &[
                ("user-name", user.as_str()),
                ("at", "@"),
                ("host-name", host.as_str()),
                ("pretty-name", distro().as_str()),
            ]);
            return Some((String::new(), out));
        }
        "os" | "distro" => {
            let v = distro();
            ("OS", v, vec![("pretty-name", distro()), ("name", distinct_name()), ("version", String::new()), ("id", distro_id())])
        }
        "host" => ("Host", host.clone(), vec![("name", host.clone()), ("pretty-name", host.clone())]),
        "kernel" => {
            let k = kernel();
            ("Kernel", k.clone(), vec![("release", k), ("version", String::new()), ("build", String::new())])
        }
        "uptime" => {
            let u = uptime();
            ("Uptime", u.clone(), vec![("total", u), ("days", String::new()), ("hours", String::new()), ("minutes", String::new())])
        }
        "shell" => {
            let name = shell();
            ("Shell", name, vec![("shell", shell_path.clone()), ("path", shell_path), ("version", String::new())])
        }
        "de" => {
            let d = de();
            ("DE", d, vec![("name", de())])
        }
        "wm" => {
            let w = wm();
            ("WM", w, vec![("name", wm())])
        }
        "terminal" => {
            let t = terminal();
            ("Terminal", t, vec![("name", terminal())])
        }
        "cpu" => {
            if let Some(cpu) = sys.cpus().first() {
                let brand = cpu.brand().trim().to_string();
                let cores = sys.cpus().len().to_string();
                (
                    "CPU",
                    format!("{} ({})", brand, cores),
                    vec![("brand", brand), ("cores", cores), ("percent", String::new())],
                )
            } else {
                return None;
            }
        }
        "gpu" => {
            let g = gpu();
            if g == "unknown" {
                return None;
            }
            ("GPU", g, vec![("name", gpu())])
        }
        "memory" => {
            let used = format_bytes(sys.used_memory());
            let total = format_bytes(sys.total_memory());
            let free = format_bytes(sys.total_memory() - sys.used_memory());
            let pct = if sys.total_memory() > 0 {
                (sys.used_memory() * 100 / sys.total_memory()).to_string()
            } else {
                "0".into()
            };
            (
                "Memory",
                format!("{} / {}", used, total),
                vec![("used", used), ("total", total), ("free", free), ("used-percent", pct)],
            )
        }
        "disk" => {
            let mount = m.mount.as_deref().unwrap_or("/");
            if let Some(disk) = sysinfo::Disks::new_with_refreshed_list()
                .iter()
                .find(|d| d.mount_point().to_str() == Some(mount))
            {
                let used = format_bytes(disk.total_space() - disk.available_space());
                let total = format_bytes(disk.total_space());
                let free = format_bytes(disk.available_space());
                let pct = if disk.total_space() > 0 {
                    ((disk.total_space() - disk.available_space()) * 100 / disk.total_space()).to_string()
                } else {
                    "0".into()
                };
                (
                    "Disk",
                    format!("{} / {}", used, total),
                    vec![("used", used), ("total", total), ("free", free), ("used-percent", pct)],
                )
            } else {
                return None;
            }
        }
        "packages" => {
            let p = package_counts();
            if p == "unknown" {
                return None;
            }
            ("Packages", p, vec![("count", package_counts()), ("manager", package_counts())])
        }
        "gap" | "break" => return Some((String::new(), String::new())),
        _ => return None,
    };

    let label = m.label.clone().unwrap_or_else(|| label.to_string());
    if value.is_empty() && ty != "gap" {
        return None;
    }

    if let Some(fmt) = &m.format {
        let mut vals: Vec<(&str, &str)> =
            placeholders.iter().map(|(k, v)| (*k, v.as_str())).collect();
        vals.push(("label", label.as_str()));
        vals.push(("value", value.as_str()));
        let rendered = template(fmt, &vals);
        return Some((String::new(), rendered));
    }

    Some((label, value))
}

fn distinct_name() -> String {
    let os = fs::read_to_string("/etc/os-release").unwrap_or_default();
    for line in os.lines() {
        if let Some(v) = line.strip_prefix("NAME=") {
            return v.trim_matches('"').to_string();
        }
    }
    String::new()
}

// ------------------------------------------------------------- output --

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
    let row: Vec<String> = cols
        .iter()
        .map(|c| "████".color(DynColors::Ansi(*c)).to_string())
        .collect();
    let bright_row: Vec<String> = cols
        .iter()
        .map(|c| "████".color(DynColors::Ansi(bright(*c))).bold().to_string())
        .collect();
    println!("  {}", row.join(" "));
    println!("  {}", bright_row.join(" "));
}

use owo_colors::DynColors;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        println!("proto fetch — neofetch-style system information");
        println!();
        println!("  USAGE:");
        println!("    proto fetch                Show system info (neofetch-style)");
        println!("    proto fetch --color <c>    Logo/accent color (cyan, blue, magenta, ...)");
        println!("    proto fetch --config <p>   Use a custom JSONC config file");
        println!("    proto fetch --help         Show this help");
        println!();
        println!("  CONFIG:");
        println!("    On first run a default config is installed to");
        println!("    ~/.config/proto/fetch/arch.jsonc and used automatically.");
        println!("    Custom: --config /path/to/my.jsonc");
        return;
    }

    let mut cli_color: Option<String> = None;
    let mut cli_config: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--color" => {
                cli_color = args.get(i + 1).cloned();
                i += 2;
            }
            "--config" => {
                cli_config = args.get(i + 1).cloned();
                i += 2;
            }
            _ => i += 1,
        }
    }

    let path = match resolve_config(cli_config.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("proto fetch: {}", e);
            std::process::exit(1);
        }
    };

    let cfg = match load_config(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warning: {} — using built-in defaults", e);
            Config::default()
        }
    };

    let logo_style = logo_color(&cfg, cli_color.as_deref());
    let label_style = style_for(&cfg.display.color.label);
    let sep = &cfg.display.separator;

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let mut rows: Vec<String> = Vec::new();
    let mut colors_after = false;
    for m in &cfg.modules.list {
        match m.mtype.as_deref() {
            Some("colors") => colors_after = true,
            _ => {
                if let Some((label, value)) = module_value(m, &mut sys) {
                    if label.is_empty() {
                        rows.push(value);
                        continue;
                    }
                    let lt = format!("{}{} {}",
                        label.style(label_style.unwrap_or(Style::new())),
                        sep,
                        value);
                    rows.push(lt);
                }
            }
        }
    }

    println!();

    let mut logo = logo_lines(&cfg);
    if cfg.logo.padding.top > 0 {
        let blanks: Vec<&'static str> = (0..cfg.logo.padding.top).map(|_| "").collect();
        logo.splice(0..0, blanks);
    }
    let styled: Vec<String> = logo
        .iter()
        .map(|l| l.style(logo_style).to_string())
        .collect();
    let logo_w = styled.iter().map(|l| l.chars().count()).max().unwrap_or(0)
        + cfg.logo.padding.left
        + 2;

    for (i, l) in styled.iter().enumerate() {
        let pad: String = " ".repeat(logo_w.saturating_sub(l.chars().count()));
        let right = if i < rows.len() {
            rows[i].as_str()
        } else {
            ""
        };
        println!("{}{}{}", l, pad, right);
    }
    for line in rows.iter().skip(styled.len()) {
        println!("{}{}", " ".repeat(logo_w), line);
    }

    if colors_after {
        color_block();
    }
}