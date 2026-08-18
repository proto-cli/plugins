use clap::{Parser, Subcommand};
use proto_plugin_sdk::*;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ai", about = "AI-powered CLI assistant")]
struct Cli {
    #[command(subcommand)]
    action: AiAction,
}

#[derive(Subcommand, Debug, Clone)]
enum AiAction {
    #[command(about = "Start an interactive AI chat session")]
    Chat,
    #[command(about = "Configure AI provider, key, and personality")]
    Setup,
    #[command(about = "Summarize git log into a changelog")]
    Summarize {
        #[arg(value_name = "FROM", help = "Starting tag or commit")]
        from: Option<String>,
        #[arg(value_name = "TO", default_value = "HEAD", help = "Ending tag or commit")]
        to: String,
        #[arg(short, long, default_value = "CHANGELOG.md", value_name = "FILE")]
        output: String,
    },
    #[command(about = "Explain the last failed command using AI")]
    Explain,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AiConfig {
    provider: String,
    api_key: String,
    model: String,
    endpoint: Option<String>,
    personality: String,
    custom_prompt: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            api_key: String::new(),
            model: "gpt-4o-mini".into(),
            endpoint: None,
            personality: "engineer".into(),
            custom_prompt: None,
        }
    }
}

fn config_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".config/proto/ai.toml")
}

fn load_config() -> AiConfig {
    let path = config_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        AiConfig::default()
    }
}

fn personality_prompt(personality: &str, custom: Option<&str>) -> String {
    if personality == "custom" {
        return custom.unwrap_or("You are a helpful assistant.").to_string();
    }
    match personality {
        "furry" => "You are a cute protogen AI assistant. Be playful, use uwu occasionally, add :3 emotes, be friendly and warm. Keep answers helpful but adorable. Keep it short and use cute and silly slangs.. Dont repeat yourself too often.. be creative a lot. DONT USE EMOJIS".into(),
        "engineer" => "You are a precise software engineering assistant. Be technical, concise, and accurate. Use code examples. No fluff. Answer like a senior engineer code reviewing.".into(),
        "minimal" => "You are a terse assistant. Answer in the fewest words possible. No explanations unless asked. Use modern slangs and keep everything short and simple unless asked otherwise. Dont ask 'want me to explain thi?' User will ask if they want you to. DONT USE EMOJIS. keep it in slangs but dont be repeative. Use slangs like lol, tbh, icl (i cant lie), gng (gang/bro), vro instead of bro. Swear a little but in a little way.. Nothing crazy".into(),
        _ => "You are Proto, a helpful CLI assistant. Be friendly, concise, and practical. Use code blocks when showing commands.".into(),
    }
}

fn main() {
    let cli = Cli::parse();
    match &cli.action {
        AiAction::Chat => chat(),
        AiAction::Setup => setup(),
        AiAction::Summarize { from, to, output } => summarize(from.as_deref(), to, output),
        AiAction::Explain => explain(),
    }
}

fn proto_banner() -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), "Proto AI".style(Theme::HEADER))
}

fn setup() {
    use dialoguer::{Input, Password, Select};

    println!("{}", proto_banner());
    println!("{}\n", "AI Setup".style(Theme::HEADER));

    let providers = &["openai", "gemini", "custom  (Ollama, OpenRouter, vLLM, etc.)"];
    let prov_keys = &["openai", "gemini", "custom"];

    let prov_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("AI Provider")
        .items(providers)
        .default(0)
        .interact()
        .unwrap_or(0);

    let provider = prov_keys[prov_idx].to_string();

    let default_model = match provider.as_str() {
        "openai" => "gpt-4o-mini",
        "gemini" => "gemini-2.0-flash",
        "custom" => "llama3",
        _ => "gpt-4o-mini",
    };

    let endpoint = if provider == "custom" {
        let ep: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("API endpoint (OpenAI-compatible)")
            .default("http://localhost:11434/v1".into())
            .interact_text()
            .unwrap();
        Some(ep)
    } else {
        None
    };

    let api_key_hint = if provider == "custom" {
        "(or 'ollama' / leave blank)"
    } else {
        ""
    };
    let api_key: String = Password::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt(&format!("API Key {}", api_key_hint))
        .allow_empty_password(true)
        .interact()
        .unwrap_or_default();

    let model: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Model")
        .default(default_model.into())
        .interact_text()
        .unwrap();

    let personalities = &[
        "engineer  — technical, precise, code-focused",
        "helpful   — friendly, concise, practical",
        "furry     — cute protogen, uwu, playful :3",
        "minimal   — terse, few words",
        "custom    — define your own prompt",
    ];
    let pers_keys = &["engineer", "helpful", "furry", "minimal", "custom"];

    let pers_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Personality")
        .items(personalities)
        .default(0)
        .interact()
        .unwrap_or(0);

    let personality = pers_keys[pers_idx].to_string();

    let custom_prompt = if personality == "custom" {
        let cp: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Custom system prompt")
            .default("You are a helpful assistant.".into())
            .interact_text()
            .unwrap();
        Some(cp)
    } else {
        None
    };

    let config = AiConfig {
        provider,
        api_key,
        model,
        endpoint,
        personality,
        custom_prompt,
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();

    let dir = config_path().parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(config_path(), toml_str).unwrap();

    println!("\n{} AI configuration saved!", success(""));
    println!("  Try: {}", "proto ai".style(Theme::ACCENT));
}

fn chat() {
    let config = load_config();
    if config.api_key.is_empty() {
        eprintln!(
            "{} AI not configured. Run: {}",
            error(""),
            "proto ai setup".style(Theme::ACCENT)
        );
        return;
    }

    let system = personality_prompt(&config.personality, config.custom_prompt.as_deref());
    println!(
        "{} {}\n",
        "◆".style(Theme::ACCENT),
        format!("Proto AI ({})", config.model).style(Theme::HEADER)
    );
    println!("{} Type /quit to exit, /clear to reset context.", "  ".dimmed());
    println!("{}", divider());

    let mut messages: Vec<(String, String)> = vec![("system".into(), system)];

    loop {
        print!("{} ", "you ›".style(Theme::ACCENT).bold());
        let _ = std::io::stdout().flush();

        let stdin = std::io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line).unwrap();
        let input = line.trim().to_string();

        if input.is_empty() {
            continue;
        }
        if input == "/quit" || input == "/exit" {
            break;
        }
        if input == "/clear" {
            messages.truncate(1);
            println!("{} Context cleared.", "  ".dimmed());
            continue;
        }

        messages.push(("user".into(), input.clone()));

        print!("\n{} ", "bot ›".style(Theme::SUCCESS).bold());
        let _ = std::io::stdout().flush();

        let mut full_response = String::new();
        let result = call_ai_stream(&config, &messages, |token| {
            print!("{}", token);
            let _ = std::io::stdout().flush();
            full_response.push_str(token);
        });

        match result {
            Ok(_) => {
                println!("\n");
                messages.push(("assistant".into(), full_response));
            }
            Err(e) => {
                println!("\n{} {}\n", error(""), e);
            }
        }
    }
    println!("\n{} Goodbye! :3", "  ".dimmed());
}

fn summarize(from: Option<&str>, to: &str, output: &str) {
    let config = load_config();
    if config.api_key.is_empty() {
        eprintln!(
            "{} AI not configured. Run: {}",
            error(""),
            "proto ai setup".style(Theme::ACCENT)
        );
        return;
    }

    let sp = Spinner::new("Collecting git log...");

    let range = if let Some(f) = from {
        format!("{}..{}", f, to)
    } else {
        to.to_string()
    };
    let log = run_command_output("git", &["log", "--oneline", &range]).unwrap_or_default();

    if log.is_empty() {
        sp.fail("No commits found in range.");
        return;
    }

    let commit_count = log.lines().count();
    sp.update(&format!("Sending {} commits to AI...", commit_count));

    let system = "You are a changelog generator. Output a clean, well-formatted CHANGELOG.md in Keep a Changelog format. Group commits into Added, Changed, Fixed, Removed. Be concise. Only include the changelog, no preamble.".to_string();

    let prompt = format!(
        "Generate a changelog from these {} git commits:\n\n{}",
        commit_count, log
    );

    let messages = vec![("system".into(), system), ("user".into(), prompt)];

    match call_ai(&config, &messages) {
        Ok(response) => {
            std::fs::write(output, &response).unwrap();
            sp.done(&format!("Changelog written to {}", output));
            println!(
                "\n{}",
                response.lines().take(20).collect::<Vec<_>>().join("\n")
            );
            if response.lines().count() > 20 {
                println!("...");
            }
        }
        Err(e) => {
            sp.fail(&e);
        }
    }
}

fn explain() {
    let config = load_config();
    if config.api_key.is_empty() {
        eprintln!(
            "{} AI not configured. Run: {}",
            error(""),
            "proto ai setup".style(Theme::ACCENT)
        );
        return;
    }

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
    let shell_name = std::path::Path::new(&shell)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "sh".into());

    println!(
        "{} {} {}",
        "◆".style(Theme::ACCENT),
        "AI Shell Wrapper".style(Theme::HEADER),
        format!("({})", shell_name).style(Theme::MUTED)
    );
    println!(
        "{} Commands run in your {}. Errors get auto-explained.",
        "  ".dimmed(),
        shell_name.style(Theme::ACCENT)
    );
    println!("{} /quit to exit, /clear to clear screen.", "  ".dimmed());
    println!("{}", divider());

    let mut cwd = std::env::current_dir().unwrap_or_default();

    loop {
        let prompt = build_prompt(&cwd);
        print!("{}", prompt);
        let _ = std::io::stdout().flush();

        let stdin = std::io::stdin();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() {
            break;
        }
        let cmd = line.trim().to_string();

        if cmd.is_empty() {
            continue;
        }
        if cmd == "/quit" || cmd == "/exit" {
            break;
        }
        if cmd == "/clear" {
            print!("\x1b[2J\x1b[H");
            continue;
        }

        if cmd.starts_with("cd ") {
            let new_dir = cmd[3..].trim().trim_matches(&['"', '\''][..]);
            let target = if new_dir.starts_with('~') {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(&new_dir[1..].trim_start_matches('/'))
            } else if new_dir.starts_with('/') {
                std::path::PathBuf::from(new_dir)
            } else {
                cwd.join(new_dir)
            };
            if target.is_dir() {
                let _ = std::env::set_current_dir(&target);
                cwd = target;
            } else {
                eprintln!("{} Not a directory: {}", warn(""), target.display());
            }
            continue;
        }

        let output = std::process::Command::new(&shell)
            .arg("-c")
            .arg(&cmd)
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let exit_code = out.status.code().unwrap_or(-1);

                if !stdout.is_empty() {
                    print!("{}", stdout);
                }

                if exit_code == 0 {
                    if !stderr.is_empty() {
                        eprint!("{}", stderr.as_ref().dimmed());
                    }
                } else {
                    if !stderr.is_empty() {
                        eprint!("{}", stderr.as_ref().style(Theme::ERROR));
                    }

                    let hist_file = dirs::home_dir().unwrap_or_default().join(".proto_last_error");
                    let _ = std::fs::write(&hist_file, stderr.as_bytes());

                    let combined_err = if stderr.is_empty() {
                        format!("Exit code: {}", exit_code)
                    } else {
                        stderr.to_string()
                    };

                    println!(
                        "\n{} {}",
                        "  ".dimmed(),
                        format!("exit {}", exit_code)
                            .red()
                            .bold()
                            .to_string()
                    );
                    println!("{} Analyzing error...", "  ".dimmed());

                    let system = "You are a terminal error explainer. Given the stderr output from a failed shell command, explain what went wrong in plain English and suggest the most likely fix command. Be super concise — one or two sentences then the fix command in a code block. Do not ask questions or make conversation.".to_string();

                    let prompt = format!(
                        "Command: `{}`\n\nStderr:\n```\n{}\n```",
                        cmd, combined_err
                    );

                    let messages = vec![("system".into(), system), ("user".into(), prompt)];
                    match call_ai(&config, &messages) {
                        Ok(response) => {
                            println!(
                                "\n{} {}\n",
                                "▸".style(Theme::SUCCESS).bold(),
                                response.trim()
                            );
                        }
                        Err(e) => {
                            eprintln!("\n{} AI: {}", error(""), e);
                        }
                    }
                    println!("{}", divider());
                }
            }
            Err(e) => {
                eprintln!("{} Failed to run: {}", error(""), e);
            }
        }
    }

    println!("\n{} Shell session ended.", "  ".dimmed());
}

fn build_prompt(cwd: &std::path::Path) -> String {
    let home = dirs::home_dir().unwrap_or_default();

    let path_str = if let Ok(rel) = cwd.strip_prefix(&home) {
        format!("~/{}", rel.to_string_lossy())
    } else {
        cwd.to_string_lossy().to_string()
    };

    let mut prompt = path_str.style(Theme::ACCENT).bold().to_string();

    if let Some(branch) = git_branch(cwd) {
        prompt.push_str(&format!(
            " {}",
            format!("on  {}", branch).style(Theme::MUTED)
        ));
    }

    prompt.push_str(&format!(" {} ", "❯".cyan().bold()));
    prompt
}

fn git_branch(cwd: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

fn call_ai(config: &AiConfig, messages: &[(String, String)]) -> Result<String, String> {
    let mut full = String::new();
    call_ai_stream(config, messages, |token| {
        full.push_str(token);
    })?;
    Ok(full)
}

fn call_ai_stream(
    config: &AiConfig,
    messages: &[(String, String)],
    mut on_token: impl FnMut(&str),
) -> Result<String, String> {
    match config.provider.as_str() {
        "openai" | "custom" => call_openai_compat(config, messages, &config.provider, on_token),
        "gemini" => call_gemini_stream(config, messages, on_token),
        _ => Err(format!("Unknown provider: {}", config.provider)),
    }
}

fn call_openai_compat(
    config: &AiConfig,
    messages: &[(String, String)],
    provider: &str,
    mut on_token: impl FnMut(&str),
) -> Result<String, String> {
    let url = if provider == "custom" {
        let base = config
            .endpoint
            .as_deref()
            .unwrap_or("http://localhost:11434/v1")
            .trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        }
    } else {
        "https://api.openai.com/v1/chat/completions".into()
    };

    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .map(|(role, content)| serde_json::json!({"role": role, "content": content}))
        .collect();

    let body = serde_json::json!({
        "model": config.model,
        "messages": msgs,
        "temperature": 0.7,
        "max_tokens": 2048,
        "stream": true,
    });

    let req = ureq::post(&url).header("Content-Type", "application/json");
    let req = if provider == "custom" && (config.api_key.is_empty() || config.api_key == "ollama")
    {
        req
    } else {
        req.header("Authorization", &format!("Bearer {}", config.api_key))
    };

    let mut resp = req.send_json(&body).map_err(|e| format!("API error: {}", e))?;
    let status = resp.status();

    if status == 401 {
        return Err(
            "401 Unauthorized — your API key is missing or invalid. Run `proto ai setup` to reconfigure."
                .into(),
        );
    }
    if status == 404 {
        return Err(format!("404 Not Found — check your endpoint URL: {}", url));
    }
    if status != 200 {
        let body = resp.body_mut().read_to_string().unwrap_or_default();
        return Err(format!(
            "HTTP {} from {}: {}",
            status,
            url,
            body.chars().take(200).collect::<String>()
        ));
    }

    read_openai_sse(resp, on_token)
}

fn call_gemini_stream(
    config: &AiConfig,
    messages: &[(String, String)],
    mut on_token: impl FnMut(&str),
) -> Result<String, String> {
    let contents: Vec<serde_json::Value> = messages
        .iter()
        .filter(|(r, _)| r != "system")
        .map(|(role, content)| {
            let r = if role == "assistant" { "model" } else { "user" };
            serde_json::json!({"role": r, "parts": [{"text": content}]})
        })
        .collect();

    let system_instruction = messages
        .iter()
        .find(|(r, _)| r == "system")
        .map(|(_, c)| c.clone());

    let mut body = serde_json::json!({
        "contents": contents,
        "generationConfig": { "temperature": 0.7, "maxOutputTokens": 2048 }
    });

    if let Some(sys) = &system_instruction {
        body["systemInstruction"] = serde_json::json!({"parts": [{"text": sys}]});
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
        config.model, config.api_key
    );

    let mut resp = ureq::post(&url)
        .header("Content-Type", "application/json")
        .send_json(body)
        .map_err(|e| format!("Gemini API error: {}", e))?;

    let status = resp.status();
    if status == 403 {
        return Err(
            "403 Forbidden — your API key is invalid. Run `proto ai setup` to reconfigure.".into(),
        );
    }
    if status != 200 {
        let msg = resp.body_mut().read_to_string().unwrap_or_default();
        return Err(format!(
            "Gemini HTTP {}: {}",
            status,
            msg.chars().take(200).collect::<String>()
        ));
    }

    read_gemini_sse(resp, on_token)
}

fn read_openai_sse(resp: http::Response<ureq::Body>, mut on_token: impl FnMut(&str)) -> Result<String, String> {
    use std::io::Read;
    let mut full = String::new();
    let mut reader = resp.into_body().into_reader();
    let mut buf = [0u8; 4096];

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Stream read error: {}", e))?;
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buf[..n]);
        for line in chunk.lines() {
            let line = line.trim();
            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                        on_token(content);
                        full.push_str(content);
                    }
                }
            }
        }
    }
    Ok(full)
}

fn read_gemini_sse(
    resp: http::Response<ureq::Body>,
    mut on_token: impl FnMut(&str),
) -> Result<String, String> {
    use std::io::Read;
    let mut full = String::new();
    let mut reader = resp.into_body().into_reader();
    let mut buf = [0u8; 4096];

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Stream read error: {}", e))?;
        if n == 0 {
            break;
        }
        let chunk = String::from_utf8_lossy(&buf[..n]);
        for line in chunk.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(text) =
                        json["candidates"][0]["content"]["parts"][0]["text"].as_str()
                    {
                        on_token(text);
                        full.push_str(text);
                    }
                }
            }
        }
    }
    Ok(full)
}
