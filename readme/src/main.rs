use clap::Parser;
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
fn success(msg: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), msg)
}
fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}
fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}

#[derive(Parser)]
#[command(name = "readme", about = "Generate a README.md")]
struct Cli {
    /// Project name
    name: Option<String>,
    /// Short description
    #[arg(long)]
    desc: Option<String>,
    /// License (e.g. MIT, Apache-2.0)
    #[arg(long)]
    license: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    run(cli.name, cli.desc, cli.license);
}

fn run(name: Option<String>, desc: Option<String>, license: Option<String>) {
    println!("{}", header("Readme Init"));

    let name = name.unwrap_or_else(|| {
        dialoguer::Input::<String>::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Project name")
            .interact_text()
            .unwrap_or_else(|_| "my-project".to_string())
    });
    let desc = desc.unwrap_or_else(|| {
        dialoguer::Input::<String>::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Short description")
            .allow_empty(true)
            .interact_text()
            .unwrap_or_default()
    });
    let license = license.unwrap_or_else(|| {
        dialoguer::Input::<String>::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("License (e.g. MIT, Apache-2.0)")
            .default("MIT".to_string())
            .interact_text()
            .unwrap_or_else(|_| "MIT".to_string())
    });

    let md = format!(
        r#"# {name}

{desc}

## Install

```bash
git clone https://github.com/user/{name}.git
cd {name}
```

## Usage

```bash
# TODO: add usage
```

## License

{license}

"#,
        name = name,
        desc = if desc.is_empty() {
            "> TODO: describe the project".to_string()
        } else {
            desc
        },
        license = license,
    );

    let path = "README.md";
    let overwrite = std::path::Path::new(path).exists();
    if overwrite {
        let confirm =
            dialoguer::Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
                .with_prompt("README.md already exists. Overwrite?")
                .default(false)
                .interact()
                .unwrap_or(false);
        if !confirm {
            println!("  {} Cancelled.", muted(""));
            return;
        }
    }

    if let Err(e) = std::fs::write(path, &md) {
        eprintln!("  {} Failed to write: {}", error(""), e);
        return;
    }
    println!(
        "  {} Wrote {}",
        success(""),
        path.style(Theme::VALUE)
    );
}
