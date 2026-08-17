use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

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
}

fn header(s: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER))
}
fn success(s: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), s)
}
fn error(s: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), s)
}
fn warn(s: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), s)
}
fn muted(s: &str) -> String {
    format!("{}", s.style(Theme::MUTED))
}
fn divider() -> String {
    "─".repeat(50).dimmed().to_string()
}
fn label_value(label: &str, value: &str) -> String {
    format!("  {}: {}", label.style(Theme::LABEL), value.style(Theme::VALUE))
}

fn which(binary: &str) -> bool {
    std::process::Command::new("which")
        .arg(binary)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

struct Spinner {
    spinner: indicatif::ProgressBar,
}
impl Spinner {
    fn new(msg: &str) -> Self {
        let sp = indicatif::ProgressBar::new_spinner()
            .with_message(msg.to_string())
            .with_style(
                indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg}")
                    .unwrap()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { spinner: sp }
    }
    fn update(&self, msg: &str) {
        self.spinner.set_message(msg.to_string());
    }
    fn done(&self, msg: &str) {
        self.spinner.finish_with_message(msg.to_string());
    }
    fn fail(&self, msg: &str) {
        self.spinner.finish_with_message(format!("{} {}", "✗".style(Theme::ERROR), msg.style(Theme::ERROR)));
    }
}

const OSV_BATCH: &str = "https://api.osv.dev/v1/querybatch";
const OSV_VULN: &str = "https://api.osv.dev/v1/vulns";
const AST_ISSUES: &str = "https://security.archlinux.org/issues/all.json";
const KEV_URL: &str = "https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json";
const OSV_CHUNK: usize = 150;

#[derive(Parser)]
#[command(name = "audit-deps", about = "Dependency vulnerability audit")]
struct Cli {
    #[arg(value_name = "DIR", help = "Directory to scan")]
    dir: Option<String>,
    #[arg(long, help = "Skip interactive prompts")]
    no_prompt: bool,
    #[arg(long, help = "Don't open advisories in browser")]
    no_open: bool,
    #[arg(long, value_name = "SEVERITY", help = "Minimum severity to report")]
    min_severity: Option<String>,
    #[arg(long, value_name = "CATEGORY", help = "Filter by category (infected, exploited, unmaintained, vulnerable)")]
    category: Option<String>,
}

#[derive(Debug, Clone)]
struct PackageQuery {
    ecosystem: &'static str,
    name: String,
    version: String,
    source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Category {
    Infected,
    Exploited,
    Unmaintained,
    Vulnerable,
    Other,
}

#[derive(Debug, Clone)]
struct Vuln {
    id: String,
    alias: Option<String>,
    severity: Option<String>,
    summary: Option<String>,
    fixed: Option<String>,
    url: Option<String>,
    category: Category,
    kev_ransomware: bool,
}

#[derive(Debug, Clone)]
struct Finding {
    source: String,
    package: String,
    version: String,
    vulns: Vec<Vuln>,
}

struct Checked {
    source: String,
    packages: usize,
}

enum Source {
    Lockfile(PathBuf),
    System,
}

enum SysKind {
    Pacman,
    Dpkg(&'static str),
}

fn main() {
    let cli = Cli::parse();
    run(cli.dir, cli.no_prompt, cli.no_open, cli.min_severity, cli.category);
}

fn run(
    dir: Option<String>,
    no_prompt: bool,
    no_open: bool,
    min_severity: Option<String>,
    category_filter: Option<String>,
) {
    println!("{}", header("Dependency Audit"));
    println!("{}", divider());

    let dir = match dir {
        Some(d) => d,
        None => {
            let input = dialoguer::Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
                .with_prompt("Directory to scan")
                .default(".".to_string())
                .allow_empty(false)
                .interact_text()
                .unwrap_or_else(|_| ".".to_string());
            input
        }
    };
    println!("  {} Scanning {} — lockfiles & system packages\n", muted(""), dir);

    let agent = new_agent();
    let dir_path = PathBuf::from(&dir);
    let files = find_lockfiles(&dir_path);

    let system = detect_system();
    let mut sources: Vec<(String, Source)> = Vec::new();
    for f in &files {
        if let Some(ec) = ecosystem_for(f) {
            let list = parse_lockfile(f);
            if !list.is_empty() {
                sources.push((
                    format!("{} ({}, {} pkgs)", rel_label(&dir_path, f), ec, list.len()),
                    Source::Lockfile(f.clone()),
                ));
            }
        }
    }
    if let Some(sys) = &system {
        let label = match sys {
            SysKind::Pacman => "system (pacman + AUR)".to_string(),
            SysKind::Dpkg(ec) => format!("system ({} / apt)", ec),
        };
        sources.push((label, Source::System));
    }

    if sources.is_empty() {
        println!(
            "  {} No supported lockfiles found in '{}' and no supported package manager.",
            muted(""),
            dir
        );
        return;
    }

    let selected: Vec<usize> = if no_prompt {
        (0..sources.len()).collect()
    } else {
        let opts: Vec<String> = sources.iter().map(|(l, _)| l.clone()).collect();
        let defaults: Vec<bool> = vec![true; opts.len()];
        dialoguer::MultiSelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Select sources to audit (space to toggle, enter to confirm)")
            .items(&opts)
            .defaults(&defaults)
            .interact()
            .unwrap_or_default()
    };
    if selected.is_empty() {
        println!("{} Nothing selected.", muted(""));
        return;
    }

    let only_cats = category_filter.as_ref().map(|s| parse_categories(s));
    let sev_min = if let Some(s) = min_severity {
        match s.to_ascii_lowercase().as_str() {
            "critical" => 9.0,
            "high" => 7.0,
            "moderate" | "medium" => 4.0,
            "low" => 1.0,
            _ => 0.0,
        }
    } else if no_prompt {
        0.0
    } else {
        let levels = [
            "All severities",
            "Critical + High",
            "Critical + High + Moderate",
            "Critical only",
            "High risk (infected / exploited / critical)",
        ];
        let idx = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Minimum severity to report")
            .items(&levels)
            .default(0)
            .interact()
            .unwrap_or(0);
        match idx {
            1 => 7.0,
            2 => 4.0,
            3 => 9.0,
            4 => 9.0,
            _ => 0.0,
        }
    };

    let mut findings: Vec<Finding> = Vec::new();
    let mut checked: Vec<Checked> = Vec::new();
    let mut ecosystems: Vec<&'static str> = Vec::new();

    let mut queries: Vec<PackageQuery> = Vec::new();
    for &i in &selected {
        if let Source::Lockfile(f) = &sources[i].1 {
            let ec = ecosystem_for(f).expect("lockfile has ecosystem");
            if !ecosystems.contains(&ec) {
                ecosystems.push(ec);
            }
            let label = rel_label(&dir_path, f);
            for (name, version) in parse_lockfile(f) {
                queries.push(PackageQuery {
                    ecosystem: ec,
                    name,
                    version,
                    source: label.clone(),
                });
            }
        }
    }

    if !queries.is_empty() {
        let spin = Spinner::new(&format!("Querying {} OSV databases...", queries.len()));
        let hits = osv_batch(&agent, &queries);
        let details = vuln_details(&agent, &hits);
        spin.done("OSV check complete");

        for q in &queries {
            let key = query_key(q);
            let Some(ids) = hits.get(&key) else {
                continue;
            };
            if ids.is_empty() {
                continue;
            }
            let mut vulns: Vec<Vuln> = dedup_vulns(
                ids.iter()
                    .filter_map(|id| details.get(id).cloned())
                    .collect(),
            );
            vulns.retain(|v| {
                let cat_ok = match &only_cats {
                    Some(cats) => cats.contains(&v.category),
                    None => true,
                };
                if !cat_ok {
                    return false;
                }
                let sev_ok = sev_score(v.severity.as_deref()) >= sev_min;
                sev_ok
                    || matches!(
                        v.category,
                        Category::Infected | Category::Exploited | Category::Unmaintained
                    )
            });
            if vulns.is_empty() {
                continue;
            }
            findings.push(Finding {
                source: q.source.clone(),
                package: q.name.clone(),
                version: q.version.clone(),
                vulns,
            });
        }

        let mut per_source: BTreeMap<String, usize> = BTreeMap::new();
        for q in &queries {
            *per_source.entry(q.source.clone()).or_insert(0) += 1;
        }
        for (source, n) in per_source {
            checked.push(Checked { source, packages: n });
        }
    }

    let mut ast_updated: Option<String> = None;
    let system_selected =
        system.is_some() && selected.iter().any(|&i| matches!(sources[i].1, Source::System));
    if let Some(sys) = &system {
        if system_selected {
            match sys {
                SysKind::Pacman => {
                    ecosystems.push("Arch Security Tracker");
                    let spin = Spinner::new("Checking Arch package advisories...");
                    let (sys_f, sys_count, ast_upd) = scan_system_arch(&agent);
                    spin.done("Arch advisory check complete");
                    ast_updated = ast_upd;
                    checked.push(Checked {
                        source: "system (pacman)".to_string(),
                        packages: sys_count,
                    });
                    findings.extend(sys_f);
                }
                SysKind::Dpkg(ec) => {
                    ecosystems.push(ec);
                    let spin = Spinner::new(&format!("Checking {} package advisories...", ec));
                    let (sys_f, sys_count) = scan_system_dpkg(&agent, ec);
                    spin.done("OSV package check complete");
                    checked.push(Checked {
                        source: format!("system ({})", ec),
                        packages: sys_count,
                    });
                    findings.extend(sys_f);
                }
            }
        } else {
            println!("  {} System package audit skipped.", muted(""));
        }
    }

    if !findings.is_empty() || system_selected {
        let spin = Spinner::new("Checking CISA KEV catalog...");
        let (kev_set, rw_set) = fetch_kev(&agent);
        spin.done(&format!("CISA KEV: {} known exploited vulns", kev_set.len()));
        tag_kev(&mut findings, &kev_set, &rw_set);
        println!(
            "  {} CISA KEV: {} known exploited vulns loaded.",
            muted(""),
            kev_set.len().style(Theme::VALUE)
        );
    }

    println!("{}", divider());
    print_findings(&findings);
    print_summary(&findings, &checked);
    print_freshness(&agent, &ecosystems, ast_updated);
    print_high_risk(&findings);
    println!();

    if !findings.is_empty() && !no_open {
        let confirm = dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Open the advisory pages in your browser?")
            .default(false)
            .interact()
            .unwrap_or(false);
        if confirm {
            open_advisories(&findings);
        }
    }
}

fn new_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .build()
}

fn fetch_kev(agent: &ureq::Agent) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut kev = BTreeSet::new();
    let mut ransomware = BTreeSet::new();
    let resp = match agent.get(KEV_URL).call() {
        Ok(r) => r,
        Err(_) => return (kev, ransomware),
    };
    let value: serde_json::Value = match resp.into_json() {
        Ok(v) => v,
        Err(_) => return (kev, ransomware),
    };
    if let Some(vulns) = value.get("vulnerabilities").and_then(|v| v.as_array()) {
        for entry in vulns {
            if let Some(cve) = entry.get("cveID").and_then(|x| x.as_str()) {
                kev.insert(cve.to_ascii_lowercase());
                let is_rw = entry
                    .get("knownRansomwareCampaignUse")
                    .and_then(|x| x.as_str())
                    .map(|s| s == "Known")
                    == Some(true);
                if is_rw {
                    ransomware.insert(cve.to_ascii_lowercase());
                }
            }
        }
    }
    (kev, ransomware)
}

fn tag_kev(findings: &mut [Finding], kev: &BTreeSet<String>, ransomware: &BTreeSet<String>) {
    if kev.is_empty() {
        return;
    }
    for f in findings {
        for v in &mut f.vulns {
            let check = |id: &str| {
                let lower = id.to_ascii_lowercase();
                if ransomware.contains(&lower) {
                    return (true, true);
                }
                if kev.contains(&lower) {
                    return (true, false);
                }
                (false, false)
            };
            let (is_kev, is_rw) = check(&v.id);
            if is_kev {
                v.kev_ransomware = is_rw;
                v.category = Category::Exploited;
                continue;
            }
            if let Some(ref alias) = v.alias {
                let (is_kev, is_rw) = check(alias);
                if is_kev {
                    v.kev_ransomware = is_rw;
                    v.category = Category::Exploited;
                }
            }
        }
    }
}

fn query_key(q: &PackageQuery) -> String {
    format!("{}|{}|{}", q.ecosystem, q.name, q.version)
}

fn find_lockfiles(dir: &Path) -> Vec<PathBuf> {
    const SKIP_DIRS: &[&str] = &[
        "node_modules", "target", ".git", "vendor", "dist", "build", ".cache", ".cargo",
        ".npm", ".yarn", ".pnpm-store", "Pods", "site-packages", ".venv", "venv", ".tox",
        ".mypy_cache", ".pytest_cache", "__pycache__", ".gradle",
    ];
    let lockfile_names: &[&str] = &[
        "package-lock.json", "yarn.lock", "pnpm-lock.yaml", "Cargo.lock", "go.sum",
        "requirements.txt", "Pipfile.lock", "poetry.lock", "Gemfile.lock", "composer.lock",
        "pom.xml", "packages.lock.json", "pubspec.lock", "mix.lock", "Package.resolved",
        "conan.lock",
    ];
    let mut out = Vec::new();
    let mut seen: BTreeMap<[u8; 32], PathBuf> = BTreeMap::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];
    let max_depth = 8;
    while let Some((d, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let rd = match std::fs::read_dir(&d) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let p = e.path();
            if p.is_dir() {
                if SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                let ps = p.to_string_lossy().to_string();
                if ps.contains("/proc/")
                    || ps.contains("/sys/")
                    || ps.contains("/dev/")
                    || ps.contains("/run/")
                    || ps.contains("/go/pkg/mod/")
                {
                    continue;
                }
                stack.push((p, depth + 1));
            } else if lockfile_names.contains(&name.as_str()) {
                if let Ok(bytes) = std::fs::read(&p) {
                    let hash: [u8; 32] = sha2::Sha256::digest(&bytes).into();
                    if seen.insert(hash, p.clone()).is_none() {
                        out.push(p);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

fn ecosystem_for(path: &Path) -> Option<&'static str> {
    match path.file_name()?.to_str()? {
        "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" => Some("npm"),
        "Cargo.lock" => Some("crates.io"),
        "go.sum" => Some("Go"),
        "requirements.txt" | "Pipfile.lock" | "poetry.lock" => Some("PyPI"),
        "Gemfile.lock" => Some("RubyGems"),
        "composer.lock" => Some("Packagist"),
        "pom.xml" => Some("Maven"),
        "packages.lock.json" => Some("NuGet"),
        "pubspec.lock" => Some("Pub"),
        "mix.lock" => Some("Hex"),
        "Package.resolved" => Some("SwiftURL"),
        "conan.lock" => Some("Conan"),
        _ => None,
    }
}

fn rel_label(dir: &Path, path: &Path) -> String {
    path.strip_prefix(dir).unwrap_or(path).display().to_string()
}

fn parse_lockfile(path: &Path) -> Vec<(String, String)> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    match name {
        "package-lock.json" => {
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            if let Some(pkgs) = v.get("packages").and_then(|p| p.as_object()) {
                for (key, val) in pkgs {
                    let Some(version) = val.get("version").and_then(|x| x.as_str()) else {
                        continue;
                    };
                    if version == "0.0.0" || version.is_empty() {
                        continue;
                    }
                    let pkg = key
                        .split("node_modules/")
                        .last()
                        .unwrap_or(key)
                        .to_string();
                    if !pkg.is_empty() {
                        out.push((pkg, version.to_string()));
                    }
                }
            } else if let Some(deps) = v.get("dependencies").and_then(|d| d.as_object()) {
                for (name, val) in deps {
                    if let Some(version) = val.get("version").and_then(|x| x.as_str()) {
                        out.push((name.clone(), version.to_string()));
                    }
                }
            }
        }
        "yarn.lock" => parse_yarn(&content, &mut out),
        "pnpm-lock.yaml" => {
            for line in content.lines() {
                let t = line.trim();
                if (t.starts_with('\'') || t.starts_with('"')) && t.ends_with("':") {
                    let key = t
                        .trim_start_matches(['\'', '"'])
                        .trim_end_matches("':");
                    if let Some(idx) = key.rfind('@') {
                        let name = &key[..idx];
                        let version = key[idx + 1..].trim();
                        if !name.is_empty() && !version.is_empty() {
                            out.push((name.to_string(), version.to_string()));
                        }
                    }
                }
            }
        }
        "Cargo.lock" | "poetry.lock" => parse_toml_lock(&content, &mut out),
        "go.sum" => {
            for line in content.lines() {
                if line.contains("/go.mod") {
                    continue;
                }
                let mut it = line.split_whitespace();
                let (Some(module), Some(version)) = (it.next(), it.next()) else {
                    continue;
                };
                out.push((module.to_string(), version.to_string()));
            }
        }
        "requirements.txt" => {
            for line in content.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                let mut it = line.splitn(2, "==");
                if let (Some(pkg), Some(version)) = (it.next(), it.next()) {
                    let version = version.trim();
                    if version
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
                    {
                        out.push((pkg.trim().to_string(), version.to_string()));
                    }
                }
            }
        }
        "Pipfile.lock" => {
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            for section in ["default", "develop"] {
                if let Some(obj) = v.get(section).and_then(|o| o.as_object()) {
                    for (name, val) in obj {
                        if let Some(version) = val.get("version").and_then(|x| x.as_str()) {
                            let v = version
                                .trim_start_matches("==")
                                .trim_start_matches('=')
                                .trim();
                            if !v.is_empty() {
                                out.push((name.clone(), v.to_string()));
                            }
                        }
                    }
                }
            }
        }
        "Gemfile.lock" => {
            for line in content.lines() {
                let line = line.trim();
                if let Some(open) = line.find('(') {
                    if line.starts_with("    ") || line.starts_with("  ") || line.starts_with(' ') {
                        if let Some(close) = line.find(')') {
                            if close > open + 1 {
                                let name = line[..open].trim();
                                let version = &line[open + 1..close];
                                if !name.is_empty() && !version.contains(' ') {
                                    out.push((name.to_string(), version.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
        "composer.lock" => {
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            for section in ["packages", "packages-dev"] {
                if let Some(arr) = v.get(section).and_then(|a| a.as_array()) {
                    for p in arr {
                        let Some(name) = p.get("name").and_then(|x| x.as_str()) else {
                            continue;
                        };
                        let Some(version) = p.get("version").and_then(|x| x.as_str()) else {
                            continue;
                        };
                        out.push((name.to_string(), version.to_string()));
                    }
                }
            }
        }
        "pom.xml" => {
            let mut in_dep = false;
            let mut artifact = String::new();
            let mut group = String::new();
            let mut version: Option<String> = None;
            for line in content.lines() {
                let t = line.trim();
                if t.starts_with("<dependency>") {
                    in_dep = true;
                    artifact.clear();
                    group.clear();
                    version = None;
                    continue;
                }
                if t.starts_with("</dependency>") {
                    if !artifact.is_empty() {
                        if let Some(v) = &version {
                            if !v.starts_with('$') && !v.is_empty() {
                                let name = if group.is_empty() {
                                    artifact.clone()
                                } else {
                                    format!("{}:{}", group, artifact)
                                };
                                out.push((name, v.clone()));
                            }
                        }
                    }
                    in_dep = false;
                    continue;
                }
                if !in_dep {
                    continue;
                }
                if let Some(g) = strip_tag(t, "groupId") {
                    group = g;
                } else if let Some(a) = strip_tag(t, "artifactId") {
                    artifact = a;
                } else if let Some(v) = strip_tag(t, "version") {
                    version = Some(v);
                }
            }
        }
        "packages.lock.json" => {
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            if let Some(deps) = v.get("dependencies").and_then(|d| d.as_object()) {
                for arr in deps.values() {
                    if let Some(arr) = arr.as_array() {
                        for p in arr {
                            let Some(name) = p.get("name").and_then(|x| x.as_str()) else {
                                continue;
                            };
                            let version = p
                                .get("version")
                                .or_else(|| p.get("resolved"))
                                .and_then(|x| x.as_str());
                            if let Some(v) = version {
                                out.push((name.to_string(), v.to_string()));
                            }
                        }
                    }
                }
            }
        }
        "pubspec.lock" => {
            let mut pkg_name = String::new();
            for line in content.lines() {
                let trimmed = line.trim();
                if line.starts_with("    ") && !pkg_name.is_empty() {
                    if let Some(rest) = trimmed.strip_prefix("version:") {
                        let v = rest.trim().trim_matches('"');
                        if !v.is_empty() {
                            out.push((pkg_name.clone(), v.to_string()));
                            pkg_name.clear();
                        }
                        continue;
                    }
                    continue;
                }
                if line.starts_with("  ") && !line.starts_with("    ") && trimmed.ends_with(':') {
                    let name = trimmed.trim_end_matches(':').trim();
                    if !name.is_empty() && !name.contains([' ', ':']) {
                        pkg_name = name.to_string();
                    }
                }
            }
        }
        "mix.lock" => {
            for line in content.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix('"') {
                    if let Some(comma) = rest.find("\":") {
                        let name = &rest[..comma];
                        let body = &rest[comma + 2..];
                        let parts: Vec<&str> = body.split(',').map(|s| s.trim()).collect();
                        if parts.len() >= 3 {
                            let version = parts[2].trim_matches('"');
                            if !version.is_empty() && version.contains('.') {
                                out.push((name.to_string(), version.to_string()));
                            }
                        }
                    }
                }
            }
        }
        "Package.resolved" => {
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            if let Some(pins) = v.get("pins").and_then(|p| p.as_array()) {
                for p in pins {
                    let name = p
                        .get("identity")
                        .or_else(|| p.get("package"))
                        .and_then(|x| x.as_str());
                    let version = p.pointer("/state/version").and_then(|x| x.as_str());
                    if let (Some(n), Some(vv)) = (name, version) {
                        out.push((n.to_string(), vv.to_string()));
                    }
                }
            } else if let Some(objs) = v.get("object").and_then(|o| o.as_array()) {
                for p in objs {
                    let name = p.get("package").and_then(|x| x.as_str());
                    let version = p.get("version").and_then(|x| x.as_str());
                    if let (Some(n), Some(vv)) = (name, version) {
                        out.push((n.to_string(), vv.to_string()));
                    }
                }
            }
        }
        "conan.lock" => {
            let v: serde_json::Value = serde_json::from_str(&content).unwrap_or_default();
            let list = v
                .get("requires")
                .or_else(|| v.get("require"))
                .and_then(|r| r.as_array());
            if let Some(list) = list {
                for r in list {
                    if let Some(s) = r.as_str() {
                        let s = s.split('#').next().unwrap_or(s);
                        if let Some(idx) = s.find('/') {
                            let name = &s[..idx];
                            let version = s[idx + 1..]
                                .split('@')
                                .next()
                                .unwrap_or("")
                                .to_string();
                            if !name.is_empty() && !version.is_empty() {
                                out.push((name.to_string(), version));
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    out.sort();
    out.dedup();
    out
}

fn strip_tag(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    if line.starts_with(&open) && line.ends_with(&close) {
        let inner = &line[open.len()..line.len() - close.len()];
        return Some(inner.trim().to_string());
    }
    None
}

fn parse_yarn(content: &str, out: &mut Vec<(String, String)>) {
    let mut pending: Option<String> = None;
    for line in content.lines() {
        let line = line.trim_end();
        if line.starts_with("  ") {
            if let Some(name) = pending.take() {
                if let Some(rest) = line.trim_start().strip_prefix("version ") {
                    let version = rest.trim().trim_matches('"');
                    if !version.is_empty() {
                        out.push((name, version.to_string()));
                    }
                }
            }
            continue;
        }
        if line.ends_with(':') && line.contains('@') {
            let key = line.trim_end_matches(':').trim();
            let name = key
                .split(',')
                .next()
                .unwrap_or(key)
                .trim()
                .trim_matches('"');
            let name = name
                .split('@')
                .take_while(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("@");
            pending = if name.is_empty() { None } else { Some(name) };
        } else if !line.is_empty() {
            pending = None;
        }
    }
}

fn parse_toml_lock(content: &str, out: &mut Vec<(String, String)>) {
    let val: toml::Value =
        toml::from_str(content).unwrap_or_else(|_| toml::Value::Table(Default::default()));
    let Some(pkgs) = val.get("package").and_then(|p| p.as_array()) else {
        return;
    };
    for p in pkgs {
        let Some(name) = p.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        let Some(version) = p.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        out.push((name.to_string(), version.to_string()));
    }
}

fn osv_batch(agent: &ureq::Agent, queries: &[PackageQuery]) -> BTreeMap<String, Vec<String>> {
    let mut uniq: Vec<&PackageQuery> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for q in queries {
        if seen.insert(query_key(q)) {
            uniq.push(q);
        }
    }
    let mut hits: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for chunk in uniq.chunks(OSV_CHUNK) {
        let body = serde_json::json!({
            "queries": chunk.iter().map(|q| {
                serde_json::json!({
                    "package": { "ecosystem": q.ecosystem, "name": q.name },
                    "version": q.version,
                })
            }).collect::<Vec<_>>(),
        });
        let resp = match agent.post(OSV_BATCH).send_json(&body) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  {} OSV query failed: {}", error(""), e);
                continue;
            }
        };
        let value: serde_json::Value = match resp.into_json() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let results = value.get("results").and_then(|r| r.as_array());
        if let Some(results) = results {
            for (i, res) in results.iter().enumerate() {
                let Some(q) = chunk.get(i) else { continue };
                let ids: Vec<String> = res
                    .get("vulns")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.get("id").and_then(|x| x.as_str()))
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                if !ids.is_empty() {
                    hits.insert(query_key(q), ids);
                }
            }
        }
    }
    hits
}

fn vuln_details(agent: &ureq::Agent, hits: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vuln> {
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for list in hits.values() {
        for id in list {
            ids.insert(id.clone());
        }
    }
    let mut out: BTreeMap<String, Vuln> = BTreeMap::new();
    let ids: Vec<String> = ids.into_iter().collect();
    let wave = 10;
    for chunk in ids.chunks(wave) {
        std::thread::scope(|s| {
            let mut handles = Vec::new();
            for id in chunk {
                let agent = agent.clone();
                let id = id.clone();
                handles.push(s.spawn(move || {
                    let url = format!("{}/{}", OSV_VULN, id);
                    let resp = match agent.get(&url).call() {
                        Ok(r) => r,
                        Err(_) => return None,
                    };
                    let value: serde_json::Value = match resp.into_json() {
                        Ok(v) => v,
                        Err(_) => return None,
                    };
                    let url = format!("https://osv.dev/vulnerability/{}", id);
                    Some((
                        id.clone(),
                        Vuln {
                            id,
                            alias: value
                                .get("aliases")
                                .and_then(|a| a.as_array())
                                .and_then(|a| a.first())
                                .and_then(|x| x.as_str())
                                .map(|s| s.to_string()),
                            severity: vuln_severity(&value),
                            summary: value
                                .get("summary")
                                .and_then(|s| s.as_str())
                                .map(|s| s.to_string()),
                            fixed: vuln_fixed(&value),
                            url: Some(url),
                            category: classify_vuln(&value),
                            kev_ransomware: false,
                        },
                    ))
                }));
            }
            for h in handles {
                if let Ok(Some((id, vuln))) = h.join() {
                    out.insert(id, vuln);
                }
            }
        });
    }
    out
}

fn vuln_severity(rec: &serde_json::Value) -> Option<String> {
    if let Some(s) = rec
        .pointer("/database_specific/severity")
        .and_then(|v| v.as_str())
    {
        return Some(s.to_uppercase());
    }
    if let Some(arr) = rec.get("severity").and_then(|s| s.as_array()) {
        for sev in arr {
            if let Some(score) = sev.get("score").and_then(|s| s.as_str()) {
                let score = score.trim();
                if score.starts_with("CVSS:3") {
                    if let Some(n) = cvss3_base(score) {
                        return Some(qualitative(n));
                    }
                } else if let Some(num) = score.split_whitespace().next() {
                    if let Ok(n) = num.trim_end_matches(['.', ',']).parse::<f64>() {
                        return Some(qualitative(n));
                    }
                }
            }
        }
    }
    None
}

fn cvss3_base(vector: &str) -> Option<f64> {
    let mut m: BTreeMap<String, String> = BTreeMap::new();
    for part in vector.split('/') {
        let Some(idx) = part.find(':') else {
            continue;
        };
        let (k, v) = (&part[..idx], &part[idx + 1..]);
        m.insert(k.to_string(), v.to_string());
    }
    let av = match m.get("AV").map(|s| s.as_str()) {
        Some("N") => 0.85,
        Some("A") => 0.62,
        Some("L") => 0.55,
        Some("P") => 0.2,
        _ => return None,
    };
    let ac = match m.get("AC").map(|s| s.as_str()) {
        Some("L") => 0.77,
        Some("H") => 0.44,
        _ => return None,
    };
    let scope_c = m.get("S").map(|s| s.as_str()) == Some("C");
    let pr = match (scope_c, m.get("PR").map(|s| s.as_str())) {
        (false, Some("N")) => 0.85,
        (false, Some("L")) => 0.62,
        (false, Some("H")) => 0.27,
        (true, Some("N")) => 0.85,
        (true, Some("L")) => 0.68,
        (true, Some("H")) => 0.5,
        _ => return None,
    };
    let ui = match m.get("UI").map(|s| s.as_str()) {
        Some("N") => 0.85,
        Some("R") => 0.62,
        _ => return None,
    };
    let impact = |x: &str| -> Option<f64> {
        match x {
            "H" => Some(0.56),
            "L" => Some(0.22),
            "N" => Some(0.0),
            _ => None,
        }
    };
    let c = impact(m.get("C").map(|s| s.as_str())?)?;
    let i = impact(m.get("I").map(|s| s.as_str())?)?;
    let a = impact(m.get("A").map(|s| s.as_str())?)?;
    let isc = 1.0 - (1.0 - c) * (1.0 - i) * (1.0 - a);
    let impact = if scope_c {
        7.52 * (isc - 0.029) - 3.25 * (isc - 0.02).powi(15)
    } else {
        6.42 * isc
    };
    let expl = 8.22 * av * ac * pr * ui;
    let base = (impact + expl).min(10.0);
    Some((base * 10.0).ceil() / 10.0)
}

fn qualitative(score: f64) -> String {
    if score >= 9.0 {
        "CRITICAL".to_string()
    } else if score >= 7.0 {
        "HIGH".to_string()
    } else if score >= 4.0 {
        "MODERATE".to_string()
    } else {
        "LOW".to_string()
    }
}

fn vuln_fixed(rec: &serde_json::Value) -> Option<String> {
    let mut best: Option<String> = None;
    if let Some(affected) = rec.get("affected").and_then(|a| a.as_array()) {
        for a in affected {
            if let Some(ranges) = a.get("ranges").and_then(|r| r.as_array()) {
                for range in ranges {
                    if let Some(events) = range.get("events").and_then(|e| e.as_array()) {
                        for ev in events {
                            if let Some(f) = ev.get("fixed").and_then(|x| x.as_str()) {
                                best = Some(f.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    best
}

fn classify_vuln(rec: &serde_json::Value) -> Category {
    let id = rec
        .get("id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_lowercase();
    let summary = rec
        .get("summary")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_lowercase();
    let details = rec
        .get("database_specific")
        .map(|d| d.to_string())
        .unwrap_or_default()
        .to_lowercase();
    let text = format!("{} {} {}", id, summary, details);
    let has = |words: &[&str]| words.iter().any(|w| text.contains(w));
    if id.starts_with("mal-")
        || has(&[
            "malware", "malicious code", "malicious package", "supply-chain",
            "supply chain", "compromised", "trojan", "backdoor", "dropper",
            "account takeover", "poisoned", "typosquat",
        ])
    {
        return Category::Infected;
    }
    if has(&[
        "actively exploited", "exploited in the wild", "known exploited",
        "cisa kev", "under active exploitation", "targeted attacks", "ransomware",
    ]) {
        return Category::Exploited;
    }
    if has(&[
        "unmaintained", "abandoned", "deprecated", "unsafe by design",
        "no longer maintained", "not maintained", "archived", "end of life",
        "out of support", "no security fixes",
    ]) {
        return Category::Unmaintained;
    }
    if id.starts_with("ghsa-")
        || id.starts_with("pysec-")
        || id.starts_with("rustsec-")
        || id.starts_with("go-")
        || id.starts_with("cve-")
        || id.starts_with("dsa-")
        || id.starts_with("dla-")
        || id.starts_with("usn-")
        || id.starts_with("dsab-")
        || id.starts_with("cga-")
        || id.starts_with("bit-")
        || id.starts_with("avg-")
        || id.starts_with("asa-")
    {
        return Category::Vulnerable;
    }
    Category::Other
}

fn parse_categories(s: &str) -> Vec<Category> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let part = part.trim().to_ascii_lowercase();
        match part.as_str() {
            "infected" | "malware" => out.push(Category::Infected),
            "exploited" => out.push(Category::Exploited),
            "unmaintained" | "abandoned" => out.push(Category::Unmaintained),
            "vulnerable" | "cve" => out.push(Category::Vulnerable),
            "other" => out.push(Category::Other),
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

fn detect_system() -> Option<SysKind> {
    if which("pacman") {
        return Some(SysKind::Pacman);
    }
    if which("dpkg-query") {
        let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
        let get = |key: &str| {
            os_release.lines().find_map(|l| {
                l.strip_prefix(&format!("{}=", key))
                    .map(|v| v.trim_matches('"').to_lowercase())
            })
        };
        let id = get("ID").unwrap_or_default();
        let id_like = get("ID_LIKE").unwrap_or_default();
        if id == "ubuntu" {
            return Some(SysKind::Dpkg("Ubuntu"));
        }
        if id == "debian" || id_like.contains("debian") {
            return Some(SysKind::Dpkg("Debian"));
        }
        return Some(SysKind::Dpkg("Debian"));
    }
    None
}

fn scan_system_arch(agent: &ureq::Agent) -> (Vec<Finding>, usize, Option<String>) {
    let mut findings = Vec::new();
    let out = match Command::new("pacman").arg("-Q").output() {
        Ok(o) if o.status.success() => o,
        _ => return (findings, 0, None),
    };
    let mut installed: Vec<(String, String)> = Vec::new();
    let mut foreign: BTreeSet<String> = BTreeSet::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut it = line.split_whitespace();
        if let (Some(name), Some(version)) = (it.next(), it.next()) {
            installed.push((name.to_string(), version.to_string()));
        }
    }
    if let Ok(fout) = Command::new("pacman").args(["-Qm"]).output() {
        for line in String::from_utf8_lossy(&fout.stdout).lines() {
            if let Some(name) = line.split_whitespace().next() {
                foreign.insert(name.to_string());
            }
        }
    }

    let (issues_value, ast_updated) = match get_json_meta(agent, AST_ISSUES) {
        Some(v) => v,
        None => {
            eprintln!("  {} Arch Security Tracker unreachable.", error(""));
            return (findings, installed.len(), None);
        }
    };
    let Some(issues) = issues_value.as_array() else {
        return (findings, installed.len(), None);
    };

    let mut by_pkg: BTreeMap<String, Vec<&serde_json::Value>> = BTreeMap::new();
    for issue in issues {
        let Some(pkgs) = issue.get("packages").and_then(|p| p.as_array()) else {
            continue;
        };
        for p in pkgs {
            if let Some(name) = p.as_str() {
                by_pkg.entry(name.to_string()).or_default().push(issue);
            }
        }
    }

    for (name, version) in &installed {
        let Some(matches) = by_pkg.get(name) else {
            continue;
        };
        let mut vulns: Vec<Vuln> = Vec::new();
        for issue in matches {
            let Some(fixed) = issue.get("fixed").and_then(|f| f.as_str()) else {
                continue;
            };
            if fixed.is_empty() || !arch_version_lt(version, fixed) {
                continue;
            }
            let mut id = issue
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("AVG")
                .to_string();
            let avg_url = format!("https://security.archlinux.org/{}", id);
            if let Some(cves) = issue.get("issues").and_then(|i| i.as_array()) {
                let cves: Vec<String> = cves
                    .iter()
                    .filter_map(|i| i.as_str())
                    .map(|s| s.to_string())
                    .collect();
                if !cves.is_empty() {
                    id = format!("{} ({})", id, cves.join(", "));
                }
            }
            let severity = issue
                .get("severity")
                .and_then(|s| s.as_str())
                .map(|s| s.to_uppercase());
            let atype = issue.get("type").and_then(|t| t.as_str()).unwrap_or("");
            vulns.push(Vuln {
                id,
                alias: None,
                severity,
                summary: if atype.is_empty() {
                    None
                } else {
                    Some(format!("Arch advisory: {}", atype))
                },
                fixed: Some(fixed.to_string()),
                url: Some(avg_url),
                category: Category::Vulnerable,
                kev_ransomware: false,
            });
        }
        if !vulns.is_empty() {
            let src = if foreign.contains(name) {
                "system (AUR)"
            } else {
                "system (pacman)"
            };
            findings.push(Finding {
                source: src.to_string(),
                package: name.clone(),
                version: version.clone(),
                vulns,
            });
        }
    }
    (findings, installed.len(), ast_updated)
}

fn scan_system_dpkg(agent: &ureq::Agent, ecosystem: &'static str) -> (Vec<Finding>, usize) {
    let mut findings = Vec::new();
    let out = match Command::new("dpkg-query")
        .args(["-W", "-f=${Package} ${Version}\n"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return (findings, 0),
    };
    let mut installed: BTreeMap<String, String> = BTreeMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut it = line.split_whitespace();
        if let (Some(name), Some(version)) = (it.next(), it.next()) {
            let name = name.split(':').next().unwrap_or(name).to_string();
            installed.entry(name).or_insert_with(|| version.to_string());
        }
    }
    let queries: Vec<PackageQuery> = installed
        .iter()
        .map(|(n, v)| PackageQuery {
            ecosystem,
            name: n.clone(),
            version: v.clone(),
            source: format!("system ({})", ecosystem),
        })
        .collect();
    let hits = osv_batch(agent, &queries);
    let details = vuln_details(agent, &hits);
    for q in &queries {
        let Some(ids) = hits.get(&query_key(q)) else {
            continue;
        };
        let vulns: Vec<Vuln> = dedup_vulns(
            ids.iter()
                .filter_map(|id| details.get(id).cloned())
                .collect(),
        );
        if vulns.is_empty() {
            continue;
        }
        findings.push(Finding {
            source: format!("system ({})", ecosystem),
            package: q.name.clone(),
            version: q.version.clone(),
            vulns,
        });
    }
    (findings, installed.len())
}

fn dedup_vulns(vulns: Vec<Vuln>) -> Vec<Vuln> {
    let mut map: BTreeMap<String, Vuln> = BTreeMap::new();
    for v in vulns {
        let key = v.alias.clone().unwrap_or_else(|| v.id.clone());
        let better = match map.get(&key) {
            None => true,
            Some(existing) => existing.severity.is_none() && v.severity.is_some(),
        };
        if better {
            map.insert(key, v);
        }
    }
    map.into_values().collect()
}

fn get_json_meta(agent: &ureq::Agent, url: &str) -> Option<(serde_json::Value, Option<String>)> {
    let resp = agent.get(url).call().ok()?;
    let last_modified = resp
        .header("last-modified")
        .or_else(|| resp.header("date"))
        .map(|s| s.to_string());
    resp.into_json().ok().map(|v| (v, last_modified))
}

fn arch_version_lt(installed: &str, fixed: &str) -> bool {
    if which("vercmp") {
        if let Ok(out) = Command::new("vercmp").args([installed, fixed]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Ok(n) = text.trim().parse::<i32>() {
                return n < 0;
            }
        }
    }
    vercmp_manual(installed, fixed) < 0
}

fn vercmp_manual(a: &str, b: &str) -> i32 {
    let split = |s: &str| -> (u64, String, String) {
        let s = s.trim();
        let mut rest = s;
        let mut epoch = 0u64;
        if let Some(idx) = s.find(':') {
            if let Ok(e) = s[..idx].parse::<u64>() {
                epoch = e;
                rest = &s[idx + 1..];
            }
        }
        let (version, release) = match rest.find('-') {
            Some(idx) => (rest[..idx].to_string(), rest[idx + 1..].to_string()),
            None => (rest.to_string(), "0".to_string()),
        };
        (epoch, version, release)
    };
    let (ea, va, ra) = split(a);
    let (eb, vb, rb) = split(b);
    if ea != eb {
        return if ea < eb { -1 } else { 1 };
    }
    let cv = cmp_segments(&va, &vb);
    if cv != 0 {
        return cv;
    }
    cmp_segments(&ra, &rb)
}

fn cmp_segments(x: &str, y: &str) -> i32 {
    let mut xi = x.split('.');
    let mut yi = y.split('.');
    loop {
        let (xa, ya) = (xi.next(), yi.next());
        match (xa, ya) {
            (None, None) => return 0,
            (None, Some(_)) => return -1,
            (Some(_), None) => return 1,
            (Some(xs), Some(ys)) => {
                let c = cmp_token(xs, ys);
                if c != 0 {
                    return c;
                }
            }
        }
    }
}

fn cmp_token(x: &str, y: &str) -> i32 {
    let xnum: Option<u64> = x.parse().ok();
    let ynum: Option<u64> = y.parse().ok();
    match (xnum, ynum) {
        (Some(a), Some(b)) => {
            if a < b {
                -1
            } else if a > b {
                1
            } else {
                0
            }
        }
        (Some(_), None) => 1,
        (None, Some(_)) => -1,
        (None, None) => {
            if x < y {
                -1
            } else if x > y {
                1
            } else {
                0
            }
        }
    }
}

fn print_findings(findings: &[Finding]) {
    if findings.is_empty() {
        return;
    }
    let mut by_source: BTreeMap<String, Vec<&Finding>> = BTreeMap::new();
    for f in findings {
        by_source.entry(f.source.clone()).or_default().push(f);
    }
    for (source, list) in &by_source {
        println!("  {}", format!("SOURCE: {}", source).style(Theme::LABEL));
        for f in list {
            println!(
                "    {} {}",
                f.package.style(Theme::BOLD),
                f.version.style(Theme::MUTED)
            );
            for v in &f.vulns {
                let sev = v
                    .severity
                    .as_deref()
                    .map(sev_style)
                    .unwrap_or_else(|| "[?]".style(Theme::MUTED).to_string());
                let cat = category_badge(v);
                let alias = v
                    .alias
                    .as_ref()
                    .map(|a| format!(" ({})", a.style(Theme::MUTED)))
                    .unwrap_or_default();
                let summary = v
                    .summary
                    .as_ref()
                    .map(|s| format!(" — {}", s.chars().take(90).collect::<String>()))
                    .unwrap_or_default();
                let fix = v
                    .fixed
                    .as_ref()
                    .map(|f| format!(" → fix {}", f.style(Theme::SUCCESS)))
                    .unwrap_or_default();
                println!(
                    "      {} {}{} {}{}{}{}",
                    warn(""),
                    cat,
                    sev,
                    v.id.style(Theme::BOLD),
                    alias,
                    fix,
                    summary
                );
            }
        }
        println!();
    }
}

fn sev_style(s: &str) -> String {
    let up = s.to_uppercase();
    if up.starts_with("CRIT") || up.starts_with("HIGH") {
        format!("[{}]", up.style(Theme::ERROR))
    } else if up.starts_with("MOD") || up.starts_with("MED") {
        format!("[{}]", up.style(Theme::WARN))
    } else if up.starts_with("LOW") {
        format!("[{}]", up.style(Theme::MUTED))
    } else {
        format!("[{}]", up.style(Theme::VALUE))
    }
}

fn print_summary(findings: &[Finding], checked: &[Checked]) {
    let total_pkgs: usize = checked.iter().map(|c| c.packages).sum();
    let vuln_pkgs = findings.len();
    println!("{}", divider());
    for c in checked {
        println!(
            "  {}",
            label_value(
                &c.source,
                &format!("{} package(s) checked", c.packages.style(Theme::VALUE))
            )
        );
    }
    println!("  {}", label_value("Total", &format!("{} package(s)", total_pkgs)));
    println!();
    if vuln_pkgs == 0 {
        println!("  {} No known vulnerabilities found.", success(""));
    } else {
        println!(
            "  {} {} vulnerable package(s) found — check the advisories above and update.",
            warn(""),
            vuln_pkgs.style(Theme::ERROR)
        );
    }
}

fn category_badge(v: &Vuln) -> String {
    if v.kev_ransomware {
        return format!("[{}] ", "RANSOMWARE".red().bold());
    }
    match v.category {
        Category::Infected => format!("[{}] ", "INFECTED".red().bold()),
        Category::Exploited => format!("[{}] ", "KEV".red()),
        Category::Unmaintained => format!("[{}] ", "UNMAINTAINED".yellow()),
        _ => String::new(),
    }
}

fn print_high_risk(findings: &[Finding]) {
    let mut infected: BTreeSet<String> = BTreeSet::new();
    let mut exploited: BTreeSet<String> = BTreeSet::new();
    let mut ransomware: BTreeSet<String> = BTreeSet::new();
    let mut critical: BTreeSet<String> = BTreeSet::new();
    for f in findings {
        for v in &f.vulns {
            if v.kev_ransomware {
                ransomware.insert(f.package.clone());
            }
            match v.category {
                Category::Infected => {
                    infected.insert(f.package.clone());
                }
                Category::Exploited => {
                    exploited.insert(f.package.clone());
                }
                _ => {
                    if v.severity.as_deref().is_some_and(|s| {
                        s.to_ascii_uppercase().starts_with("CRIT")
                    }) {
                        critical.insert(f.package.clone());
                    }
                }
            }
        }
    }
    if infected.is_empty() && exploited.is_empty() && critical.is_empty() && ransomware.is_empty()
    {
        return;
    }
    println!("{}", divider());
    println!(
        "  {} HIGH RISK PACKAGES (names only — act on these first):",
        error("")
    );
    let groups = [
        ("RANSOMWARE", ransomware),
        ("INFECTED", infected),
        ("EXPLOITED", exploited),
        ("CRITICAL", critical),
    ];
    for (label, set) in groups {
        if set.is_empty() {
            continue;
        }
        let names: Vec<String> = set.into_iter().collect();
        let joined = if names.len() > 12 {
            format!(
                "{}, (+{} more)",
                names[..12].join(", "),
                names.len() - 12
            )
        } else {
            names.join(", ")
        };
        println!("    {}: {}", label, joined.style(Theme::VALUE));
    }
    println!();
}

fn sev_score(sev: Option<&str>) -> f64 {
    match sev.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("critical") => 9.0,
        Some("high") => 7.0,
        Some("moderate") | Some("medium") => 4.0,
        Some("low") => 1.0,
        _ => 0.0,
    }
}

fn print_freshness(agent: &ureq::Agent, ecosystems: &[&str], ast_updated: Option<String>) {
    println!("  {} Data freshness:", muted(""));
    for (ec, updated) in osv_freshness(agent, ecosystems) {
        println!(
            "    {}  {} {}",
            muted("•"),
            format!("{:<12}", ec).style(Theme::VALUE),
            updated.style(Theme::VALUE)
        );
    }
    if let Some(u) = ast_updated {
        println!(
            "    {}  {} {}",
            muted("•"),
            format!("{:<12}", "Arch Security Tracker").style(Theme::VALUE),
            fmt_updated(Some(u)).style(Theme::VALUE)
        );
    }
}

fn osv_freshness(agent: &ureq::Agent, ecosystems: &[&str]) -> Vec<(String, String)> {
    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for ec in ecosystems {
            if *ec == "Arch Security Tracker" {
                continue;
            }
            let agent = agent.clone();
            let ec = ec.to_string();
            handles.push(s.spawn(move || {
                let url = format!(
                    "https://osv-vulnerabilities.storage.googleapis.com/{}/all.zip",
                    ec
                );
                let lm = agent
                    .head(&url)
                    .call()
                    .ok()
                    .and_then(|r| r.header("last-modified").map(|x| x.to_string()));
                (ec, fmt_updated(lm))
            }));
        }
        let mut v: Vec<(String, String)> = handles.into_iter().filter_map(|h| h.join().ok()).collect();
        v.sort();
        v
    })
}

fn fmt_updated(last_modified: Option<String>) -> String {
    let Some(lm) = last_modified else {
        return "unknown".to_string();
    };
    match rfc2822_to_unix(&lm) {
        Some(secs) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if secs > now {
                return format!("updated {}", rfc2822_date_short(&lm));
            }
            format!(
                "updated {} ({} ago)",
                rfc2822_date_short(&lm),
                fmt_age(now - secs)
            )
        }
        None => lm,
    }
}

fn rfc2822_date_short(s: &str) -> String {
    let toks: Vec<&str> = s.split_ascii_whitespace().collect();
    if toks.len() < 4 {
        return s.to_string();
    }
    let day: u32 = toks[1].trim_end_matches(',').parse().unwrap_or(0);
    let month = month_num(toks[2]);
    let time = toks.get(4).unwrap_or(&"");
    match (day, month, time.len()) {
        (d, Some(m), n) if n >= 5 => format!("{}-{:02}-{:02} {}", toks[3], m, d, &time[..5]),
        (d, Some(m), _) => format!("{}-{:02}-{:02}", toks[3], m, d),
        _ => s.to_string(),
    }
}

fn month_num(s: &str) -> Option<u32> {
    match s {
        "Jan" => Some(1),
        "Feb" => Some(2),
        "Mar" => Some(3),
        "Apr" => Some(4),
        "May" => Some(5),
        "Jun" => Some(6),
        "Jul" => Some(7),
        "Aug" => Some(8),
        "Sep" => Some(9),
        "Oct" => Some(10),
        "Nov" => Some(11),
        "Dec" => Some(12),
        _ => None,
    }
}

fn fmt_age(secs: u64) -> String {
    if secs < 90 {
        return format!("{}s", secs);
    }
    if secs < 3600 {
        return format!("{}m", secs / 60);
    }
    if secs < 86_400 {
        return format!("{}h", secs / 3600);
    }
    format!("{}d", secs / 86_400)
}

fn rfc2822_to_unix(s: &str) -> Option<u64> {
    let toks: Vec<&str> = s.split_ascii_whitespace().collect();
    if toks.len() < 5 {
        return None;
    }
    let day: i64 = toks[1].trim_end_matches(',').parse().ok()?;
    let month: i64 = month_num(toks[2])? as i64;
    let year: i64 = toks[3].parse().ok()?;
    let hms: Vec<&str> = toks[4].split(':').collect();
    if hms.len() < 3 {
        return None;
    }
    let hh: i64 = hms[0].parse().ok()?;
    let mm: i64 = hms[1].parse().ok()?;
    let ss: i64 = hms[2].parse().ok()?;
    let days = days_from_civil(year, month, day);
    Some((days * 86_400 + hh * 3600 + mm * 60 + ss) as u64)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn open_advisories(findings: &[Finding]) {
    let mut urls: Vec<String> = Vec::new();
    for f in findings {
        for v in &f.vulns {
            if let Some(u) = &v.url {
                if !urls.contains(u) {
                    urls.push(u.clone());
                }
            }
        }
    }
    if urls.is_empty() {
        return;
    }
    let opener = if which("xdg-open") {
        "xdg-open"
    } else if which("open") {
        "open"
    } else {
        println!("  {} No browser opener found — open these manually:", muted(""));
        for u in urls.iter().take(5) {
            println!("    {}", u);
        }
        return;
    };
    for u in urls.iter().take(5) {
        let _ = Command::new(opener).arg(u).spawn();
    }
    if urls.len() > 5 {
        println!(
            "  {} Opened the first 5 of {} advisories in the browser.",
            muted(""),
            urls.len()
        );
    }
}
