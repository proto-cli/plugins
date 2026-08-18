use proto_plugin_sdk::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let file = match args.first() {
        Some(f) => f.clone(),
        None => {
            eprintln!("  {} Usage: reader <FILE>", error(""));
            return;
        }
    };

    println!("{}", header("Reader"));
    println!("{}", divider());

    let content = match std::fs::read_to_string(&file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  {} Cannot read {}: {}", error(""), file, e);
            return;
        }
    };

    let mut line_count = 0;
    let mut in_code = false;

    for line in content.lines() {
        line_count += 1;

        let num_fmt = format!("{:>4} ", line_count);
        let num = num_fmt.style(Theme::MUTED);
        let t = line.trim();

        if t.starts_with("```") {
            in_code = !in_code;
            println!("{}{}", num, line.style(Theme::MUTED));
        } else if in_code {
            if t.starts_with("//") || t.starts_with('#') {
                println!("{}{}", num, line.style(Theme::MUTED));
            } else {
                println!("{}{}", num, line.style(Theme::VALUE));
            }
        } else if t.starts_with("# ") {
            println!("{}{}", num, line.style(Theme::HEADER));
        } else if t.starts_with("## ") {
            println!("{}{}", num, line.style(Theme::LABEL));
        } else if t.starts_with("- ") || t.starts_with("* ") {
            println!("{}{}", num, line.style(Theme::VALUE));
        } else if t.starts_with("> ") {
            println!("{}{}", num, line.style(Theme::MUTED));
        } else {
            println!("{}{}", num, line);
        }
    }

    println!();
    println!(
        "  {} {} lines",
        muted(""),
        line_count.style(Theme::VALUE)
    );
}
