use clap::{Subcommand, Parser};
use owo_colors::OwoColorize;

struct Theme;
impl Theme {
    const HEADER: &'static str = "cyan";
    const ACCENT: &'static str = "magenta";
    const SUCCESS: &'static str = "green";
    const ERROR: &'static str = "red";
    const MUTED: &'static str = "dark_grey";
    const WARN: &'static str = "yellow";
    const LABEL: &'static str = "cyan";
    const VALUE: &'static str = "bright_white";
}
fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(40).dimmed().to_string() }
fn label_value(label: &str, value: &str) -> String {
    format!("{} {}", format!("{:>14}:", label).style(Theme::LABEL), value.style(Theme::VALUE))
}

#[derive(Parser)]
#[command(name = "encrypt", about = "Crypto operations — base64, hex, hash, uuid, bcrypt")]
struct Cli {
    #[command(subcommand)]
    action: EncryptAction,
}

#[derive(Subcommand)]
enum EncryptAction {
    Base64 { #[command(subcommand)] action: CodecAction },
    Hex { #[command(subcommand)] action: CodecAction },
    Hash { algo: String, text: String },
    Uuid,
    Bcrypt { password: String, #[arg(short, long, default_value = "12")] rounds: u32 },
}

#[derive(Subcommand)]
enum CodecAction {
    Encode { text: String },
    Decode { text: String },
}

fn main() {
    let cli = Cli::parse();
    match cli.action {
        EncryptAction::Base64 { action } => b64(&action),
        EncryptAction::Hex { action } => hex_cmd(&action),
        EncryptAction::Hash { algo, text } => hash(&algo, &text),
        EncryptAction::Uuid => gen_uuid(),
        EncryptAction::Bcrypt { password, rounds } => do_bcrypt(&password, rounds),
    }
}

fn b64(action: &CodecAction) {
    use base64::Engine;
    match action {
        CodecAction::Encode { text } => { let out = base64::engine::general_purpose::STANDARD.encode(text.as_bytes()); println!("{}", label_value("Base64", &out)); }
        CodecAction::Decode { text } => { match base64::engine::general_purpose::STANDARD.decode(text.trim()) { Ok(bytes) => println!("{}", label_value("Decoded", &String::from_utf8_lossy(&bytes))), Err(e) => eprintln!("{} Invalid base64: {}", error(""), e) } }
    }
}

fn hex_cmd(action: &CodecAction) {
    match action {
        CodecAction::Encode { text } => { let hex: String = text.as_bytes().iter().map(|b| format!("{:02x}", b)).collect(); println!("{}", label_value("Hex", &hex)); }
        CodecAction::Decode { text } => {
            let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            if cleaned.len() % 2 != 0 { eprintln!("{} Invalid hex length", error("")); return; }
            let bytes: Vec<u8> = (0..cleaned.len()).step_by(2).filter_map(|i| u8::from_str_radix(&cleaned[i..i+2], 16).ok()).collect();
            if bytes.len() * 2 == cleaned.len() { println!("{}", label_value("Decoded", &String::from_utf8_lossy(&bytes))); }
            else { eprintln!("{} Invalid hex string", error("")); }
        }
    }
}

fn hash(algo: &str, text: &str) {
    use sha2::{Sha256, Sha512, Digest}; use sha1::Sha1; use md5::Md5;
    let (name, output): (String, String) = match algo.to_lowercase().as_str() {
        "md5" => { let mut h = Md5::new(); h.update(text.as_bytes()); ("MD5".into(), format!("{:x}", h.finalize())) }
        "sha1" => { let mut h = Sha1::new(); h.update(text.as_bytes()); ("SHA-1".into(), format!("{:x}", h.finalize())) }
        "sha256" => { let mut h = Sha256::new(); h.update(text.as_bytes()); ("SHA-256".into(), format!("{:x}", h.finalize())) }
        "sha512" => { let mut h = Sha512::new(); h.update(text.as_bytes()); ("SHA-512".into(), format!("{:x}", h.finalize())) }
        _ => { eprintln!("{} Unknown algorithm: '{}'. Use: md5, sha1, sha256, sha512", error(""), algo); return; }
    };
    println!("{}", label_value(&name, &output));
}

fn gen_uuid() { let id = uuid::Uuid::new_v4(); println!("{}", label_value("UUID v4", &id.to_string())); }

fn do_bcrypt(password: &str, rounds: u32) {
    println!("  Hashing with bcrypt ({} rounds)...", rounds);
    match bcrypt::hash(password, rounds) { Ok(hash) => println!("\n{}", label_value("Bcrypt", &hash)), Err(e) => eprintln!("{} Bcrypt error: {}", error(""), e) }
}
