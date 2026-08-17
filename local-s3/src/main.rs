use clap::Parser;
use owo_colors::OwoColorize;
use std::process::{Child, Command, Stdio};
use std::path::PathBuf;

struct Theme;
impl Theme {
    const HEADER: &'static str = "cyan";
    const ACCENT: &'static str = "magenta";
    const SUCCESS: &'static str = "green";
    const ERROR: &'static str = "red";
    const MUTED: &'static str = "dark_grey";
    const WARN: &'static str = "yellow";
    const LABEL: &'static str = "cyan";
    const VALUE: &'static str = "bright_white";
}
fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn warn(s: &str) -> String { format!("{} {}", "⚠".style(Theme::WARN), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(40).dimmed().to_string() }
fn label_value(label: &str, value: &str) -> String {
    format!("{} {}", format!("{:>14}:", label).style(Theme::LABEL), value.style(Theme::VALUE))
}
fn which(binary: &str) -> bool {
    Command::new("which").arg(binary).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

struct Spinner { msg: String }
impl Spinner {
    fn new(msg: &str) -> Self { print!("  ⠋ {}...", msg); std::io::Write::flush(&mut std::io::stdout()).ok(); Self { msg: msg.to_string() } }
    fn update(&self, msg: &str) { print!("\r  ⠋ {}...", msg); std::io::Write::flush(&mut std::io::stdout()).ok(); }
    fn done(&self, msg: &str) { println!("\r  ✔ {}", msg); }
    fn fail(&self, msg: &str) { println!("\r  ✗ {}", msg); }
}

const ROOT_USER: &str = "protouser";
const ROOT_PASSWORD: &str = "protopass123";

#[derive(Parser)]
#[command(name = "local-s3", about = "Local S3 server using MinIO")]
struct Cli;

fn main() {
    println!("{}", header("Local S3 (MinIO)"));
    println!("{}", divider());
    let cwd = match std::env::current_dir() { Ok(d) => d, Err(_) => { eprintln!("{} Cannot determine current directory.", error("")); return; } };
    let data = cwd.join(".proto-s3-data");
    if std::fs::create_dir_all(&data).is_err() {
        eprintln!("{} Cannot create data dir: {}", error(""), data.display());
        return;
    }
    let engine = if which("minio") { "minio" } else if which("docker") { "docker" } else {
        eprintln!("{} No engine found. Install MinIO or Docker:", error(""));
        println!("  {}   Run the MinIO server binary (https://min.io/download)", "minio".style(Theme::ACCENT));
        println!("  {}   Or install Docker, then retry.", "docker".style(Theme::ACCENT));
        return;
    };
    println!("  {}", label_value("Engine", if engine == "minio" { "minio binary" } else { "docker" }));
    println!("  {}", label_value("Data dir", &data.to_string_lossy().to_string()));
    let mut child = match spawn_engine(engine, &data) { Ok(c) => c, Err(e) => { eprintln!("{} Failed to start {}: {}", error(""), engine, e); return; } };
    let sp = Spinner::new("Waiting for MinIO to come online...");
    if !wait_healthy() {
        sp.fail("MinIO did not become healthy on :9000");
        let _ = child.kill(); let _ = child.wait();
        eprintln!("{} Check the engine logs above.", error(""));
        return;
    }
    sp.done("MinIO is healthy");
    println!();
    println!("  {} {}", "API endpoint:".style(Theme::LABEL), "http://127.0.0.1:9000".style(Theme::VALUE));
    println!("  {} {}", "Console:     ".style(Theme::LABEL), "http://127.0.0.1:9001".style(Theme::VALUE));
    println!("  {} {}", "Access key:  ".style(Theme::LABEL), ROOT_USER);
    println!("  {} {}", "Secret key:  ".style(Theme::LABEL), ROOT_PASSWORD);
    println!();
    println!("  {}", "aws cli:".style(Theme::HEADER));
    println!("    {}", format!("AWS_ACCESS_KEY_ID={} AWS_SECRET_ACCESS_KEY={} aws --endpoint-url http://127.0.0.1:9000 s3 mb s3://test", ROOT_USER, ROOT_PASSWORD).style(Theme::MUTED));
    println!("  {}", "mc:".style(Theme::HEADER));
    println!("    {}", format!("mc alias set proto http://127.0.0.1:9000 {} {} && mc mb proto/test", ROOT_USER, ROOT_PASSWORD).style(Theme::MUTED));
    println!();
    println!("  {} Press Ctrl+C to stop the server.", "▶".style(Theme::WARN));
    let _ = child.wait();
    println!("\n{} Local S3 server stopped.", warn(""));
}

fn spawn_engine(engine: &str, data: &PathBuf) -> std::io::Result<Child> {
    if engine == "minio" {
        Command::new("minio").args(["server", data.to_str().unwrap_or(""), "--address", "127.0.0.1:9000", "--console-address", "127.0.0.1:9001"]).env("MINIO_ROOT_USER", ROOT_USER).env("MINIO_ROOT_PASSWORD", ROOT_PASSWORD).stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn()
    } else {
        Command::new("docker").args(["run", "--rm", "--name", "proto-s3", "-p", "127.0.0.1:9000:9000", "-p", "127.0.0.1:9001:9001", "-e", &format!("MINIO_ROOT_USER={}", ROOT_USER), "-e", &format!("MINIO_ROOT_PASSWORD={}", ROOT_PASSWORD), "-v", &format!("{}:/data", data.to_str().unwrap_or("")), "minio/minio", "server", "/data", "--console-address", ":9001"]).stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn()
    }
}

fn wait_healthy() -> bool {
    for _ in 0..20 {
        if ureq::get("http://127.0.0.1:9000/minio/health/live").timeout(std::time::Duration::from_millis(500)).call().is_ok() { return true; }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    false
}
