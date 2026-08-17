use owo_colors::OwoColorize;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(text: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), text.style(Theme::HEADER))
}

fn divider() -> String {
    "─".repeat(40).style(Theme::MUTED).to_string()
}

fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}

fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut query = String::new();
    let mut source: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--source" => {
                if let Some(v) = args.get(i + 1) {
                    source = Some(v.clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                if query.is_empty() {
                    query = args[i].clone();
                } else {
                    query.push(' ');
                    query.push_str(&args[i]);
                }
                i += 1;
            }
        }
    }

    if query.is_empty() {
        eprintln!("  {} Usage: search-docs <QUERY> [--source SOURCE]", error(""));
        return;
    }

    println!("{}", header("Search Docs"));
    println!("{}", divider());

    let source = source.unwrap_or_else(|| "cheat.sh".to_string());
    let query_enc = query.trim().replace(' ', "+");
    let url = match source.as_str() {
        "tldr" => format!("https://tldr.sh/{}?format=raw", query_enc),
        "cheat" | "cheat.sh" | _ => format!("https://cheat.sh/{}?T", query_enc),
    };
    println!(
        "  {} {} {}\n",
        muted("Searching"),
        source.style(Theme::VALUE),
        format!("\"{}\"", query).style(Theme::VALUE)
    );

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(5))
        .timeout_read(std::time::Duration::from_secs(10))
        .build();

    match agent.get(&url).call() {
        Ok(resp) => {
            let body = resp.into_string().unwrap_or_else(|e| format!("Error: {}", e));
            for line in body.lines() {
                let t = line.trim();
                if t.starts_with('#') {
                    println!("  {}", t.style(Theme::HEADER));
                } else if t.starts_with('>') || t.starts_with("//") {
                    println!("  {}", t.style(Theme::MUTED));
                } else if t.starts_with("$ ") || t.starts_with("  ") {
                    println!("  {}", t.style(Theme::VALUE));
                } else if !t.is_empty() {
                    println!("  {}", t);
                } else {
                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("  {} Failed to fetch: {}", error(""), e);
        }
    }
    println!();
}
