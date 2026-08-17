use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};

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
fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}
fn warn(msg: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), msg)
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

#[derive(Parser)]
#[command(name = "webhook", about = "Listen for webhooks, print formatted JSON, tunnel via ngrok")]
struct Cli {
    #[command(subcommand)]
    action: WebhookAction,
}

#[derive(Subcommand, Debug, Clone)]
enum WebhookAction {
    #[command(name = "listen", about = "Listen for webhooks")]
    Listen {
        #[arg(value_name = "PORT", help = "Local port to listen on (default: 9000)")]
        port: Option<u16>,
        #[arg(long, help = "Skip the ngrok public tunnel")]
        no_tunnel: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.action {
        WebhookAction::Listen { port, no_tunnel } => listen(port.unwrap_or(9000), no_tunnel),
    }
}

fn parse_ngrok_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|p| p.starts_with("url="))
        .map(|p| p[4..].to_string())
}

fn start_ngrok(port: u16) -> (String, u32) {
    let sp = Spinner::new("Opening ngrok tunnel...");
    let mut child = std::process::Command::new("ngrok")
        .args(["http", &port.to_string(), "--log", "stdout"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to start ngrok");

    let pid = child.id();
    let url = {
        let mut url = String::new();
        let stdout = child.stdout.take().unwrap();
        for line in BufReader::new(stdout).lines().flatten() {
            if let Some(u) = parse_ngrok_url(&line) {
                url = u;
                break;
            }
            if line.contains("ERR_NGROK") || line.contains("failed to start tunnel") {
                break;
            }
        }
        url
    };

    if url.is_empty() {
        sp.fail("ngrok failed to start (is your auth token set?)");
        let _ = child.kill();
        return (String::new(), 0);
    }
    sp.done("Public tunnel ready");
    (url, pid)
}

fn colorize_json(value: &serde_json::Value, depth: usize) -> String {
    use serde_json::Value;
    let indent = "  ".repeat(depth);
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            let mut out = String::from("{\n");
            let entries: Vec<String> = map
                .iter()
                .map(|(k, v)| {
                    let key = format!("\"{}\"", k);
                    let colored_key = key.style(Theme::LABEL);
                    format!(
                        "{}{}: {}",
                        format!("{}  ", indent),
                        colored_key,
                        colorize_json(v, depth + 1)
                    )
                })
                .collect();
            out.push_str(&entries.join(",\n"));
            out.push('\n');
            out.push_str(&indent);
            out.push('}');
            out
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                return "[]".to_string();
            }
            let mut out = String::from("[\n");
            let items: Vec<String> = arr
                .iter()
                .map(|v| {
                    format!(
                        "{}{}",
                        format!("{}  ", indent),
                        colorize_json(v, depth + 1)
                    )
                })
                .collect();
            out.push_str(&items.join(",\n"));
            out.push('\n');
            out.push_str(&indent);
            out.push(']');
            out
        }
        Value::String(s) => format!("\"{}\"", s).style(Theme::SUCCESS).to_string(),
        Value::Number(n) => n.to_string().style(Theme::WARN).to_string(),
        Value::Bool(b) => b.to_string().style(Theme::ACCENT).to_string(),
        Value::Null => "null".style(Theme::MUTED).to_string(),
    }
}

fn handle_request(mut stream: TcpStream) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let mut header_end: Option<usize> = None;

    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = find_header_end(&buf) {
                    header_end = Some(pos);
                    break;
                }
                if buf.len() > 1_000_000 {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let header_end = header_end.unwrap_or(buf.len());
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();

    let request_line = lines.next().unwrap_or_default().to_string();
    let mut headers = std::collections::HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }

    let content_length: usize = headers
        .get("content-length")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);

    let body = if content_length > 0 {
        while buf.len() < header_end + content_length {
            match stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                Err(_) => break,
            }
        }
        String::from_utf8_lossy(&buf[header_end..].get(..content_length).unwrap_or_default())
            .to_string()
    } else {
        String::from_utf8_lossy(&buf[header_end..]).to_string()
    };

    println!();
    println!("{}", divider());
    println!(
        "  {}",
        request_line.style(Theme::ACCENT).bold()
    );
    for (k, v) in &headers {
        if k == "user-agent"
            || k == "content-type"
            || k == "x-github-event"
            || k == "x-gitlab-event"
            || k == "x-signature"
            || k == "authorization"
        {
            println!(
                "  {}: {}",
                k.style(Theme::LABEL),
                v.style(Theme::MUTED)
            );
        }
    }

    if !body.trim().is_empty() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            println!();
            println!("{}", colorize_json(&v, 0));
        } else {
            println!("\n{}", body.style(Theme::MUTED));
        }
    } else {
        println!("  {}", "(empty body)".style(Theme::MUTED));
    }

    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    let _ = stream.flush();
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
}

fn listen(port: u16, no_tunnel: bool) {
    println!(
        "{} {}",
        "◆".style(Theme::ACCENT),
        "Webhook Listener".style(Theme::HEADER)
    );
    println!("{}", divider());

    let listener = match TcpListener::bind(format!("0.0.0.0:{}", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{} Cannot bind to port {}: {}", error(""), port, e);
            std::process::exit(1);
        }
    };

    println!(
        "{}",
        label_value("Listening", &format!("http://0.0.0.0:{}", port))
    );
    let local_ip = run_command_output("hostname", &["-I"])
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|s| s.to_string()))
        .unwrap_or_else(|| "127.0.0.1".into());
    println!(
        "{}",
        label_value("Local", &format!("http://{}:{}", local_ip, port))
    );

    let mut ngrok_pid = 0u32;
    if !no_tunnel && which("ngrok") {
        let (url, pid) = start_ngrok(port);
        if !url.is_empty() {
            println!("{}", label_value("Public", &url));
            println!(
                "{}",
                label_value(
                    "Tunnel",
                    &format!("{}", "ngrok".style(Theme::SUCCESS))
                )
            );
        }
        ngrok_pid = pid;
    } else if !no_tunnel {
        println!("{} ngrok not found — local only.", warn(""));
    }

    println!("{}", divider());
    println!(
        "\n{} Waiting for webhooks... (Ctrl+C to stop)\n",
        "✦".style(Theme::SUCCESS).bold()
    );

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Err(e) = std::thread::Builder::new().spawn(move || handle_request(s)) {
                    eprintln!("{} Failed to spawn handler: {}", error(""), e);
                }
            }
            Err(e) => eprintln!("{} Accept error: {}", error(""), e),
        }
    }

    if ngrok_pid != 0 {
        let _ = std::process::Command::new("kill")
            .arg(ngrok_pid.to_string())
            .status();
    }
}
