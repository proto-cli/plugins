use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const ADD: owo_colors::Style = owo_colors::Style::new().bright_green().bold();
    const DEL: owo_colors::Style = owo_colors::Style::new().bright_red().bold();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(50).dimmed().to_string() }

#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    Same(String),
    Del(String),
    Add(String),
}

fn lcs_diff(a: &[String], b: &[String]) -> Vec<Op> {
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut ops = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push(Op::Same(a[i].clone()));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            ops.push(Op::Del(a[i].clone()));
            i += 1;
        } else {
            ops.push(Op::Add(b[j].clone()));
            j += 1;
        }
    }
    while i < n {
        ops.push(Op::Del(a[i].clone()));
        i += 1;
    }
    while j < m {
        ops.push(Op::Add(b[j].clone()));
        j += 1;
    }
    ops
}

fn read_lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|s| s.lines().map(|l| l.to_string()).collect())
        .unwrap_or_default()
}

fn render_side_by_side(a: &[String], b: &[String], ops: &[Op], width: usize) {
    let mut num_add = 0usize;
    let mut num_del = 0usize;
    let left_w = width / 2;
    let pad = |s: &str| -> String {
        let l = s.chars().count();
        if l >= left_w {
            s.chars().take(left_w).collect()
        } else {
            format!("{}{}", s, " ".repeat(left_w - l))
        }
    };

    for op in ops {
        match op {
            Op::Same(line) => println!("{} │ {}", pad(line).style(Theme::MUTED), "".dimmed()),
            Op::Del(line) => {
                println!("{} │ {}", pad(&format!("-{}", line)).style(Theme::DEL), "");
                num_del += 1;
            }
            Op::Add(line) => {
                println!("{} │ {}", "".dimmed(), format!("+{}", line).style(Theme::ADD));
                num_add += 1;
            }
        }
    }
    let _ = (width, num_add, num_del);
    let _ = (a, b);
}

fn render_stats(ops: &[Op]) {
    let add = ops.iter().filter(|o| matches!(o, Op::Add(_))).count();
    let del = ops.iter().filter(|o| matches!(o, Op::Del(_))).count();
    let same = ops.iter().filter(|o| matches!(o, Op::Same(_))).count();
    if add == 0 && del == 0 {
        println!("{}", success("Files are identical"));
        return;
    }
    println!(
        "  {}+{} added  {}-{} deleted  •{} unchanged",
        "+".style(Theme::ADD),
        add,
        "−".style(Theme::DEL),
        del,
        same
    );
}

fn render_unified(a: &[String], ops: &[Op], context: usize) {
    let mut shown = std::collections::HashSet::new();
    let (mut i, mut j) = (0usize, 0usize);
    let mut pending: Vec<(usize, Op)> = Vec::new();
    for op in ops {
        match op {
            Op::Same(line) => {
                if !pending.is_empty() {
                    let last = pending
                        .iter()
                        .map(|(idx, _)| *idx)
                        .max()
                        .unwrap_or(0);
                    for k in (last + 1).saturating_sub(context)..=(i + context).min(a.len().saturating_sub(1)) {
                        if shown.insert(k) {
                            println!("  {} {}", " ".dimmed(), a[k]);
                        }
                    }
                    for (idx, p) in &pending {
                        match p {
                            Op::Del(v) => println!("  {} {}", "-".style(Theme::DEL), v),
                            Op::Add(v) => println!("  {} {}", "+".style(Theme::ADD), v),
                            _ => {}
                        }
                        let _ = idx;
                    }
                    pending.clear();
                    println!("");
                }
                if shown.insert(i) {
                    println!("  {} {}", " ".dimmed(), line);
                }
                i += 1;
                j += 1;
            }
            Op::Del(v) => { pending.push((i, Op::Del(v.clone()))); i += 1; }
            Op::Add(v) => { pending.push((j, Op::Add(v.clone()))); j += 1; }
        }
    }
    if !pending.is_empty() {
        for p in &pending {
            match p {
                (_, Op::Del(v)) => println!("  {} {}", "-".style(Theme::DEL), v),
                (_, Op::Add(v)) => println!("  {} {}", "+".style(Theme::ADD), v),
                _ => {}
            }
        }
    }
    render_stats(ops);
}

fn diff_files(a: &Path, b: &Path, unified: bool) {
    let a_lines = read_lines(a);
    let b_lines = read_lines(b);
    let ops = lcs_diff(&a_lines, &b_lines);

    println!("{}", header(&format!("diff {} ⇄ {}", a.display(), b.display())));
    println!("{}", divider());
    if unified {
        render_unified(&a_lines, &ops, 3);
    } else {
        render_side_by_side(&a_lines, &b_lines, &ops, 60);
    }
    let add = ops.iter().filter(|o| matches!(o, Op::Add(_))).count();
    let del = ops.iter().filter(|o| matches!(o, Op::Del(_))).count();
    if add > 0 || del > 0 {
        println!(
            "  {} +{} {} -{}",
            "Δ".style(Theme::WARN),
            (add).to_string().style(Theme::ADD),
            "→".style(Theme::MUTED),
            (del).to_string().style(Theme::DEL)
        );
    }
    println!();
}

fn collect_files(dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut items: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    items.sort();
    for p in items {
        if p.is_dir() {
            collect_files(&p, out);
        } else {
            let rel = p
                .strip_prefix(dir)
                .unwrap_or(&p)
                .to_string_lossy()
                .to_string();
            out.push((rel, p));
        }
    }
}

fn diff_dirs(a: &Path, b: &Path) {
    let mut af = Vec::new();
    let mut bf = Vec::new();
    collect_files(a, &mut af);
    collect_files(b, &mut bf);

    let bmap: std::collections::HashMap<&str, &PathBuf> =
        bf.iter().map(|(k, v)| (k.as_str(), v)).collect();
    let amap: std::collections::HashMap<&str, &PathBuf> =
        af.iter().map(|(k, v)| (k.as_str(), v)).collect();

    let mut keys: Vec<&str> = af
        .iter()
        .map(|(k, _)| k.as_str())
        .chain(bf.iter().map(|(k, _)| k.as_str()))
        .collect();
    keys.sort();
    keys.dedup();

    let mut only_a = 0;
    let mut only_b = 0;
    let mut changed = 0;
    let mut identical = 0;

    println!("{}", header(&format!("diff dir {} ⇄ {}", a.display(), b.display())));
    println!("{}", divider());
    for key in keys {
        match (amap.get(key), bmap.get(key)) {
            (Some(pa), Some(pb)) => {
                let al = read_lines(pa);
                let bl = read_lines(pb);
                if al == bl {
                    identical += 1;
                    continue;
                }
                changed += 1;
                println!("  {} {}", "≠".style(Theme::WARN), key.style(Theme::VALUE));
                let ops = lcs_diff(&al, &bl);
                for op in ops.iter().take(8) {
                    match op {
                        Op::Del(v) => println!("      {} {}", "-".style(Theme::DEL), v),
                        Op::Add(v) => println!("      {} {}", "+".style(Theme::ADD), v),
                        _ => {}
                    }
                }
                if ops.len() > 8 {
                    println!("      {}…", muted(&format!("{} more lines", ops.len() - 8)));
                }
            }
            (Some(_), None) => { only_a += 1; println!("  {} {} {}", "✗".style(Theme::DEL), "only in src".style(Theme::MUTED), key.style(Theme::VALUE)); }
            (None, Some(_)) => { only_b += 1; println!("  {} {} {}", "✗".style(Theme::DEL), "only in dst".style(Theme::MUTED), key.style(Theme::VALUE)); }
            _ => {}
        }
    }
    println!("{}", divider());
    println!(
        "  {} {} identical  {} changed  {} only-in-a  {} only-in-b",
        "Σ".style(Theme::ACCENT),
        identical,
        changed.to_string().style(Theme::WARN),
        only_a.to_string().style(Theme::DEL),
        only_b.to_string().style(Theme::ADD)
    );
    println!();
}

fn print_help() {
    println!("{} Side-by-side Diff", header("proto"));
    println!("{}", divider());
    println!();
    println!("  USAGE:");
    println!("    proto diff <file1> <file2>          Side-by-side diff of two files");
    println!("    proto diff unified <f1> <f2>        Unified diff");
    println!("    proto diff dir <dir1> <dir2>        Recursive directory diff");
    println!("    proto diff --help                   Show this help");
    println!();
    println!("  EXAMPLES:");
    println!("    proto diff old.rs new.rs");
    println!("    proto diff dir build/ deploy/");
    println!("    proto diff unified a.txt b.txt");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        print_help();
        return;
    }

    match args[0].as_str() {
        "unified" => {
            if args.len() < 3 {
                eprintln!("  {} Usage: diff unified <file1> <file2>", error(""));
                std::process::exit(1);
            }
            diff_files(Path::new(&args[1]), Path::new(&args[2]), true);
        }
        "dir" => {
            if args.len() < 3 {
                eprintln!("  {} Usage: diff dir <dir1> <dir2>", error(""));
                std::process::exit(1);
            }
            diff_dirs(Path::new(&args[1]), Path::new(&args[2]));
        }
        "--help" | "-h" | "help" => print_help(),
        other => {
            if other.starts_with('-') {
                eprintln!("  {} Unknown flag: {}", error(""), other);
                print_help();
                std::process::exit(1);
            }
            if args.len() < 2 {
                eprintln!("  {} Usage: diff <file1> <file2>", error(""));
                std::process::exit(1);
            }
            diff_files(Path::new(&args[0]), Path::new(&args[1]), false);
        }
    }
}