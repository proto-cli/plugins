use clap::{Parser, Subcommand};
use proto_plugin_sdk::*;

#[derive(Parser)]
#[command(name = "pr", about = "PR utilities")]
struct Cli {
    #[command(subcommand)]
    action: PrAction,
}

#[derive(Subcommand, Debug, Clone)]
enum PrAction {
    #[command(about = "Run tests, linter, formatter, scan for debug statements, then open the PR page")]
    Prep {
        #[arg(long, help = "Skip running the test suite")]
        skip_tests: bool,
        #[arg(long, help = "Skip running the linter")]
        skip_lint: bool,
        #[arg(long, help = "Skip auto-formatting")]
        skip_fmt: bool,
        #[arg(long, help = "Don't open the PR page in a browser")]
        no_open: bool,
    },
    #[command(about = "Check out a GitHub/GitLab pull request locally into a temp branch")]
    Checkout {
        #[arg(required = true, value_name = "PR#|URL", help = "PR number, e.g. 42, or URL like https://github.com/owner/repo/pull/42")]
        target: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match &cli.action {
        PrAction::Prep { skip_tests, skip_lint, skip_fmt, no_open } => prep(*skip_tests, *skip_lint, *skip_fmt, *no_open),
        PrAction::Checkout { target } => checkout(target),
    }
}

fn is_git_repo() -> bool {
    std::process::Command::new("git").args(["rev-parse", "--git-dir"])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

fn has_file(name: &str) -> bool { std::path::Path::new(name).exists() }

fn npm_has_script(script: &str) -> bool {
    if let Ok(content) = std::fs::read_to_string("package.json") {
        let pattern = format!("\"{}\"", script);
        let in_scripts = content.find("\"scripts\"").is_some();
        return in_scripts && content.contains(&pattern);
    }
    false
}

struct ProjectType { lang: &'static str, test_cmd: Option<&'static [&'static str]>, lint_cmd: Option<&'static [&'static str]>, fmt_cmd: Option<&'static [&'static str]> }

fn detect_project() -> ProjectType {
    if has_file("Cargo.toml") {
        ProjectType { lang: "Rust", test_cmd: Some(&["cargo", "test"]), lint_cmd: Some(&["cargo", "clippy", "--all-targets", "--", "-D", "warnings"]), fmt_cmd: Some(&["cargo", "fmt"]) }
    } else if has_file("go.mod") {
        ProjectType { lang: "Go", test_cmd: Some(&["go", "test", "./..."]), lint_cmd: Some(&["go", "vet", "./..."]), fmt_cmd: Some(&["gofmt", "-w", "."]) }
    } else if has_file("package.json") {
        ProjectType { lang: "Node", test_cmd: if npm_has_script("test") { Some(&["npm", "test"]) } else { None }, lint_cmd: if npm_has_script("lint") { Some(&["npm", "run", "lint"]) } else { None }, fmt_cmd: if npm_has_script("format") { Some(&["npm", "run", "format"]) } else { None } }
    } else if has_file("pyproject.toml") || has_file("requirements.txt") {
        let (lint, fmt) = if which("ruff") { (Some(&["ruff", "check", "."] as &[&str]), Some(&["ruff", "format", "."] as &[&str])) } else if which("black") { (None, Some(&["black", "."] as &[&str])) } else if which("flake8") { (Some(&["flake8", "."] as &[&str]), None) } else { (None, None) };
        ProjectType { lang: "Python", test_cmd: if which("pytest") { Some(&["pytest"]) } else { None }, lint_cmd: lint, fmt_cmd: fmt }
    } else {
        ProjectType { lang: "Unknown", test_cmd: None, lint_cmd: None, fmt_cmd: None }
    }
}

fn run_step(spinner_msg: &str, cmd: Option<&[&str]>, skip: bool) -> Result<bool, String> {
    if skip { println!("  {} skipped", "→".dimmed()); return Ok(false); }
    match cmd {
        None => { println!("  {} no command detected", "→".dimmed()); Ok(false) }
        Some(args) => {
            let sp = Spinner::new(spinner_msg);
            let status = std::process::Command::new(args[0]).args(&args[1..]).stdout(std::process::Stdio::inherit()).stderr(std::process::Stdio::inherit())
                .status().map_err(|e| format!("Failed to run {}: {}", args[0], e))?;
            if status.success() { sp.done(spinner_msg); Ok(true) } else { sp.fail(&format!("{} exited with {}", args[0], status.code().unwrap_or(-1))); Ok(false) }
        }
    }
}

fn scan_debug_statements() -> Vec<String> {
    let output = run_command_output("git", &["diff", "--name-only", "HEAD"]).unwrap_or_default();
    let mut hits = Vec::new();
    for file in output.lines().filter(|l| !l.is_empty()) {
        if !std::path::Path::new(file).is_file() { continue; }
        let ext = file.rsplit('.').next().unwrap_or("");
        if ["png", "jpg", "jpeg", "gif", "zip", "ico", "woff", "woff2", "ttf", "lock", "min.js"].contains(&ext) { continue; }
        let patterns = debug_patterns(ext);
        if patterns.is_empty() { continue; }
        let content = std::fs::read_to_string(file).unwrap_or_default();
        for (idx, line) in content.lines().enumerate() { if patterns.iter().any(|p| line.contains(p)) { hits.push(format!("{}:{}", file, idx + 1)); } }
    }
    hits
}

fn debug_patterns(ext: &str) -> Vec<&str> {
    match ext { "js" | "jsx" | "ts" | "tsx" | "vue" | "svelte" => vec!["console.log", "console.error", "console.warn", "console.debug", "debugger"], "rs" => vec!["dbg!", "todo!", "unimplemented!"], "py" => vec!["print(", "pprint", "breakpoint()"], _ => Vec::new() }
}

fn remote_url() -> Option<String> {
    run_command_output("git", &["config", "--get", "remote.origin.url"]).ok().filter(|s| !s.is_empty())
}

fn pr_page_url() -> Option<String> {
    let url = remote_url()?;
    let url = url.strip_suffix(".git").unwrap_or(&url);
    let url = url.trim_start_matches("git@").replace(':', "/");
    let (host, rest) = url.split_once('/')?;
    let branch = run_command_output("git", &["branch", "--show-current"]).ok().unwrap_or_default();
    Some(format!("https://{}/{}/pull/new/{}", host, rest, branch))
}

fn open_browser(url: &str) {
    for opener in &["xdg-open", "open"] {
        if which(opener) { let _ = std::process::Command::new(opener).arg(url).status(); return; }
    }
    println!("  {}", url.style(Theme::ACCENT));
}

fn prep(skip_tests: bool, skip_lint: bool, skip_fmt: bool, no_open: bool) {
    if !which("git") || !is_git_repo() { eprintln!("{} Must be inside a git repository.", error("")); std::process::exit(1); }
    println!("{} {}", "◆".style(Theme::ACCENT), "PR Prep".style(Theme::HEADER));
    println!("{}", divider());
    let proj = detect_project();
    println!("  {} {}", "Project:".style(Theme::LABEL), proj.lang.style(Theme::VALUE));
    println!();
    if !skip_fmt { let _ = run_step("Formatting code...", proj.fmt_cmd, skip_fmt); } else { println!("  {} formatter", "→".dimmed()); }
    if !skip_lint { let _ = run_step("Linting...", proj.lint_cmd, skip_lint); } else { println!("  {} linter", "→".dimmed()); }
    if !skip_tests { let _ = run_step("Running tests...", proj.test_cmd, skip_tests); } else { println!("  {} tests", "→".dimmed()); }
    let sp = Spinner::new("Scanning for debug statements...");
    let hits = scan_debug_statements();
    if hits.is_empty() { sp.done("No debug statements found"); } else { sp.fail(&format!("{} debug statement(s) found", hits.len())); for h in &hits { println!("  {}", h.style(Theme::WARN)); } }
    println!(); println!("{}", divider());
    if hits.is_empty() { println!("{}", success("Ready to open a PR!")); } else { println!("{} Clean up the statements above first.", warn("")); }
    if no_open { return; }
    if let Some(url) = pr_page_url() { println!("{}", label_value("PR page", &url)); open_browser(&url); } else { println!("{} No origin remote — can't build a PR link.", warn("")); }
}

struct ParsedTarget { host: String, owner: String, repo: String, number: String, is_gitlab: bool, local: bool }

fn parse_target(target: &str) -> Option<ParsedTarget> {
    if let Ok(n) = target.parse::<u32>() {
        let origin = remote_url()?;
        let origin = origin.strip_suffix(".git").unwrap_or(&origin);
        let origin = origin.trim_start_matches("git@").replace(':', "/");
        let origin = origin.trim_start_matches("file://").trim_start_matches("https://").trim_start_matches("http://");
        let is_local = origin.starts_with('/') || origin.starts_with("./") || origin.starts_with("..");
        let (host, rest) = origin.split_once('/').unwrap_or((origin, ""));
        let mut parts = rest.split('/');
        let owner = parts.next().unwrap_or("").to_string();
        let repo = parts.next().unwrap_or("").to_string();
        return Some(ParsedTarget { host: host.to_string(), owner, repo, number: n.to_string(), is_gitlab: host.contains("gitlab"), local: is_local });
    }
    let url = target.trim();
    let is_gitlab = url.contains("gitlab");
    let (owner, repo, number) = if let Some(idx) = url.find("/-/merge_requests/") {
        let base = &url[..idx];
        let mut parts = base.trim_start_matches("https://").trim_start_matches("http://").split('/');
        let _host = parts.next()?;
        let owner = parts.next()?;
        let repo = parts.next()?.to_string();
        let num = url[idx + "/-/merge_requests/".len()..].split(['/', '?']).next()?;
        (owner.to_string(), repo, num.to_string())
    } else if let Some(idx) = url.find("/pull/") {
        let base = &url[..idx];
        let mut parts = base.trim_start_matches("https://").trim_start_matches("http://").split('/');
        let _host = parts.next()?;
        let owner = parts.next()?;
        let repo = parts.next()?.to_string();
        let num = url[idx + "/pull/".len()..].split(['/', '?']).next()?;
        (owner.to_string(), repo, num.to_string())
    } else if let Some(idx) = url.find('#') {
        let base = &url[..idx];
        let mut parts = base.split('/');
        let owner = parts.next()?;
        let repo = parts.next()?.to_string();
        let num = url[idx + 1..].to_string();
        (owner.to_string(), repo, num.to_string())
    } else { return None; };
    Some(ParsedTarget { host: "github.com".to_string(), owner, repo, number, is_gitlab, local: false })
}

fn checkout(target: &str) {
    let parsed = match parse_target(target) { Some(p) => p, None => { eprintln!("{} Could not parse '{}'. Use a PR number, owner/repo#N, or a GitHub/GitLab PR URL.", error(""), target); std::process::exit(1); } };
    println!("{} {}", "◆".style(Theme::ACCENT), "PR Checkout".style(Theme::HEADER));
    println!("{}", divider());
    let in_repo = is_git_repo();
    let branch_name = format!("pr-{}", parsed.number);
    if !in_repo {
        if !which("gh") { eprintln!("{} Not in a git repo and no 'gh' CLI to clone with.", error("")); println!("  Run inside a repo, or install gh: {}", "sudo pacman -S github-cli".dimmed()); std::process::exit(1); }
        let sp = Spinner::new(&format!("Cloning {}/{}...", parsed.owner, parsed.repo));
        let status = std::process::Command::new("gh").args(["repo", "clone", &format!("{}/{}", parsed.owner, parsed.repo)]).stdout(std::process::Stdio::inherit()).stderr(std::process::Stdio::inherit()).status();
        if !status.map(|s| s.success()).unwrap_or(false) { eprintln!("{} Clone failed.", error("")); std::process::exit(1); }
        sp.done(&format!("Cloned {}/{}", parsed.owner, parsed.repo));
    }
    let sp = Spinner::new(&format!("Fetching PR #{}...", parsed.number));
    let same_repo = !parsed.local && remote_url().map(|u| u.contains(&format!("{}/{}", parsed.owner, parsed.repo))).unwrap_or(false);
    let fetch = if parsed.is_gitlab {
        std::process::Command::new("git").args(["fetch", "origin", &format!("merge-requests/{}/head:{}", parsed.number, branch_name)]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
    } else if parsed.local || same_repo {
        std::process::Command::new("git").args(["fetch", "origin", &format!("pull/{}/head:{}", parsed.number, branch_name)]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
    } else if which("gh") {
        sp.update("Using gh to fetch PR...");
        std::process::Command::new("gh").args(["pr", "checkout", &parsed.number, "--branch", &branch_name]).stdout(std::process::Stdio::inherit()).stderr(std::process::Stdio::inherit()).status()
    } else {
        std::process::Command::new("git").args(["fetch", "origin", &format!("pull/{}/head:{}", parsed.number, branch_name)]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
    };
    if !fetch.map(|s| s.success()).unwrap_or(false) {
        sp.fail("PR fetch failed");
        if parsed.is_gitlab { eprintln!("  GitLab: ensure the PR exists (merge-requests/{}/head ref).", parsed.number); }
        else if !parsed.local && !same_repo && !which("gh") { eprintln!("  PR #{}, which isn't from this repo's remote — install gh to fetch cross-repo PRs.", parsed.number); }
        else if !parsed.local && !same_repo { eprintln!("  PR #{}, which isn't from this repo's remote — gh couldn't fetch it.", parsed.number); }
        else { eprintln!("  The ref refs/pull/{}/head doesn't exist on this repo's remote.", parsed.number); }
        std::process::exit(1);
    }
    let checkout = std::process::Command::new("git").args(["checkout", &branch_name]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    if !checkout.map(|s| s.success()).unwrap_or(false) { sp.fail("Checkout failed"); std::process::exit(1); }
    sp.done(&format!("On branch {}", branch_name));
    println!();
    if parsed.local { println!("{} PR #{} checked out from the local remote.", success(""), parsed.number.style(Theme::ACCENT)); println!("{}", label_value("Ref", &format!("refs/pull/{}/head", parsed.number))); }
    else { println!("{} PR #{} of {}/{}", success(""), parsed.number.style(Theme::ACCENT), parsed.owner, parsed.repo); let pr_url = format!("https://{}/{}/{}/{}/{}", parsed.host, parsed.owner, parsed.repo, if parsed.is_gitlab { "merge_requests" } else { "pull" }, parsed.number); println!("{}", label_value("URL", &pr_url)); }
}
