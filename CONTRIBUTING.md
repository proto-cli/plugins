# Contributing to Proto CLI Plugins

## Creating a Plugin

1. Copy the `template/` directory
2. Rename it to your plugin name
3. Update `plugin.toml` with your plugin metadata
4. Implement your commands in `src/main.rs`
5. Submit a PR to this repository

## Plugin Manifest (plugin.toml)

```toml
[plugin]
name = "my-plugin"
scope = "proto"
version = "0.1.0"
description = "What your plugin does"
author = "your-username"
repository = "https://github.com/proto-cli/plugins/tree/main/my-plugin"

[plugin.commands]
# Maps proto subcommand names to binary paths
my-plugin = "bin/my-plugin"
```

## Plugin Binary Requirements

- Must be a standalone Rust binary
- Receives arguments via CLI args (same as any CLI tool)
- Should print output to stdout/stderr
- Should exit with appropriate exit codes

## Example

```rust
use std::env;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    
    match args.first().map(|s| s.as_str()) {
        Some("hello") => {
            let name = args.get(1).map(|s| s.as_str()).unwrap_or("world");
            println!("Hello, {}!", name);
        }
        Some("--help") | None => {
            println!("Usage: proto my-plugin <command> [args]");
            println!();
            println!("Commands:");
            println!("  hello [name]   Say hello");
            println!("  --help         Show this help");
        }
        Some(cmd) => {
            eprintln!("Unknown command: {}", cmd);
            std::process::exit(1);
        }
    }
}
```

## Building

```bash
cargo build --release
```

The binary will be at `target/release/<plugin-name>`.

## Publishing

Releases are automatically built via GitHub Actions when you push a tag:

```bash
git tag v0.1.0
git push --tags
```

This will create a release with pre-compiled binaries for Linux, macOS, and Windows.

## Code Style

- Use `clap` for argument parsing if your plugin has complex args
- Keep dependencies minimal
- Follow Rust standard conventions
- Handle errors gracefully with descriptive messages

## Testing

Test your plugin by installing it locally:

```bash
# Build your plugin
cd my-plugin && cargo build --release

# Create the plugin directory
mkdir -p ~/.config/proto/plugins/my-plugin/bin

# Copy the binary
cp target/release/my-plugin ~/.config/proto/plugins/my-plugin/bin/

# Create plugin.toml
cp plugin.toml ~/.config/proto/plugins/my-plugin/

# Test it
proto my-plugin --help
```
