use clap::Parser;
use proto_plugin_sdk::*;

#[derive(Parser)]
#[command(name = "fmt", about = "Universal formatter — auto-detects cargo fmt/prettier/black/gofmt")]
struct Cli {
    /// Check only — exit 1 if unformatted (useful for CI)
    #[arg(long)]
    check: bool,

    /// Show what would change without modifying files
    #[arg(long)]
    diff: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum ProjectType {
    Cargo,
    Prettier,
    Python,
    Go,
}

fn detect_project() -> Option<ProjectType> {
    let cwd = std::env::current_dir().ok()?;

    if cwd.join("Cargo.toml").exists() {
        return Some(ProjectType::Cargo);
    }
    if cwd.join("package.json").exists() {
        let has_prettier = cwd.join(".prettierrc").exists()
            || cwd.join(".prettierrc.json").exists()
            || cwd.join(".prettierrc.js").exists()
            || cwd.join(".prettierrc.yml").exists()
            || cwd.join("prettier.config.js").exists()
            || cwd.join("prettier.config.mjs").exists()
            || {
                let content = std::fs::read_to_string(cwd.join("package.json")).unwrap_or_default();
                content.contains("prettier")
            };
        if has_prettier {
            return Some(ProjectType::Prettier);
        }
    }
    if cwd.join("pyproject.toml").exists() || cwd.join("setup.cfg").exists() {
        return Some(ProjectType::Python);
    }
    if cwd.join("go.mod").exists() {
        return Some(ProjectType::Go);
    }

    None
}

fn run_fmt(project: &ProjectType, check: bool, diff: bool) -> i32 {
    let spinner = Spinner::new("Formatting...");

    let exit_code = match project {
        ProjectType::Cargo => {
            let mut args = vec![];
            if check {
                args.push("--check");
            }
            spinner.done(&success("Format complete"));
            run_command("cargo", &["fmt"].iter().chain(args.iter()).copied().collect::<Vec<_>>())
        }
        ProjectType::Prettier => {
            let mut args = vec!["--write", "."];
            if check {
                args = vec!["--check", "."];
            } else if diff {
                args = vec!["--list-different", "."];
            }
            spinner.done(&success("Format complete"));
            run_command("npx", &["prettier"].iter().chain(args.iter()).copied().collect::<Vec<_>>())
        }
        ProjectType::Python => {
            let cwd = std::env::current_dir().unwrap();
            let has_ruff = cwd.join("pyproject.toml").exists()
                && {
                    let content = std::fs::read_to_string(cwd.join("pyproject.toml")).unwrap_or_default();
                    content.contains("[tool.ruff]")
                };
            if has_ruff {
                let mut args = vec!["format", "."];
                if check {
                    args = vec!["format", "--check", "."];
                }
                spinner.done(&success("Format complete"));
                run_command("ruff", &args)
            } else {
                let mut args = vec!["."];
                if check {
                    args = vec!["--check", "."];
                }
                spinner.done(&success("Format complete"));
                run_command("black", &args)
            }
        }
        ProjectType::Go => {
            if diff {
                spinner.done(&success("Format check complete"));
                run_command("gofmt", &["-d", "."])
            } else if check {
                spinner.done(&success("Format check complete"));
                let output = std::process::Command::new("gofmt")
                    .args(["-l", "."])
                    .output();
                match output {
                    Ok(o) => {
                        let files = String::from_utf8_lossy(&o.stdout);
                        if !files.trim().is_empty() {
                            eprintln!("{}", error("Files need formatting:"));
                            eprintln!("{}", files);
                            return 1;
                        }
                        return 0;
                    }
                    Err(e) => {
                        eprintln!("{} {}", error("Failed:"), e);
                        return 1;
                    }
                }
            } else {
                spinner.done(&success("Format complete"));
                run_command("gofmt", &["-w", "."])
            }
        }
    };

    match exit_code {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("{} {}", error("Failed to format:"), e);
            1
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let project = match detect_project() {
        Some(p) => p,
        None => {
            eprintln!("{} No supported project type detected.", error("Error"));
            eprintln!("  Searched for: Cargo.toml, package.json + prettier, pyproject.toml/setup.cfg, go.mod");
            std::process::exit(1);
        }
    };

    println!("{}", header(&format!("Formatting ({:?})", project)));

    let exit_code = run_fmt(&project, cli.check, cli.diff);
    std::process::exit(exit_code);
}
