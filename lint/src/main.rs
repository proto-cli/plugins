use clap::Parser;
use proto_plugin_sdk::*;

#[derive(Parser)]
#[command(name = "lint", about = "Universal linter — auto-detects clippy/eslint/ruff/golangci-lint")]
struct Cli {
    /// Auto-fix issues where possible
    #[arg(long)]
    fix: bool,

    /// Output as JSON for IDE integration
    #[arg(long)]
    format: bool,
}

#[derive(Debug, Clone, PartialEq)]
enum ProjectType {
    Cargo,
    Eslint,
    Python,
    Go,
}

fn detect_project() -> Option<ProjectType> {
    let cwd = std::env::current_dir().ok()?;

    if cwd.join("Cargo.toml").exists() {
        return Some(ProjectType::Cargo);
    }
    if cwd.join("package.json").exists() {
        let has_eslint = cwd.join(".eslintrc").exists()
            || cwd.join(".eslintrc.js").exists()
            || cwd.join(".eslintrc.json").exists()
            || cwd.join(".eslintrc.yml").exists()
            || cwd.join("eslint.config.js").exists()
            || cwd.join("eslint.config.mjs").exists()
            || cwd.join("eslint.config.cjs").exists();
        if has_eslint {
            return Some(ProjectType::Eslint);
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

fn run_lint(project: &ProjectType, fix: bool, json_output: bool) -> i32 {
    let spinner = Spinner::new("Linting...");

    let exit_code = match project {
        ProjectType::Cargo => {
            let mut args = vec!["clippy", "--", "-D", "warnings"];
            if json_output {
                args = vec!["clippy", "--message-format=json", "--", "-D", "warnings"];
            }
            spinner.done(&success("Lint complete"));
            run_command("cargo", &args)
        }
        ProjectType::Eslint => {
            let mut args = vec!["."];
            if fix {
                args.insert(0, "--fix");
            }
            if json_output {
                args.insert(0, "--format=json");
            }
            spinner.done(&success("Lint complete"));
            run_command("eslint", &args)
        }
        ProjectType::Python => {
            let cwd = std::env::current_dir().unwrap();
            let has_ruff = cwd.join("pyproject.toml").exists()
                && {
                    let content = std::fs::read_to_string(cwd.join("pyproject.toml")).unwrap_or_default();
                    content.contains("[tool.ruff]")
                };
            if has_ruff {
                let mut args = vec!["check", "."];
                if fix {
                    args.insert(0, "--fix");
                }
                if json_output {
                    args.insert(0, "--output-format=json");
                }
                spinner.done(&success("Lint complete"));
                run_command("ruff", &args)
            } else {
                let mut args = vec!["."];
                if fix {
                    args = vec!["--fix", "."];
                }
                spinner.done(&success("Lint complete"));
                run_command("flake8", &args)
            }
        }
        ProjectType::Go => {
            let mut args = vec!["run"];
            if fix {
                args.push("--fix");
            }
            spinner.done(&success("Lint complete"));
            run_command("golangci-lint", &args)
        }
    };

    match exit_code {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("{} {}", error("Failed to run linter:"), e);
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
            eprintln!("  Searched for: Cargo.toml, package.json + eslint config, pyproject.toml/setup.cfg, go.mod");
            std::process::exit(1);
        }
    };

    println!("{}", header(&format!("Linting ({:?})", project)));

    let exit_code = run_lint(&project, cli.fix, cli.format);
    std::process::exit(exit_code);
}
