use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

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

fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn warn(s: &str) -> String { format!("{} {}", "⚠".style(Theme::WARN), s) }
fn divider() -> String { "─".repeat(50).dimmed().to_string() }

fn which(binary: &str) -> bool {
    std::process::Command::new("which").arg(binary)
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}
fn run_command_output(program: &str, args: &[&str]) -> std::io::Result<String> {
    let output = std::process::Command::new(program).args(args).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct Spinner {
    spinner: indicatif::ProgressBar,
}
impl Spinner {
    fn new(msg: &str) -> Self {
        let sp = indicatif::ProgressBar::new_spinner().with_message(msg.to_string())
            .with_style(indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg}").unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]));
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { spinner: sp }
    }
    fn done(&self, msg: &str) { self.spinner.finish_with_message(msg.to_string()); }
    fn fail(&self, msg: &str) {
        self.spinner.finish_with_message(format!("{} {}", "✗".style(Theme::ERROR), msg.style(Theme::ERROR)));
    }
}

#[derive(Parser)]
#[command(name = "git-extras", about = "Extra git utilities")]
struct Cli {
    #[command(subcommand)]
    action: GitExtrasAction,
}

#[derive(Subcommand, Debug, Clone)]
enum GitExtrasAction {
    #[command(about = "Use git bisect to find the commit that broke tests")]
    WhoBroke {
        #[arg(trailing_var_arg = true, help = "Test command to run (auto-detected if omitted)")]
        args: Vec<String>,
    },
    #[command(about = "Show branch impact and risk score")]
    Impact,
    #[command(about = "Catch up with upstream changes")]
    Catchup,
    #[command(about = "Create a release tag and optionally a GitHub release")]
    Release {
        #[arg(help = "Version string (e.g. 1.2.3 or v1.2.3)")]
        version: Option<String>,
        #[arg(short, long, help = "Push tag to origin")]
        push: bool,
        #[arg(short, long, help = "Create GitHub release via gh")]
        github: bool,
        #[arg(short, long, help = "Release notes")]
        notes: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();
    match &cli.action {
        GitExtrasAction::WhoBroke { args } => who_broke(args),
        GitExtrasAction::Impact => impact(),
        GitExtrasAction::Catchup => catchup(),
        GitExtrasAction::Release { version, push, github, notes } => release(version.as_deref(), *push, *github, notes.as_deref()),
    }
}

fn is_git_repo() -> bool {
    std::process::Command::new("git").args(["rev-parse", "--git-dir"])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

fn run_test(cmd: &str) -> bool {
    std::process::Command::new("sh").arg("-c").arg(cmd)
        .stdout(std::process::Stdio::inherit()).stderr(std::process::Stdio::inherit())
        .status().map(|s| s.success()).unwrap_or(false)
}

fn default_test_cmd() -> String {
    if std::path::Path::new("Cargo.toml").exists() { "cargo test".to_string() }
    else if std::path::Path::new("package.json").exists() { "npm test".to_string() }
    else if std::path::Path::new("pyproject.toml").exists() || std::path::Path::new("requirements.txt").exists() { "pytest".to_string() }
    else if std::path::Path::new("go.mod").exists() { "go test ./...".to_string() }
    else { "true".to_string() }
}

fn git_ok(args: &[&str]) -> bool {
    std::process::Command::new("git").args(args)
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

fn git_stdout(args: &[&str]) -> Option<String> {
    run_command_output("git", args).ok().filter(|s| !s.is_empty())
}

fn who_broke(cmd_args: &[String]) {
    if !which("git") || !is_git_repo() { eprintln!("{} Must be inside a git repository.", error("")); std::process::exit(1); }
    let test_cmd = if cmd_args.is_empty() { default_test_cmd() } else { cmd_args.join(" ") };

    println!("{} {}", "◆".style(Theme::ACCENT), "Git Bisect Hunter".style(Theme::HEADER));
    println!("{}", divider());
    println!("  {} {}", "Test command:".style(Theme::LABEL), test_cmd.style(Theme::VALUE));

    let branch = git_stdout(&["branch", "--show-current"]).unwrap_or_else(|| "HEAD".to_string());
    let dirty = git_stdout(&["status", "--porcelain"]).map(|s| !s.is_empty()).unwrap_or(false);
    if dirty {
        println!("  {} Stashing uncommitted changes...", "→".dimmed());
        if !git_ok(&["stash", "push", "-u", "-m", "proto git-who-broke"]) { eprintln!("{} Could not stash changes.", error("")); std::process::exit(1); }
    }

    let ancestors = git_stdout(&["rev-list", "HEAD"]).unwrap_or_default();
    let shas: Vec<&str> = ancestors.lines().collect();
    if shas.is_empty() { eprintln!("{} No commits to bisect.", error("")); std::process::exit(1); }
    let broken_head = shas[0].to_string();

    println!("\n  {} Running tests on current HEAD...", "▶".style(Theme::ACCENT));
    if run_test(&test_cmd) {
        println!("  {} Tests pass on HEAD — nothing broken.", success(""));
        if dirty { let _ = git_ok(&["stash", "pop"]); }
        return;
    }

    println!("\n  {} Walking back to find a known-good commit...", "▶".style(Theme::ACCENT));
    let mut good: Option<String> = None;
    for (i, sha) in shas.iter().enumerate().skip(1).take(50) {
        let sha = *sha;
        println!("    {} {}  (test running...)", format!("{}/50", i).dimmed(), (&sha[..8]).dimmed());
        if git_ok(&["checkout", "--quiet", sha]) && run_test(&test_cmd) {
            good = Some(sha.to_string());
            println!("    {} Found good commit: {}", "✔".style(Theme::SUCCESS), (&sha[..12]).style(Theme::ACCENT));
            break;
        }
    }

    let good = match good {
        Some(g) => g,
        None => {
            let _ = git_ok(&["checkout", &branch]);
            if dirty { let _ = git_ok(&["stash", "pop"]); }
            eprintln!("\n{} No passing commit found in the last 50 commits.", error(""));
            std::process::exit(1);
        }
    };

    println!("\n  {} Bisecting between good {} and bad {}...", "▶".style(Theme::ACCENT), (&good[..12]).style(Theme::MUTED), (&broken_head[..12]).style(Theme::ERROR));
    if !git_ok(&["bisect", "start"]) || !git_ok(&["bisect", "bad", &broken_head]) || !git_ok(&["bisect", "good", &good]) {
        eprintln!("{} Failed to start bisect.", error(""));
        std::process::exit(1);
    }

    let mut culprit: Option<String> = None;
    let mut steps = 0;
    while steps < 40 {
        steps += 1;
        let pass = run_test(&test_cmd);
        let out = if pass { git_stdout(&["bisect", "good"]) } else { git_stdout(&["bisect", "bad"]) };
        match out {
            Some(o) => {
                if let Some(line) = o.lines().find(|l| l.contains("is the first bad commit")) {
                    if let Some(sha) = line.split_whitespace().find(|w| w.len() == 40) { culprit = Some(sha.to_string()); }
                    break;
                }
            }
            None => break,
        }
    }

    let _ = git_ok(&["bisect", "reset"]);
    let _ = git_ok(&["checkout", &branch]);
    if dirty { let _ = git_ok(&["stash", "pop"]); }

    println!(); println!("{}", divider());
    match culprit {
        Some(sha) => {
            println!("{}", success(&format!("First bad commit found in {} step(s):", steps)));
            println!("  {}", sha.style(Theme::ACCENT).bold());
            if let Some(details) = git_stdout(&["show", "--no-patch", "--format=format:%h %an <%ae>%n%ad%n%n%s", "--date=short", &sha]) {
                println!("{}", details);
            }
            let _ = std::process::Command::new("git").args(["show", "--stat", "--oneline", &sha])
                .stdout(std::process::Stdio::inherit()).stderr(std::process::Stdio::null()).status();
        }
        None => eprintln!("{} Bisect finished but no culprit identified.", error("")),
    }
}

fn default_base() -> Option<String> {
    for candidate in ["origin/main", "origin/master", "origin/develop", "main", "master", "develop"] {
        if git_stdout(&["rev-parse", "--verify", "--quiet", candidate]).is_some() { return Some(candidate.to_string()); }
    }
    None
}

fn impact() {
    if !which("git") || !is_git_repo() { eprintln!("{} Must be inside a git repository.", error("")); std::process::exit(1); }

    let branch = git_stdout(&["branch", "--show-current"]).unwrap_or_else(|| "HEAD".to_string());
    let base = default_base().or_else(|| git_stdout(&["rev-parse", "--abbrev-ref", "@{upstream}"])).unwrap_or_default();
    if base.is_empty() { eprintln!("{} No default branch found (looked for main/master/develop).", error("")); std::process::exit(1); }

    let merge_base = match git_stdout(&["merge-base", &base, "HEAD"]) {
        Some(mb) => mb,
        None => { eprintln!("{} No common history with {}.", error(""), base); std::process::exit(1); }
    };

    println!("{} {}", "◆".style(Theme::ACCENT), "Branch Impact".style(Theme::HEADER));
    println!("{}", divider());
    println!("  {} {}", "Branch:".style(Theme::LABEL), branch.style(Theme::VALUE));
    println!("  {} {}", "Base:  ".style(Theme::LABEL), base.style(Theme::VALUE));

    let numstat = git_stdout(&["diff", "--numstat", &merge_base, "HEAD"]).unwrap_or_default();
    if numstat.trim().is_empty() { println!("\n{} No changes on this branch yet.", warn("")); return; }

    struct FileInfo { name: String, added: u64, deleted: u64 }
    let mut files: Vec<FileInfo> = numstat.lines().filter_map(|l| {
        let mut parts = l.split_whitespace();
        let added = parts.next()?.parse().unwrap_or(0);
        let deleted = parts.next()?.parse().unwrap_or(0);
        let name = parts.collect::<Vec<&str>>().join(" ");
        Some(FileInfo { name, added, deleted })
    }).collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));

    println!(); println!("  {}", "Changed files".style(Theme::HEADER));
    let mut total_score = 0.0;
    let mut total_added = 0u64;
    let mut total_deleted = 0u64;

    for f in &files {
        total_added += f.added; total_deleted += f.deleted;
        let churn = (f.added + f.deleted) as f64;
        let (label, weight) = file_risk(&f.name);
        let mut score = weight + (churn / 500.0).min(5.0);
        if f.name.ends_with(".test.rs") || f.name.ends_with("_test.go") || f.name.starts_with("test/") || f.name.starts_with("tests/") { score *= 0.3; }
        total_score += score;
        let tag = risk_tag(score);
        println!("  {} {} {} {} {}", format!("{:>8}", format!("+{}/-{}", f.added, f.deleted)).dimmed(), tag, label.dimmed(), "·".dimmed(), f.name.style(Theme::VALUE));
    }

    let total_score = total_score.min(100.0).round() as u64;
    println!(); println!("{}", divider());
    println!("  {} {}", "Files changed:".style(Theme::LABEL), format!("{}", files.len()).style(Theme::VALUE));
    println!("  {} {}", "Line churn:".style(Theme::LABEL), format!("+{} -{}", total_added, total_deleted).style(Theme::VALUE));

    let verdict = if total_score < 30 { "LOW" } else if total_score < 60 { "MEDIUM" } else { "HIGH" };
    let verdict_style = match verdict { "LOW" => Theme::SUCCESS, "MEDIUM" => Theme::WARN, _ => Theme::ERROR };
    println!("  {} {}", "Risk score:".style(Theme::LABEL), format!("{}/100", total_score).style(verdict_style).bold());
    println!("  {} {}", "Verdict:".style(Theme::LABEL), verdict.style(verdict_style).bold());
    if total_score >= 60 { println!("  {} Review core files carefully and split into smaller PRs.", warn("")); }
    else if total_score >= 30 { println!("  {} Reasonable scope — worth a second pair of eyes.", "  ".dimmed()); }
    else { println!("  {} Safe to merge.", success("")); }
}

fn file_risk(name: &str) -> (&'static str, f64) {
    let n = name.to_lowercase();
    if n.starts_with(".github/") || n.starts_with(".gitlab/") || n == "dockerfile" || n.contains("docker-compose") || n == "cargo.lock" || n == "package-lock.json" || n == "yarn.lock" || n == "pnpm-lock.yaml" || n == "go.sum" || n == "poetry.lock" { ("build/CI", 18.0) }
    else if n.contains("auth") || n.contains("security") || n.contains("password") || n.contains("migration") || n.contains("database/schema") || n.ends_with(".sql") { ("security/data", 18.0) }
    else if n.ends_with("src/main.rs") || n.ends_with("src/lib.rs") || n == "main.go" || n == "app.py" || n == "manage.py" || n == "index.ts" || n == "index.js" || n == "main.py" { ("entrypoint", 14.0) }
    else if n == "package.json" || n.contains("config") || n.ends_with(".env.example") || n.contains("settings") { ("config", 10.0) }
    else if n.ends_with(".rs") || n.ends_with(".go") || n.ends_with(".py") || n.ends_with(".ts") || n.ends_with(".js") || n.ends_with(".tsx") || n.ends_with(".jsx") || n.ends_with(".java") || n.ends_with(".kt") || n.ends_with(".c") || n.ends_with(".cpp") || n.ends_with(".h") { ("code", 6.0) }
    else { ("other", 3.0) }
}

fn risk_tag(score: f64) -> String {
    if score >= 15.0 { "●".style(Theme::ERROR).to_string() }
    else if score >= 9.0 { "◐".style(Theme::WARN).to_string() }
    else { "○".style(Theme::MUTED).to_string() }
}

fn catchup() {
    if !which("git") || !is_git_repo() { eprintln!("{} Must be inside a git repository.", error("")); std::process::exit(1); }

    let sp = Spinner::new("Fetching latest from origin...");
    let fetch = std::process::Command::new("git").args(["fetch", "origin"])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    if !fetch.map(|s| s.success()).unwrap_or(false) { sp.fail("git fetch failed"); std::process::exit(1); }
    sp.done("Fetched from origin");

    let default = git_stdout(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .or_else(|| default_base()).unwrap_or_default();
    if default.is_empty() { eprintln!("{} Could not determine default branch.", error("")); std::process::exit(1); }

    let merge_base = match git_stdout(&["merge-base", "HEAD", &default]) {
        Some(mb) => mb,
        None => { eprintln!("{} No common history with {}.", error(""), default); std::process::exit(1); }
    };

    let commits = git_stdout(&["log", "--oneline", "--no-merges", &format!("{}..{}", merge_base, default)]).unwrap_or_default();
    let commit_lines: Vec<&str> = commits.lines().filter(|l| !l.is_empty()).collect();

    println!(); println!("{} {}", "◆".style(Theme::ACCENT), "Git Catch-up".style(Theme::HEADER));
    println!("{}", divider());
    println!("  {} {}", "Default branch:".style(Theme::LABEL), default.replace("origin/", "").style(Theme::VALUE));

    if commit_lines.is_empty() { println!("\n{} You're up to date with {}!", success(""), default.replace("origin/", "")); return; }

    println!("  {} You are {} commit(s) behind.", warn(""), commit_lines.len().style(Theme::ACCENT).bold());
    println!("\n  {}", "New commits".style(Theme::HEADER));
    for c in commit_lines.iter().take(30) {
        let (hash, rest) = c.split_once(' ').unwrap_or((c, ""));
        let category = commit_category(rest);
        let styled = match category { "feature" => "✨", "fix" => "🐛", "docs" => "📄", "perf" => "⚡", _ => "  " };
        println!("  {} {} {}{}", styled, (&hash[..7]).style(Theme::MUTED), rest.style(Theme::VALUE), "");
    }
    if commit_lines.len() > 30 { println!("  {} ... and {} more", "  ".dimmed(), (commit_lines.len() - 30).style(Theme::MUTED)); }

    if let Some(diffstat) = git_stdout(&["diff", "--stat", "--color=never", &format!("{}..{}", merge_base, default)]) {
        let lines: Vec<&str> = diffstat.lines().filter(|l| !l.is_empty() && !l.contains("files changed")).collect();
        if !lines.is_empty() {
            println!("\n  {}", "Files changed".style(Theme::HEADER));
            for l in lines.iter().take(15) { println!("  {}", l.style(Theme::MUTED)); }
            if lines.len() > 15 { println!("  {} ... and {} more", "  ".dimmed(), (lines.len() - 15).style(Theme::MUTED)); }
        }
    }

    let doc_commits: Vec<&str> = commit_lines.iter().filter(|c| c.to_lowercase().contains("doc") || c.to_lowercase().contains("readme")).copied().collect();
    if !doc_commits.is_empty() {
        println!("\n  {}", "Docs updated".style(Theme::HEADER));
        for c in doc_commits.iter().take(10) { let (hash, rest) = c.split_once(' ').unwrap_or((c, "")); println!("  📄 {} {}", (&hash[..7]).style(Theme::MUTED), rest.style(Theme::VALUE)); }
    }

    if which("gh") {
        if let Some(date) = git_stdout(&["log", "-1", "--format=%cI", &merge_base]) {
            let search = format!("merged:>={}", date);
            let out = std::process::Command::new("gh").args(["pr", "list", "--state", "merged", "--search", &search, "--limit", "15"])
                .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).output();
            if let Ok(o) = out {
                let text = String::from_utf8_lossy(&o.stdout).to_string();
                let prs: Vec<&str> = text.lines().filter(|l| l.contains('#')).collect();
                if !prs.is_empty() { println!("\n  {}", "Merged PRs".style(Theme::HEADER)); for p in prs.iter().take(10) { println!("  🔀 {}", p.style(Theme::MUTED)); } }
            }
        }
    }

    println!("\n  {} Pull with: {}", "→".style(Theme::ACCENT), format!("git pull origin {}", default.replace("origin/", "")).style(Theme::ACCENT));
}

fn commit_category(msg: &str) -> &'static str {
    let m = msg.to_lowercase();
    if m.starts_with("feat") || m.starts_with("feature") || m.contains("add ") || m.contains("new ") { "feature" }
    else if m.starts_with("fix") || m.starts_with("bug") || m.contains("hotfix") { "fix" }
    else if m.starts_with("doc") || m.starts_with("readme") { "docs" }
    else if m.starts_with("perf") || m.starts_with("optim") || m.starts_with("speed") { "perf" }
    else { "" }
}

fn detect_version() -> Option<String> {
    // Cargo.toml
    if let Ok(content) = std::fs::read_to_string("Cargo.toml") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("version") && trimmed.contains('"') {
                if let Some(v) = trimmed.split('"').nth(1) {
                    return Some(v.to_string());
                }
            }
        }
    }
    // package.json
    if let Ok(content) = std::fs::read_to_string("package.json") {
        if let Some(v) = content.lines().find(|l| l.contains("\"version\"")) {
            if let Some(v) = v.split('"').nth(3) {
                return Some(v.to_string());
            }
        }
    }
    // pyproject.toml
    if let Ok(content) = std::fs::read_to_string("pyproject.toml") {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("version") && trimmed.contains('"') {
                if let Some(v) = trimmed.split('"').nth(1) {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn generate_changelog() -> String {
    let mut out = String::new();
    // Get commits since last tag
    let last_tag = git_stdout(&["describe", "--tags", "--abbrev=0", "HEAD"]);
    let range = match &last_tag {
        Some(tag) => format!("{}..HEAD", tag),
        None => "HEAD".into(),
    };

    let log = git_stdout(&["log", "--format=%h|%s|%an", &range]).unwrap_or_default();
    let mut features = Vec::new();
    let mut fixes = Vec::new();
    let mut other = Vec::new();

    for line in log.lines() {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() < 2 { continue; }
        let (hash, msg) = (parts[0], parts[1]);
        let cat = commit_category(msg);
        let entry = format!("- {} ({})", msg, hash);
        match cat {
            "feature" => features.push(entry),
            "fix" => fixes.push(entry),
            _ => other.push(entry),
        }
    }

    if !features.is_empty() {
        out.push_str("### Features\n");
        out.push_str(&features.join("\n"));
        out.push_str("\n\n");
    }
    if !fixes.is_empty() {
        out.push_str("### Fixes\n");
        out.push_str(&fixes.join("\n"));
        out.push_str("\n\n");
    }
    if !other.is_empty() {
        out.push_str("### Other\n");
        out.push_str(&other.join("\n"));
        out.push_str("\n");
    }
    out
}

fn release(version: Option<&str>, do_push: bool, do_github: bool, notes: Option<&str>) {
    if !which("git") || !is_git_repo() {
        eprintln!("{} Must be inside a git repository.", error(""));
        std::process::exit(1);
    }

    let ver = match version {
        Some(v) => v.to_string(),
        None => match detect_version() {
            Some(v) => {
                println!("  {} Detected version: {}", "→".dimmed(), v.style(Theme::VALUE));
                v
            }
            None => {
                eprintln!("{} No version provided and none detected from Cargo.toml/package.json/pyproject.toml.", error(""));
                std::process::exit(1);
            }
        }
    };

    let tag = if ver.starts_with('v') { ver.clone() } else { format!("v{}", ver) };

    println!("{} {}", "◆".style(Theme::ACCENT), "Create Release".style(Theme::HEADER));
    println!("{}", divider());
    println!("  {} {}", "Tag:".style(Theme::LABEL), tag.style(Theme::ACCENT));

    // Check tag doesn't already exist
    if git_ok(&["rev-parse", &tag]) {
        eprintln!("  {} Tag '{}' already exists.", error(""), tag);
        std::process::exit(1);
    }

    // Check working tree is clean
    let dirty = git_stdout(&["status", "--porcelain"]).map(|s| !s.is_empty()).unwrap_or(false);
    if dirty {
        eprintln!("  {} Working tree is dirty. Commit or stash changes first.", error(""));
        std::process::exit(1);
    }

    // Detect branch
    let branch = git_stdout(&["branch", "--show-current"]).unwrap_or_else(|| "HEAD".to_string());
    println!("  {} {}", "Branch:".style(Theme::LABEL), branch.style(Theme::VALUE));

    // Generate changelog
    let changelog = if notes.is_some() {
        notes.unwrap().to_string()
    } else {
        println!("  {} Generating changelog...", "→".dimmed());
        generate_changelog()
    };

    if !changelog.trim().is_empty() {
        println!("\n  {}", "Release notes:".style(Theme::HEADER));
        for line in changelog.lines().take(20) {
            println!("  {}", line.style(Theme::MUTED));
        }
    }

    // Create annotated tag
    let mut tag_cmd = std::process::Command::new("git");
    tag_cmd.args(["tag", "-a", &tag, "-m", &format!("Release {}", tag)]);

    let sp = Spinner::new(&format!("Creating tag {}...", tag));
    let status = tag_cmd.status();
    match status {
        Ok(s) if s.success() => sp.done(&format!("Tag {} created", tag)),
        _ => { sp.fail("Failed to create tag"); std::process::exit(1); }
    }

    // Push tag
    if do_push {
        let sp = Spinner::new(&format!("Pushing {} to origin...", tag));
        let ok = std::process::Command::new("git")
            .args(["push", "origin", &tag])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            sp.done(&format!("Pushed {} to origin", tag));
        } else {
            sp.fail("Push failed — push manually with: git push origin <tag>");
        }
    }

    // GitHub release
    if do_github {
        if !which("gh") {
            eprintln!("  {} gh CLI not found — skipping GitHub release.", warn(""));
        } else {
            let sp = Spinner::new("Creating GitHub release...");
            let mut cmd = std::process::Command::new("gh");
            cmd.args(["release", "create", &tag, "--title", &tag]);
            if !changelog.trim().is_empty() {
                cmd.args(["--notes", &changelog]);
            } else {
                cmd.args(["--generate-notes"]);
            }
            let ok = cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                sp.done("GitHub release created");
            } else {
                sp.fail("gh release create failed");
            }
        }
    }

    println!();
    println!("{}", divider());
    println!("  {} Release {} complete", success(""), tag.style(Theme::ACCENT).bold());
    if !do_push {
        println!("  {} Push tag with: git push origin {}", "→".dimmed(), tag.style(Theme::ACCENT));
    }
    if !do_github && which("gh") {
        println!("  {} Create GitHub release with: gh release create {}", "→".dimmed(), tag.style(Theme::ACCENT));
    }
}
