use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
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

fn success(msg: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), msg)
}

fn warn(msg: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), msg)
}

fn label_value(label: &str, value: &str) -> String {
    format!(
        "{} {}",
        format!("{:>14}:", label).style(Theme::LABEL),
        value.style(Theme::VALUE)
    )
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Task {
    id: usize,
    text: String,
    done: bool,
}

fn todo_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("proto")
        .join("todos.json")
}

fn load() -> Vec<Task> {
    let path = todo_path();
    if !path.exists() {
        return Vec::new();
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&content).unwrap_or_default()
}

fn save(tasks: &[Task]) {
    let path = todo_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(tasks).unwrap_or_default();
    let _ = std::fs::write(&path, json);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let action = args.first().map(|s| s.as_str()).unwrap_or("list");
    let id: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let text: Vec<String> = if action == "add" {
        args[1..].to_vec()
    } else {
        Vec::new()
    };

    println!("{}", header("Todo"));
    println!("{}", divider());

    match action {
        "add" => {
            let text = text.join(" ");
            if text.is_empty() {
                println!("  {} Usage: todo add <task text>", muted(""));
                return;
            }
            let mut tasks = load();
            let new_id = tasks.last().map(|t| t.id + 1).unwrap_or(1);
            tasks.push(Task {
                id: new_id,
                text,
                done: false,
            });
            save(&tasks);
            println!(
                "  {} Added task #{}.",
                success(""),
                new_id.style(Theme::VALUE)
            );
        }
        "done" => {
            if id == 0 {
                println!("  {} Usage: todo done <ID>", muted(""));
                return;
            }
            let mut tasks = load();
            if let Some(t) = tasks.iter_mut().find(|t| t.id == id) {
                t.done = true;
                save(&tasks);
                println!(
                    "  {} Marked #{} as done.",
                    success(""),
                    id.style(Theme::VALUE)
                );
            } else {
                println!("  {} Task #{} not found.", warn(""), id);
            }
        }
        "remove" => {
            if id == 0 {
                println!("  {} Usage: todo remove <ID>", muted(""));
                return;
            }
            let mut tasks = load();
            if let Some(pos) = tasks.iter().position(|t| t.id == id) {
                tasks.remove(pos);
                save(&tasks);
                println!(
                    "  {} Removed task #{}.",
                    success(""),
                    id.style(Theme::VALUE)
                );
            } else {
                println!("  {} Task #{} not found.", warn(""), id);
            }
        }
        "list" | _ => {
            let tasks = load();
            if tasks.is_empty() {
                println!(
                    "  {} No tasks yet. Use {} to add one.",
                    muted(""),
                    "todo add <text>".style(Theme::VALUE)
                );
                return;
            }
            let pending: Vec<&Task> = tasks.iter().filter(|t| !t.done).collect();
            let completed: Vec<&Task> = tasks.iter().filter(|t| t.done).collect();

            if !pending.is_empty() {
                println!("  {}", label_value("Pending", ""));
                for t in &pending {
                    println!(
                        "    #{}  {}",
                        t.id.to_string().style(Theme::VALUE),
                        t.text
                    );
                }
                println!();
            }
            if !completed.is_empty() {
                println!("  {} Done:", muted(""));
                for t in &completed {
                    println!(
                        "    #{}  {} {}",
                        t.id.to_string().style(Theme::MUTED),
                        t.text.style(Theme::MUTED),
                        "✔".green()
                    );
                }
                println!();
            }
            println!(
                "  {} {} tasks ({} pending, {} done)",
                muted(""),
                tasks.len().style(Theme::VALUE),
                pending.len().style(Theme::VALUE),
                completed.len().style(Theme::MUTED)
            );
        }
    }
    println!();
}
