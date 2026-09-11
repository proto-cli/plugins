use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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

struct FontInfo {
    name: String,
    path: PathBuf,
    style: String,
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        // Linux
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
        // User local
        dirs::home_dir().map(|h| h.join(".local/share/fonts")).unwrap_or_default(),
        dirs::home_dir().map(|h| h.join(".fonts")).unwrap_or_default(),
        // macOS
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
        dirs::home_dir().map(|h| h.join("Library/Fonts")).unwrap_or_default(),
        // Windows
        PathBuf::from("C:\\Windows\\Fonts"),
    ];
    dirs.retain(|d| d.exists());
    dirs
}

fn scan_fonts() -> Vec<FontInfo> {
    let exts = ["ttf", "otf", "woff", "woff2", "ttc", "dfont"];
    let mut fonts = Vec::new();

    for dir in font_dirs() {
        for entry in WalkDir::new(&dir).max_depth(3).into_iter().flatten() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if exts.contains(&ext.to_lowercase().as_str()) {
                    let name = entry
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    let style = infer_style(&name);
                    fonts.push(FontInfo {
                        name,
                        path: entry.path().to_path_buf(),
                        style,
                    });
                }
            }
        }
    }
    fonts.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    fonts.dedup_by(|a, b| a.name == b.name);
    fonts
}

fn infer_style(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("bold") && lower.contains("italic") { "Bold Italic".into() }
    else if lower.contains("bold") || lower.contains("heavy") || lower.contains("black") { "Bold".into() }
    else if lower.contains("italic") || lower.contains("oblique") { "Italic".into() }
    else if lower.contains("light") || lower.contains("thin") { "Light".into() }
    else if lower.contains("medium") { "Medium".into() }
    else if lower.contains("semibold") || lower.contains("demi") { "SemiBold".into() }
    else { "Regular".into() }
}

fn preview_font(font: &FontInfo) {
    let preview_lines = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "abcdefghijklmnopqrstuvwxyz",
        "0123456789 !@#$%^&*()_+-=[]{}|;':\",./<>?",
        "The quick brown fox jumps over the lazy dog.",
        "Pack my box with five dozen liquor jugs!",
        "0Oo 1lI 5S6G8B9qgp",
    ];

    println!();
    for line in &preview_lines {
        println!("  {}", line.style(Theme::VALUE));
    }
    println!();
}

fn install_font(font: &FontInfo) {
    let target_dir = dirs::home_dir()
        .map(|h| h.join(".local/share/fonts"))
        .ok_or_else(|| "Could not determine home directory".to_string());

    match target_dir {
        Ok(target) => {
            if let Err(e) = std::fs::create_dir_all(&target) {
                eprintln!("  {} {} {}", error(""), target.display(), e);
                return;
            }
            let dest = target.join(font.path.file_name().unwrap_or_default());
            match std::fs::copy(&font.path, &dest) {
                Ok(_) => {
                    println!("  {}", success(&format!("Installed to {}", dest.display())));
                    // Update font cache
                    let _ = std::process::Command::new("fc-cache")
                        .arg("-f")
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                    println!("  {}", muted("fc-cache updated"));
                }
                Err(e) => {
                    eprintln!("  {} {}", error(""), e);
                }
            }
        }
        Err(e) => eprintln!("  {} {}", error(""), e),
    }
}

fn print_help() {
    println!("{} Font Manager", header("proto"));
    println!("{}", divider());
    println!();
    println!("  USAGE:");
    println!("    proto font list [search]      List installed fonts");
    println!("    proto font preview <name>     Preview a font's glyphs");
    println!("    proto font install <file>     Install a font file");
    println!("    proto font info               Show font directories");
    println!("    proto font --help             Show this help");
    println!();
    println!("  EXAMPLES:");
    println!("    proto font list mono           List monospace fonts");
    println!("    proto font preview FiraCode   Preview Fira Code");
    println!("    proto font install MyFont.ttf Install a font");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("list") => {
            let search = args.get(1).map(|s| s.to_lowercase());
            println!("{}", header("Font List"));
            println!("{}", divider());

            let fonts = scan_fonts();
            let filtered: Vec<&FontInfo> = if let Some(ref q) = search {
                fonts.iter().filter(|f| f.name.to_lowercase().contains(q)).collect()
            } else {
                fonts.iter().collect()
            };

            println!("  {} fonts found\n", filtered.len().to_string().style(Theme::ACCENT));

            for (i, font) in filtered.iter().enumerate() {
                println!(
                    "  {} {} {}",
                    format!("{:>3}.", i + 1).style(Theme::MUTED),
                    font.name.style(Theme::VALUE),
                    font.style.style(Theme::MUTED),
                );
            }
        }
        Some("preview") => {
            let query = match args.get(1) {
                Some(q) => q.to_lowercase(),
                None => {
                    eprintln!("  {} Usage: font preview <name>", error(""));
                    return;
                }
            };
            let fonts = scan_fonts();
            let font = fonts.iter().find(|f| f.name.to_lowercase().contains(&query));
            match font {
                Some(f) => {
                    println!("{}", header(&format!("Preview: {}", f.name)));
                    println!("{}", divider());
                    println!("  {} {}", "Path:".style(Theme::MUTED), f.path.display().to_string().style(Theme::ACCENT));
                    println!("  {} {}", "Style:".style(Theme::MUTED), f.style.style(Theme::VALUE));
                    preview_font(f);
                }
                None => {
                    eprintln!("  {} Font matching '{}' not found", error(""), query);
                }
            }
        }
        Some("install") => {
            let path = match args.get(1) {
                Some(p) => PathBuf::from(p),
                None => {
                    eprintln!("  {} Usage: font install <file.ttf|file.otf>", error(""));
                    return;
                }
            };
            if !path.exists() {
                eprintln!("  {} File not found: {}", error(""), path.display());
                return;
            }
            let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let font = FontInfo {
                name,
                path: path.clone(),
                style: "Unknown".into(),
            };
            println!("{}", header("Install Font"));
            println!("{}", divider());
            install_font(&font);
        }
        Some("info") => {
            println!("{}", header("Font Directories"));
            println!("{}", divider());
            for dir in font_dirs() {
                let exists = "✔".style(Theme::SUCCESS);
                println!("  {} {}", exists, dir.display().to_string().style(Theme::VALUE));
            }
        }
        Some("--help") | Some("-h") | None => print_help(),
        Some(cmd) => {
            eprintln!("  {} Unknown command: {}", error(""), cmd);
            print_help();
            std::process::exit(1);
        }
    }
}
