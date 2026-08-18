use clap::Parser;
use proto_plugin_sdk::*;
use std::process::Command;
#[derive(Parser)]
#[command(name = "ports", about = "Listening ports dashboard")]
struct Cli {
    /// Run in serve mode
    #[arg(long)]
    serve: bool,
    /// Port for serve mode
    #[arg(long, default_value = "9101")]
    port: u16,
}

fn main() {
    let cli = Cli::parse();
    run(cli.serve, cli.port);
}

struct PortInfo {
    proto: String,
    local: String,
    port: u16,
    pid: Option<u32>,
    process: Option<String>,
}

fn run(serve: bool, port: u16) {
    if serve {
        serve_mode(port);
        return;
    }
    interactive();
}

fn serve_mode(port: u16) {
    println!();
    println!(
        "  {} Listening ports scan. Port {}. Ctrl+C to stop.",
        "◉".style(Theme::ACCENT),
        port
    );

    loop {
        let _list = scan().unwrap_or_default();
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}

fn interactive() {
    println!("{}", header("Listening Ports"));
    println!("{}", divider());

    loop {
        let list = match scan() {
            Some(l) if !l.is_empty() => l,
            Some(_) => {
                println!("{} No listening ports found.", muted(""));
                return;
            }
            None => {
                eprintln!("{} Could not read ports (ss required).", error(""));
                return;
            }
        };

        print_table(&list);

        let mut options: Vec<String> = Vec::new();
        options.push("↻ Refresh".to_string());
        for s in &list {
            let proc = s.process.clone().unwrap_or_else(|| "?".to_string());
            options.push(format!(
                "{} {}:{}  {}",
                "✗".style(Theme::WARN),
                s.port,
                proc,
                s.local.dimmed()
            ));
        }
        options.push("Done".to_string());

        let idx = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Select a port to kill (or refresh)")
            .items(&options)
            .default(0)
            .interact()
            .unwrap_or(options.len() - 1);

        if idx == 0 {
            continue;
        }
        if idx >= options.len() - 1 {
            break;
        }

        let target = &list[idx - 1];
        let proc = target
            .process
            .clone()
            .unwrap_or_else(|| "unknown".into());
        let confirm =
            dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
                .with_prompt(format!(
                    "Kill {} on port {} (pid {})?",
                    proc,
                    target.port,
                    target
                        .pid
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "?".into())
                ))
                .default(false)
                .interact()
                .unwrap_or(false);
        if !confirm {
            continue;
        }

        if let Some(pid) = target.pid {
            let ok = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                println!(
                    "  {} Killed {} (pid {})\n",
                    success(""),
                    proc,
                    pid
                );
            } else {
                println!("  {} Failed to kill {}\n", error(""), proc);
            }
        } else {
            println!(
                "  {} No PID found for that socket.\n",
                "⚠".style(Theme::WARN)
            );
        }
    }
}

fn print_table(list: &[PortInfo]) {
    println!(
        "  {} {:>8}  {:6}  {:<24} {}",
        "PROTO".style(Theme::LABEL),
        "PORT".style(Theme::LABEL),
        "PID".style(Theme::LABEL),
        "PROCESS".style(Theme::LABEL),
        "ADDRESS".style(Theme::LABEL),
    );
    for s in list {
        println!(
            "  {} {:>8}  {:6}  {:<24} {}",
            s.proto.style(Theme::ACCENT),
            s.port.to_string().style(Theme::VALUE),
            s.pid.map(|p| p.to_string())
                .unwrap_or_else(|| "-".into()),
            s.process.as_deref().unwrap_or("?"),
            s.local.dimmed(),
        );
    }
    println!();
}

fn scan() -> Option<Vec<PortInfo>> {
    let out = Command::new("ss")
        .args(["-tulpnH"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut list = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let proto = it.next()?.to_string();
        let state = it.next()?.to_string();
        let _ = it.next();
        let _ = it.next();
        let local = it.next()?.to_string();
        let _peer = it.next();
        let proc_col = it.next().map(|s| s.to_string());

        if state != "LISTEN" && state != "UNCONN" {
            continue;
        }
        let port: u16 = match local.rsplit(':').next().and_then(|p| p.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        let (pid, process) = parse_proc_col(proc_col.as_deref());
        list.push(PortInfo {
            proto,
            local,
            port,
            pid,
            process,
        });
    }
    list.sort_by_key(|s| (s.port, s.proto.clone()));
    Some(list)
}

fn parse_proc_col(col: Option<&str>) -> (Option<u32>, Option<String>) {
    let col = match col {
        Some(c) => c,
        None => return (None, None),
    };
    let mut pid = None;
    let mut name = None;
    if let Some(start) = col.find("pid=") {
        let rest = &col[start + 4..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        pid = digits.parse().ok();
    }
    if let Some(start) = col.find("(\"") {
        let rest = &col[start + 2..];
        name = rest.split('"').next().map(|s| s.to_string());
    }
    (pid, name)
}
