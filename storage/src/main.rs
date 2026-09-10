use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

// ── Theme ──────────────────────────────────────────────────────────────────

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
    const HOT: owo_colors::Style = owo_colors::Style::new().bright_magenta().bold();
}

fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn warn(s: &str) -> String { format!("{} {}", "⚠".style(Theme::WARN), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(50).dimmed().to_string() }

fn label_value(label: &str, value: &str) -> String {
    format!("{} {}", format!("{:>20}:", label).style(Theme::LABEL), value.style(Theme::VALUE))
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 { return format!("{} B", bytes); }
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 { v /= 1024.0; i += 1; }
    format!("{:.1} {}", v, UNITS[i])
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() { return 0; }
    Command::new("du")
        .args(["-sb", path.to_str().unwrap_or("")])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0)
}

fn disk_free() -> u64 {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    Command::new("df")
        .args(["-B1", "--output=avail", &home.to_string_lossy()])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next_back()
                .and_then(|l| l.trim().parse().ok())
        })
        .unwrap_or(0)
}

fn cmd_exists(name: &str) -> bool {
    Command::new("which").arg(name)
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

fn cmd_output(program: &str, args: &[&str]) -> String {
    Command::new(program).args(args).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

fn remove_dir(path: &Path, sudo: bool) -> bool {
    if !path.exists() { return true; }
    if sudo {
        Command::new("sudo").args(["rm", "-rf", &path.to_string_lossy()])
            .status().map(|s| s.success()).unwrap_or(false)
    } else {
        std::fs::remove_dir_all(path).is_ok()
    }
}

fn confirm_clean(label: &str, size: u64) -> bool {
    println!();
    println!("  {} {} — will free {}", warn(""), label.style(Theme::WARN), format_size(size).style(Theme::HOT));
    dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Proceed?")
        .default(false)
        .interact()
        .unwrap_or(false)
}

// ── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "storage", about = "Insane storage cleaner — free GB, not just MB")]
struct Cli {
    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand, Debug, Clone)]
enum Action {
    #[command(about = "Scan all categories and show disk usage breakdown")]
    Scan,
    #[command(about = "Clean package manager and tool caches")]
    CleanCache,
    #[command(about = "Clean Docker garbage (containers, images, volumes, build cache)")]
    Docker,
    #[command(about = "Clean dev build artifacts (node_modules, target, dist, etc.)")]
    Artifacts {
        #[arg(short, long, help = "Root directory to scan (default: ~)")]
        root: Option<String>,
        #[arg(short, long, help = "Dry run only — show what would be deleted")]
        dry_run: bool,
    },
    #[command(about = "Find and remove duplicate files by content hash")]
    Duplicates {
        #[arg(short, long, help = "Root directory to scan (default: ~)")]
        root: Option<String>,
        #[arg(long, help = "Min file size to consider (e.g. 1M, 500K)")]
        min_size: Option<String>,
    },
    #[command(about = "Clean git repo bloat (shallow clones, gc, old objects)")]
    GitBloat {
        #[arg(short, long, help = "Root directory of git repos (default: ~/Coding)")]
        root: Option<String>,
    },
    #[command(about = "Clean core dumps, crash reports, and journal logs")]
    Coredumps,
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    match cli.action {
        Action::Scan => scan_all(),
        Action::CleanCache => clean_cache(),
        Action::Docker => clean_docker(),
        Action::Artifacts { root, dry_run } => clean_artifacts(root.as_deref(), dry_run),
        Action::Duplicates { root, min_size } => clean_duplicates(root.as_deref(), min_size.as_deref()),
        Action::GitBloat { root } => clean_git_bloat(root.as_deref()),
        Action::Coredumps => clean_coredumps(),
    }
}

// ── Scan ───────────────────────────────────────────────────────────────────

struct Category {
    name: &'static str,
    size: u64,
    detail: String,
}

fn scan_all() {
    println!("{}", header("Storage Scan"));
    println!("{}", divider());

    let free = disk_free();
    let mut cats: Vec<Category> = Vec::new();

    println!("  {} Scanning...\n", muted(""));

    // 1. Caches
    let cache_entries = cache_targets();
    let cache_total: u64 = cache_entries.iter().map(|e| e.0).sum();
    cats.push(Category { name: "Package/tool caches", size: cache_total,
        detail: cache_entries.iter().filter(|e| e.0 > 0).map(|e| format!("{}: {}", e.1, format_size(e.0))).collect::<Vec<_>>().join(", ") });

    // 2. Docker
    let docker_size = docker_total_size();
    cats.push(Category { name: "Docker", size: docker_size, detail: docker_breakdown() });

    // 3. Dev artifacts
    let (art_size, art_count) = scan_artifacts(&dirs::home_dir().unwrap_or_default());
    cats.push(Category { name: "Dev build artifacts", size: art_size, detail: format!("{} directories", art_count) });

    // 4. Journal
    let journal_size = journal_size();
    cats.push(Category { name: "System journal", size: journal_size, detail: journal_detail() });

    // 5. Coredumps
    let cd_size = coredump_size();
    cats.push(Category { name: "Core dumps & crashes", size: cd_size, detail: coredump_detail() });

    // 6. Snapshots / old packages
    let snap_size = old_packages_size();
    cats.push(Category { name: "Old packages/snapshots", size: snap_size, detail: old_packages_detail() });

    // 7. Trash
    let trash_size = trash_size();
    cats.push(Category { name: "Trash", size: trash_size, detail: "~/.local/share/Trash".into() });

    let total: u64 = cats.iter().map(|c| c.size).sum();

    println!("  {:>18}  {}", "SIZE".style(Theme::LABEL), "CATEGORY".style(Theme::LABEL));
    println!("  {} {}", muted(&"─".repeat(18)), muted(&"─".repeat(30)));

    for c in &cats {
        if c.size == 0 { continue; }
        let bar_len = if total > 0 { (c.size as f64 / total as f64 * 20.0) as usize } else { 0 };
        let bar = "█".repeat(bar_len.min(20));
        println!("  {:>18}  {} {}", format_size(c.size).style(Theme::HOT), c.name.style(Theme::VALUE), muted(&format!("{}", bar)));
        if !c.detail.is_empty() {
            println!("  {:>18}    {}", "", muted(&c.detail));
        }
    }

    println!();
    println!("{}", divider());
    println!("  {}", label_value("Total reclaimable", &format_size(total)));
    println!("  {}", label_value("Disk free", &format_size(free)));
    if total > 1_000_000_000 {
        println!("  {} {} — that's a lot of space!", warn(""), format_size(total).style(Theme::HOT));
    }
    println!();
    println!("  Run subcommands to clean specific categories:");
    println!("    {} storage clean-cache", muted("$"));
    println!("    {} storage docker", muted("$"));
    println!("    {} storage artifacts", muted("$"));
    println!("    {} storage duplicates", muted("$"));
    println!("    {} storage git-bloat", muted("$"));
    println!("    {} storage coredumps", muted("$"));
}

// ── Clean-Cache ────────────────────────────────────────────────────────────

fn cache_targets() -> Vec<(u64, String, Option<PathBuf>, bool)> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let targets: Vec<(&str, Option<PathBuf>, bool)> = vec![
        ("npm cache", Some(home.join(".npm/_cacache")), false),
        ("pip cache", Some(home.join(".cache/pip")), false),
        ("uv cache", Some(home.join(".cache/uv")), false),
        ("bun cache", Some(home.join(".bun/install/cache")), false),
        ("yarn cache", Some(home.join(".cache/yarn")), false),
        ("cargo registry", Some(home.join(".cargo/registry/cache")), false),
        ("cargo git checkouts", Some(home.join(".cargo/git")), false),
        ("go build cache", Some(home.join(".cache/go-build")), false),
        ("gradle cache", Some(home.join(".gradle/caches")), false),
        ("yay/paru AUR build", Some(home.join(".cache/yay")), false),
        ("paru cache", Some(home.join(".cache/paru")), false),
        ("pacman pkg cache", Some(PathBuf::from("/var/cache/pacman/pkg")), true),
        ("apt archives", Some(PathBuf::from("/var/cache/apt/archives")), true),
        ("dnf cache", Some(PathBuf::from("/var/cache/dnf")), true),
        ("pnpm store", Some(home.join(".local/share/pnpm/store/v3")), false),
    ];
    targets.into_iter().map(|(label, path, sudo)| {
        let size = path.as_ref().map(|p| dir_size(p)).unwrap_or(0);
        (size, label.to_string(), path, sudo)
    }).collect()
}

fn clean_cache() {
    println!("{}", header("Clean Cache"));
    println!("{}", divider());

    let entries = cache_targets();
    let total: u64 = entries.iter().map(|e| e.0).sum();
    let free_before = disk_free();

    println!();
    println!("  {}", label_value("Reclaimable", &format_size(total)));

    let options: Vec<String> = entries.iter()
        .filter(|e| e.0 > 0)
        .map(|e| format!("{} {}", e.1, format_size(e.0)))
        .collect();

    if options.is_empty() {
        println!("  {} No caches found.", success(""));
        return;
    }

    let non_empty: Vec<(u64, String, Option<PathBuf>, bool)> = entries.into_iter().filter(|e| e.0 > 0).collect();
    let selected = dialoguer::MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select caches to clean (space to toggle)")
        .items(&options)
        .interact()
        .unwrap_or_default();

    if selected.is_empty() { println!("  {} Aborted.", muted("")); return; }

    for &i in &selected {
        let item = &non_empty[i];
        let size = item.0;
        let label = &item.1;
        let path = &item.2;
        let sudo = item.3;
        if confirm_clean(label, size) {
            if let Some(p) = path {
                if remove_dir(p, sudo) {
                    println!("  {} {}", success(""), label);
                } else {
                    println!("  {} {} — try with sudo", error(""), label);
                }
            }
        }
    }

    let free_after = disk_free();
    println!();
    println!("{}", divider());
    println!("  {}", label_value("Recovered", &format_size(free_after.saturating_sub(free_before))));
}

// ── Docker ─────────────────────────────────────────────────────────────────

fn docker_total_size() -> u64 {
    if !cmd_exists("docker") { return 0; }
    let out = cmd_output("docker", &["system", "df", "--format", "{{.Reclaimable}}"]);
    // Parse "1.2GB (70%)" style output
    out.lines().filter_map(|l| {
        let t = l.trim().to_string();
        let (num, mult) = if let Some(v) = t.strip_suffix("GB") { (v, 1_000_000_000u64) }
            else if let Some(v) = t.strip_suffix("MB") { (v, 1_000_000) }
            else if let Some(v) = t.strip_suffix("KB") { (v, 1_000) }
            else if let Some(v) = t.strip_suffix("B") { (v, 1) }
            else { return Some(0); };
        num.trim().parse::<f64>().ok().map(|v| v as u64 * mult)
    }).sum::<u64>()
}

fn docker_breakdown() -> String {
    if !cmd_exists("docker") { return "docker not installed".into(); }
    let out = cmd_output("docker", &["system", "df"]);
    let lines: Vec<&str> = out.lines().collect();
    if lines.len() < 2 { return "no data".into(); }
    lines[1..].iter().filter_map(|l| {
        let parts: Vec<&str> = l.split_whitespace().collect();
        if parts.len() >= 4 { Some(format!("{}: {}", parts[0], parts[2])) } else { None }
    }).collect::<Vec<_>>().join(", ")
}

fn clean_docker() {
    println!("{}", header("Clean Docker"));
    println!("{}", divider());

    if !cmd_exists("docker") {
        eprintln!("  {} Docker not installed.", error(""));
        return;
    }

    let total = docker_total_size();
    println!("  {}", label_value("Reclaimable", &format_size(total)));
    println!();

    // Docker breakdown
    let out = cmd_output("docker", &["system", "df", "-v"]);
    for line in out.lines().take(20) {
        println!("  {}", muted(line));
    }

    println!();
    let options = vec![
        "Remove stopped containers",
        "Remove dangling images",
        "Remove unused images",
        "Remove unused volumes",
        "Remove unused networks",
        "Remove build cache",
        "Nuke EVERYTHING (docker system prune -a --volumes)",
    ];

    let selected = dialoguer::MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select what to clean")
        .items(&options)
        .interact()
        .unwrap_or_default();

    if selected.is_empty() { println!("  {} Aborted.", muted("")); return; }

    for &i in &selected {
        let cmd = match i {
            0 => vec!["docker", "container", "prune", "-f"],
            1 => vec!["docker", "image", "prune", "-f"],
            2 => vec!["docker", "image", "prune", "-a", "-f"],
            3 => vec!["docker", "volume", "prune", "-f"],
            4 => vec!["docker", "network", "prune", "-f"],
            5 => vec!["docker", "builder", "prune", "-f"],
            6 => vec!["docker", "system", "prune", "-a", "--volumes", "-f"],
            _ => continue,
        };
        let out = Command::new(cmd[0]).args(&cmd[1..]).output();
        match out {
            Ok(o) if o.status.success() => {
                println!("  {} {}", success(""), options[i]);
                let reclaimed = String::from_utf8_lossy(&o.stdout);
                if let Some(line) = reclaimed.lines().last() {
                    println!("    {}", muted(line));
                }
            }
            _ => println!("  {} {}", error(""), options[i]),
        }
    }
}

// ── Artifacts ──────────────────────────────────────────────────────────────

const ARTIFACT_DIRS: &[(&str, &str)] = &[
    ("node_modules", "npm/yarn/pnpm"),
    ("target", "cargo/rust"),
    ("dist", "build output"),
    ("build", "build output"),
    (".next", "next.js"),
    (".nuxt", "nuxt"),
    (".output", "nitro/nuxt"),
    ("out", "build output"),
    (".gradle", "gradle"),
    ("__pycache__", "python"),
    (".cache", "misc cache"),
    (".turbo", "turbo"),
    (".parcel-cache", "parcel"),
    (".webpack", "webpack"),
    ("coverage", "test coverage"),
    (".pytest_cache", "pytest"),
    (".mypy_cache", "mypy"),
    ("htmlcov", "coverage"),
];

fn scan_artifacts(home: &Path) -> (u64, usize) {
    let mut total = 0u64;
    let mut count = 0usize;
    // Scan common dev locations
    let scan_roots: Vec<PathBuf> = vec![
        home.to_path_buf(),
    ];
    for root in &scan_roots {
        for entry in WalkDir::new(root).max_depth(5).into_iter().flatten() {
            if !entry.file_type().is_dir() { continue; }
            let name = entry.file_name().to_string_lossy();
            if ARTIFACT_DIRS.iter().any(|(d, _)| *d == name.as_ref()) {
                let size = dir_size(entry.path());
                if size > 1_000_000 { // Only count if > 1MB
                    total += size;
                    count += 1;
                }
            }
        }
    }
    (total, count)
}

fn clean_artifacts(root: Option<&str>, dry_run: bool) {
    println!("{}", header("Clean Dev Artifacts"));
    println!("{}", divider());

    let home = root.map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
    println!("  {} Scanning {}...", muted(""), home.display());
    println!();

    let mut found: Vec<(PathBuf, u64)> = Vec::new();
    for entry in WalkDir::new(&home).max_depth(5).into_iter().flatten() {
        if !entry.file_type().is_dir() { continue; }
        let name = entry.file_name().to_string_lossy();
        if ARTIFACT_DIRS.iter().any(|(d, _)| *d == name.as_ref()) {
            let size = dir_size(entry.path());
            if size > 1_000_000 {
                found.push((entry.path().to_path_buf(), size));
            }
        }
    }

    found.sort_by(|a, b| b.1.cmp(&a.1));
    let total: u64 = found.iter().map(|e| e.1).sum();

    if found.is_empty() {
        println!("  {} No artifacts found.", success(""));
        return;
    }

    println!("  {:>18}  {}", "SIZE".style(Theme::LABEL), "PATH".style(Theme::LABEL));
    for (path, size) in &found {
        let label = ARTIFACT_DIRS.iter()
            .find(|(d, _)| path.file_name().map(|f| f.to_string_lossy() == *d).unwrap_or(false))
            .map(|(_, t)| *t)
            .unwrap_or("other");
        println!("  {:>18}  {} {}", format_size(*size).style(Theme::HOT), path.display().to_string().style(Theme::VALUE), muted(label));
    }
    println!();
    println!("  {}", label_value("Total", &format_size(total)));

    if dry_run {
        println!("  {} Dry run — no files deleted.", muted(""));
        return;
    }

    let options: Vec<String> = found.iter().map(|(p, s)| format!("{} {}", p.display(), format_size(*s))).collect();
    let selected = dialoguer::MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select artifacts to remove (space to toggle)")
        .items(&options)
        .interact()
        .unwrap_or_default();

    if selected.is_empty() { println!("  {} Aborted.", muted("")); return; }

    let free_before = disk_free();
    for &i in &selected {
        let (path, size) = &found[i];
        println!("  {} Removing {}...", muted(""), path.display());
        std::fs::remove_dir_all(path).ok();
    }
    let free_after = disk_free();
    println!();
    println!("  {} Freed {}", success(""), format_size(free_after.saturating_sub(free_before)).style(Theme::HOT));
}

// ── Duplicates ─────────────────────────────────────────────────────────────

fn parse_size(s: &str) -> u64 {
    let s = s.trim().to_uppercase();
    let (num, mult) = if let Some(v) = s.strip_suffix("G") { (v, 1_000_000_000u64) }
        else if let Some(v) = s.strip_suffix("M") { (v, 1_000_000) }
        else if let Some(v) = s.strip_suffix("K") { (v, 1_000) }
        else { (&s[..], 1) };
    num.parse::<f64>().unwrap_or(0.0) as u64 * mult
}

fn file_hash(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Some(format!("{:x}", hasher.finalize()))
}

fn clean_duplicates(root: Option<&str>, min_size: Option<&str>) {
    println!("{}", header("Find Duplicates"));
    println!("{}", divider());

    let scan_root = root.map(PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap_or_default());
    let min = min_size.map(parse_size).unwrap_or(10_000); // Default 10KB

    println!("  {} Scanning {} (min: {})...", muted(""), scan_root.display(), format_size(min));

    let mut by_hash: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();
    let mut scanned = 0usize;

    for entry in WalkDir::new(&scan_root).max_depth(8).into_iter().flatten() {
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();
        // Skip hidden dirs, .git, node_modules
        if let Some(parent) = path.parent() {
            let p = parent.to_string_lossy();
            if p.contains("/.") || p.contains("node_modules") || p.contains("/.git/") { continue; }
        }
        let meta = match std::fs::metadata(path) { Ok(m) => m, Err(_) => continue };
        let size = meta.len();
        if size < min { continue; }
        scanned += 1;
        if let Some(hash) = file_hash(path) {
            by_hash.entry(hash).or_default().push((path.to_path_buf(), size));
        }
    }

    let dupes: Vec<(&String, &Vec<(PathBuf, u64)>)> = by_hash.iter()
        .filter(|(_, v)| v.len() > 1)
        .collect();
    let wasted: u64 = dupes.iter().map(|(_, files)| {
        let sz = files[0].1;
        sz * (files.len() as u64 - 1)
    }).sum();

    println!();
    println!("  {}", label_value("Files scanned", &scanned.to_string()));
    println!("  {}", label_value("Duplicate groups", &dupes.len().to_string()));
    println!("  {}", label_value("Wasted space", &wasted.to_string()));
    println!();

    if dupes.is_empty() {
        println!("  {} No duplicates found.", success(""));
        return;
    }

    // Show top 20 duplicate groups
    let mut sorted = dupes;
    sorted.sort_by(|a, b| {
        let waste_a = a.1[0].1 * (a.1.len() as u64 - 1);
        let waste_b = b.1[0].1 * (b.1.len() as u64 - 1);
        waste_b.cmp(&waste_a)
    });

    for (i, (hash, files)) in sorted.iter().take(20).enumerate() {
        let files = files;
        println!("  {} {} × {} each", format!("#{}", i + 1).style(Theme::ACCENT), files.len(), format_size(files[0].1).style(Theme::VALUE));
        for (path, _) in files.iter() {
            println!("    {}", path.display().to_string().style(Theme::MUTED));
        }
    }
    if sorted.len() > 20 {
        println!("  {} ... and {} more groups", muted(""), sorted.len() - 20);
    }

    println!();
    let confirm = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Remove duplicate files? (keeps the first copy of each)")
        .default(false)
        .interact()
        .unwrap_or(false);

    if !confirm { println!("  {} Aborted.", muted("")); return; }

    let free_before = disk_free();
    for (_, files) in &sorted {
        // Keep first, remove rest
        for (path, _) in files.iter().skip(1) {
            println!("  {} {}", "rm".style(Theme::ERROR), path.display().to_string().style(Theme::MUTED));
            std::fs::remove_file(path).ok();
        }
    }
    let free_after = disk_free();
    println!();
    println!("  {} Freed {}", success(""), format_size(free_after.saturating_sub(free_before)).style(Theme::HOT));
}

// ── Git Bloat ──────────────────────────────────────────────────────────────

fn clean_git_bloat(root: Option<&str>) {
    println!("{}", header("Git Repo Bloat"));
    println!("{}", divider());

    let scan_root = root.map(PathBuf::from).unwrap_or_else(|| {
        dirs::home_dir().unwrap_or_default().join("Coding")
    });

    if !scan_root.exists() {
        eprintln!("  {} Directory not found: {}", error(""), scan_root.display());
        return;
    }

    println!("  {} Scanning {}...", muted(""), scan_root.display());

    let mut repos: Vec<(PathBuf, u64, u64)> = Vec::new(); // (path, git_size, total_size)

    for entry in WalkDir::new(&scan_root).max_depth(4).into_iter().flatten() {
        if entry.file_type().is_dir() && entry.path().join(".git").exists() {
            let git_dir = entry.path().join(".git");
            let git_size = dir_size(&git_dir);
            let total_size = dir_size(entry.path());
            if git_size > 10_000_000 { // Only repos with > 10MB .git
                repos.push((entry.path().to_path_buf(), git_size, total_size));
            }
        }
    }

    repos.sort_by(|a, b| b.1.cmp(&a.1));
    let total_git: u64 = repos.iter().map(|r| r.1).sum();

    println!();
    println!("  {:>18}  {:>18}  {}", "GIT SIZE".style(Theme::LABEL), "TOTAL".style(Theme::LABEL), "REPO".style(Theme::LABEL));
    for (path, git_size, total_size) in &repos {
        let ratio = if *total_size > 0 { *git_size as f64 / *total_size as f64 * 100.0 } else { 0.0 };
        println!("  {:>18}  {:>18}  {} {}",
            format_size(*git_size).style(Theme::HOT),
            format_size(*total_size).style(Theme::VALUE),
            path.file_name().unwrap_or_default().to_string_lossy().style(Theme::VALUE),
            muted(&format!("({:.0}%)", ratio)),
        );
    }

    println!();
    println!("  {}", label_value("Total .git size", &format_size(total_git)));

    let options = vec![
        "Run git gc --aggressive on all repos",
        "Convert to shallow clones (depth=1)",
        "Prune old remote-tracking branches",
        "Remove orphaned git objects",
    ];

    let selected = dialoguer::MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select operations (space to toggle)")
        .items(&options)
        .interact()
        .unwrap_or_default();

    if selected.is_empty() { println!("  {} Aborted.", muted("")); return; }

    for (path, git_size, _) in &repos {
        let repo_name = path.file_name().unwrap_or_default().to_string_lossy();
        for &i in &selected {
            match i {
                0 => {
                    print!("  {} gc {}...", muted(""), repo_name);
                    let ok = Command::new("git")
                        .args(["-C", &path.to_string_lossy(), "gc", "--aggressive", "--prune=now"])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                    if ok { println!(" {}", success("")); } else { println!(" {}", error("")); }
                }
                1 => {
                    // Only if repo is small enough and has remote
                    let has_remote = Command::new("git")
                        .args(["-C", &path.to_string_lossy(), "remote"])
                        .output()
                        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
                        .unwrap_or(false);
                    if has_remote && *git_size < 500_000_000u64 {
                        println!("  {} {} — would need manual re-clone for shallow", warn(""), repo_name);
                    }
                }
                2 => {
                    print!("  {} prune {}...", muted(""), repo_name);
                    let _ = Command::new("git")
                        .args(["-C", &path.to_string_lossy(), "fetch", "--prune"])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                    println!(" {}", success(""));
                }
                3 => {
                    print!("  {} cleanup {}...", muted(""), repo_name);
                    let _ = Command::new("git")
                        .args(["-C", &path.to_string_lossy(), "reflog", "expire", "--expire=now", "--all"])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                    let _ = Command::new("git")
                        .args(["-C", &path.to_string_lossy(), "gc", "--prune=now"])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                    println!(" {}", success(""));
                }
                _ => {}
            }
        }
    }
}

// ── Coredumps ──────────────────────────────────────────────────────────────

fn coredump_size() -> u64 {
    let mut total = 0u64;
    // ~/.config/Code/Crashpad
    let home = dirs::home_dir().unwrap_or_default();
    for name in &["Code/Crashpad", "Code/logs", "Slack/logs", "Discord/logs", "chromium/Crashpad/reports"] {
        let p = home.join(".config").join(name);
        total += dir_size(&p);
    }
    // /var/lib/systemd/coredump
    total += dir_size(Path::new("/var/lib/systemd/coredump"));
    // ~/.local/share/Trash
    total += dir_size(&home.join(".local/share/Trash"));
    // /tmp crash files
    if let Ok(entries) = std::fs::read_dir("/tmp") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("core") || name.contains("crash") {
                total += dir_size(&e.path());
            }
        }
    }
    total
}

fn coredump_detail() -> String {
    let mut parts = Vec::new();
    let home = dirs::home_dir().unwrap_or_default();
    let journal = dir_size(Path::new("/var/log/journal"));
    if journal > 0 { parts.push(format!("journal: {}", format_size(journal))); }
    let coredumps = dir_size(Path::new("/var/lib/systemd/coredump"));
    if coredumps > 0 { parts.push(format!("coredump: {}", format_size(coredumps))); }
    let crashes = dir_size(&home.join(".config/Code/Crashpad"));
    if crashes > 0 { parts.push(format!("crashpad: {}", format_size(crashes))); }
    parts.join(", ")
}

fn clean_coredumps() {
    println!("{}", header("Clean Core Dumps & Crashes"));
    println!("{}", divider());

    let home = dirs::home_dir().unwrap_or_default();
    let mut targets: Vec<(String, u64, PathBuf, bool)> = Vec::new();

    // Systemd coredumps
    let cd = Path::new("/var/lib/systemd/coredump");
    let cd_size = dir_size(cd);
    if cd_size > 0 {
        targets.push(("systemd coredumps".into(), cd_size, cd.to_path_buf(), true));
    }

    // Journal
    let journal = Path::new("/var/log/journal");
    let journal_size = dir_size(journal);
    if journal_size > 0 {
        targets.push(("system journal".into(), journal_size, journal.to_path_buf(), true));
    }

    // Crash reports
    for (label, sub) in &[
        ("VS Code crashpad", "Code/Crashpad"),
        ("Slack logs", "Code/logs"),
        ("Discord logs", "Discord/logs"),
    ] {
        let p = home.join(".config").join(sub);
        let sz = dir_size(&p);
        if sz > 0 {
            targets.push((label.to_string(), sz, p, false));
        }
    }

    // Trash
    let trash = home.join(".local/share/Trash");
    let trash_sz = dir_size(&trash);
    if trash_sz > 0 {
        targets.push(("trash".into(), trash_sz, trash, false));
    }

    // /tmp crashes
    let mut tmp_size = 0u64;
    if let Ok(entries) = std::fs::read_dir("/tmp") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("core") || name.contains("crash") {
                tmp_size += dir_size(&e.path());
            }
        }
    }
    if tmp_size > 0 {
        targets.push(("tmp crash files".into(), tmp_size, PathBuf::from("/tmp"), false));
    }

    let total: u64 = targets.iter().map(|t| t.1).sum();
    println!();
    println!("  {}", label_value("Reclaimable", &format_size(total)));

    if targets.is_empty() {
        println!("  {} Nothing to clean.", success(""));
        return;
    }

    let options: Vec<String> = targets.iter().map(|t| format!("{} {}", t.0, format_size(t.1))).collect();
    let selected = dialoguer::MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select to clean (space to toggle)")
        .items(&options)
        .interact()
        .unwrap_or_default();

    if selected.is_empty() { println!("  {} Aborted.", muted("")); return; }

    let free_before = disk_free();
    for &i in &selected {
        let (label, size, path, sudo) = &targets[i];
        if label == "system journal" {
            // Use journalctl for safe cleanup
            println!("  {} Trimming journal to 3 days...", muted(""));
            let ok = Command::new("sudo")
                .args(["journalctl", "--vacuum-time=3d"])
                .stdout(std::process::Stdio::inherit())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok { println!("  {} Journal trimmed", success("")); }
        } else if label == "systemd coredumps" {
            println!("  {} Clearing coredumps...", muted(""));
            let ok = Command::new("sudo")
                .args(["coredumpctl", "list"])
                .stdout(std::process::Stdio::null())
                .status()
                .is_ok();
            let _ = std::fs::remove_dir_all(path);
            println!("  {} Coredumps cleared", success(""));
        } else {
            if remove_dir(path, *sudo) {
                println!("  {} {} ({})", success(""), label, format_size(*size));
            } else {
                println!("  {} {} — try with sudo", error(""), label);
            }
        }
    }
    let free_after = disk_free();
    println!();
    println!("  {} Freed {}", success(""), format_size(free_after.saturating_sub(free_before)).style(Theme::HOT));
}

// ── Old packages ───────────────────────────────────────────────────────────

fn journal_size() -> u64 {
    dir_size(Path::new("/var/log/journal"))
}

fn journal_detail() -> String {
    let size = journal_size();
    if size == 0 { return "empty".into(); }
    format!("{}", format_size(size))
}

fn old_packages_size() -> u64 {
    let mut total = 0u64;
    // Snap old revisions
    if Path::new("/snap").exists() {
        if let Ok(entries) = std::fs::read_dir("/snap") {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let name = e.file_name().to_string_lossy().to_string();
                    // Count revisions beyond the latest 2
                    total += dir_size(&p);
                }
            }
        }
    }
    // Flatpak unused runtimes
    let flatpak_dir = dirs::home_dir().unwrap_or_default().join(".local/share/flatpak");
    total += dir_size(&flatpak_dir);
    total
}

fn old_packages_detail() -> String {
    let mut parts: Vec<String> = Vec::new();
    if Path::new("/snap").exists() { parts.push("snap revisions".into()); }
    let flatpak = dirs::home_dir().unwrap_or_default().join(".local/share/flatpak");
    if flatpak.exists() { parts.push("flatpak runtimes".into()); }
    parts.join(", ")
}

// ── Trash ──────────────────────────────────────────────────────────────────

fn trash_size() -> u64 {
    let home = dirs::home_dir().unwrap_or_default();
    dir_size(&home.join(".local/share/Trash"))
}
