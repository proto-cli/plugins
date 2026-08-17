use regex::Regex;
use std::path::{Path, PathBuf};

struct Theme;
impl Theme {
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
}

fn error(msg: &str) -> String {
    use owo_colors::OwoColorize;
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir = String::new();
    let mut depth = 4usize;
    let mut hidden = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--depth" => {
                if let Some(v) = args.get(i + 1) {
                    depth = v.parse().unwrap_or(depth);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--hidden" => {
                hidden = true;
                i += 1;
            }
            _ => {
                dir = args[i].clone();
                i += 1;
            }
        }
    }

    let root = if dir.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(dir)
    };
    if !root.is_dir() {
        eprintln!("{} Not a directory: {}", error(""), root.display());
        return;
    }
    for line in render(&root, depth, hidden) {
        println!("{}", line);
    }
}

struct Rule {
    anchored: bool,
    dir_only: bool,
    negate: bool,
    glob: Regex,
}

struct GitIgnore {
    rules: Vec<Rule>,
}

impl GitIgnore {
    fn parse(content: &str) -> GitIgnore {
        let mut rules = Vec::new();
        for raw in content.lines() {
            let mut line = raw.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut negate = false;
            if let Some(rest) = line.strip_prefix('!') {
                negate = true;
                line = rest;
            }
            let mut dir_only = false;
            if let Some(rest) = line.strip_suffix('/') {
                dir_only = true;
                line = rest;
            }
            if line.is_empty() {
                continue;
            }
            let anchored = line.starts_with('/') || line.contains('/');
            let pattern = line.trim_start_matches('/');
            if let Ok(g) = Regex::new(&glob_to_regex(pattern)) {
                rules.push(Rule {
                    anchored,
                    dir_only,
                    negate,
                    glob: g,
                });
            }
        }
        GitIgnore { rules }
    }

    fn is_ignored(&self, rel: &str, is_dir: bool) -> bool {
        let mut ignored = false;
        for r in &self.rules {
            if r.dir_only && !is_dir {
                continue;
            }
            let matched = if r.anchored {
                r.glob.is_match(rel)
            } else {
                rel.split('/').any(|seg| r.glob.is_match(seg))
            };
            if matched {
                ignored = !r.negate;
            }
        }
        ignored
    }
}

fn glob_to_regex(pattern: &str) -> String {
    let mut re = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    re.push_str(".*");
                    i += 2;
                    continue;
                }
                re.push_str("[^/]*");
            }
            '?' => re.push_str("[^/]"),
            '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            _ => re.push(c),
        }
        i += 1;
    }
    re.push('$');
    re
}

fn collect_ignores(root: &Path) -> Vec<(PathBuf, GitIgnore)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                }
            }
        }
        let gi_file = dir.join(".gitignore");
        if let Ok(content) = std::fs::read_to_string(&gi_file) {
            out.push((dir.clone(), GitIgnore::parse(&content)));
        }
    }
    out
}

fn render(root: &Path, depth: usize, hidden: bool) -> Vec<String> {
    let ignores = collect_ignores(root);
    let mut out = Vec::new();
    let root_name = if root.file_name().is_some() {
        root.display().to_string()
    } else {
        ".".to_string()
    };
    out.push(root_name);
    walk(root, root, 0, depth, "", hidden, &ignores, &mut out);
    out
}

#[allow(clippy::too_many_arguments)]
fn walk(
    root: &Path,
    dir: &Path,
    level: usize,
    max_depth: usize,
    prefix: &str,
    hidden: bool,
    ignores: &[(PathBuf, GitIgnore)],
    out: &mut Vec<String>,
) {
    if level >= max_depth {
        return;
    }
    let mut entries: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            entries.push(e.path());
        }
    }
    entries.sort();

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for p in entries {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name == ".git" || name == "." || name == ".." {
            continue;
        }
        if !hidden && name.starts_with('.') {
            continue;
        }
        let is_dir = p.is_dir();
        if ignored(root, dir, &p, ignores, is_dir) {
            continue;
        }
        if is_dir {
            dirs.push((p, name));
        } else {
            files.push((p, name));
        }
    }

    let mut all: Vec<(&PathBuf, &String, bool)> = Vec::new();
    for (p, n) in &dirs {
        all.push((p, n, true));
    }
    for (p, n) in &files {
        all.push((p, n, false));
    }

    let total = all.len();
    for (i, (p, name, is_dir)) in all.iter().enumerate() {
        let last = i + 1 == total;
        let branch = if last { "└── " } else { "├── " };
        let child_prefix = if last { "    " } else { "│   " };
        if *is_dir {
            out.push(format!("{}{}{}", prefix, branch, name));
            walk(
                root,
                p,
                level + 1,
                max_depth,
                &format!("{}{}", prefix, child_prefix),
                hidden,
                ignores,
                out,
            );
        } else {
            out.push(format!("{}{}{}", prefix, branch, name));
        }
    }
}

fn ignored(
    root: &Path,
    _dir: &Path,
    path: &Path,
    ignores: &[(PathBuf, GitIgnore)],
    is_dir: bool,
) -> bool {
    let rel_root = match path.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let rel_root = rel_root.to_string_lossy();
    for (base, gi) in ignores {
        let base_rel = match base.strip_prefix(root) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let base_rel = base_rel.to_string_lossy();
        let rel = if base_rel.is_empty() {
            rel_root.to_string()
        } else if let Some(rest) = rel_root.strip_prefix(&format!("{}/", base_rel)) {
            rest.to_string()
        } else {
            continue;
        };
        if gi.is_ignored(&rel, is_dir) {
            return true;
        }
    }
    false
}
