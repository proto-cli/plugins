use owo_colors::{AnsiColors, DynColors, OwoColorize, Style};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const DEFAULT_CONFIG: &str = include_str!("../arch.jsonc");
const DEFAULT_LOGO: &str = include_str!("../arch.txt");

fn run_output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn hostname() -> String {
    run_output("hostname", &[])
}

fn distro() -> String {
    let os = fs::read_to_string("/etc/os-release").unwrap_or_default();
    os.lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string()))
        .unwrap_or_default()
}

fn distro_id() -> String {
    let os = fs::read_to_string("/etc/os-release").unwrap_or_default();
    os.lines()
        .find_map(|l| l.strip_prefix("ID=").map(|v| v.trim().to_string()))
        .unwrap_or_default()
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
        .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
        .unwrap_or_else(|| "unknown".into())
}

fn de() -> String {
    std::env::var("XDG_CURRENT_DESKTOP")
        .or_else(|_| std::env::var("DESKTOP_SESSION"))
        .unwrap_or_default()
        .to_lowercase()
}

fn wm() -> String {
    std::env::var("XDG_SESSION_TYPE").unwrap_or_default().to_lowercase()
}

fn terminal() -> String {
    std::env::var("TERM").unwrap_or_else(|_| "unknown".into())
}

fn gpu() -> String {
    let mm = run_output("lspci", &["-mm"]);
    for l in mm.lines() {
        if l.contains("VGA") || l.contains("3D controller") || l.contains("Display controller") {
            let quoted: Vec<&str> = l.split('"').collect();
            let odd: Vec<&str> = quoted.iter().skip(1).step_by(2).map(|s| *s).collect();
            let name = odd.get(2).or_else(|| odd.get(1)).unwrap_or(&"").to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    "unknown".into()
}

fn media() -> Option<String> {
    let out = run_output("playerctl", &["metadata", "-f", "{{artist}} - {{title}}"]);
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn first_package_count() -> String {
    let full = package_counts();
    full.split('(').next().unwrap_or("").trim().to_string()
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
            "pacman" => run_output("pacman", &["-Qq"]).lines().count(),
            "dpkg" => run_output("dpkg", &["--list"]).lines().filter(|l| l.starts_with("ii")).count(),
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

// ASCII logo for the 1:1 neofetch arch preset (Bina config).
fn arch_logo() -> Vec<&'static str> {
    vec![
        "                   .",
        "                  / \\",
        "                 /   \\",
        "                /\\    \\",
        "               /       \\",
        "              /         \\",
        "             /    .-.    \\",
        "            /    |   |   _\\",
        "           /   _.'   '._   \\",
        "          /_.-'         '-._\\",
    ]
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
    right: usize,
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

#[derive(Clone)]
struct Modules {
    list: Vec<ModuleEntry>,
}

impl<'de> serde::Deserialize<'de> for Modules {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = serde_json::Value::deserialize(d)?;
        let arr = v
            .as_array()
            .cloned()
            .or_else(|| {
                v.get("list")
                    .and_then(|x| x.as_array())
                    .cloned()
            })
            .unwrap_or_default();
        let mut list = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                list.push(ModuleEntry::Str(s.to_string()));
            } else if item.is_object() {
                if let Ok(m) = serde_json::from_value::<Module>(item) {
                    list.push(ModuleEntry::Obj(m));
                }
            }
        }
        Ok(Modules { list })
    }
}

#[derive(Clone)]
enum ModuleEntry {
    Str(String),
    Obj(Module),
}

impl Default for Modules {
    fn default() -> Self {
        Self {
            list: vec![
                ModuleEntry::Obj(module("title", Some("{user-name}{at}{host-name}"))),
                ModuleEntry::Obj(module("os", None)),
                ModuleEntry::Obj(module("host", None)),
                ModuleEntry::Obj(module("kernel", None)),
                ModuleEntry::Obj(module("uptime", None)),
                ModuleEntry::Obj(module("shell", None)),
                ModuleEntry::Obj(module("de", None)),
                ModuleEntry::Obj(module("wm", None)),
                ModuleEntry::Obj(module("terminal", None)),
                ModuleEntry::Obj(module("cpu", None)),
                ModuleEntry::Obj(module("gpu", None)),
                ModuleEntry::Obj(module("memory", None)),
                ModuleEntry::Obj(module("disk", None)),
                ModuleEntry::Obj(module("packages", None)),
                ModuleEntry::Obj(module("colors", None)),
            ],
        }
    }
}

fn module(mtype: &str, format: Option<&str>) -> Module {
    Module {
        mtype: Some(mtype.into()),
        format: format.map(|s| s.into()),
        ..Module::default()
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
    key: Option<String>,
    #[serde(rename = "keyColor")]
    key_color: Option<String>,
    #[serde(rename = "keyWidth")]
    key_width: Option<usize>,
}

fn parse_jsonc(text: &str) -> String {
    let strip = |s: &str, skip_comments: bool| {
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::with_capacity(s.len());
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
                '/' if skip_comments && i + 1 < chars.len() && chars[i + 1] == '/' => {
                    while i < chars.len() && chars[i] != '\n' {
                        i += 1;
                    }
                }
                '/' if skip_comments && i + 1 < chars.len() && chars[i + 1] == '*' => {
                    i += 2;
                    while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                        i += 1;
                    }
                    i += 2;
                }
                ',' if !skip_comments && {
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
    };

    let no_comments = strip(text, true);
    strip(&no_comments, false)
}

fn load_config(path: &PathBuf) -> Result<Config, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("unable to read {}: {}", path.display(), e))?;
    let cleaned = parse_jsonc(&text);
    serde_json::from_str(&cleaned).map_err(|e| format!("invalid config {}: {}", path.display(), e))
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

    // Install the ascii logo referenced by the default config so the
    // config works even when fastfetch's logo dir does not exist yet.
    let fast_dir = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".local/share/fastfetch/ascii");
    let logo_path = fast_dir.join("arch.txt");
    if !logo_path.exists() {
        if let Some(home) = std::env::var("HOME").ok() {
            if !home.is_empty() {
                let _ = fs::create_dir_all(&fast_dir);
                let _ = fs::write(&logo_path, DEFAULT_LOGO);
            }
        }
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

// ANSI 16-color index → Style (e.g. keyColor "33" = yellow, "96" = bright cyan).
fn ansi_style(idx: &str) -> Option<Style> {
    let code = idx.trim().parse::<u8>().ok()?;
    match code {
        30 => Some(Style::new().black()),
        31 => Some(Style::new().red()),
        32 => Some(Style::new().green()),
        33 => Some(Style::new().yellow()),
        34 => Some(Style::new().blue()),
        35 => Some(Style::new().magenta()),
        36 => Some(Style::new().cyan()),
        37 => Some(Style::new().white()),
        90 => Some(Style::new().bright_black()),
        91 => Some(Style::new().bright_red()),
        92 => Some(Style::new().bright_green()),
        93 => Some(Style::new().bright_yellow()),
        94 => Some(Style::new().bright_blue()),
        95 => Some(Style::new().bright_magenta()),
        96 => Some(Style::new().bright_cyan()),
        97 => Some(Style::new().bright_white()),
        _ => None,
    }
}

fn key_style(key_color: &Option<String>) -> Style {
    key_color
        .as_deref()
        .and_then(|c| ansi_style(c).or_else(|| style_for(c)))
        .unwrap_or_else(|| Style::new().bold())
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

fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix('~') {
        if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
            return PathBuf::from(home).join(rest.trim_start_matches('/'));
        }
    }
    PathBuf::from(p)
}

fn custom_ascii(p: &str) -> Option<Vec<String>> {
    let pb = expand_tilde(p);
    let text = fs::read_to_string(&pb).ok()?;
    let width = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    Some(
        text.lines()
            .map(|l| l.trim_end_matches('\r').to_string())
            .map(|l| format!("{:<w$}", l, w = width))
            .collect(),
    )
}

fn logo_lines(cfg: &Config) -> Vec<String> {
    let src = &cfg.logo.source;
    let low = src.to_lowercase();
    match low.as_str() {
        "" | "none" | "off" | "false" | "disabled" => vec![],
        "auto" => logo_for(&distro_id()).into_iter().map(|s| s.to_string()).collect(),
        "arch" | "arch.txt" => arch_logo().into_iter().map(|s| s.to_string()).collect(),
        _ => {
            if let Some(lines) = custom_ascii(src) {
                lines
            } else if src.rsplit('/').next().unwrap_or("").to_lowercase().contains("arch") {
                arch_logo().into_iter().map(|s| s.to_string()).collect()
            } else {
                logo_for(&low)
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect()
            }
        }
    }
}

fn template(fmt: &str, vals: &[(&str, &str)]) -> String {
    let mut out = fmt.to_string();
    for (k, v) in vals {
        out = out.replace(&format!("{{{}}}", k), v);
    }
    out
}

struct Row {
    key_plain: String,
    key_style: Style,
    value: String,
    min_width: usize,
}

fn module_value(m: &Module, sys: &mut sysinfo::System) -> Option<(String, String, Vec<(&'static str, String)>)> {
    let ty = m.mtype.as_deref().unwrap_or("");
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    let host = hostname();
    let shell_path = std::env::var("SHELL").unwrap_or_default();

    let (label, value, placeholders): (&str, String, Vec<(&'static str, String)>) = match ty {
"title" => {
            let fmt = m.format.clone().unwrap_or_else(|| "{user-name}{at}{host-name}".into());
            let vals: Vec<(&'static str, String)> = vec![
                ("user-name", user),
                ("at", "@".into()),
                ("host-name", host),
            ];
            let out = template(&fmt, &vals.iter().map(|(k, v)| (*k, v.as_str())).collect::<Vec<_>>());
            return Some((String::new(), out, vals));
        }
        "os" | "distro" => {
            let v = distro();
            (
                "OS",
                v,
                vec![("pretty-name", distro()), ("name", String::new()), ("version", String::new()), ("id", distro_id())],
            )
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
            (
                "Shell",
                name,
                vec![("shell", shell_path.clone()), ("path", shell_path), ("version", String::new())],
            )
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
        "media" => {
            if let Some(track) = media() {
                ("Media", track, vec![("name", String::new())])
            } else {
                return None;
            }
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
            let full = package_counts();
            let use_first = m.format.as_deref().map_or(false, |f| f.contains("{}"));
            let (v, ph) = if use_first {
                let first = first_package_count();
                (first.clone(), vec![("count", first)])
            } else {
                (full.clone(), vec![("count", full)])
            };
            ("Packages", v, ph.into_iter().map(|(a, b)| (a, b)).collect::<Vec<_>>())
        }
        "gap" | "break" => return Some((String::new(), String::new(), Vec::new())),
        _ => return None,
    };

    Some((label.to_string(), value, placeholders))
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
    let default_label_style = style_for(&cfg.display.color.label).unwrap_or_else(Style::new);
    let sep = &cfg.display.separator;

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let mut rows: Vec<Row> = Vec::new();
    let mut colors_after = false;
    for entry in &cfg.modules.list {
        let m: Module = match entry {
            ModuleEntry::Str(s) => Module {
                mtype: Some(s.clone()),
                ..Module::default()
            },
            ModuleEntry::Obj(m) => m.clone(),
        };
        match m.mtype.as_deref() {
            Some("colors") => colors_after = true,
            Some("gap") | Some("break") => rows.push(Row {
                key_plain: String::new(),
                key_style: Style::new(),
                value: String::new(),
                min_width: 0,
            }),
            _ => {
                if let Some((label, value, placeholders)) = module_value(&m, &mut sys) {
                    let fmt = m.format.clone().unwrap_or_default();
                    // format with positional {} or named placeholders
                    if !fmt.is_empty() {
                        let rendered = if fmt.contains("{}") {
                            fmt.replacen("{}", &value, 1)
                        } else {
                            let mut vals: Vec<(&str, &str)> = placeholders
                                .iter()
                                .map(|(k, v)| (*k, v.as_str()))
                                .collect();
                            vals.push(("label", label.as_str()));
                            vals.push(("value", value.as_str()));
                            template(&fmt, &vals)
                        };
                        rows.push(Row {
                            key_plain: String::new(),
                            key_style: Style::new(),
                            value: rendered,
                            min_width: m.key_width.unwrap_or(0),
                        });
                        continue;
                    }

                    let key_plain = m.key.clone().unwrap_or_else(|| label.clone());
                    let style = if m.key.is_some() {
                        key_style(&m.key_color)
                    } else {
                        default_label_style
                    };
                    rows.push(Row {
                        key_plain,
                        key_style: style,
                        value,
                        min_width: m.key_width.unwrap_or(0),
                    });
                }
            }
        }
    }

    println!();

    let mut logo = logo_lines(&cfg);
    if cfg.logo.padding.top > 0 {
        let blanks: Vec<String> = (0..cfg.logo.padding.top).map(|_| String::new()).collect();
        logo.splice(0..0, blanks);
    }
    let styled: Vec<String> = logo
        .iter()
        .map(|l| l.style(logo_style).to_string())
        .collect();
    let logo_w = styled.iter().map(|l| l.chars().count()).max().unwrap_or(0)
        + cfg.logo.padding.left
        + cfg.logo.padding.right;

    let key_w = rows
        .iter()
        .map(|r| r.key_plain.chars().count().max(r.min_width))
        .max()
        .unwrap_or(0);

    for (i, l) in styled.iter().enumerate() {
        let pad: String = " ".repeat(logo_w.saturating_sub(l.chars().count()));
        let right = rows.get(i).map(|r| render_row(r, key_w, sep)).unwrap_or_default();
        println!("{}{}{}", l, pad, right);
    }
    for row in rows.iter().skip(styled.len()) {
        println!("{}{}", " ".repeat(logo_w), render_row(row, key_w, sep));
    }

    if colors_after {
        println!();
        color_block();
    }
}

fn render_row(r: &Row, key_w: usize, sep: &str) -> String {
    if r.key_plain.is_empty() {
        if r.value.is_empty() {
            return String::new();
        }
        return if r.min_width > 0 {
            format!("{:<w$}{}", "", r.value, w = r.min_width)
        } else {
            r.value.clone()
        };
    }
    format!(
        "{}{}{} {}",
        r.key_plain.clone().style(r.key_style),
        " ".repeat(key_w.saturating_sub(r.key_plain.chars().count())),
        sep,
        r.value
    )
}