use clap::{Subcommand, Parser};
use proto_plugin_sdk::*;

#[derive(Parser)]
#[command(name = "status", about = "Network monitoring — ping, monitor, serve dashboard, report")]
struct Cli {
    #[command(subcommand)]
    action: StatusAction,
}

#[derive(Subcommand)]
enum StatusAction {
    Ping { ip: String },
    Monitor { ip: String, #[arg(short = 'n', long, default_value = "5")] interval: u64 },
    Serve { targets: Vec<String>, #[arg(short = 'p', long, default_value = "5050")] port: u16, #[arg(short = 'n', long, default_value = "5")] interval: u64 },
    Report { ip: String, #[arg(short = 'n', long, default_value = "30")] checks: u64, #[arg(short = 'o', long, default_value = "status_report.txt")] output: String },
}

fn main() {
    let cli = Cli::parse();
    match cli.action {
        StatusAction::Ping { ip } => cmd_ping(&ip),
        StatusAction::Monitor { ip, interval } => cmd_monitor(&ip, interval),
        StatusAction::Serve { targets, port, interval } => cmd_serve(&targets, port, interval),
        StatusAction::Report { ip, checks, output } => cmd_report(&ip, checks, &output),
    }
}

fn parse_ip(ip: &str) -> (String, u16) {
    if let Some((host, port_str)) = ip.rsplit_once(':') { if let Ok(port) = port_str.parse::<u16>() { return (host.to_string(), port); } }
    (ip.to_string(), 80)
}

fn resolve_addr(host: &str, port: u16) -> Option<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    format!("{}:{}", host, port).to_socket_addrs().ok()?.next()
}

fn probe(host: &str, port: u16) -> (bool, u128) {
    match resolve_addr(host, port) {
        Some(addr) => { let start = std::time::Instant::now(); match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3)) { Ok(_) => (true, start.elapsed().as_millis()), Err(_) => (false, 0) } }
        None => (false, 0),
    }
}

fn cmd_ping(ip: &str) {
    let (host, port) = parse_ip(ip);
    println!("  Pinging {}:{}...", host, port);
    let (online, latency) = probe(&host, port);
    if online { println!("\n{} {}:{} is {}", success(""), host.style(Theme::ACCENT), port, "ONLINE".green().bold()); println!("{}", label_value("Latency", &format!("{}ms", latency))); }
    else { println!("\n{} {}:{} is {}", error(""), host.style(Theme::ACCENT), port, "OFFLINE".red().bold()); }
}

fn cmd_monitor(ip: &str, interval: u64) {
    let (host, port) = parse_ip(ip);
    println!("{} {} {}", "▶".style(Theme::ACCENT), "Monitoring".style(Theme::HEADER), format!("{}:{}", host, port).style(Theme::MUTED));
    println!("  Interval: {}s  |  Ctrl+C to stop", interval);
    println!("{}", divider());
    let mut checks: u64 = 0; let mut online_count: u64 = 0; let mut streak: u64 = 0; let mut total_latency: u128 = 0;
    loop {
        checks += 1;
        let (online, latency) = probe(&host, port);
        if online { online_count += 1; streak += 1; total_latency += latency; } else { streak = 0; }
        let uptime_pct = (online_count as f64 / checks as f64) * 100.0;
        let avg_latency = if online_count > 0 { total_latency / online_count as u128 } else { 0 };
        let time = chrono_now();
        let status_icon = if online { "●".green().bold().to_string() } else { "●".red().bold().to_string() };
        print!("\r{}{} {} {}{}{} {}", " ".dimmed(), time.dimmed(), status_icon, format!("{:>4}ms", latency).dimmed(), format!("  uptime:{:.1}%", uptime_pct).dimmed(), format!("  avg:{}ms", avg_latency).dimmed(), format!("  streak:{}", streak).dimmed());
        use std::io::Write; let _ = std::io::stdout().flush();
        if checks > 0 && checks % 10 == 0 { println!(); }
        std::thread::sleep(std::time::Duration::from_secs(interval));
    }
}

fn cmd_serve(targets: &[String], port: u16, interval: u64) {
    let parsed: Vec<(String, u16)> = targets.iter().map(|t| parse_ip(t)).collect();
    println!("{} {} http://localhost:{}", "▶".style(Theme::ACCENT), "Dashboard →".style(Theme::HEADER), port.to_string().style(Theme::ACCENT).bold());
    println!("  Watching: {}", targets.iter().map(|t| t.style(Theme::ACCENT).to_string()).collect::<Vec<_>>().join(", "));
    println!("{}", divider());
    let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).expect("Failed to bind port");
    listener.set_nonblocking(true).ok();
    let start = std::time::Instant::now();
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                use std::io::{Read, Write};
                let mut buf = [0u8; 4096]; let _ = stream.read(&mut buf);
                let statuses: Vec<String> = parsed.iter().map(|(host, port)| { let (online, latency) = probe(host, *port); format!("<div class=\"target {}\"><div class=\"dot\"></div><span class=\"name\">{}:{}</span><span class=\"latency\">{}</span></div>", if online { "online" } else { "offline" }, host, port, if online { format!("{}ms", latency) } else { "—".into() }) }).collect();
                let html = dashboard_html(&statuses, port, interval, start.elapsed().as_secs());
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", html.len(), html);
                let _ = stream.write_all(response.as_bytes());
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

fn dashboard_html(statuses: &[String], port: u16, interval: u64, uptime: u64) -> String {
    format!(r##"<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Proto Monitor</title><meta http-equiv="refresh" content="{}"><style>*{{margin:0;padding:0;box-sizing:border-box}}body{{background:#0d1117;color:#c9d1d9;font-family:system-ui,sans-serif;min-height:100vh}}.header{{background:#161b22;border-bottom:1px solid #30363d;padding:16px 24px;display:flex;align-items:center;justify-content:space-between}}.header h1{{font-size:18px;color:#58a6ff}}.header .uptime{{font-size:13px;color:#8b949e}}.grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:16px;padding:24px}}.target{{background:#161b22;border:1px solid #30363d;border-radius:8px;padding:20px;display:flex;align-items:center;gap:16px}}.target.online{{border-color:#238636}}.target.offline{{border-color:#da3633;opacity:.7}}.dot{{width:14px;height:14px;border-radius:50%;flex-shrink:0}}.online .dot{{background:#3fb950;box-shadow:0 0 12px #3fb95066}}.offline .dot{{background:#da3633;box-shadow:0 0 12px #da363366}}.name{{font-size:16px;font-weight:600;flex:1}}.latency{{font-size:14px;color:#8b949e;font-variant-numeric:tabular-nums}}.footer{{text-align:center;padding:16px;color:#484f58;font-size:12px}}</style></head><body><div class="header"><h1>Proto Monitor</h1><span class="uptime">up {}s</span></div><div class="grid">{}</div><div class="footer">auto-refresh every {}s · port {}</div></body></html>"##, interval, uptime, if statuses.is_empty() { "<div style=\"text-align:center;padding:60px;color:#484f58\">No targets configured</div>".to_string() } else { statuses.join("\n") }, interval, port)
}

fn cmd_report(ip: &str, checks: u64, output_path: &str) {
    let (host, port) = parse_ip(ip);
    println!("  Running {} checks against {}:{}...", checks, host, port);
    let mut results: Vec<(String, bool, u128)> = Vec::new();
    let mut online_count = 0u64; let mut total_latency = 0u128; let mut min_latency = u128::MAX; let mut max_latency = 0u128; let mut longest_outage = 0u64; let mut current_outage = 0u64;
    for i in 0..checks {
        let (online, latency) = probe(&host, port);
        let time = chrono_now(); results.push((time, online, latency));
        if online { online_count += 1; total_latency += latency; if latency < min_latency { min_latency = latency; } if latency > max_latency { max_latency = latency; } current_outage = 0; }
        else { current_outage += 1; if current_outage > longest_outage { longest_outage = current_outage; } }
        if i < checks - 1 { std::thread::sleep(std::time::Duration::from_secs(2)); }
    }
    let uptime_pct = (online_count as f64 / checks as f64) * 100.0;
    let avg_latency = if online_count > 0 { total_latency / online_count as u128 } else { 0 };
    let now = chrono_now();
    let mut report = String::new();
    report.push_str(&format!("PROTO STATUS REPORT\n══════════════════════════════════════\nTarget: {}:{}\nGenerated: {}\n══════════════════════════════════════\n\nSUMMARY\n───────\n  Total checks:    {}\n  Online:          {} ({:.1}%)\n  Offline:         {} ({:.1}%)\n  Avg latency:     {}ms\n", host, port, now, checks, online_count, uptime_pct, checks - online_count, 100.0 - uptime_pct, avg_latency));
    if online_count > 0 { report.push_str(&format!("  Min latency:     {}ms\n  Max latency:     {}ms\n", min_latency, max_latency)); }
    report.push_str(&format!("  Longest outage:  {} checks\n\nTIMELINE\n────────\n", longest_outage));
    for (time, online, latency) in &results { let mark = if *online { "✓" } else { "✗" }; let info = if *online { format!("{}ms", latency) } else { "timeout".into() }; report.push_str(&format!("  {}  {}  {}\n", time, mark, info)); }
    report.push_str(&format!("\n══════════════════════════════════════\nGenerated by Proto CLI\n"));
    std::fs::write(output_path, &report).expect("Failed to write report");
    println!("\n{} Report saved to {}", success(""), output_path);
    println!("{}", label_value("Uptime", &format!("{:.1}%", uptime_pct)));
    println!("{}", label_value("Latency", &format!("{}ms avg", avg_latency)));
    if longest_outage > 0 { println!("{}", label_value("Worst outage", &format!("{} consecutive failures", longest_outage))); }
}

fn chrono_now() -> String {
    let dur = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let total_secs = dur.as_secs() as i64;
    let days = total_secs / 86400; let rem = total_secs % 86400;
    let hours = rem / 3600; let minutes = (rem % 3600) / 60; let seconds = rem % 60;
    let (y, mo, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, hours, minutes, seconds)
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468; let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as i64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as i64;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
