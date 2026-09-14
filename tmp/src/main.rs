use chrono::{DateTime, Duration, Local, Utc};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const VERSION: &str = "0.1.0";
const ENV_HOME: &str = "PROTO_TMP_HOME";
const STATE_HOME: &str = "PROTO_TMP_STATE";
const NO_PROMPT: &str = "PROTO_TMP_NO_PROMPT";

const PKG_DEFAULTS: &str = "base python python-pip nodejs npm";

// ─── data ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnvEntry {
    name: String,
    kind: String, // "bwrap" or "rootfs"
    root: String,
    status: String, // saved | stopped | running
    pid: Option<u32>,
    created: i64,
    updated: i64,
    zip: Option<String>,
    zip_at: Option<i64>,
    expires_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct Registry {
    version: u32,
    envs: HashMap<String, EnvEntry>,
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn err(msg: String) -> ! {
    eprintln!("{} {}", "proto tmp:".red().bold(), msg);
    std::process::exit(1);
}

fn now_epoch() -> i64 {
    Utc::now().timestamp()
}

fn epoch_str(s: i64) -> String {
    DateTime::from_timestamp(s, 0)
        .map(|d| d.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

fn random_hash(n: usize) -> String {
    let mut buf = vec![0u8; n];
    fs::File::open("/dev/urandom")
        .and_then(|mut f| io::Read::read_exact(&mut f, &mut buf))
        .ok();
    let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
    buf.iter()
        .map(|&b| alphabet[b as usize % alphabet.len()] as char)
        .collect()
}

fn home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn config_dir() -> PathBuf {
    env::var(ENV_HOME).map(PathBuf::from).unwrap_or_else(|_| {
        env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".config"))
            .join("proto")
            .join("tmp")
    })
}

fn state_dir() -> PathBuf {
    env::var(STATE_HOME).map(PathBuf::from).unwrap_or_else(|_| {
        env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".local").join("state"))
            .join("proto")
            .join("tmp")
    })
}

fn registry_path() -> PathBuf {
    config_dir().join("registry.json")
}

fn env_state_dir(name: &str) -> PathBuf {
    state_dir().join(name)
}

fn env_home_dir(name: &str) -> PathBuf {
    env_state_dir(name).join("home").join("dev")
}

fn env_tmp_dir(name: &str) -> PathBuf {
    env_state_dir(name).join("tmp")
}

fn zip_path(name: &str) -> PathBuf {
    state_dir().join(format!("{}.zip", name))
}

fn current_term() -> String {
    env::var("TERM").unwrap_or_else(|_| "xterm".into())
}

// ─── registry ────────────────────────────────────────────────────────────────

fn load_registry() -> Registry {
    let p = registry_path();
    if !p.exists() {
        let mut r = Registry::default();
        r.version = 1;
        return r;
    }
    let data = fs::read_to_string(&p).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

fn save_registry(reg: &Registry) {
    let dir = config_dir();
    fs::create_dir_all(&dir).ok();
    let data = serde_json::to_string_pretty(reg).unwrap();
    fs::write(registry_path(), data).ok();
}

fn prune_expired(reg: &mut Registry) {
    let now = now_epoch();
    let expired: Vec<String> = reg
        .envs
        .iter()
        .filter(|(_, e)| {
            e.status == "stopped"
                && e.expires_at.map_or(false, |exp| exp <= now)
        })
        .map(|(n, _)| n.clone())
        .collect();
    for name in &expired {
        if let Some(e) = reg.envs.remove(name) {
            let dir = env_state_dir(name);
            fs::remove_dir_all(&dir).ok();
            if let Some(z) = &e.zip {
                fs::remove_file(z).ok();
            }
        }
    }
}

// ─── shell utilities ─────────────────────────────────────────────────────────

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn du_size(path: &Path) -> String {
    Command::new("du")
        .args(["-sh", "--block-size=1"])
        .arg(path)
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.split_whitespace().next().map(|s| s.to_string())
        })
        .unwrap_or_else(|| "?".into())
}

fn fmt_size(bytes: &str) -> String {
    bytes
        .parse::<u64>()
        .ok()
        .map(|b| {
            if b >= 1024 * 1024 * 1024 {
                format!("{:.1}G", b as f64 / 1024.0 / 1024.0 / 1024.0)
            } else if b >= 1024 * 1024 {
                format!("{:.1}M", b as f64 / 1024.0 / 1024.0)
            } else if b >= 1024 {
                format!("{:.1}K", b as f64 / 1024.0)
            } else {
                format!("{}B", b)
            }
        })
        .unwrap_or_else(|| bytes.to_string())
}

// ─── env setup ───────────────────────────────────────────────────────────────

fn ensure_env_dirs(name: &str) {
    let home = env_home_dir(name);
    let tmp = env_tmp_dir(name);
    fs::create_dir_all(&home).ok();
    fs::create_dir_all(&tmp).ok();
}

fn write_profile(name: &str) {
    let home = env_home_dir(name);
    let profile = home.join(".profile");
    let contents = format!(
        "# proto tmp env: {name}\n\
         export LANG=C.UTF-8\n\
         export HOME=/home/dev\n\
         export PATH=\"/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/bin/site_perl:/usr/bin/vendor_perl:/usr/bin/core_perl:$HOME/.local/bin\"\n\
         export PS1='\\[\\033[1;36m\\][proto-tmp:{name}]\\[\\033[0m\\] \\w \\$ '\n\
         cd ~/ 2>/dev/null\n\
         echo 'proto tmp env: type exit to leave and save/discard'\n",
        name = name,
    );
    fs::write(&profile, contents).ok();
}

fn create_new_entry(name: &str, kind: &str) -> EnvEntry {
    ensure_env_dirs(name);
    write_profile(name);
    let ts = now_epoch();
    EnvEntry {
        name: name.to_string(),
        kind: kind.to_string(),
        root: env_state_dir(name).display().to_string(),
        status: "saved".into(),
        pid: None,
        created: ts,
        updated: ts,
        zip: None,
        zip_at: None,
        expires_at: None,
    }
}

// ─── zip / unzip ─────────────────────────────────────────────────────────────

fn zip_env(name: &str) -> Option<PathBuf> {
    let zip = zip_path(name);
    let dir = env_state_dir(name);
    if !dir.exists() {
        return None;
    }
    let parent = dir.parent().map(Path::to_path_buf)?;
    let status = Command::new("zip")
        .current_dir(&parent)
        .args(["-r", "-q"])
        .arg(&zip)
        .arg(name)
        .status();
    match status {
        Ok(s) if s.success() => Some(zip),
        _ => {
            eprintln!("warning: zip failed for env '{name}'");
            None
        }
    }
}

fn unzip_env(name: &str, zip: &Path) -> bool {
    let dir = env_state_dir(name);
    fs::create_dir_all(&dir).ok();
    let status = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(zip)
        .arg("-d")
        .arg(state_dir())
        .status();
    status.map(|s| s.success()).unwrap_or(false)
}

// ─── bwrap commands ──────────────────────────────────────────────────────────

fn bwrap_base_args(name: &str) -> Vec<String> {
    vec![
        "--unshare-all".into(),
        "--uid".into(),
        "1000".into(),
        "--gid".into(),
        "1000".into(),
        "--new-session".into(),
        "--ro-bind".into(),
        "/".into(),
        "/".into(),
        "--bind".into(),
        format!("{}/home", env_state_dir(name).display()),
        "/home".into(),
        "--bind".into(),
        format!("{}/tmp", env_state_dir(name).display()),
        "/tmp".into(),
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/run".into(),
        "--hostname".into(),
        format!("tmp-{}", name),
        "--setenv".into(),
        "HOME".into(),
        "/home/dev".into(),
        "--setenv".into(),
        "TERM".into(),
        current_term(),
        "--setenv".into(),
        "LANG".into(),
        "C.UTF-8".into(),
        "--setenv".into(),
        "PATH".into(),
        "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/bin/site_perl:/usr/bin/vendor_perl:/usr/bin/core_perl:/home/dev/.local/bin"
            .into(),
        "--chdir".into(),
        "/home/dev".into(),
    ]
}

fn bwrap_rootfs_args(name: &str) -> Vec<String> {
    let rootfs = env_state_dir(name).join("rootfs");
    vec![
        "--new-session".into(),
        "--bind".into(),
        format!("{}/", rootfs.display()),
        "/".into(),
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/run".into(),
        "--hostname".into(),
        format!("tmp-{}", name),
        "--setenv".into(),
        "HOME".into(),
        "/root".into(),
        "--setenv".into(),
        "TERM".into(),
        current_term(),
        "--setenv".into(),
        "LANG".into(),
        "C.UTF-8".into(),
        "--setenv".into(),
        "PATH".into(),
        "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/bin/site_perl:/usr/bin/vendor_perl:/usr/bin/core_perl"
            .into(),
        "--chdir".into(),
        "/root".into(),
    ]
}

fn run_bwrap(entry: &EnvEntry) -> i32 {
    let mut args = if entry.kind == "rootfs" {
        bwrap_rootfs_args(&entry.name)
    } else {
        bwrap_base_args(&entry.name)
    };
    args.push("--".into());
    args.push("/bin/bash".into());
    args.push("-l".into());

    let use_sudo = entry.kind == "rootfs" && !is_root();
    let mut cmd = if use_sudo {
        let mut c = Command::new("sudo");
        c.args(["bwrap"]);
        c
    } else {
        Command::new("bwrap")
    };
    cmd.args(&args);
    cmd.status()
        .map(|s| s.code().unwrap_or(127))
        .unwrap_or(127)
}

fn run_bwrap_local(base_dir: &Path) -> i32 {
    let tmp = base_dir.join("tmp");
    let name = base_dir.file_name().unwrap().to_string_lossy();
    let args = vec![
        "--unshare-all".into(),
        "--uid".into(),
        "1000".into(),
        "--gid".into(),
        "1000".into(),
        "--new-session".into(),
        "--ro-bind".into(),
        "/".into(),
        "/".into(),
        "--bind".into(),
        base_dir.join("home").display().to_string(),
        "/home".into(),
        "--bind".into(),
        tmp.display().to_string(),
        "/tmp".into(),
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/run".into(),
        "--hostname".into(),
        format!("proto-{}", name),
        "--setenv".into(),
        "HOME".into(),
        "/home/dev".into(),
        "--setenv".into(),
        "TERM".into(),
        current_term(),
        "--setenv".into(),
        "LANG".into(),
        "C.UTF-8".into(),
        "--setenv".into(),
        "PATH".into(),
        "/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/bin/site_perl:/usr/bin/vendor_perl:/usr/bin/core_perl:/home/dev/.local/bin"
            .into(),
        "--chdir".into(),
        "/home/dev".into(),
        "--".into(),
        "/bin/bash".into(),
        "-l".into(),
    ];
    let status = Command::new("bwrap")
        .args(&args)
        .status()
        .map(|s| s.code().unwrap_or(127))
        .unwrap_or(127);
    status
}

// ─── rootfs creation ─────────────────────────────────────────────────────────

fn create_rootfs(name: &str, packages: &str) {
    let rootfs = env_state_dir(name).join("rootfs");
    fs::create_dir_all(&rootfs).ok();
    let pkgs: Vec<&str> = packages.split_whitespace().collect();
    if which("pacstrap") {
        let status = Command::new("sudo")
            .args(["pacstrap", "-G", "-c"])
            .arg(&rootfs)
            .args(&pkgs)
            .status();
        if !status.map(|s| s.success()).unwrap_or(false) {
            err("pacstrap failed".into());
        }
    } else if which("debootstrap") {
        let mut include = String::from("--include=");
        include.push_str(&packages.replace("base", "systemd"));
        let status = Command::new("sudo")
            .args([
                "debootstrap",
                &include,
                "--variant=minbase",
                "bookworm",
            ])
            .arg(&rootfs)
            .arg("http://deb.debian.org/debian")
            .status();
        if !status.map(|s| s.success()).unwrap_or(false) {
            err("debootstrap failed".into());
        }
    } else {
        err("no pacstrap or debootstrap found — install arch-install-scripts or debootstrap".into());
    }
}

fn is_root() -> bool {
    env::var("EUID")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .map_or(false, |e| e == 0)
}

// ─── prompt ──────────────────────────────────────────────────────────────────

fn prompt_save_discard() -> bool {
    if env::var(NO_PROMPT).map_or(false, |v| v == "1" || v == "true") {
        return true;
    }
    eprint!("save or discard? [s/d] ");
    io::stderr().flush().ok();
    let stdin = io::stdin();
    let line = stdin.lock().lines().next();
    match line {
        Some(Ok(l)) => {
            let l = l.trim().to_lowercase();
            !(l == "d" || l == "discard" || l == "q" || l == "exit" || l == "n" || l == "no")
        }
        _ => true, // default: save
    }
}

// ─── list ────────────────────────────────────────────────────────────────────

fn cmd_list() {
    let mut reg = load_registry();
    prune_expired(&mut reg);
    save_registry(&reg);

    if reg.envs.is_empty() {
        println!(
            "{}",
            "no envs yet — create one with: proto tmp <name>".dimmed()
        );
        return;
    }

    let active = env::var("PROTO_TMP_CURRENT").ok();
    let mut envs: Vec<&EnvEntry> = reg.envs.values().collect();
    envs.sort_by(|a, b| a.name.cmp(&b.name));

    println!(
        "{:<12}  {:<8}  {:<10}  {:<10}  {}",
        "NAME".bold(),
        "STATUS".bold(),
        "SIZE".bold(),
        "CREATED".bold(),
        "KIND".bold(),
    );
    for e in envs {
        let name_s = if active.as_deref() == Some(e.name.as_str()) {
            format!("{}*", e.name).cyan().bold().to_string()
        } else {
            format!("{}", e.name.cyan().bold())
        };
        let status_s = match e.status.as_str() {
            "running" => format!("{}", "running".green()),
            "saved" => format!("{}", "saved".yellow()),
            "stopped" => format!("{}", "stopped".red()),
            other => format!("{}", other.dimmed()),
        };
        let size_s = {
            let dir = env_state_dir(&e.name);
            if dir.exists() {
                let raw = du_size(&dir);
                fmt_size(&raw)
            } else {
                "-".into()
            }
        };
        let kind_s = if e.kind == "rootfs" {
            format!("{}", "rootfs".magenta())
        } else {
            format!("{}", "bwrap".dimmed())
        };
        println!(
            "{:<12}  {:<8}  {:<10}  {:<10}  {}",
            name_s,
            status_s,
            size_s,
            epoch_str(e.created).dimmed(),
            kind_s,
        );
    }
    println!();
    println!(
        "{}",
        "tip: proto tmp <name>  to enter or create".dimmed()
    );
}

// ─── info ────────────────────────────────────────────────────────────────────

fn cmd_info(name: &str) {
    let reg = load_registry();
    let e = reg
        .envs
        .get(name)
        .unwrap_or_else(|| err(format!("env '{}' not found (see 'proto tmp list')", name)));
    println!("{}   {}", "name".dimmed(), e.name.bold());
    println!("{}   {}", "kind".dimmed(), e.kind);
    println!("{}   {}", "status".dimmed(), e.status);
    println!("{}   {}", "root".dimmed(), e.root);
    if let Some(p) = e.pid {
        println!("{}   {}", "pid".dimmed(), p);
    }
    println!(
        "{}   {}",
        "created".dimmed(),
        epoch_str(e.created)
    );
    println!(
        "{}   {}",
        "updated".dimmed(),
        epoch_str(e.updated)
    );
    if let Some(z) = &e.zip {
        println!("{}   {}", "zip".dimmed(), z);
        println!(
            "{}   {}",
            "zip_at".dimmed(),
            e.zip_at.map(epoch_str).unwrap_or_default()
        );
        println!(
            "{}   {}",
            "expires".dimmed(),
            e.expires_at.map(epoch_str).unwrap_or_default()
        );
    }
    let dir = env_state_dir(name);
    if dir.exists() {
        let raw = du_size(&dir);
        println!("{}   {}", "size".dimmed(), fmt_size(&raw));
    }
    println!();
    println!(
        "{}",
        format!("enter: proto tmp {}", e.name).dimmed()
    );
}

// ─── rm ──────────────────────────────────────────────────────────────────────

fn cmd_rm(name: &str, force: bool) {
    let mut reg = load_registry();
    if !reg.envs.contains_key(name) {
        err(format!("env '{}' not found", name));
    }
    if !force {
        eprint!("delete env '{}'? [y/N] ", name);
        io::stderr().flush().ok();
        let mut line = String::new();
        io::stdin().read_line(&mut line).ok();
        if !line.trim().to_lowercase().starts_with('y') {
            return;
        }
    }
    let e = reg.envs.remove(name).unwrap();
    fs::remove_dir_all(env_state_dir(name)).ok();
    if let Some(z) = &e.zip {
        fs::remove_file(z).ok();
    }
    save_registry(&reg);
    println!("{} deleted '{}'", "✓".green().bold(), name.bold());
}

// ─── cleanup ─────────────────────────────────────────────────────────────────

fn cmd_cleanup() {
    let mut reg = load_registry();
    let names: Vec<String> = reg.envs.keys().cloned().collect();
    let mut count = 0;
    for name in &names {
        if let Some(e) = reg.envs.remove(name) {
            fs::remove_dir_all(env_state_dir(name)).ok();
            if let Some(z) = &e.zip {
                fs::remove_file(z).ok();
            }
            count += 1;
        }
    }
    save_registry(&reg);
    if count == 0 {
        println!("{}", "nothing to clean up".dimmed());
    } else {
        println!(
            "{} cleaned up {} env(s)",
            "✓".green().bold(),
            count
        );
    }
}

// ─── snapshot / restore ──────────────────────────────────────────────────────

fn cmd_snapshot(name: &str) {
    let mut reg = load_registry();
    let e = reg
        .envs
        .get_mut(name)
        .unwrap_or_else(|| err(format!("env '{}' not found", name)));
    if e.status == "running" {
        err(format!("env '{}' is running — stop it first", name));
    }
    let dir = env_state_dir(name);
    if e.status == "stopped" {
        println!(
            "{} env '{}' is already stopped (zip kept until {})",
            "✓".green().bold(),
            name.bold(),
            e.expires_at.map(epoch_str).unwrap_or_default()
        );
        return;
    }
    if !dir.exists() {
        err(format!("env '{}' has no data to snapshot", name));
    }
    let zip = zip_env(name);
    if let Some(z) = &zip {
        fs::remove_dir_all(&dir).ok();
        e.status = "stopped".into();
        e.zip = Some(z.display().to_string());
        e.zip_at = Some(now_epoch());
        e.expires_at = Some(now_epoch() + Duration::hours(24).num_seconds());
        e.updated = now_epoch();
        save_registry(&reg);
        println!(
            "{} snapshot '{}' (kept 24h — run 'proto tmp restore {}' to resume)",
            "✓".green().bold(),
            name.bold(),
            name
        );
    } else {
        err("zip failed".into());
    }
}

fn cmd_restore(name: &str) {
    let mut reg = load_registry();
    let e = reg
        .envs
        .get_mut(name)
        .unwrap_or_else(|| err(format!("env '{}' not found", name)));
    if e.status != "stopped" {
        err(format!("env '{}' is not stopped (status: {})", name, e.status));
    }
    let zip = e
        .zip
        .clone()
        .unwrap_or_else(|| err(format!("env '{}' has no zip", name)));
    let zp = PathBuf::from(&zip);
    if !zp.exists() {
        err(format!("zip file missing: {}", zip));
    }
    ensure_env_dirs(name);
    if !unzip_env(name, &zp) {
        err("unzip failed".into());
    }
    fs::remove_file(&zp).ok();
    e.status = "saved".into();
    e.zip = None;
    e.zip_at = None;
    e.expires_at = None;
    e.updated = now_epoch();
    save_registry(&reg);
    println!(
        "{} restored '{}' — enter with: proto tmp {}",
        "✓".green().bold(),
        name.bold(),
        name
    );
}

// ─── enter ───────────────────────────────────────────────────────────────────

fn cmd_enter(name: &str, rootfs: bool, packages: &str, create: bool) {
    let mut reg = load_registry();

    // restore if stopped + zip available
    if let Some(e) = reg.envs.get(name) {
        if e.status == "stopped" {
            if let Some(z) = &e.zip {
                let zp = PathBuf::from(z);
                if zp.exists() {
                    eprintln!("restoring env '{}' from snapshot...", name);
                    ensure_env_dirs(name);
                    if unzip_env(name, &zp) {
                        fs::remove_file(&zp).ok();
                    } else {
                        err("failed to restore snapshot".into());
                    }
                }
            }
        }
    }

    if !reg.envs.contains_key(name) {
        if !create {
            err(format!(
                "env '{}' not found — create it with: proto tmp {}",
                name, name
            ));
        }
        let kind = if rootfs { "rootfs" } else { "bwrap" };
        let entry = create_new_entry(name, kind);
        if rootfs {
            create_rootfs(name, packages);
        }
        reg.envs.insert(name.to_string(), entry);
    } else if reg.envs.get(name).map(|e| e.kind.as_str()) == Some("rootfs") {
        let rootfs_dir = env_state_dir(name).join("rootfs");
        if !rootfs_dir.exists() {
            eprintln!(
                "env '{}' is a rootfs but has no rootfs dir — bootstrapping...",
                name
            );
            create_rootfs(name, packages);
        }
    }

    // ensure dirs
    ensure_env_dirs(name);

    let entry = reg.envs.get(name).cloned().unwrap();
    let dir = env_state_dir(name);

    // mark running
    if let Some(e) = reg.envs.get_mut(name) {
        e.status = "running".into();
        e.pid = Some(std::process::id());
        e.updated = now_epoch();
    }
    save_registry(&reg);

    env::set_var("PROTO_TMP_CURRENT", name);
    let code = run_bwrap(&entry);

    // handle post-exit
    let mut reg = load_registry();
    if let Some(e) = reg.envs.get_mut(name) {
        e.pid = None;
        if code == 0 {
            // normal exit — prompt save/discard
            let save = prompt_save_discard();
            if save {
                e.status = "saved".into();
                e.zip = None;
                e.zip_at = None;
                e.expires_at = None;
                e.updated = now_epoch();
                save_registry(&reg);
                println!("{} env '{}' saved", "✓".green().bold(), name.bold());
            } else {
                let z = e.zip.clone();
                let _ = reg.envs.remove(name);
                fs::remove_dir_all(&dir).ok();
                if let Some(zp) = &z {
                    fs::remove_file(zp).ok();
                }
                save_registry(&reg);
                println!("{} env '{}' discarded", "✓".yellow().bold(), name.bold());
            }
        } else {
            // abnormal exit — zip + mark stopped for 24h
            eprintln!(
                "{} env '{}' exited abnormally (code {}) — zipping for recovery (kept 24h)",
                "!".red().bold(),
                name.bold(),
                code
            );
            let zip = zip_env(name);
            fs::remove_dir_all(&dir).ok();
            e.status = "stopped".into();
            e.zip = zip.as_ref().map(|p| p.display().to_string());
            e.zip_at = Some(now_epoch());
            e.expires_at = Some(now_epoch() + Duration::hours(24).num_seconds());
            e.updated = now_epoch();
            save_registry(&reg);
        }
    }
    env::remove_var("PROTO_TMP_CURRENT");
}

// ─── local (ephemeral) ───────────────────────────────────────────────────────

fn cmd_local() {
    let base = PathBuf::from(format!("/tmp/proto/{}", random_hash(6)));
    let home = base.join("home").join("dev");
    let tmp = base.join("tmp");
    fs::create_dir_all(&home).ok();
    fs::create_dir_all(&tmp).ok();

    // write profile
    let profile = home.join(".profile");
    let contents = format!(
        "# proto tmp --local\n\
         export LANG=C.UTF-8\n\
         export HOME=/home/dev\n\
         export PATH=\"/usr/local/sbin:/usr/local/bin:/usr/bin:/usr/bin/site_perl:/usr/bin/vendor_perl:/usr/bin/core_perl:$HOME/.local/bin\"\n\
         export PS1='\\[\\033[1;33m\\][tmp:{}]\\[\\033[0m\\] \\w \\$ '\n\
         cd /tmp/proto/{} 2>/dev/null || cd ~/ 2>/dev/null || cd /\n",
        base.file_name().unwrap().to_string_lossy(),
        base.file_name().unwrap().to_string_lossy(),
    );
    fs::write(&profile, contents).ok();

    eprintln!(
        "{} ephemeral sandbox at {}",
        "✓".green().bold(),
        base.display().dimmed()
    );

    let code = run_bwrap_local(&base);

    // always discard
    fs::remove_dir_all(&base).ok();
    if code == 0 {
        println!("{} sandbox discarded", "✓".dimmed());
    } else {
        println!("{} sandbox discarded (exit code {})", "✗".dimmed(), code);
    }
}

// ─── help ────────────────────────────────────────────────────────────────────

fn print_help() {
    println!(
        "{}\n{}",
        format!("proto tmp v{} — ephemeral sandboxed environments", VERSION)
            .bold()
            .cyan(),
        "spin up a quick sandbox with isolated home/tmp, all host tools visible"
    );
    println!();
    println!("  USAGE:  proto tmp [flags] [<command>] [<name>] [args]");
    println!();
    println!("  FLAGS");
    println!("    --local               Ephemeral /tmp/proto/<hash6>, auto-discards on exit");
    println!("    --rootfs              Use a real rootfs (pacstrap/debootstrap, requires sudo)");
    println!("    --packages <list>     Packages for rootfs (default: {})", PKG_DEFAULTS);
    println!("    -h, --help            This help");
    println!();
    println!("  COMMANDS");
    println!("    <name>                Enter or create+enter a sandbox (default if no command)");
    println!("    list | ls             List all sandboxes + status");
    println!("    info <name>           Show full details for a sandbox");
    println!("    new <name>            Create without entering");
    println!("    rm <name>             Delete a sandbox (asks confirmation)");
    println!("    rm <name> --force     Delete without confirmation");
    println!("    cleanup               Delete ALL inactive sandboxes");
    println!("    snapshot <name>       Zip + mark stopped (kept 24h)");
    println!("    restore <name>        Restore a stopped snapshot");
    println!("    help                  This help");
    println!();
    println!("  EXIT BEHAVIOUR");
    println!("    Normal exit (exit)    Prompts: save / discard ?");
    println!("    Abnormal exit (kill)  Auto-zips, kept 24h, recover with restore");
    println!();
    println!("  EXAMPLES");
    println!("    proto tmp myproj              # enter (or create+enter) a sandbox");
    println!("    proto tmp --local             # ephemeral sandbox under /tmp/proto");
    println!("    proto tmp new dev --rootfs    # create a real rootfs sandbox");
    println!("    proto tmp snapshot dev        # zip + pause (keep 24h)");
    println!("    proto tmp restore dev         # restore a paused sandbox");
    println!("    proto tmp cleanup             # purge all inactive");
    println!();
    println!(
        "{}",
        "env vars: PROTO_TMP_HOME, PROTO_TMP_STATE, PROTO_TMP_NO_PROMPT".dimmed()
    );
}

// ─── main ────────────────────────────────────────────────────────────────────

struct Flags {
    local: bool,
    rootfs: bool,
    packages: String,
    help: bool,
    force: bool,
}

fn parse_args(args: &[String]) -> (Flags, Vec<String>) {
    let mut flags = Flags {
        local: false,
        rootfs: false,
        packages: PKG_DEFAULTS.into(),
        help: false,
        force: false,
    };
    let mut positionals = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--local" => flags.local = true,
            "--rootfs" => flags.rootfs = true,
            "-h" | "--help" => flags.help = true,
            "--force" => flags.force = true,
            "--packages" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    flags.packages = v.clone();
                }
            }
            _ => positionals.push(args[i].clone()),
        }
        i += 1;
    }
    (flags, positionals)
}

fn main() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let args: Vec<String> = env::args().skip(1).collect();
    let (flags, pos) = parse_args(&args);

    if flags.help {
        print_help();
        return;
    }

    if flags.local {
        cmd_local();
        return;
    }

    let cmd = pos.first().map(|s| s.as_str()).unwrap_or("list");

    match cmd {
        "list" | "ls" => cmd_list(),
        "info" => {
            let name = pos.get(1).unwrap_or_else(|| err("usage: proto tmp info <name>".into()));
            cmd_info(name);
        }
        "rm" | "delete" => {
            let name = pos.get(1).unwrap_or_else(|| err("usage: proto tmp rm <name>".into()));
            cmd_rm(name, flags.force);
        }
        "cleanup" => cmd_cleanup(),
        "snapshot" | "stop" => {
            let name = pos
                .get(1)
                .unwrap_or_else(|| err("usage: proto tmp snapshot <name>".into()));
            cmd_snapshot(name);
        }
        "restore" | "resume" => {
            let name = pos
                .get(1)
                .unwrap_or_else(|| err("usage: proto tmp restore <name>".into()));
            cmd_restore(name);
        }
        "new" | "create" => {
            let name = pos
                .get(1)
                .unwrap_or_else(|| err("usage: proto tmp new <name>".into()));
            if !flags.rootfs {
                let mut reg = load_registry();
                if reg.envs.contains_key(name) {
                    err(format!("env '{}' already exists", name));
                }
                let entry = create_new_entry(name, "bwrap");
                reg.envs.insert(name.to_string(), entry);
                save_registry(&reg);
                println!(
                    "{} created '{}' — enter with: proto tmp {}",
                    "✓".green().bold(),
                    name.bold(),
                    name
                );
            } else {
                // rootfs create — bootstrap first, register only on success
                let mut reg = load_registry();
                if reg.envs.contains_key(name) {
                    err(format!("env '{}' already exists", name));
                }
                create_rootfs(name, &flags.packages);
                let entry = create_new_entry(name, "rootfs");
                reg.envs.insert(name.to_string(), entry);
                save_registry(&reg);
                println!(
                    "{} created rootfs '{}' — enter with: proto tmp {}",
                    "✓".green().bold(),
                    name.bold(),
                    name
                );
            }
        }
        "help" => print_help(),
        _ => {
            // treat as name → enter or create+enter
            cmd_enter(cmd, flags.rootfs, &flags.packages, true);
        }
    }
}
