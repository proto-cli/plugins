use dialoguer::Select;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(text: &str) -> String {
    format!(
        "{} {}",
        "◆".style(Theme::ACCENT),
        text.style(Theme::HEADER)
    )
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

fn warn(msg: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), msg)
}

fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}

fn label_value(label: &str, value: &str) -> String {
    format!(
        "{} {}",
        format!("{:>14}:", label).style(Theme::ACCENT),
        value.style(Theme::VALUE)
    )
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{} B", bytes);
    }
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", v, UNITS[i])
}

struct Spinner {
    pb: ProgressBar,
}

impl Spinner {
    fn new(msg: &str) -> Self {
        let pb = ProgressBar::new_spinner()
            .with_message(msg.to_string())
            .with_style(
                ProgressStyle::with_template("{spinner:.cyan} {msg}")
                    .unwrap()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { pb }
    }

    fn done(&self, msg: &str) {
        self.pb.finish_with_message(msg.to_string());
    }
}

#[derive(Clone)]
struct FileEntry {
    path: PathBuf,
    len: u64,
    dev: u64,
    ino: u64,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first() {
        Some(d) => d.as_str(),
        None => {
            eprintln!("  {} Usage: dedupe <DIR>", error(""));
            return;
        }
    };

    let root = Path::new(dir);
    if !root.is_dir() {
        eprintln!("{} Not a directory: {}", error(""), dir);
        return;
    }

    println!("{}", header("Duplicate Finder"));
    println!("{}", divider());
    println!(
        "  {}",
        label_value("Scanning", &root.to_string_lossy().to_string())
    );

    let sp = Spinner::new("Indexing files...");
    let mut files = Vec::new();
    collect_files(root, &mut files);
    sp.done(&format!("{} files indexed", files.len()));

    let sp = Spinner::new("Hashing candidates...");
    let groups = find_duplicates(&mut files);
    sp.done(&format!("{} duplicate group(s) found", groups.len()));

    if groups.is_empty() {
        println!("\n{} No exact duplicates found.", success(""));
        return;
    }

    let mut deleted = 0usize;
    let mut symlinked = 0usize;
    let mut reclaimed = 0u64;

    for (i, group) in groups.iter().enumerate() {
        println!();
        println!(
            "  {} {} ({})",
            format!("[{}]", i + 1).style(Theme::ACCENT),
            format!("{}x", group.len()).style(Theme::HEADER),
            format_size(group[0].len)
        );
        println!("  {}", "─".repeat(50).dimmed());
        for (j, entry) in group.iter().enumerate() {
            println!(
                "    {} {}",
                j.style(Theme::ACCENT),
                entry.path.to_string_lossy().dimmed()
            );
        }

        let keep = 0usize;
        let choices = vec![
            format!("Skip this group"),
            format!(
                "Delete duplicates (keep: {})",
                group[keep]
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
            format!(
                "Symlink duplicates to the primary file (keep: {})",
                group[keep]
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
        ];
        let choice = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt(format!("Group {} action", i + 1))
            .items(&choices)
            .default(0)
            .interact()
            .unwrap_or(0);

        match choice {
            1 => {
                if confirm_delete(group) {
                    let mut freed = 0u64;
                    for entry in group.iter().skip(1) {
                        if std::fs::remove_file(&entry.path).is_ok() {
                            freed += entry.len;
                            deleted += 1;
                        }
                    }
                    reclaimed += freed;
                    println!(
                        "  {} {} freed",
                        success(""),
                        format_size(freed)
                    );
                }
            }
            2 => {
                if confirm_delete(group) {
                    let primary = &group[keep].path;
                    let mut freed = 0u64;
                    for entry in group.iter().skip(1) {
                        if let Some(rel) =
                            relative_path(entry.path.parent().unwrap_or(primary), primary)
                        {
                            if std::fs::remove_file(&entry.path).is_ok()
                                && std::os::unix::fs::symlink(&rel, &entry.path).is_ok()
                            {
                                freed += entry.len;
                                symlinked += 1;
                            }
                        }
                    }
                    reclaimed += freed;
                    println!(
                        "  {} {} symlinked, {} freed",
                        success(""),
                        symlinked,
                        format_size(freed)
                    );
                }
            }
            _ => {}
        }
    }

    println!();
    println!("{}", divider());
    if deleted + symlinked > 0 {
        println!(
            "  {} {} deleted, {} symlinked, {} reclaimed",
            success(""),
            deleted,
            symlinked,
            format_size(reclaimed)
        );
    } else {
        println!("  {} No changes made.", warn(""));
    }
}

fn confirm_delete(group: &[FileEntry]) -> bool {
    use dialoguer::Confirm;
    let freed = group[1..].iter().map(|e| e.len).sum::<u64>();
    Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt(format!(
            "Process {} duplicate(s) ({} reclaimed)?",
            group.len() - 1,
            format_size(freed)
        ))
        .default(false)
        .interact()
        .unwrap_or(false)
}

fn collect_files(root: &Path, out: &mut Vec<FileEntry>) {
    use std::os::unix::fs::MetadataExt;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.is_symlink() {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                if let Ok(md) = entry.metadata() {
                    if md.len() > 0 {
                        out.push(FileEntry {
                            path,
                            len: md.len(),
                            dev: md.dev(),
                            ino: md.ino(),
                        });
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
}

fn find_duplicates(files: &mut Vec<FileEntry>) -> Vec<Vec<FileEntry>> {
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        by_size.entry(f.len).or_default().push(i);
    }

    let mut groups: HashMap<(u64, String), Vec<usize>> = HashMap::new();
    for idxs in by_size.values() {
        if idxs.len() < 2 {
            continue;
        }
        for &i in idxs {
            let hash = sha256(&files[i].path);
            groups
                .entry((files[i].len, hash))
                .or_default()
                .push(i);
        }
    }

    let mut result: Vec<Vec<FileEntry>> = Vec::new();
    for idxs in groups.values() {
        if idxs.len() < 2 {
            continue;
        }
        let mut seen_dev_ino = std::collections::HashSet::new();
        let mut uniq: Vec<&FileEntry> = Vec::new();
        for &i in idxs {
            let e = &files[i];
            if seen_dev_ino.insert((e.dev, e.ino)) {
                uniq.push(e);
            }
        }
        if uniq.len() > 1 {
            result.push(uniq.into_iter().cloned().collect());
        }
    }
    result.sort_by(|a, b| b[0].len.cmp(&a[0].len));
    result
}

fn sha256(path: &Path) -> String {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                hasher.update(&buf[..n]);
            }
            Err(_) => break,
        }
    }
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn relative_path(from_dir: &Path, to: &Path) -> Option<PathBuf> {
    let from: Vec<_> = from_dir.components().collect();
    let to: Vec<_> = to.components().collect();
    let mut i = 0;
    while i < from.len() && i < to.len() && from[i] == to[i] {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    let mut rel = PathBuf::new();
    for _ in i..from.len() {
        rel.push("..");
    }
    for c in &to[i..] {
        rel.push(c.as_os_str());
    }
    Some(rel)
}
