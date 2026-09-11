use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::process::Command;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
    const DIR: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
}

fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(50).dimmed().to_string() }

fn on_path(binary: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join(binary).is_file()))
        .unwrap_or(false)
}

fn run_quiet(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

fn find_with_tool(query: &str, root: &str, max_depth: usize, files_only: bool, dirs_only: bool) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if on_path("rg") {
        if let Some(out) = run_quiet("rg", &[
            "--files",
            "--hidden",
            "--no-ignore",
            "-g",
            &format!("*{}*", query),
            root,
        ]) {
            results.extend(out.lines().map(PathBuf::from));
        }
    } else if on_path("fd") {
        let mut args = vec!["-H", "-d", "", ".*"];
        if files_only { args.push("-t"); args.push("f"); }
        if dirs_only { args.push("-t"); args.push("d"); }
        if let Some(out) = run_quiet("fd", &args) {
            results.extend(out.lines().map(PathBuf::from));
        }
    } else {
        let pattern = format!("*{}*", query);
        let mut args = vec![root, "-iname", &pattern];
        if let Some(out) = run_quiet("find", &args) {
            results.extend(out.lines().map(PathBuf::from));
        }
    }

    if results.is_empty() && !query.is_empty() {
        let depth = max_depth.to_string();
        let pattern = format!("*{}*", query);
        let mut args = vec![root, "-maxdepth", &depth, "-iname", &pattern];
        if files_only { args.push("-type"); args.push("f"); }
        if dirs_only { args.push("-type"); args.push("d"); }
        if let Some(out) = run_quiet("find", &args) {
            results.extend(out.lines().map(PathBuf::from));
        }
    }

    results.retain(|p| {
        let is_dir = p.is_dir();
        !(files_only && is_dir) && !(dirs_only && !is_dir)
    });
    results.sort();
    results.dedup();
    results
}

fn preview_file(path: &PathBuf) {
    if path.is_dir() {
        println!("  {}", path.display().to_string().style(Theme::DIR));
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .map(|rd| rd.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        entries.sort();
        for e in entries.iter().take(12) {
            let marker = if e.is_dir() { "/".to_string() } else { " ".to_string() };
            println!("    {} {}", marker, e.file_name().unwrap_or_default().to_string_lossy().dimmed());
        }
        return;
    }
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let size = std::fs::metadata(path).map(|m| bytes_to_human(m.len())).unwrap_or_default();
    let lines = std::fs::read_to_string(path)
        .unwrap_or_else(|_| "<binary>".to_string());
    println!("  {} {}", path.display().to_string().style(Theme::ACCENT), muted(&size));
    for line in lines.lines().take(20) {
        println!("  {}", line.dimmed());
    }
    if lines.lines().count() > 20 {
        println!("  {}", muted("… truncated"));
    }
}

fn bytes_to_human(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    if bytes < 1024 { return format!("{} B", bytes); }
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 { v /= 1024.0; i += 1; }
    format!("{:.1} {}", v, UNITS[i])
}

fn interactive(root: &str, files_only: bool, dirs_only: bool) {
    use dialoguer::FuzzySelect;
    let engine = if on_path("rg") { "ripgrep" } else if on_path("fd") { "fd" } else { "find" };
    println!("{}", header(&format!("Find interactive — engine: {}", engine)));
    println!("{}", divider());

    let all = find_with_tool("", root, 5, files_only, dirs_only);
    if all.is_empty() {
        println!("  {} No matches", muted(""));
        return;
    }
    let mut current = all.clone();
    loop {
        if current.is_empty() {
            println!("  {} No matches", muted(""));
            return;
        }
        let items: Vec<String> = current
            .iter()
            .map(|p| {
                let display = p.to_string_lossy();
                if p.is_dir() { format!("{} {}/", "◆".style(Theme::ACCENT), display.style(Theme::DIR)) } else { display.to_string() }
            })
            .collect();

        let sel = FuzzySelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Search (type to filter, Enter to select, Esc to quit)")
            .items(&items)
            .default(0)
            .interact_opt()
            .ok()
            .flatten();

        match sel {
            None => { println!("  {} {}", muted("⇤"), muted("cancelled")); return; }
            Some(idx) => {
                let path = &current[idx];
                println!("\n{}", divider());
                preview_file(path);
                println!("{}", divider());
                println!("  {} {} {}", "Enter".green(), "open".dimmed(), "  ".dimmed());
                println!("  {} {} {}", "p".cyan(), "print path".dimmed(), "  ".dimmed());
                println!("  {} {}", "any other key".dimmed(), "continue browsing".dimmed());
                use std::io::Read;
                let mut buf = [0u8; 1];
                let _ = std::io::stdin().read(&mut buf);
                match buf[0] as char {
                    '\n' => {
                        let cwd = std::env::current_dir().unwrap_or_default();
                        let target = if path.is_dir() {
                            path.clone()
                        } else {
                            path.parent().map(|p| p.to_path_buf()).unwrap_or(path.clone())
                        };
                        let rel = target.strip_prefix(&cwd).unwrap_or(&target);
                        println!("  {} {}", "cd".green(), rel.display().to_string().style(Theme::ACCENT));
                        return;
                    }
                    'p' | 'P' => {
                        println!("{}", path.display());
                        return;
                    }
                    _ => {
                        // refresh and continue
                        current = find_with_tool("", root, 5, files_only, dirs_only);
                    }
                }
            }
        }
    }
}

fn print_help() {
    println!("{} Interactive Finder", header("proto"));
    println!("{}", divider());
    println!();
    println!("  USAGE:");
    println!("    proto find [pattern] [dir]    Search files, print matches");
    println!("    proto find -i [pattern]       Interactive fuzzy selection");
    println!("    proto find -f -i [pattern]    Files only");
    println!("    proto find -d -i [pattern]    Directories only");
    println!("    proto find --help             Show this help");
    println!();
    println!("  Backends: ripgrep → fd → find (auto-detected)");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let mut interactive_mode = false;
    let mut files_only = false;
    let mut dirs_only = false;
    let mut positional: Vec<String> = Vec::new();

    for a in &args {
        match a.as_str() {
            "-i" | "--interactive" => interactive_mode = true,
            "-f" | "--files" => files_only = true,
            "-d" | "--dirs" => dirs_only = true,
            _ => positional.push(a.clone()),
        }
    }

    let query = positional.first().cloned().unwrap_or_default();
    let root = positional.get(1).cloned().unwrap_or_else(|| ".".into());

    if interactive_mode {
        interactive(&root, files_only, dirs_only);
        return;
    }

    let results = find_with_tool(&query, &root, 5, files_only, dirs_only);
    if results.is_empty() {
        println!("  {} No matches for '{}' in {}", muted(""), query.style(Theme::WARN), root.style(Theme::MUTED));
        return;
    }
    println!("  {} {} match(es)\n", results.len().to_string().style(Theme::ACCENT), "●".dimmed());
    for p in &results {
        if p.is_dir() {
            println!("  {} {}/", "◆".style(Theme::ACCENT), p.display().to_string().style(Theme::DIR));
        } else {
            println!("  {}", p.display().to_string().dimmed());
        }
    }
}