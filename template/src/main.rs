use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("hello") => {
            let name = args.get(1).map(|s| s.as_str()).unwrap_or("world");
            println!("Hello, {}!", name);
        }
        Some("version") => {
            println!(
                "template v{}",
                env!("CARGO_PKG_VERSION")
            );
        }
        Some("--help") | None => {
            print_help();
        }
        Some(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            eprintln!("Run 'proto template --help' for usage.");
            std::process::exit(1);
        }
    }
}

fn print_help() {
    println!("Template Plugin for Proto CLI");
    println!();
    println!("USAGE:");
    println!("  proto template <command> [args]");
    println!();
    println!("COMMANDS:");
    println!("  hello [name]    Say hello (default: world)");
    println!("  version         Show plugin version");
    println!("  --help          Show this help");
    println!();
    println!("EXAMPLES:");
    println!("  proto template hello");
    println!("  proto template hello Alice");
    println!("  proto template version");
}
