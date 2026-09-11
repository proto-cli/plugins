use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};
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
fn warn(s: &str) -> String { format!("{} {}", "⚠".style(Theme::WARN), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(55).dimmed().to_string() }

fn on_path(binary: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|d| d.join(binary).is_file()))
        .unwrap_or(false)
}

fn is_archive(path: &Path) -> bool {
    let lower = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        lower.as_str(),
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "tbz2" | "xz" | "txz" | "7z"
    )
}

fn archive_lines(path: &Path) -> Vec<String> {
    let lower = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    let out = match lower.as_str() {
        "zip" => {
            if on_path("zipinfo") {
                run_output("zipinfo", &["-1", path.to_str().unwrap_or("")])
            } else if on_path("unzip") {
                run_output("unzip", &["-l", path.to_str().unwrap_or("")])
            } else {
                Vec::new()
            }
        }
        "gz" if name.contains("tar") || name.ends_with(".tgz") => {
            run_output("tar", &["-tzf", path.to_str().unwrap_or("")])
        }
        "tgz" => run_output("tar", &["-tzf", path.to_str().unwrap_or("")]),
        "bz2" | "tbz2" => run_output("tar", &["-tjf", path.to_str().unwrap_or("")]),
        "xz" | "txz" => run_output("tar", &["-tJf", path.to_str().unwrap_or("")]),
        "tar" => run_output("tar", &["-tf", path.to_str().unwrap_or("")]),
        "7z" => run_output("7z", &["l", path.to_str().unwrap_or("")]),
        _ => Vec::new(),
    };
    out.into_iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect()
}

fn run_output(program: &str, args: &[&str]) -> Vec<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn list_archive(path: &Path, pattern: Option<&str>) {
    if !path.exists() {
        eprintln!("  {} Archive not found: {}", error(""), path.display());
        std::process::exit(1);
    }
    if !is_archive(path) {
        eprintln!("  {} Not a supported archive: {}", error(""), path.display());
        std::process::exit(1);
    }

    let files = archive_lines(path);
    if files.is_empty() {
        println!("  {} Could not read archive contents", warn(""));
        return;
    }
    let filtered: Vec<&String> = match pattern {
        Some(p) => files.iter().filter(|f| f.to_lowercase().contains(&p.to_lowercase())).collect(),
        None => files.iter().collect(),
    };
    println!("{} {}", header(&format!("{} ({})", path.file_name().unwrap_or_default().to_string_lossy(), path.display())), muted(""));
    println!("{}", divider());
    println!("  {} {} entries", filtered.len().to_string().style(Theme::ACCENT), "·".dimmed());
    println!();
    for (i, f) in filtered.iter().enumerate().take(200) {
        let is_dir = f.ends_with('/');
        if is_dir {
            println!("  {} {} {}", "◆".style(Theme::ACCENT), f.trim_end_matches('/').style(Theme::DIR), "/".style(Theme::DIR));
        } else {
            println!("  {}", f.dimmed());
        }
        let _ = i;
    }
    if filtered.len() > 200 {
        println!("  {} … and {} more", muted(""), filtered.len() - 200);
    }
    println!("{}", divider());
}

fn create_archive(target: &str, sources: &[String], compression: &str) {
    if sources.is_empty() {
        eprintln!("  {} Usage: archive create <out.tar.gz> <file|dir>...", error(""));
        std::process::exit(1);
    }
    let out = PathBuf::from(target);
    if out.exists() {
        eprintln!("  {} Output already exists: {}", error(""), out.display());
        std::process::exit(1);
    }
    let lower = target.to_lowercase();

    let status = if lower.ends_with(".zip") {
        if !on_path("zip") {
            eprintln!("  {} `zip` not found on PATH", error(""));
            std::process::exit(1);
        }
        let mut args = vec!["-r", target];
        args.extend(sources.iter().map(|s| s.as_str()));
        Command::new("zip").args(&args).status()
    } else {
        if !on_path("tar") {
            eprintln!("  {} `tar` not found on PATH", error(""));
            std::process::exit(1);
        }
        let flag = match lower.split('.').last().unwrap_or("") {
            "gz" | "tgz" => "-czf",
            "bz2" | "tbz2" => "-cjf",
            "xz" | "txz" => "-cJf",
            _ if compression == "gz" => "-czf",
            _ if compression == "bz2" => "-cjf",
            _ => "-cf",
        };
        let mut args = vec![flag, target];
        args.extend(sources.iter().map(|s| s.as_str()));
        Command::new("tar").args(&args).status()
    };

    match status {
        Ok(s) if s.success() => {
            let size = std::fs::metadata(&out)
                .map(|m| bytes_to_human(m.len()))
                .unwrap_or_default();
            println!("  {} {} {} created", success(""), target.style(Theme::ACCENT), muted(&size));
        }
        _ => {
            eprintln!("  {} Archive creation failed", error(""));
            std::process::exit(1);
        }
    }
}

fn extract(archive: &Path, dest: Option<&str>) {
    if !archive.exists() {
        eprintln!("  {} Archive not found: {}", error(""), archive.display());
        std::process::exit(1);
    }
    let lower = archive.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let name = archive.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    let dest_path = dest.map(|d| PathBuf::from(d));

    if let Some(d) = &dest_path {
        std::fs::create_dir_all(d).ok();
    }

    let (program, args): (&str, Vec<String>) = match lower.as_str() {
        "zip" => {
            let mut a = vec!["-qq".to_string(), "-o".to_string()];
            if let Some(d) = &dest_path { a.push("-d".into()); a.push(d.display().to_string()); }
            a.push(archive.display().to_string());
            ("unzip", a)
        }
        "tgz" | "gz" if name.contains("tar") || name.ends_with(".tgz") => {
            let mut a = vec!["-xzf".to_string(), archive.display().to_string()];
            if let Some(d) = &dest_path { a.push("-C".into()); a.push(d.display().to_string()); }
            ("tar", a)
        }
        "bz2" | "tbz2" => {
            let mut a = vec!["-xjf".to_string(), archive.display().to_string()];
            if let Some(d) = &dest_path { a.push("-C".into()); a.push(d.display().to_string()); }
            ("tar", a)
        }
        "xz" | "txz" => {
            let mut a = vec!["-xJf".to_string(), archive.display().to_string()];
            if let Some(d) = &dest_path { a.push("-C".into()); a.push(d.display().to_string()); }
            ("tar", a)
        }
        "tar" => {
            let mut a = vec!["-xf".to_string(), archive.display().to_string()];
            if let Some(d) = &dest_path { a.push("-C".into()); a.push(d.display().to_string()); }
            ("tar", a)
        }
        "7z" => {
            let mut a = vec!["x".to_string()];
            if let Some(d) = &dest_path { a.push(format!("-o{}", d.display())); }
            a.push(archive.display().to_string());
            ("7z", a)
        }
        _ => {
            eprintln!("  {} Unsupported archive type", error(""));
            std::process::exit(1);
        }
    };

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let Ok(status) = Command::new(program).args(&arg_refs).status() else {
        eprintln!("  {} `{}` not available", error(""), program);
        std::process::exit(1);
    };
    if status.success() {
        let where_to = dest_path
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| "current dir".into());
        println!("  {} {} → {}", success("extracted"), archive.display().to_string().dimmed(), where_to.style(Theme::VALUE));
    } else {
        eprintln!("  {} Extraction failed", error(""));
        std::process::exit(1);
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

fn print_help() {
    println!("{} Archive Toolkit", header("proto"));
    println!("{}", divider());
    println!();
    println!("  USAGE:");
    println!("    proto archive list <archive>                List contents with preview");
    println!("    proto archive list <archive> <pattern>      Filter entries");
    println!("    proto archive create <out.tar.gz> <src>...  Create an archive");
    println!("    proto archive extract <archive> [dest]      Extract archive");
    println!("    proto archive --help                        Show this help");
    println!();
    println!("  SUPPORTED: zip, tar, tar.gz, tar.bz2, tar.xz, 7z");
    println!();
    println!("  EXAMPLES:");
    println!("    proto archive list dist.tar.gz");
    println!("    proto archive create backup.zip ./src ./docs");
    println!("    proto archive extract app-1.0.tgz ./out");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" || args[0] == "help" {
        print_help();
        return;
    }

    match args[0].as_str() {
        "list" | "ls" => {
            if args.len() < 2 {
                eprintln!("  {} Usage: archive list <archive>", error(""));
                std::process::exit(1);
            }
            list_archive(Path::new(&args[1]), args.get(2).map(|s| s.as_str()));
        }
        "create" | "pack" => {
            let target = match args.get(1) {
                Some(t) => t.clone(),
                None => {
                    eprintln!("  {} Usage: archive create <out> <src>...", error(""));
                    std::process::exit(1);
                }
            };
            create_archive(&target, &args[2..].to_vec(), "");
        }
        "extract" | "x" | "unpack" => {
            if args.len() < 2 {
                eprintln!("  {} Usage: archive extract <archive> [dest]", error(""));
                std::process::exit(1);
            }
            extract(Path::new(&args[1]), args.get(2).map(|s| s.as_str()));
        }
        other => {
            eprintln!("  {} Unknown command: {}", error(""), other);
            print_help();
            std::process::exit(1);
        }
    }
}