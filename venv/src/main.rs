use owo_colors::{OwoColorize, Style as C};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ---------------------------------------------------------------- models --

#[derive(Serialize, Deserialize, Clone)]
struct Project {
    name: String,
    root: String,
    python: String,
    venv_dir: String,
    created: String,
    updated: String,
}

impl Project {
    fn venv_path(&self) -> PathBuf {
        Path::new(&self.root).join(&self.venv_dir)
    }
    fn bin(&self, exe: &str) -> PathBuf {
        self.venv_path().join("bin").join(exe)
    }
    fn site_packages(&self) -> Option<PathBuf> {
        let lib = self.venv_path().join("lib");
        let entry = fs::read_dir(&lib).ok()?.find_map(|e| {
            let e = e.ok()?;
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("python") && e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                Some(e.path())
            } else {
                None
            }
        })?;
        Some(entry.join("site-packages"))
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Registry {
    version: u32,
    projects: HashMap<String, Project>,
}

#[derive(Serialize, Deserialize, Default)]
struct State {
    last: Option<String>,
}

// ---------------------------------------------------------------- io ----

fn err(msg: String) -> ! {
    eprintln!("{}", format!("proto venv: {}", msg).red());
    exit(1);
}

fn registry_dir() -> PathBuf {
    if let Ok(d) = env::var("PROTO_VENV_HOME") {
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    if let Ok(d) = env::var("XDG_CONFIG_HOME") {
        if !d.is_empty() {
            return PathBuf::from(d).join("proto").join("venv");
        }
    }
    env::var("HOME")
        .map(|h| PathBuf::from(h).join(".config").join("proto").join("venv"))
        .unwrap_or_else(|_| PathBuf::from(".proto-venv"))
}

fn registry_path() -> PathBuf {
    registry_dir().join("projects.json")
}

fn state_path() -> PathBuf {
    registry_dir().join("state.json")
}

fn load_registry() -> Registry {
    let p = registry_path();
    match fs::read_to_string(&p) {
        Ok(txt) => serde_json::from_str(&txt).unwrap_or_default(),
        Err(_) => Registry::default(),
    }
}

fn save_registry(r: &Registry) {
    let dir = registry_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        err(format!("cannot create {}: {}", dir.display(), e));
    }
    let pretty = serde_json::to_string_pretty(r).unwrap_or_else(|_| "{}".into());
    if let Err(e) = fs::write(registry_path(), pretty) {
        err(format!("cannot write {}: {}", registry_path().display(), e));
    }
}

fn load_state() -> State {
    match fs::read_to_string(&state_path()) {
        Ok(txt) => serde_json::from_str(&txt).unwrap_or_default(),
        Err(_) => State::default(),
    }
}

fn save_state(s: &State) {
    let _ = fs::create_dir_all(&registry_dir());
    if let Ok(pretty) = serde_json::to_string_pretty(s) {
        let _ = fs::write(state_path(), pretty);
    }
}

fn set_last(name: &str) {
    let mut s = load_state();
    s.last = Some(name.to_string());
    save_state(&s);
}

fn now_str() -> String {
    Command::new("date")
        .args(["+%Y-%m-%d %H:%M"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            )
        })
}

fn current_shell() -> String {
    env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
        .unwrap_or_else(|| "bash".into())
}

// --------------------------------------------------------------- args ---

struct Flags {
    values: HashMap<String, String>,
    pos: Vec<String>,
    passthrough: Vec<String>,
}

fn parse_flags(args: &[String], flags: &[&str], booleans: &[&str]) -> Flags {
    let mut values = HashMap::new();
    let mut pos: Vec<String> = Vec::new();
    let mut pass: Vec<String> = Vec::new();
    let mut i = 0;
    let double_dash_at = args.iter().position(|a| a == "--");
    while i < args.len() {
        if Some(i) == double_dash_at {
            pass = args[i + 1..].to_vec();
            break;
        }
        let a = &args[i];
        if a.starts_with("--") {
            if booleans.contains(&a.as_str()) {
                values.insert(a.clone(), "1".into());
                i += 1;
            } else if flags.contains(&a.as_str()) {
                if let Some(v) = args.get(i + 1) {
                    values.insert(a.clone(), v.clone());
                    i += 2;
                } else {
                    err(format!("missing value for {}", a));
                }
            } else {
                err(format!("unknown flag {}", a));
            }
        } else {
            pos.push(a.clone());
            i += 1;
        }
    }
    Flags { values, pos, passthrough: pass }
}

fn need<'a>(values: &'a HashMap<String, String>, key: &str) -> Option<&'a String> {
    values.get(key).filter(|v| !v.is_empty())
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && !n.contains(['/', '\\', ' ', '\t', '\n'])
        && n != "." 
        && n != ".."
}

// ------------------------------------------------------------ util ----

fn run_out(cmd: &str, args: &[&str]) -> (i32, String, String) {
    match Command::new(cmd).args(args).output() {
        Ok(o) => (
            o.status.code().unwrap_or(1),
            String::from_utf8_lossy(&o.stdout).to_string(),
            String::from_utf8_lossy(&o.stderr).to_string(),
        ),
        Err(e) => (127, String::new(), format!("cannot spawn {}: {}", cmd, e)),
    }
}

fn spawn_out(cmd: &mut Command) -> i32 {
    match cmd.status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            err(format!("cannot run command: {}", e));
        }
    }
}

fn abs(p: &str) -> PathBuf {
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(pb)
    }
}

fn find_python(spec: Option<&String>) -> String {
    let spec = spec.map(|s| s.as_str()).unwrap_or("");
    if spec.is_empty() {
        for cand in ["python3", "python"] {
            if run_out(cand, &[]).0 == 0 {
                return cand.to_string();
            }
        }
        err("cannot find a python interpreter; pass --python <path>".into());
    }
    if spec.contains('/') {
        return spec.to_string();
    }
    if spec.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let base = "python";
        return format!("{}{}", base, spec);
    }
    spec.to_string()
}

fn resolve_project(reg: &Registry, name: &str) -> Project {
    reg.projects
        .get(name)
        .cloned()
        .unwrap_or_else(|| err(format!("unknown venv '{}' (see 'proto venv list')", name)))
}

fn check_venv(p: &Project) -> bool {
    p.bin("python").exists() && p.venv_path().exists()
}

// ----------------------------------------------------------- enter ----

fn enter_snippet(p: &Project, fish: bool) -> String {
    let root = p.root.clone();
    let venv = p.venv_path().display().to_string();
    let name = p.name.clone();
    let py = p.bin("python").display().to_string();
    let pip = p.bin("pip").display().to_string();
    let py3 = p.bin("python3").display().to_string();
    if fish {
        format!(
            "# proto venv enter: {name}\n\
             cd '{root}'\n\
             source '{venv}/bin/activate.fish'\n\
             alias pip '{pip}'\n\
             alias python '{py}'\n\
             alias python3 '{py3}'\n\
             set -gx PROTO_VENV '{name}'\n\
             set -gx PROTO_VENV_ROOT '{root}'\n\
             set -gx PROTO_VENV_ENTERED 'yes'\n"
        )
    } else {
        format!(
            "# proto venv enter: {name}\n\
             cd '{root}'\n\
             source '{venv}/bin/activate'\n\
             alias pip='{pip}'\n\
             alias python='{py}'\n\
             alias python3='{py3}'\n\
             export PROTO_VENV='{name}'\n\
             export PROTO_VENV_ROOT='{root}'\n\
             export PROTO_VENV_ENTERED='yes'\n"
        )
    }
}

fn cmd_enter(reg: &Registry, name: &str, print_only: bool) -> i32 {
    let p = resolve_project(reg, name);
    if !check_venv(&p) {
        err(format!(
            "venv '{}' has no valid environment at {} — run 'proto venv create {}'",
            name,
            p.venv_path().display(),
            name
        ));
    }
    set_last(name);
    if print_only {
        let fish = current_shell().contains("fish");
        print!("{}", enter_snippet(&p, fish));
        return 0;
    }
    spawn_enter(&p)
}

fn enter_bash_rc(p: &Project) -> PathBuf {
    let dir = registry_dir();
    fs::create_dir_all(&dir).ok();
    let rc = dir.join(format!("rc-{}.sh", p.name));
    let src = format!(
        "# proto venv enter: {name} (bash)\n\
         [ -f \"$HOME/.bashrc\" ] && source \"$HOME/.bashrc\"\n\
         cd '{root}'\n\
         source '{venv}/bin/activate'\n\
         export PROTO_VENV='{name}'\n\
         export PROTO_VENV_ROOT='{root}'\n\
         export PROTO_VENV_ENTERED=yes\n",
        name = p.name,
        root = p.root,
        venv = p.venv_path().display()
    );
    fs::write(&rc, src).ok();
    rc
}

fn enter_zsh_zdot(p: &Project) -> PathBuf {
    let dir = registry_dir().join("zdot").join(&p.name);
    fs::create_dir_all(&dir).ok();
    let zshrc = dir.join(".zshrc");
    let src = format!(
        "# proto venv enter: {name} (zsh)\n\
         [ -f \"$HOME/.zshrc\" ] && source \"$HOME/.zshrc\"\n\
         cd '{root}'\n\
         source '{venv}/bin/activate'\n\
         export PROTO_VENV='{name}'\n\
         export PROTO_VENV_ROOT='{root}'\n\
         export PROTO_VENV_ENTERED=yes\n"
        ,
        name = p.name,
        root = p.root,
        venv = p.venv_path().display()
    );
    fs::write(&zshrc, src).ok();
    dir
}

fn spawn_enter(p: &Project) -> i32 {
    let shell = current_shell();
    let root = p.root.clone();
    let venv = p.venv_path().display().to_string();
    let name = p.name.clone();
    let envs: Vec<(String, String)> = vec![
        ("PROTO_VENV".into(), name.clone()),
        ("PROTO_VENV_ROOT".into(), root.clone()),
        ("PROTO_VENV_ENTERED".into(), "yes".into()),
    ];
    let mut c = Command::new(&shell);
    c.envs(envs).current_dir(&root);

    if shell.contains("fish") {
        c.arg("-C").arg(format!(
            "cd '{root}'; source '{venv}/bin/activate.fish'; set -gx PROTO_VENV '{name}'; set -gx PROTO_VENV_ROOT '{root}'; set -gx PROTO_VENV_ENTERED yes",
            root = root, venv = venv, name = name
        ));
    } else if shell.contains("bash") {
        let rc = enter_bash_rc(p);
        c.arg("--rcfile").arg(&rc);
    } else if shell.contains("zsh") {
        let zdot = enter_zsh_zdot(p);
        c.env("ZDOTDIR", zdot);
    } else {
        // sh/dash/ash — best effort: source activate in a fresh interactive shell
        let rc = enter_bash_rc(p).display().to_string();
        if let Some(rest) = shell.rsplit('/').next() {
            if let Ok(code) = Command::new(rest).arg("-i").arg("-c").arg(format!(
                "[ -f '{}' ] && . '{}'; exec {}",
                rc, rc, shell
            )).status() {
                return code.code().unwrap_or(1);
            }
        }
        return 1;
    }
    spawn_out(&mut c)
}

// ------------------------------------------------------------ main ----

fn print_help() {
    println!(
        "{}\n{}",
        format!("proto venv v{} — Python virtualenv manager", VERSION).bold().cyan(),
        "manage per-project venvs: create, enter, resume, list, modify, delete & more"
    );
    println!();
    println!("  USAGE:  proto venv <command> [options]");
    println!();
    println!("  PROJECTS");
    println!("    create <name> [--dir <path>] [--python <ver|path>] [--venv-dir <name>] [--force]");
    println!("      Make a venv named <name> rooted at <dir> (default: current folder).");
    println!("    enter <name>          Enter an interactive shell inside the venv at the");
    println!("                            project root.  --print shows an eval snippet instead.");
    println!("    resume                Enter the last venv you entered.");
    println!("    shell <name>          Same as enter (alias).");
    println!("    list [--json]         List projects (alias: ls).");
    println!("    info <name>           Show details for one project.");
    println!("    modify <name> [--python <p>] [--dir <d>] [--rename <new>]");
    println!("    delete <name> [--purge]   Remove the venv; --purge removes the whole folder.");
    println!();
    println!("  PYTHON / PACKAGES");
    println!("    python <name>              Print the venv's python path.");
    println!("    pip <name> [-- args...]    Run the venv pip.");
    println!("    run <name> -- <cmd...>     Run a command with the venv on PATH.");
    println!("    install <name> <pkg>...    pip install into the venv.");
    println!("    uninstall <name> <pkg>...  pip uninstall -y from the venv.");
    println!("    freeze <name>              Export requirements.txt into the project root.");
    println!("    upgrade <name> [--pip]     Upgrade pip (default) or all packages (--all).");
    println!();
    println!("  HOUSEKEEPING");
    println!("    doctor [name]   Check registered venvs for problems.");
    println!("    status          Show current / last / total state.");
    println!("    help            This help.");
    println!();
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");

    match sub {
        "" | "help" | "--help" | "-h" => {
            print_help();
            return;
        }

        "create" | "new" => {
            let f = parse_flags(&args[1..], &["--dir", "--python", "--venv-dir"], &["--force"]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv create <name> [--dir <path>] [--python <ver|path>]".into())
            });
            if !valid_name(&name) {
                err(format!("invalid project name '{}'", name));
            }
            let dir = need(&f.values, "--dir").cloned().unwrap_or_else(|| ".".into());
            let root = abs(&dir).canonicalize().unwrap_or_else(|_| abs(&dir));
            if !root.is_dir() {
                if let Err(e) = fs::create_dir_all(&root) {
                    err(format!("cannot create folder {}: {}", root.display(), e));
                }
            }
            let venv_name = need(&f.values, "--venv-dir").cloned().unwrap_or_else(|| ".venv".into());
            let venv_path = root.join(&venv_name);
            if venv_path.exists() {
                if f.values.contains_key("--force") {
                    fs::remove_dir_all(&venv_path).ok();
                } else {
                    err(format!(
                        "{} already exists at {} — use --force to rebuild",
                        venv_name,
                        venv_path.display()
                    ));
                }
            }
            let py = find_python(f.values.get("--python"));
            let vs = venv_path.to_string_lossy().to_string();
            let (code, _o, e) = run_out(&py, &["-m", "venv", &vs]);
            if code != 0 {
                err(format!("python -m venv failed ({}):\n{}", py, e.trim()));
            }
            let ts = now_str();
            let p = Project {
                name: name.clone(),
                root: root.display().to_string(),
                python: py,
                venv_dir: venv_name,
                created: ts.clone(),
                updated: ts,
            };
            let mut reg = load_registry();
            if reg.projects.contains_key(&name) {
                err(format!("project '{}' already registered — delete first or pick a new name", name));
            }
            reg.projects.insert(name.clone(), p.clone());
            reg.version = 1;
            save_registry(&reg);
            set_last(&name);
            println!(
                "{} created venv '{}' at {} (python {})",
                "✓".green().bold(),
                name.bold(),
                p.venv_path().display(),
                p.python
            );
            println!();
            println!(
                "{} enter it with:\n    proto venv enter {}",
                "→".cyan(),
                name
            );
        }

        "enter" => {
            let f = parse_flags(&args[1..], &[], &["--print"]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv enter <name>".into())
            });
            let reg = load_registry();
            std::process::exit(cmd_enter(&reg, &name, f.values.contains_key("--print")));
        }

        "resume" => {
            let f = parse_flags(&args[1..], &[], &["--print"]);
            let state = load_state();
            let name = state.last.unwrap_or_else(|| {
                err("no project to resume — enter one first with 'proto venv enter <name>'".into())
            });
            let reg = load_registry();
            std::process::exit(cmd_enter(&reg, &name, f.values.contains_key("--print")));
        }

        "shell" => {
            let f = parse_flags(&args[1..], &[], &[]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv shell <name>".into())
            });
            let reg = load_registry();
            std::process::exit(cmd_enter(&reg, &name, false));
        }

        "list" | "ls" => {
            let json = args.iter().any(|a| a == "--json");
            let reg = load_registry();
            if json {
                println!("{}", serde_json::to_string_pretty(&reg.projects).unwrap_or_else(|_| "{}".into()));
                return;
            }
            if reg.projects.is_empty() {
                println!("{}", "no venvs yet — try: proto venv create myproj --dir ./myproj".dimmed());
                return;
            }
            let current = env::var("PROTO_VENV").ok();
            let w_name = reg.projects.keys().map(|k| k.chars().count()).max().unwrap_or(4).max(4);
            let mut rows: Vec<(String, String, String, usize)> = Vec::new();
            for p in reg.projects.values() {
                let pkgs = p
                    .site_packages()
                    .and_then(|sp| fs::read_dir(sp).ok())
                    .map(|rd| rd.count())
                    .unwrap_or(0);
                rows.push((p.name.clone(), p.root.clone(), p.created.clone(), pkgs));
            }
            rows.sort_by(|a, b| a.0.cmp(&b.0));
            println!(
                "{:<w$}  {:<6}  {:<5}  {}",
                "NAME".bold(),
                "PKGS",
                "UP?",
                "ROOT".bold(),
                w = w_name
            );
            for (name, root, created, pkgs) in rows {
                let active = current.as_deref() == Some(name.as_str());
                let n = if active { name.style(C::new().bold().green()) } else { name.style(C::new().cyan()) };
                let healthy = if check_venv(reg.projects.get(&name).unwrap()) { format!("{}", "✓".green()) } else { format!("{}", "✗".red()) };
                println!(
                    "{:<w$}  {:<6}  {:<5}  {}  ({})",
                    n,
                    pkgs,
                    healthy,
                    root.dimmed(),
                    created.dimmed(),
                    w = w_name
                );
            }
        }

        "info" => {
            let f = parse_flags(&args[1..], &[], &["--json"]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv info <name>".into())
            });
            let reg = load_registry();
            let p = resolve_project(&reg, &name);
            if f.values.contains_key("--json") {
                println!("{}", serde_json::to_string_pretty(&p).unwrap_or_else(|_| "{}".into()));
                return;
            }
            let pkgs = p
                .site_packages()
                .and_then(|sp| fs::read_dir(sp).ok())
                .map(|rd| rd.count())
                .unwrap_or(0);
            println!("name    : {}", name.bold().cyan());
            println!("root    : {}", p.root);
            println!("venv    : {}", p.venv_path().display());
            println!("python  : {} ({})", p.python, if p.python.contains('/') || run_out(&p.python, &["--version"]).0 == 0 { run_out(&p.python, &["--version"]).1.trim().to_string() } else { "?".into() });
            println!("pip     : {}", p.bin("pip").display());
            println!("packages: {}", pkgs);
            println!("created : {}", p.created);
            println!("updated : {}", p.updated);
            println!("healthy : {}", if check_venv(&p) { format!("{}", "yes".green()) } else { format!("{}", "no".red()) });
            println!();
            println!("enter   : proto venv enter {}", name);
        }

        "modify" | "update" => {
            let f = parse_flags(
                &args[1..],
                &["--python", "--dir", "--rename"],
                &["--force"],
            );
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv modify <name> [--python <p>] [--dir <d>] [--rename <new>]".into())
            });
            let mut reg = load_registry();
            let mut p = resolve_project(&reg, &name);

            if f.values.is_empty() {
                println!("nothing to modify — pass --python, --dir or --rename");
                return;
            }

            if let Some(newdir) = need(&f.values, "--dir") {
                let newroot = abs(newdir).canonicalize().unwrap_or_else(|_| abs(newdir));
                let force = f.values.contains_key("--force");
                rebuild_venv(&mut p, None, &newroot, force);
                p.root = newroot.display().to_string();
            }

            if let Some(newpy) = need(&f.values, "--python") {
                let python = find_python(Some(newpy));
                let newroot = PathBuf::from(&p.root);
                rebuild_venv(&mut p, Some(&python), &newroot, f.values.contains_key("--force"));
                p.python = python;
            }

            let mut final_name = name.clone();
            if let Some(newty) = need(&f.values, "--rename") {
                if !valid_name(newty) {
                    err(format!("invalid project name '{}'", newty));
                }
                if reg.projects.contains_key(newty) {
                    err(format!("project '{}' already exists", newty));
                }
                reg.projects.remove(&name);
                p.name = newty.clone();
                final_name = newty.clone();
            }

            p.updated = now_str();
            reg.projects.insert(final_name.clone(), p.clone());
            save_registry(&reg);
            let mut s = load_state();
            if s.last.as_deref() == Some(name.as_str()) {
                s.last = Some(final_name.clone());
                save_state(&s);
            }
            println!("{} modified '{}'", "✓".green().bold(), final_name.bold());
        }

        "delete" | "rm" | "remove" => {
            let f = parse_flags(&args[1..], &[], &["--purge"]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv delete <name> [--purge]".into())
            });
            let mut reg = load_registry();
            let p = resolve_project(&reg, &name);
            if f.values.contains_key("--purge") {
                if p.root == "/" || p.root.is_empty() {
                    err("refusing to purge filesystem root".into());
                }
                fs::remove_dir_all(&p.root).unwrap_or_else(|e| {
                    eprintln!("warning: could not remove {}: {}", p.root, e);
                });
            } else {
                fs::remove_dir_all(&p.venv_path()).unwrap_or_else(|e| {
                    eprintln!("warning: could not remove venv: {}", e);
                });
            }
            reg.projects.remove(&name);
            save_registry(&reg);
            let mut s = load_state();
            if s.last.as_deref() == Some(name.as_str()) {
                s.last = None;
                save_state(&s);
            }
            println!("{} deleted '{}'{}", "✓".green().bold(), name.bold(), if f.values.contains_key("--purge") { " (with folder)" } else { "" });
        }

        "python" => {
            let f = parse_flags(&args[1..], &[], &[]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv python <name>".into())
            });
            let reg = load_registry();
            let p = resolve_project(&reg, &name);
            println!("{}", p.bin("python").display());
        }

        "pip" => {
            let f = parse_flags(&args[1..], &[], &[]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv pip <name> [-- args...]".into())
            });
            let reg = load_registry();
            let p = resolve_project(&reg, &name);
            let pip = p.bin("pip");
            let mut c = Command::new(&pip);
            c.args(&f.passthrough).current_dir(&p.root);
            std::process::exit(spawn_out(&mut c));
        }

        "run" => {
            let f = parse_flags(&args[1..], &[], &[]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv run <name> -- <cmd> [args...]".into())
            });
            let reg = load_registry();
            let p = resolve_project(&reg, &name);
            let cmd = f.passthrough.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv run <name> -- <cmd> [args...]".into())
            });
            let bin = p.bin("").display().to_string();
            let mut c = Command::new(&cmd);
            c.args(&f.passthrough[1..])
                .current_dir(&p.root)
                .env("PATH", format!("{}:{}", bin, env::var("PATH").unwrap_or_default()))
                .env("PROTO_VENV", name);
            std::process::exit(spawn_out(&mut c));
        }

        "install" | "add" => {
            let f = parse_flags(&args[1..], &[], &["--no-deps"]);
            if f.pos.len() < 2 {
                err("usage: proto venv install <name> <pkg...> [--no-deps]".into());
            }
            let name = f.pos[0].clone();
            let pkgs = f.pos[1..].to_vec();
            let reg = load_registry();
            let p = resolve_project(&reg, &name);
            let mut c = Command::new(p.bin("pip"));
            c.arg("install");
            if f.values.contains_key("--no-deps") {
                c.arg("--no-deps");
            }
            c.args(&pkgs).current_dir(&p.root);
            let code = spawn_out(&mut c);
            if code == 0 {
                let mut reg = reg;
                if let Some(proj) = reg.projects.get_mut(&name) {
                    proj.updated = now_str();
                    save_registry(&reg);
                }
            }
            std::process::exit(code);
        }

        "uninstall" => {
            let f = parse_flags(&args[1..], &[], &["--no-deps"]);
            if f.pos.len() < 2 {
                err("usage: proto venv uninstall <name> <pkg...>".into());
            }
            let name = f.pos[0].clone();
            let pkgs = f.pos[1..].to_vec();
            let reg = load_registry();
            let p = resolve_project(&reg, &name);
            let mut c = Command::new(p.bin("pip"));
            c.args(["uninstall", "-y"]).args(&pkgs).current_dir(&p.root);
            std::process::exit(spawn_out(&mut c));
        }

        "freeze" | "requirements" => {
            let f = parse_flags(&args[1..], &[], &[]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv freeze <name>".into())
            });
            let reg = load_registry();
            let p = resolve_project(&reg, &name);
            let pip = p.bin("pip");
            let out = match Command::new(&pip).arg("freeze").current_dir(&p.root).output() {
                Ok(o) => o,
                Err(e) => err(format!("cannot run pip: {}", e)),
            };
            if !out.status.success() {
                err(format!("pip freeze failed:\n{}", String::from_utf8_lossy(&out.stderr)));
            }
            let req_path = Path::new(&p.root).join("requirements.txt");
            fs::write(&req_path, &out.stdout).unwrap_or_else(|e| {
                err(format!("cannot write {}: {}", req_path.display(), e));
            });
            println!(
                "{} froze {} packages to {}",
                "✓".green().bold(),
                String::from_utf8_lossy(&out.stdout).lines().count(),
                req_path.display().cyan()
            );
        }

        "upgrade" => {
            let f = parse_flags(&args[1..], &[], &["--all", "--pip"]);
            let name = f.pos.first().cloned().unwrap_or_else(|| {
                err("usage: proto venv upgrade <name> [--pip|--all]".into())
            });
            let reg = load_registry();
            let p = resolve_project(&reg, &name);
            let mut c = Command::new(p.bin("python"));
            c.arg("-m").arg("pip").arg("install").arg("--upgrade");
            if f.values.contains_key("--all") {
                let freeze = Command::new(p.bin("pip"))
                    .arg("freeze")
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                    .unwrap_or_default();
                let tmp = registry_dir().join("upgrade-freeze.txt");
                let _ = fs::write(&tmp, freeze);
                c.arg("-r").arg(&tmp);
            } else {
                c.arg("pip");
            }
            c.current_dir(&p.root);
            let code = spawn_out(&mut c);
            if code == 0 {
                let mut reg = reg;
                if let Some(proj) = reg.projects.get_mut(&name) {
                    proj.updated = now_str();
                    save_registry(&reg);
                }
            }
            std::process::exit(code);
        }

        "doctor" => {
            let reg = load_registry();
            if reg.projects.is_empty() {
                println!("no registered venvs.");
                return;
            }
            let mut problems = 0usize;
            for (name, p) in &reg.projects {
                let v = p.venv_path();
                if !v.is_dir() {
                    println!("{} {}: venv dir missing at {}", "✗".red().bold(), name, v.display());
                    problems += 1;
                }
                if !p.bin("python").exists() {
                    println!("{} {}: python binary missing at {}", "✗".red().bold(), name, p.bin("python").display());
                    problems += 1;
                }
                if !p.bin("pip").exists() {
                    println!("{} {}: pip binary missing", "✗".red().bold(), name);
                    problems += 1;
                }
                if check_venv(p) {
                    let rd = p.site_packages().and_then(|sp| fs::read_dir(sp).ok()).map(|rd| rd.count()).unwrap_or(0);
                    println!(
                        "{} {} ({} packages, python {})",
                        "✓".green(),
                        name.bold(),
                        rd,
                        p.python
                    );
                }
            }
            println!();
            if problems == 0 {
                println!("{} no problems", "✓".green().bold());
            } else {
                println!(
                    "{} {} problem(s) — fix with 'proto venv modify <name> --dir <ok-path>' or re-create",
                    "!".yellow().bold(),
                    problems
                );
            }
        }

        "status" => {
            let reg = load_registry();
            let state = load_state();
            println!("projects: {}", reg.projects.len());
            match env::var("PROTO_VENV") {
                Ok(n) => match reg.projects.get(&n) {
                    Some(p) => println!("active  : {}  at {}  ({})", n.bold().green(), p.root.cyan(), p.venv_path().display()),
                    None => println!("active  : {} (not in registry)", n),
                },
                Err(_) => println!("active  : {}", "none (run 'proto venv enter <name>')".dimmed()),
            }
            match &state.last {
                Some(l) => println!("last    : {}", l.bold()),
                None => println!("last    : {}", "none".dimmed()),
            }
        }

        other => {
            err(format!(
                "unknown command '{}' — run 'proto venv help'",
                other
            ));
        }
    }
}

fn rebuild_venv(p: &mut Project, python: Option<&str>, newroot: &Path, force: bool) {
    let old = p.venv_path();
    if old.is_dir() && !force {
        err(format!(
            "{} exists — pass --force to rebuild it",
            old.display()
        ));
    }
    let py = python.unwrap_or(&p.python).to_string();
    let target = newroot.join(&p.venv_dir);
    if target.is_dir() {
        fs::remove_dir_all(&target).unwrap_or_else(|e| {
            eprintln!("warning: could not remove {}: {}", target.display(), e);
        });
    }
    let vs = target.to_string_lossy().to_string();
    let (code, _o, e) = run_out(&py, &["-m", "venv", &vs]);
    if code != 0 {
        err(format!("python -m venv failed ({}):\n{}", py, e.trim()));
    }
}