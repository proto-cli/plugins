use clap::Parser;
use proto_plugin_sdk::*;
use std::io::Write;
use std::path::Path;

#[derive(Parser)]
#[command(name = "share", about = "Upload files via temporary link services")]
struct Cli {
    /// File to upload
    file: String,
}

fn main() {
    let cli = Cli::parse();
    run(&cli.file);
}

fn run(file: &str) {
    let path = Path::new(file);
    let meta = match std::fs::metadata(path) {
        Ok(m) if m.is_file() => m,
        Ok(_) => {
            eprintln!("{} Not a file: {}", error(""), file);
            return;
        }
        Err(_) => {
            eprintln!("{} File not found: {}", error(""), file);
            return;
        }
    };

    println!("{}", header("File Upload"));
    println!("{}", divider());

    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    println!("  {}", label_value("File", &name));
    println!("  {}", label_value("Size", &format_size(meta.len())));

    if is_image(&ext) {
        preview_image(path);
    }

    println!();
    use dialoguer::Confirm;
    let prompt = format!(
        "Upload {} ({}) to bashupload.com?",
        name,
        format_size(meta.len())
    );
    if !Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt(prompt)
        .default(false)
        .interact()
        .unwrap_or(false)
    {
        println!("{} Upload cancelled.", warn(""));
        return;
    }

    let url = match upload_file(path) {
        Some(u) => u,
        None => {
            eprintln!(
                "{} Upload failed. Check your network connection and try again.",
                error("")
            );
            return;
        }
    };

    println!(
        "\n{} {}",
        success(""),
        url.style(Theme::ACCENT).bold()
    );
    println!(
        "  {}",
        "Link copied to clipboard — expires in ~3 days or after 100 downloads."
            .style(Theme::MUTED)
    );

    copy_to_clipboard(&url);
}

fn is_image(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "avif"
    )
}

fn preview_image(path: &Path) {
    let kitty = std::env::var("KITTY_WINDOW_ID").is_ok();
    let tool: Option<(&str, &[&str])> = if kitty && which("kitty") {
        Some(("kitty", &["+kitten", "icat"] as &[&str]))
    } else if which("chafa") {
        Some(("chafa", &["--format", "symbols", "--size", "44x16"]))
    } else if which("viu") {
        Some(("viu", &["-s", "44x16"]))
    } else if which("img2txt") {
        Some(("img2txt", &["-W", "44", "-H", "16"]))
    } else if which("jp2a") {
        Some(("jp2a", &["--width=44"]))
    } else {
        None
    };

    if let Some((bin, args)) = tool {
        println!();
        let mut cmd = std::process::Command::new(bin)
            .args(args)
            .arg(path)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::null())
            .spawn();
        if let Ok(ref mut c) = cmd {
            let _ = c.wait();
        }
        println!();
    }
}

fn upload_file(path: &Path) -> Option<String> {
    let sp = Spinner::new("Uploading to bashupload.com...");
    let out = std::process::Command::new("curl")
        .args(["-sL", "--connect-timeout", "5", "--max-time", "30", "-T"])
        .arg(path)
        .arg("https://bashupload.com")
        .output()
        .ok();
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    if let Some(u) = text
        .lines()
        .map(|l| l.trim())
        .find(|l| l.contains("https://"))
        .and_then(|l| l.split_whitespace().find(|w| w.starts_with("https://")))
    {
        sp.done("Upload complete");
        return Some(u.to_string());
    }

    sp.update("bashupload.com unavailable, trying file.io...");
    let out = std::process::Command::new("curl")
        .args([
            "-sL", "--connect-timeout", "5", "--max-time", "30", "-F",
            &format!("file=@{}", path.display()),
        ])
        .arg("https://file.io")
        .output()
        .ok();
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
        if v["success"].as_bool() == Some(true) {
            if let Some(link) = v["link"].as_str() {
                sp.done("Upload complete");
                return Some(link.to_string());
            }
        }
    }

    sp.update("file.io unavailable, trying tmpfiles.org...");
    let out = std::process::Command::new("curl")
        .args([
            "-sSL", "--connect-timeout", "5", "--max-time", "30", "-F",
            &format!("file=@{}", path.display()),
        ])
        .arg("https://tmpfiles.org/api/v1/upload")
        .output()
        .ok();
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
        if v["status"].as_str() == Some("success") {
            if let Some(link) = v["data"]["url"].as_str() {
                sp.done("Upload complete");
                return Some(link.to_string());
            }
        }
    }

    sp.fail("Upload failed");
    None
}

fn copy_to_clipboard(text: &str) {
    for cmd in &["wl-copy", "xclip -selection clipboard", "pbcopy"] {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let mut child = std::process::Command::new(parts[0])
            .args(&parts[1..])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        if let Ok(ref mut c) = child {
            if c.stdin
                .as_mut()
                .unwrap()
                .write_all(text.as_bytes())
                .is_ok()
            {
                let _ = c.wait();
                return;
            }
        }
    }
}
