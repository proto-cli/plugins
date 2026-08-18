use clap::Parser;
use proto_plugin_sdk::*;

const RECORD_TYPES: &[&str] = &["A", "AAAA", "CNAME", "MX", "TXT", "NS"];

#[derive(Parser)]
#[command(name = "dns", about = "DNS lookup for domains")]
struct Cli {
    /// Domain to look up
    domain: String,
}

fn main() {
    let cli = Cli::parse();
    run(&cli.domain);
}

fn run(domain: &str) {
    if !which("dig") {
        eprintln!(
            "{} dig required. Install bind-tools (e.g. {}).",
            error(""),
            "sudo pacman -S bind-tools".dimmed()
        );
        return;
    }

    println!("{}", header(format!("DNS Lookup: {}", domain).as_str()));
    println!("{}", divider());

    let mut found = false;
    for rt in RECORD_TYPES {
        let out = run_command_output("dig", &["+short", domain, rt]).unwrap_or_default();
        let values: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();

        if values.is_empty() {
            continue;
        }
        found = true;

        let label = format!("{:5}", rt);
        for (i, v) in values.iter().enumerate() {
            if i == 0 {
                println!(
                    "  {} {}",
                    label.style(Theme::ACCENT).bold(),
                    v
                );
            } else {
                println!("  {} {}", " ".repeat(5), v);
            }
        }
    }

    println!();
    if found {
        println!(
            "  {} All records above resolved for {}",
            success(""),
            domain
        );
    } else {
        println!(
            "  {} No records found — domain may not exist or has no records.",
            warn("")
        );
    }
}
