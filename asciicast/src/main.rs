use clap::Parser;
use proto_plugin_sdk::*;
use std::process::Command;

#[derive(Parser)]
#[command(name = "asciicast", about = "Terminal session recording")]
struct Cli {
    /// Output file path
    #[arg(short = 'o')]
    output: Option<String>,
    /// Command to record
    cmd: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    run(cli.output, cli.cmd);
}

fn run(output: Option<String>, cmd: Vec<String>) {
    println!("{}", header("Asciicast"));
    println!("{}", divider());

    let asciinema = which("asciinema");
    let script = which("script");

    if asciinema {
        println!("  {} Using asciinema ...\n", muted(""));
        let mut args: Vec<&str> = vec!["rec"];
        if let Some(ref out) = output {
            args.push(out);
        }
        if cmd.is_empty() {
            let status = Command::new("asciinema").args(&args).status();
            match status {
                Ok(s) if s.success() => {}
                _ => eprintln!("  {} asciinema exited with error.", error("")),
            }
        } else {
            let mut args: Vec<String> = vec!["rec".to_string()];
            if let Some(ref out) = output {
                args.push(out.clone());
            }
            args.push("--command".to_string());
            args.push(cmd.join(" "));
            let status = Command::new("asciinema").args(&args).status();
            match status {
                Ok(s) if s.success() => {}
                _ => eprintln!("  {} asciinema exited with error.", error("")),
            }
        }
    } else if script {
        let out = output.unwrap_or_else(|| "recording.cast".to_string());
        println!(
            "  {} Using script (no asciinema found). Output: {}\n",
            muted(""),
            out.style(Theme::VALUE)
        );
        if cmd.is_empty() {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let _ = Command::new("script")
                .args(["-q", &out, "-c", &shell])
                .status();
        } else {
            let _ = Command::new("script")
                .args(["-q", &out, "-c", &cmd.join(" ")])
                .status();
        }
    } else {
        println!(
            "  {} Install {} or use:\n",
            warn("asciinema not found."),
            "asciinema".style(Theme::VALUE)
        );
        println!("    pacman -S asciinema");
        println!("    brew install asciinema\n");
        println!("  Then re-run: asciicast");
    }
}
