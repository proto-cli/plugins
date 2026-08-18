use clap::Parser;
use proto_plugin_sdk::*;

#[derive(Parser)]
#[command(name = "test", about = "Universal test runner — auto-detects cargo/npm/go/pytest")]
struct Cli {
    /// Watch mode — rerun on file changes
    #[arg(short = 'w', long)]
    watch: bool,

    /// Run with coverage
    #[arg(long)]
    coverage: bool,

    /// Filter test names by pattern
    #[arg(short, long)]
    filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum ProjectType {
    Cargo,
    Npm,
    Yarn,
    Pnpm,
    Go,
    Python,
    Makefile,
}

fn detect_project() -> Option<ProjectType> {
    let cwd = std::env::current_dir().ok()?;

    if cwd.join("Cargo.toml").exists() {
        return Some(ProjectType::Cargo);
    }
    if cwd.join("package.json").exists() {
        if cwd.join("pnpm-lock.yaml").exists() {
            return Some(ProjectType::Pnpm);
        } else if cwd.join("yarn.lock").exists() {
            return Some(ProjectType::Yarn);
        } else {
            return Some(ProjectType::Npm);
        }
    }
    if cwd.join("go.mod").exists() {
        return Some(ProjectType::Go);
    }
    if cwd.join("requirements.txt").exists() || cwd.join("pyproject.toml").exists() {
        return Some(ProjectType::Python);
    }
    if cwd.join("Makefile").exists() {
        let makefile = std::fs::read_to_string(cwd.join("Makefile")).ok()?;
        if makefile.contains("test:") {
            return Some(ProjectType::Makefile);
        }
    }

    None
}

fn run_tests(project: &ProjectType, filter: &Option<String>, coverage: bool) -> i32 {
    let spinner = Spinner::new("Running tests...");
    let status = match project {
        ProjectType::Cargo => {
            let mut args = vec!["test"];
            if let Some(f) = filter {
                args.push(f);
            }
            spinner.done(&success("Tests complete"));
            run_command("cargo", &args)
        }
        ProjectType::Npm | ProjectType::Yarn | ProjectType::Pnpm => {
            let pm = match project {
                ProjectType::Pnpm => "pnpm",
                ProjectType::Yarn => "yarn",
                _ => "npm",
            };
            let mut args = vec!["test"];
            if let Some(f) = filter {
                args.push("--");
                args.push(f);
            }
            spinner.done(&success("Tests complete"));
            run_command(pm, &args)
        }
        ProjectType::Go => {
            let mut args = vec!["test", "./..."];
            if let Some(f) = filter {
                args.push("-run");
                args.push(f);
            }
            spinner.done(&success("Tests complete"));
            run_command("go", &args)
        }
        ProjectType::Python => {
            let mut args = vec!["-m", "pytest"];
            if let Some(f) = filter {
                args.push("-k");
                args.push(f);
            }
            if coverage {
                args.push("--cov=.");
            }
            spinner.done(&success("Tests complete"));
            run_command("python", &args)
        }
        ProjectType::Makefile => {
            spinner.done(&success("Tests complete"));
            run_command("make", &["test"])
        }
    };

    match status {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("{} {}", error("Failed to run tests:"), e);
            1
        }
    }
}

fn run_watch(project: &ProjectType, filter: &Option<String>, _coverage: bool) -> i32 {
    match project {
        ProjectType::Cargo => {
            println!("{}", header("Watch mode (cargo watch)"));
            let mut args = vec!["watch", "-x", "\"cargo test"];
            if let Some(f) = filter {
                args.push(f);
            }
            args.push("\"");
            let status = run_command("cargo", &args);
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("{} {}", error("cargo watch not installed. Install with:"), muted("cargo install cargo-watch"));
                    eprintln!("  {}", error(&e.to_string()));
                    1
                }
            }
        }
        ProjectType::Npm | ProjectType::Yarn | ProjectType::Pnpm => {
            println!("{}", header("Watch mode (nodemon)"));
            let mut args = vec!["--watch", ".", "--ext", "js,ts,jsx,tsx"];
            args.push("--exec");
            let mut cmd = "npm test".to_string();
            if let Some(f) = filter {
                cmd = format!("npm test -- --testNamePattern='{}'", f);
            }
            args.push(&cmd);
            let status = run_command("nodemon", &args);
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("{} {}", error("nodemon not installed. Install with:"), muted("npm install -g nodemon"));
                    eprintln!("  {}", error(&e.to_string()));
                    1
                }
            }
        }
        _ => {
            println!("{}", header("Watch mode (watchexec)"));
            let mut args = vec!["-w", ".", "-r", "500"];
            args.push("--");
            let cmd = match project {
                ProjectType::Go => "go test ./...",
                ProjectType::Python => "python -m pytest",
                ProjectType::Makefile => "make test",
                _ => unreachable!(),
            };
            args.push(cmd);
            let status = run_command("watchexec", &args);
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("{} {}", error("watchexec not installed. Install with:"), muted("cargo install watchexec-cli"));
                    eprintln!("  {}", error(&e.to_string()));
                    1
                }
            }
        }
    }
}

fn run_coverage(project: &ProjectType, filter: &Option<String>) -> i32 {
    match project {
        ProjectType::Cargo => {
            println!("{}", header("Coverage (cargo-tarpaulin)"));
            let mut args = vec!["tarpaulin"];
            if let Some(f) = filter {
                args.push("--");
                args.push(f);
            }
            let status = run_command("cargo", &args);
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("{} {}", error("cargo-tarpaulin not installed. Install with:"), muted("cargo install cargo-tarpaulin"));
                    eprintln!("  {}", error(&e.to_string()));
                    1
                }
            }
        }
        ProjectType::Npm | ProjectType::Yarn | ProjectType::Pnpm => {
            println!("{}", header("Coverage (c8)"));
            let mut args = vec!["nyc", "report", "--reporter=text"];
            if let Some(f) = filter {
                args.push("--");
                args.push(f);
            }
            let status = run_command("npx", &["c8", "--reporter=text", "npm", "test"]);
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("{} {}", error("c8 not available. Install with:"), muted("npm install --save-dev c8"));
                    eprintln!("  {}", error(&e.to_string()));
                    1
                }
            }
        }
        ProjectType::Go => {
            println!("{}", header("Coverage (go test -cover)"));
            let mut args = vec!["test", "-cover", "./..."];
            if let Some(f) = filter {
                args.push("-run");
                args.push(f);
            }
            let status = run_command("go", &args);
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("{} {}", error("Failed:"), e);
                    1
                }
            }
        }
        ProjectType::Python => {
            println!("{}", header("Coverage (pytest-cov)"));
            let mut args = vec!["-m", "pytest", "--cov=."];
            if let Some(f) = filter {
                args.push("-k");
                args.push(f);
            }
            let status = run_command("python", &args);
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("{} {}", error("pytest-cov not installed. Install with:"), muted("pip install pytest-cov"));
                    eprintln!("  {}", error(&e.to_string()));
                    1
                }
            }
        }
        ProjectType::Makefile => {
            println!("{}", warn("Coverage not supported for Makefile projects, running make test instead"));
            run_tests(project, filter, false)
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let project = match detect_project() {
        Some(p) => p,
        None => {
            eprintln!("{} No supported project type detected.", error("Error"));
            eprintln!("  Searched for: Cargo.toml, package.json, go.mod, requirements.txt, pyproject.toml, Makefile");
            std::process::exit(1);
        }
    };

    println!("{}", header(&format!("Running tests ({:?})", project)));

    let exit_code = if cli.coverage {
        run_coverage(&project, &cli.filter)
    } else if cli.watch {
        run_watch(&project, &cli.filter, false)
    } else {
        run_tests(&project, &cli.filter, false)
    };

    std::process::exit(exit_code);
}
