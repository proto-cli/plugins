use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(text: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), text.style(Theme::HEADER))
}
fn success(msg: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), msg)
}
fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}
fn warn(msg: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), msg)
}
fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}
fn divider() -> String {
    "─".repeat(40).dimmed().to_string()
}
fn label_value(label: &str, value: &str) -> String {
    format!(
        "{} {}",
        format!("{:>14}:", label).style(Theme::LABEL),
        value.style(Theme::VALUE)
    )
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

fn run_command_output(program: &str, args: &[&str]) -> std::io::Result<String> {
    let output = std::process::Command::new(program).args(args).output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
        self.spinner.finish_with_message(
            format!("{} {}", "✗".style(Theme::ERROR), msg.style(Theme::ERROR)),
        );
    }
}

#[derive(Parser)]
#[command(name = "share-session", about = "Terminal session sharing")]
struct Cli {
    #[command(subcommand)]
    action: ShareAction,
}

#[derive(Subcommand, Debug, Clone)]
enum ShareAction {
    #[command(about = "Create a shareable terminal session")]
    Create {
        #[arg(long, value_name = "BACKEND", help = "Force backend: sshx, tmate, tmux, vnc")]
        backend: Option<String>,
    },
    #[command(about = "Join an existing shared session")]
    Join {
        #[arg(required = true, value_name = "LINK")]
        link: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.action {
        ShareAction::Create { ref backend } => create(backend.as_deref()),
        ShareAction::Join { ref link } => join(link),
    }
}

fn create(force_backend: Option<&str>) {
    println!(
        "{} {}",
        "◆".style(Theme::ACCENT),
        "Share Session".style(Theme::HEADER)
    );
    println!("{}", divider());

    let backend = force_backend.unwrap_or("auto");

    match backend {
        "sshx" => share_with_sshx(),
        "tmate" => share_with_tmate(true),
        "tmux" => share_with_tmux(),
        "vnc" => share_with_vnc(),
        _ => {
            if which("sshx") {
                share_with_sshx();
            } else if which("tmate") {
                share_with_tmate(false);
            } else if which("tmux") {
                share_with_tmux();
            } else {
                eprintln!("\n{} No sharing backend found.", error(""));
                println!(
                    "\n{}",
                    "Install one of:".style(Theme::HEADER)
                );
                println!(
                    "  {} {}",
                    "▸".style(Theme::ACCENT),
                    "sshx  — web link, viewer opens in browser"
                );
                println!("    {}", "cargo install sshx".dimmed());
                println!(
                    "  {} {}",
                    "▸".style(Theme::ACCENT),
                    "tmate — SSH + web link, needs tmate.io relay"
                );
                println!(
                    "    {}",
                    "sudo pacman -S tmate  /  sudo apt install tmate".dimmed()
                );
                println!(
                    "  {} {}",
                    "▸".style(Theme::ACCENT),
                    "vnc   — full desktop, noVNC in browser + ngrok"
                );
                println!(
                    "    {}",
                    "sudo pacman -S x11vnc wayvnc ngrok".dimmed()
                );
                println!("    {}", "pip install websockify".dimmed());
                println!(
                    "  {} {}",
                    "▸".style(Theme::ACCENT),
                    "tmux  — local only, teammate must SSH in"
                );
                println!(
                    "    {}",
                    "sudo pacman -S tmux  /  sudo apt install tmux".dimmed()
                );
                println!(
                    "\n{} {}",
                    "Or force a backend:",
                    "share-session create --backend tmux".dimmed()
                );
            }
        }
    }
}

fn share_with_sshx() {
    if !which("sshx") {
        eprintln!(
            "{} sshx not installed. Run: {}",
            error(""),
            "cargo install sshx".style(Theme::ACCENT)
        );
        return;
    }
    println!(
        "\n{} Starting sshx session...",
        "  ".dimmed()
    );
    println!(
        "{} Share the link below. Viewer opens in their browser.\n",
        "  ".dimmed()
    );
    println!("{}", divider());
    let status = std::process::Command::new("sshx").status();
    match status {
        Ok(s) if s.success() => println!("\n{} Session ended.", success("")),
        _ => eprintln!("\n{} sshx session terminated.", warn("")),
    }
}

fn share_with_tmate(forced: bool) {
    let socket = format!("/tmp/proto-tmate-{}", std::process::id());
    let sp = Spinner::new("Starting tmate session...");
    let sock_path = std::path::Path::new(&socket);
    if sock_path.exists() {
        let _ = std::fs::remove_file(sock_path);
    }

    let start = std::process::Command::new("tmate")
        .args(["-S", &socket, "new-session", "-d"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if start.map(|s| !s.success()).unwrap_or(true) {
        sp.fail("Failed to start tmate session.");
        fallback_tmux();
        return;
    }

    sp.update("Waiting for tmate relay...");
    let ready = std::process::Command::new("timeout")
        .args(["30", "tmate", "-S", &socket, "wait", "tmate-ready"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if ready.map(|s| !s.success()).unwrap_or(true) {
        sp.fail("tmate relay unreachable.");
        let _ = std::process::Command::new("tmate")
            .args(["-S", &socket, "kill-session"])
            .status();
        if !forced {
            fallback_tmux();
        }
        return;
    }

    let ssh = run_command_output("tmate", &["-S", &socket, "display", "-p", "#{tmate_ssh}"])
        .unwrap_or_default();
    let web = run_command_output("tmate", &["-S", &socket, "display", "-p", "#{tmate_web}"])
        .unwrap_or_default();

    sp.done("Session ready");
    println!();
    println!("{}", "Share Links".style(Theme::HEADER));
    println!("{}", divider());
    println!("{}", label_value("SSH", &ssh));
    if !web.is_empty() && web != ssh {
        println!("{}", label_value("Web", &web));
    }
    println!("{}", divider());
    println!("\n{} Ctrl+B D to detach.\n", "  ".dimmed());
    println!("{}", divider());

    let _ = std::process::Command::new("tmate")
        .args(["-S", &socket, "attach"])
        .status();
    let _ = std::process::Command::new("tmate")
        .args(["-S", &socket, "kill-session"])
        .status();
    println!("\n{} Session ended.", success(""));
}

fn fallback_tmux() {
    if which("tmux") {
        share_with_tmux();
    } else {
        println!(
            "{} Install tmux for local sharing.",
            warn("")
        );
    }
}

fn share_with_tmux() {
    if !which("tmux") {
        eprintln!("{} tmux not installed.", error(""));
        return;
    }
    let sp = Spinner::new("Setting up tmux...");
    if std::env::var("TMUX").is_ok() {
        let session = run_command_output("tmux", &["display-message", "-p", "#S"])
            .unwrap_or_else(|_| "proto".into());
        sp.done("Using current session");
        let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
        let host = run_command_output("hostname", &[]).unwrap_or_else(|_| "localhost".into());
        println!();
        println!(
            "{}",
            label_value("Attach", &format!("tmux attach -t {}", session))
        );
        println!("  {}", format!("ssh {}@{}", user, host).dimmed());
    } else {
        let session_name = format!("proto-{}", std::process::id());
        if std::process::Command::new("tmux")
            .args(["new-session", "-d", "-s", &session_name])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            sp.done(&format!("Session '{}'", session_name));
            let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
            let host =
                run_command_output("hostname", &[]).unwrap_or_else(|_| "localhost".into());
            println!();
            println!(
                "{}",
                label_value(
                    "Attach",
                    &format!("tmux attach -t {}", session_name)
                )
            );
            println!("  {}", format!("ssh {}@{}", user, host).dimmed());
            println!("\n{} Ctrl+B D to detach.\n", "  ".dimmed());
            println!("{}", divider());
            let _ = std::process::Command::new("tmux")
                .args(["attach", "-t", &session_name])
                .status();
            println!(
                "\n{} Session detached ({}).",
                success(""),
                session_name
            );
        } else {
            sp.fail("Failed to create tmux session.");
        }
    }
}

fn share_with_vnc() {
    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|s| s == "wayland")
            .unwrap_or(false);

    let (server_bin, install_hint) = if is_wayland {
        ("wayvnc", "sudo pacman -S wayvnc")
    } else {
        ("x11vnc", "sudo pacman -S x11vnc")
    };

    if !which(server_bin) {
        eprintln!(
            "{} {} not installed. Install: {}",
            error(""),
            server_bin,
            install_hint.style(Theme::ACCENT)
        );
        return;
    }

    let vnc_port = 5900u16;
    let ws_port = 5901u16;
    let web_port = 5800u16;
    let sp = Spinner::new(&format!("Starting {}...", server_bin));

    let vnc = if is_wayland {
        std::process::Command::new("wayvnc")
            .args(["0.0.0.0", &vnc_port.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    } else {
        std::process::Command::new("x11vnc")
            .args([
                "-forever",
                "-shared",
                "-rfbport",
                &vnc_port.to_string(),
                "-passwd",
                "proto",
                "-localhost",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    };

    let mut vnc = match vnc {
        Ok(c) => c,
        Err(e) => {
            sp.fail(&format!("{}", e));
            return;
        }
    };
    std::thread::sleep(std::time::Duration::from_secs(1));
    if let Ok(Some(_)) = vnc.try_wait() {
        sp.fail(&format!("{} exited. Display accessible?", server_bin));
        return;
    }

    sp.update("Starting websockify...");
    let mut ws: Option<std::process::Child> = None;

    let ws_bin = find_websockify();
    if let Some(bin) = ws_bin {
        if let Ok(c) = std::process::Command::new(&bin)
            .args([
                &ws_port.to_string(),
                &format!("localhost:{}", vnc_port),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            ws = Some(c);
        }
    }

    let local_ip = get_local_ip().unwrap_or_else(|| "127.0.0.1".into());
    sp.done("VNC server running");

    let html = build_novnc_page(&local_ip, ws_port, vnc_port, ws.is_some());
    start_http_server(web_port, html);

    let mut public_url = String::new();
    let mut ngrok_pid: Option<u32> = None;

    if which("ngrok") {
        let sp2 = Spinner::new("Starting ngrok tunnel...");
        if let Ok(mut t) = std::process::Command::new("ngrok")
            .args(["http", &web_port.to_string(), "--log", "stdout"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            let pid = t.id();
            ngrok_pid = Some(pid);

            let stderr = t.stderr.take();
            std::thread::spawn(move || {

                if let Some(reader) = stderr {
                    for line in BufReader::new(reader).lines().flatten() {
                        if line.contains("ERR_NGROK") {
                            eprintln!("\n{} ngrok: {}", warn(""), line);
                        }
                    }
                }
            });

            use std::io::{BufRead, BufReader};
            for line in BufReader::new(t.stdout.take().unwrap()).lines().flatten() {
                if let Some(url) = parse_ngrok_url(&line) {
                    public_url = url;
                    break;
                }
                if line.contains("ERR_NGROK") || line.contains("failed to start tunnel") {
                    sp2.fail(&format!(
                        "ngrok: {}",
                        line.chars().take(80).collect::<String>()
                    ));
                    let _ = std::process::Command::new("kill")
                        .arg(pid.to_string())
                        .status();
                    ngrok_pid = None;
                    break;
                }
            }

            if !public_url.is_empty() {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let still_alive = std::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if still_alive {
                    sp2.done("Tunnel ready");
                } else {
                    sp2.fail("ngrok exited early (check auth token)");
                    public_url.clear();
                    ngrok_pid = None;
                }
            } else if ngrok_pid.is_some() {
                sp2.fail("ngrok timed out");
                let _ = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .status();
                ngrok_pid = None;
            }
        } else {
            sp2.fail("Failed to start ngrok");
        }
    }

    println!();
    println!("{}", "Desktop Share".style(Theme::HEADER));
    println!("{}", divider());
    println!(
        "{}",
        label_value("Local", &format!("http://{}:{}", local_ip, web_port))
    );
    if !public_url.is_empty() {
        println!("{}", label_value("Public", &public_url));
    }
    println!(
        "{}",
        label_value(
            "NoVNC",
            if ws.is_some() {
                "✦ browser VNC active"
            } else {
                "pipx install websockify (auto-attempted)"
            }
        )
    );
    println!("{}", divider());
    if ws.is_some() {
        println!(
            "\n{} Share the link — opens your full desktop in any browser!",
            "★".style(Theme::SUCCESS).bold()
        );
    }
    println!("{} Ctrl+C to stop.\n", "  ".dimmed());
    println!("{}", divider());

    let _ = vnc.wait();
    if let Some(mut c) = ws {
        let _ = c.kill();
    }
    if let Some(pid) = ngrok_pid {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
    println!("\n{} Session stopped.", success(""));
}

fn parse_ngrok_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|p| p.starts_with("url="))
        .map(|p| p[4..].to_string())
}

fn build_novnc_page(host: &str, ws_port: u16, vnc_port: u16, has_ws: bool) -> String {
    if has_ws {
        format!(r##"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Proto Desktop</title>
<style>*{{margin:0;padding:0;box-sizing:border-box}}body{{background:#000;overflow:hidden}}
#bar{{position:fixed;top:0;left:0;right:0;z-index:10;background:#0d1117cc;color:#58a6ff;padding:8px 16px;font-family:system-ui,sans-serif;font-size:13px;backdrop-filter:blur(8px)}}
.dot{{display:inline-block;width:8px;height:8px;background:#3fb950;border-radius:50%;margin-right:8px;box-shadow:0 0 8px #3fb95066}}
</style></head><body>
<div id="bar"><div class="dot"></div>Proto Desktop</div>
<script src="https://cdn.jsdelivr.net/npm/@novnc/novnc@1.5/lib/rfb.js"></script>
<script>new RFB(document.body,"ws://{0}:{1}",{{password:"proto"}}).viewportDrag=true;</script>
</body></html>"##, host, ws_port)
    } else {
        format!(r##"<!DOCTYPE html><html><head><meta charset="UTF-8"><title>Proto Desktop</title>
<style>*{{margin:0;padding:0}}body{{background:#0d1117;color:#c9d1d9;font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;min-height:100vh;text-align:center}}
.card{{background:#161b22;border:1px solid #30363d;border-radius:8px;padding:24px}}h1{{color:#58a6ff;margin-bottom:12px}}code{{background:#21262d;padding:3px 8px;border-radius:4px;color:#3fb950}}p{{color:#8b949e;margin:8px 0}}
</style></head><body><div class="card"><h1>Desktop Share Active</h1>
<p>Install <code>pip install websockify</code> for browser-based VNC</p>
<p>Or use VNC client: <code>{0}:{1}</code> (password: <code>proto</code>)</p></div></body></html>"##, host, vnc_port)
    }
}

fn start_http_server(port: u16, html: String) {
    let listener = match std::net::TcpListener::bind(format!("0.0.0.0:{}", port)) {
        Ok(l) => l,
        Err(_) => return,
    };
    listener.set_nonblocking(true).ok();
    std::thread::spawn(move || loop {
        if let Ok((mut stream, _)) = listener.accept() {
            use std::io::{Read, Write};
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = stream.write_all(resp.as_bytes());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    });
}

fn find_websockify() -> Option<String> {
    if which("websockify") {
        return Some("websockify".into());
    }
    let local = dirs::home_dir()
        .unwrap_or_default()
        .join(".local/bin/websockify");
    if local.exists() {
        return Some(local.to_string_lossy().to_string());
    }

    let sp = Spinner::new("Installing websockify via pipx...");
    if which("pipx") {
        let status = std::process::Command::new("pipx")
            .args(["install", "websockify"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            sp.done("websockify installed via pipx");
            let path = dirs::home_dir()
                .unwrap_or_default()
                .join(".local/bin/websockify");
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        } else {
            sp.fail("pipx install websockify failed");
        }
    } else {
        sp.fail("pipx not found");
        println!(
            "{} Install: {}",
            warn(""),
            "sudo pacman -S python-pipx && pipx install websockify".style(Theme::ACCENT)
        );
    }
    None
}

fn get_local_ip() -> Option<String> {
    run_command_output("hostname", &["-I"])
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|s| s.to_string()))
}

fn join(link: &str) {
    let sp = Spinner::new(&format!("Joining {}...", link));
    if link.starts_with("ssh ") {
        let addr = link.trim_start_matches("ssh ").trim();
        sp.done(&format!("Connecting to {}", addr));
        let _ = std::process::Command::new("ssh").arg(addr).status();
    } else if link.starts_with("http") {
        sp.done(&format!("Opening {}", link));
        if which("xdg-open") {
            let _ = std::process::Command::new("xdg-open").arg(link).status();
        }
    } else {
        sp.fail("Unrecognized link format.");
    }
}

use std::io::BufRead;
