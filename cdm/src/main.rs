use owo_colors::OwoColorize;
use std::path::PathBuf;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(50).dimmed().to_string() }

fn shell_function(shell: &str) -> Option<&'static str> {
    match shell {
        "bash" | "zsh" => Some(
            "cdm() {\n    mkdir -p \"$1\" && cd \"$1\"\n}\n",
        ),
        "fish" => Some(
            "function cdm\n    mkdir -p $argv[1]; and cd $argv[1]\nend\n",
        ),
        _ => None,
    }
}

fn rc_path(shell: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(match shell {
        "bash" => home.join(".bashrc"),
        "zsh" => home.join(".zshrc"),
        "fish" => home.join(".config/fish/config.fish"),
        _ => return None,
    })
}

fn current_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .map(|s| {
            PathBuf::from(&s)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

fn install_init(shell: &str) {
    let Some(func) = shell_function(shell) else {
        eprintln!("  {} Unsupported shell: {} (use bash, zsh, fish)", error(""), shell);
        std::process::exit(1);
    };
    let Some(path) = rc_path(shell) else {
        eprintln!("  {} Could not resolve home directory", error(""));
        std::process::exit(1);
    };

    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains("cdm()") || existing.contains("function cdm") {
        println!("  {} cdm() already defined in {}", success(""), path.display().to_string().dimmed());
        println!("  {} {} {}", "Run:".style(Theme::MUTED), format!("source {}", path.display()).style(Theme::ACCENT), "then use `cdm <dir>`".dimmed());
        return;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&path, format!("{}\n# proto cdm\n{}\n", existing, func)).ok();
    println!("  {} installed → {}", success(""), path.display().to_string().dimmed());
    println!(
        "  {} {} {}",
        "Now run:".style(Theme::MUTED),
        format!("source {}", path.display()).style(Theme::ACCENT),
        "then use `cdm <dir>`".dimmed()
    );
}

fn create_dirs(dirs: &[String]) {
    if dirs.is_empty() {
        print_help();
        return;
    }

    let single = dirs.len() == 1 && !dirs[0].starts_with("--");
    let mut created = 0;
    let mut failed = 0;

    for d in dirs {
        if d.starts_with('-') { continue; }
        let path = PathBuf::from(d);
        match std::fs::create_dir_all(&path) {
            Ok(_) => {
                created += 1;
                println!("  {} {}", success(&format!("created")), path.display().to_string().style(Theme::VALUE));
            }
            Err(e) => {
                failed += 1;
                eprintln!("  {} {} {}", error(""), path.display(), e);
            }
        }
    }

    if single && failed == 0 {
        let path = PathBuf::from(&dirs[0]);
        let abs = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()
                .unwrap_or_default()
                .join(&path)
        };
        println!();
        println!("  {} {}", "→ cd here:".style(Theme::MUTED), abs.display().to_string().style(Theme::ACCENT).bold());
        println!(
            "  {} {}",
            "  (or run".dimmed(),
            format!("cdm {}", dirs[0]).style(Theme::SUCCESS),
        );
        println!("  {} {}", "   from your shell — install with".dimmed(), "proto cdm init".style(Theme::ACCENT));
        println!();
    }

    if created == 0 && failed > 0 {
        std::process::exit(1);
    }
}

fn print_help() {
    println!("{} mkdir + cd", header("proto"));
    println!("{}", divider());
    println!();
    println!("  USAGE:");
    println!("    proto cdm <dir> [dir...]    Create one or more directories");
    println!("    proto cdm init [shell]      Install cdm() shell function (auto-cd)");
    println!("    proto cdm --help            Show this help");
    println!();
    println!("  The cdm() function makes `cdm <dir>` create AND cd into the dir.");
    println!("  Example:");
    println!("    proto cdm init              install once, then:");
    println!("    cdm work/project            create dir + jump into it");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h" || a == "help") {
        print_help();
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "init") {
        let shell = args
            .get(pos + 1)
            .cloned()
            .unwrap_or_else(current_shell);
        if shell.is_empty() {
            let shell = "bash";
            install_init(shell);
        } else {
            install_init(&shell);
        }
        return;
    }

    create_dirs(&args);
}