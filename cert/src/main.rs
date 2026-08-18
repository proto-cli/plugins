use clap::Parser;
use proto_plugin_sdk::*;
use std::process::Command;

#[derive(Parser)]
#[command(name = "cert", about = "TLS certificate inspection")]
struct Cli {
    /// Domain to inspect
    domain: String,
}

fn main() {
    let cli = Cli::parse();
    run(&cli.domain);
}

fn run(domain: &str) {
    if !which("openssl") {
        eprintln!(
            "{} OpenSSL required. Install it (e.g. {}).",
            error(""),
            "sudo pacman -S openssl".dimmed()
        );
        return;
    }

    if !which("date") {
        eprintln!("{} The 'date' utility is required.", error(""));
        return;
    }

    println!("{}", header(format!("TLS Certificate: {}", domain).as_str()));
    println!("{}", divider());

    let script = format!(
        "echo | timeout 10 openssl s_client -connect {}:443 -servername {} 2>/dev/null | openssl x509 -noout -subject -issuer -dates -ext subjectAltName 2>/dev/null",
        domain, domain
    );
    let out = Command::new("sh")
        .arg("-c")
        .arg(&script)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    if out.trim().is_empty() {
        eprintln!(
            "{} Could not retrieve a certificate for '{}'. Check the domain and port 443.",
            error(""),
            domain
        );
        return;
    }

    let mut subject = String::new();
    let mut issuer = String::new();
    let mut not_before = String::new();
    let mut not_after = String::new();
    let mut sans: Vec<String> = Vec::new();
    let mut in_san = false;

    for line in out.lines() {
        if let Some(v) = line.strip_prefix("subject=") {
            subject = clean_dn(v);
        } else if let Some(v) = line.strip_prefix("issuer=") {
            issuer = clean_dn(v);
        } else if let Some(v) = line.strip_prefix("notBefore=") {
            not_before = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("notAfter=") {
            not_after = v.trim().to_string();
        } else if line.contains("Subject Alternative Name") {
            in_san = true;
        } else if in_san {
            if line.trim().is_empty()
                || line.contains(':') && !line.contains("DNS:") && !line.contains("IP Address:")
            {
                in_san = false;
                continue;
            }
            for part in line.trim().split(',') {
                let part = part.trim();
                if part.starts_with("DNS:") {
                    sans.push(part[4..].to_string());
                } else if let Some(ip) = part.strip_prefix("IP Address:") {
                    sans.push(format!("{} (IP)", ip.trim()));
                }
            }
        }
    }

    println!(
        "  {}",
        label_value(
            "Subject",
            &if subject.is_empty() {
                domain.to_string()
            } else {
                subject
            }
        )
    );
    println!("  {}", label_value("Issuer", &issuer));
    println!("  {}", label_value("Valid from", &not_before));
    println!("  {}", label_value("Valid until", &not_after));

    let days = days_until(&not_after);
    match days {
        Some(d) => {
            let text = if d < 0 {
                format!("{} day(s) AGO", -d)
            } else {
                format!("{} day(s)", d)
            };
            let color = if d < 0 {
                Theme::ERROR
            } else if d < 30 {
                Theme::WARN
            } else {
                Theme::SUCCESS
            };
            println!(
                "  {}",
                label_value("Expires in", &text.style(color).to_string())
            );
        }
        None => println!("  {}", label_value("Expires in", "unknown")),
    }

    if !sans.is_empty() {
        println!("  {}", label_value("SANs", &sans.join(", ")));
    }
    println!("  {}", label_value("Port", "443"));

    if let Some(d) = days {
        println!();
        if d < 0 {
            println!(
                "  {} Certificate expired {} day(s) ago — renew now!",
                error(""),
                -d
            );
        } else if d < 14 {
            println!(
                "  {} Certificate expires within 2 weeks — renew soon.",
                warn("")
            );
        } else {
            println!("  {} Certificate is healthy.", success(""));
        }
    }
}

fn clean_dn(dn: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for chunk in dn.split(',') {
        if let Some(v) = chunk.trim().strip_prefix("CN=") {
            parts.push(v.trim().to_string());
        }
    }
    if parts.is_empty() {
        dn.to_string()
    } else {
        parts.join(", ")
    }
}

fn days_until(date_str: &str) -> Option<i64> {
    let to_epoch = format!("date -u -d '{}' +%s", date_str.replace('\'', "'\\''"));
    let now_epoch = "date -u +%s";
    let target: i64 = Command::new("sh")
        .arg("-c")
        .arg(&to_epoch)
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())?;
    let now: i64 = Command::new("sh")
        .arg("-c")
        .arg(now_epoch)
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())?;
    Some((target - now) / 86400)
}
