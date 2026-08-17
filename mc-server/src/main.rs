use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

// ─── Theme (inlined from proto style module) ────────────────────────────────

#[allow(dead_code)]
struct Theme;

#[allow(dead_code)]
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const BOLD: owo_colors::Style = owo_colors::Style::new().bold();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

// ─── Style helpers ──────────────────────────────────────────────────────────

#[allow(dead_code)]
fn header(text: &str) -> String {
    format!("{} {}", "◆".style(Theme::ACCENT), text.style(Theme::HEADER))
}

fn success(msg: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), msg)
}

fn warn(msg: &str) -> String {
    format!("{} {}", "⚠".style(Theme::WARN), msg)
}

fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}

#[allow(dead_code)]
fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}

fn divider() -> String {
    "─".repeat(40).dimmed().to_string()
}

fn label_value(label: &str, value: &str) -> String {
    format!(
        "{} {}",
        format!("{:>14}:", label).style(Theme::LABEL),
        value.style(Theme::VALUE)
    )
}

fn proto_banner() -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        "    ⣀⡀".cyan(),
        "⢠⣤⡀⣾⣿⣿⠀⣤⣤⡄".cyan(),
        "⢿⣿⡇⠘⠛⠁⢸⣿⣿⠃".cyan(),
        "⠈⣉⣤⣾⣿⣿⡆⠉⣴⣶⣶".cyan(),
        "⣾⣿⣿⣿⣿⣿⣿⡀⠻⠟⠃".cyan(),
        "⠙⠛⠻⢿⣿⣿⣿⡇".cyan(),
        "    ⠈⠙⠋⠁".cyan(),
    )
}

// ─── Spinner (inlined from proto style module) ──────────────────────────────

struct Spinner {
    pb: indicatif::ProgressBar,
}

impl Spinner {
    fn new(msg: &str) -> Self {
        let pb = indicatif::ProgressBar::new_spinner()
            .with_message(msg.to_string())
            .with_style(
                indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg}")
                    .unwrap()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
            );
        pb.enable_steady_tick(Duration::from_millis(80));
        Self { pb }
    }

    fn update(&self, msg: &str) {
        self.pb.set_message(msg.to_string());
    }

    fn done(&self, msg: &str) {
        self.pb.finish_with_message(msg.to_string());
    }

    fn fail(&self, msg: &str) {
        self.pb.finish_with_message(format!(
            "{} {}",
            "✗".style(Theme::ERROR),
            msg.style(Theme::ERROR)
        ));
    }
}

// ─── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "mc-server", about = "Minecraft server management plugin for proto")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Create a new Minecraft server with interactive setup")]
    Create,
    #[command(about = "Ping a Minecraft server to check if it's online")]
    Ping {
        #[arg(required = true, value_name = "IP[:PORT]")]
        ip: String,
    },
    #[command(about = "Show detailed status of a Minecraft server")]
    Status {
        #[arg(required = true, value_name = "IP[:PORT]")]
        ip: String,
    },
}

// ─── Main ───────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Create => create(),
        Commands::Ping { ref ip } => ping(ip),
        Commands::Status { ref ip } => status(ip),
    }
}

// ─── Paper API types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PaperVersions {
    versions: Vec<String>,
}

#[derive(Deserialize)]
struct PaperBuilds {
    builds: Vec<PaperBuild>,
}

#[derive(Deserialize)]
struct PaperBuild {
    build: u32,
    downloads: PaperDownloads,
}

#[derive(Deserialize)]
struct PaperDownloads {
    application: PaperApp,
}

#[derive(Deserialize)]
struct PaperApp {
    name: String,
}

// ─── Varint / SLP protocol helpers ──────────────────────────────────────────

fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        if (value & !0x7F) == 0 {
            buf.push(value as u8);
            break;
        }
        buf.push(((value & 0x7F) | 0x80) as u8);
        value = ((value as u32) >> 7) as i32;
    }
}

fn read_varint(data: &[u8]) -> (i32, usize) {
    let mut value = 0i32;
    let mut shift = 0;
    let mut i = 0;
    loop {
        let byte = data[i] as i32;
        value |= (byte & 0x7F) << shift;
        i += 1;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    (value, i)
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    write_varint(buf, s.len() as i32);
    buf.extend_from_slice(s.as_bytes());
}

fn read_varint_from_stream(stream: &mut TcpStream) -> Result<(i32, usize), ()> {
    let mut value = 0i32;
    let mut shift = 0;
    let mut bytes_read = 0;
    let mut buf = [0u8; 1];
    loop {
        stream.read_exact(&mut buf).map_err(|_| ())?;
        let byte = buf[0] as i32;
        value |= (byte & 0x7F) << shift;
        bytes_read += 1;
        if (byte & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    Ok((value, bytes_read))
}

fn send_slp_handshake(stream: &mut TcpStream, host: &str, port: u16) {
    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0x00);
    write_varint(&mut handshake, 767);
    write_string(&mut handshake, host);
    handshake.push(((port >> 8) & 0xFF) as u8);
    handshake.push((port & 0xFF) as u8);
    write_varint(&mut handshake, 1);

    let mut framed = Vec::new();
    write_varint(&mut framed, handshake.len() as i32);
    framed.extend_from_slice(&handshake);

    let mut request = Vec::new();
    write_varint(&mut request, 0x00);

    let mut req_framed = Vec::new();
    write_varint(&mut req_framed, request.len() as i32);
    req_framed.extend_from_slice(&request);

    let _ = stream.write_all(&framed);
    let _ = stream.write_all(&req_framed);
}

fn read_slp_response(stream: &mut TcpStream) -> Result<String, ()> {
    let (total_len, _) = read_varint_from_stream(stream)?;
    let mut packet = vec![0u8; total_len as usize];
    stream.read_exact(&mut packet).map_err(|_| ())?;

    let (packet_id, offset) = read_varint(&packet);
    if packet_id != 0 {
        return Err(());
    }

    let remaining = &packet[offset..];
    let (json_len, json_offset) = read_varint(remaining);
    let json_bytes = &remaining[json_offset..json_offset + json_len as usize];
    Ok(String::from_utf8_lossy(json_bytes).to_string())
}

// ─── IP parsing ─────────────────────────────────────────────────────────────

fn parse_ip(ip: &str) -> (String, u16) {
    if let Some((host, port_str)) = ip.rsplit_once(':') {
        if let Ok(port) = port_str.parse::<u16>() {
            return (host.to_string(), port);
        }
    }
    (ip.to_string(), 25565)
}

fn resolve_addr(host: &str, port: u16) -> Option<SocketAddr> {
    let addr_str = format!("{}:{}", host, port);
    addr_str.to_socket_addrs().ok()?.next()
}

// ─── Server info display ────────────────────────────────────────────────────

fn extract_motd(data: &serde_json::Value) -> String {
    if let Some(desc) = data["description"].as_str() {
        return desc.to_string();
    }
    if let Some(obj) = data["description"].as_object() {
        let mut parts = Vec::new();
        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
            parts.push(text.to_string());
        }
        if let Some(extra) = obj.get("extra").and_then(|v| v.as_array()) {
            for item in extra {
                if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                    parts.push(t.to_string());
                }
            }
        }
        return parts.join("");
    }
    String::new()
}

fn display_server_info(json_str: &str) {
    let data = match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(d) => d,
        Err(_) => {
            println!("\n{} {}", warn("Raw:"), json_str);
            return;
        }
    };

    println!(
        "\n{} {}\n{}",
        "◆".style(Theme::ACCENT),
        "Server Status".style(Theme::HEADER),
        divider()
    );

    let motd = extract_motd(&data);
    if !motd.is_empty() {
        println!("{}", label_value("MOTD", &motd));
    }

    if let Some(ver) = data["version"]["name"].as_str() {
        println!("{}", label_value("Version", ver));
    }
    if let Some(proto) = data["version"]["protocol"].as_u64() {
        println!("{}", label_value("Protocol", &proto.to_string()));
    }

    if let Some(players) = data["players"].as_object() {
        let online = players["online"].as_u64().unwrap_or(0);
        let max = players["max"].as_u64().unwrap_or(0);
        println!(
            "{}",
            label_value("Players", &format!("{}/{}", online, max))
        );

        if let Some(sample) = players.get("sample") {
            if let Some(arr) = sample.as_array() {
                if !arr.is_empty() {
                    println!("\n{}", "Online:".style(Theme::HEADER));
                    for p in arr.iter().take(15) {
                        println!(
                            "  {} {}",
                            "▶".style(Theme::ACCENT),
                            p["name"].as_str().unwrap_or("?")
                        );
                    }
                    if arr.len() > 15 {
                        println!("  ... and {} more", arr.len() - 15);
                    }
                }
            }
        }
    }

    if let Some(fav) = data.get("favicon") {
        if fav
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        {
            println!("{}", label_value("Favicon", "yes"));
        }
    }

    if let Some(modinfo) = data.get("modinfo") {
        if let Some(mods) = modinfo.get("modList") {
            if let Some(arr) = mods.as_array() {
                if !arr.is_empty() {
                    println!(
                        "\n{} ({})",
                        "Mods:".style(Theme::HEADER),
                        arr.len()
                    );
                    for m in arr.iter().take(10) {
                        println!(
                            "  {} {}",
                            "▶".style(Theme::ACCENT),
                            m["modid"].as_str().unwrap_or("?")
                        );
                    }
                }
            }
        }
    }

    println!("{}", divider());
}

// ─── Download with progress ─────────────────────────────────────────────────

fn download_with_progress(url: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    let resp = ureq::get(url)
        .set("User-Agent", "ProtoCLI/0.1.0")
        .call()
        .map_err(|e| format!("HTTP error: {}", e))?;

    let mut reader = resp.into_reader();
    let mut file =
        std::fs::File::create(dest).map_err(|e| format!("File create error: {}", e))?;

    let mut buf = [0u8; 65536];
    let mut total: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Read error: {}", e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("Write error: {}", e))?;
        total += n as u64;
        if total % (5 * 1_048_576) < 65536 {
            sp.update(&format!(
                "Downloading... ({:.1} MB)",
                total as f64 / 1_048_576.0
            ));
        }
    }
    Ok(())
}

// ─── RAM detection ──────────────────────────────────────────────────────────

fn detect_ram() -> String {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_memory();
    let total_gb = sys.total_memory() / 1_073_741_824;
    if total_gb >= 16 {
        "4G".into()
    } else if total_gb >= 8 {
        "2G".into()
    } else {
        "1G".into()
    }
}

// ─── Paper latest version ───────────────────────────────────────────────────

fn fetch_latest_paper_version() -> Result<String, String> {
    let resp = ureq::get("https://api.papermc.io/v2/projects/paper")
        .set("User-Agent", "ProtoCLI/0.1.0")
        .call()
        .map_err(|e| format!("Paper API error: {}", e))?;
    let data: PaperVersions = resp
        .into_json()
        .map_err(|e| format!("Paper API parse error: {}", e))?;
    data.versions
        .last()
        .cloned()
        .ok_or_else(|| "No versions available".into())
}

// ─── Server file writers ────────────────────────────────────────────────────

fn write_server_properties(
    path: &Path,
    _name: String,
    online: bool,
    whitelist: bool,
    motd: String,
    world_type: &str,
    max_players: &str,
    difficulty: &str,
    game_mode: &str,
    pvp: bool,
    spawn_protection: &str,
    view_distance: &str,
    rcon: bool,
    rcon_pass: &str,
) {
    let whitelist_str = if whitelist { "true" } else { "false" };
    let rcon_str = if rcon { "true" } else { "false" };

    let props = format!(
        "motd={}\nonline-mode={}\nwhite-list={}\nlevel-type={}\n\
         max-players={}\ndifficulty={}\ngamemode={}\npvp={}\n\
         spawn-protection={}\nview-distance={}\n\
         enable-rcon={}\nrcon.password={}\n\
         enable-command-block=true\nallow-flight=false\n\
         max-world-size=29999984\nnetwork-compression-threshold=256\n\
         op-permission-level=4\nprevent-proxy-connections=false\n\
         server-ip=\nserver-port=25565\nsnooper-enabled=true\n\
         use-native-transport=true\n",
        motd, online, whitelist_str, world_type, max_players, difficulty,
        game_mode, pvp, spawn_protection, view_distance, rcon_str, rcon_pass,
    );

    std::fs::write(path.join("server.properties"), props).unwrap();
}

fn accept_eula(path: &Path) {
    std::fs::write(path.join("eula.txt"), "eula=true\n").unwrap();
}

fn write_start_script(path: &Path, jar: &str, loader: &str) {
    let ram = detect_ram();
    let _ = loader;
    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
JAR="{}"
RAM="{}"
ARGS="--nogui"

start() {{
    echo "◆ Starting server (${{RAM}} RAM)..."
    java -Xmx${{RAM}} -Xms${{RAM//G/M}} -jar "${{JAR}}" ${{ARGS}}
}}

logs() {{
    if [ -f logs/latest.log ]; then
        tail -f logs/latest.log
    else
        echo "No logs found."
    fi
}}

restart() {{
    echo "◆ Restarting in 5 seconds..."
    sleep 5
    exec "$0" start
}}

console() {{
    echo "◆ Opening console (type 'stop' to exit)..."
    java -Xmx${{RAM}} -Xms${{RAM//G/M}} -jar "${{JAR}}" --nogui
}}

whitelist() {{
    case "${{1:-}}" in
        add)    shift; rcon "whitelist add $*" ;;
        remove) shift; rcon "whitelist remove $*" ;;
        list)   rcon "whitelist list" ;;
        on)     rcon "whitelist on" ;;
        off)    rcon "whitelist off" ;;
        *)      echo "Usage: whitelist {{add|remove|list|on|off}}" ;;
    esac
}}

rcon() {{
    local pass
    pass=$(grep 'rcon.password=' server.properties 2>/dev/null | cut -d= -f2 || echo "")
    if [ -n "$pass" ]; then
        echo "$*" | rcon-cli --password "$pass" 2>/dev/null || echo "Install rcon-cli for remote commands"
    else
        echo "RCON not configured. Set rcon.password in server.properties"
    fi
}}

case "${{1:-start}}" in
    start)     start ;;
    logs)      logs ;;
    restart)   restart ;;
    reboot)    restart ;;
    console)   console ;;
    whitelist) shift; whitelist "$@" ;;
    stop)      rcon "stop" ;;
    *)         echo "Usage: $0 {{start|logs|restart|console|whitelist|stop}}" ;;
esac
"#,
        jar, ram
    );

    std::fs::write(path.join("start.sh"), script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(
            path.join("start.sh"),
            std::fs::Permissions::from_mode(0o755),
        );
    }
}

// ─── Server jar downloads ───────────────────────────────────────────────────

fn download_server_jar(
    loader: &str,
    version: &str,
    dest: &Path,
    sp: &Spinner,
) -> Result<(), String> {
    match loader {
        "vanilla" => download_vanilla_jar(version, dest, sp),
        "paper" => download_paper_jar(version, dest, sp),
        "fabric" => download_fabric_jar(version, dest, sp),
        _ => Err(format!(
            "Automatic download for {} is not supported yet.",
            loader
        )),
    }
}

fn download_vanilla_jar(version: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    let manifest_url = "https://piston-meta.mojang.com/mc/game/version_manifest.json";
    let resp = ureq::get(manifest_url)
        .set("User-Agent", "ProtoCLI/0.1.0")
        .call()
        .map_err(|e| format!("Failed to fetch version manifest: {}", e))?;

    #[derive(Deserialize)]
    struct Vm {
        versions: Vec<VmEntry>,
    }
    #[derive(Deserialize)]
    struct VmEntry {
        id: String,
        url: String,
    }

    let vm: Vm = resp
        .into_json()
        .map_err(|e| format!("Parse error: {}", e))?;
    let entry = vm
        .versions
        .iter()
        .find(|v| v.id == version)
        .ok_or_else(|| format!("Version {} not found", version))?;

    #[derive(Deserialize)]
    struct Vi {
        downloads: ViDl,
    }
    #[derive(Deserialize)]
    struct ViDl {
        server: Option<ViSrv>,
    }
    #[derive(Deserialize)]
    struct ViSrv {
        url: String,
        #[allow(dead_code)]
        size: u64,
    }

    let vi: Vi = ureq::get(&entry.url)
        .set("User-Agent", "ProtoCLI/0.1.0")
        .call()
        .map_err(|e| format!("Version info error: {}", e))?
        .into_json()
        .map_err(|e| format!("Version parse error: {}", e))?;

    let url = &vi
        .downloads
        .server
        .ok_or_else(|| "No server download for this version".to_string())?
        .url;

    sp.update("Downloading vanilla server...");
    download_with_progress(url, dest, sp).map_err(|e| format!("Download failed: {}", e))?;
    Ok(())
}

fn download_paper_jar(version: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    let builds_url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{}/builds",
        version
    );
    let resp = ureq::get(&builds_url)
        .set("User-Agent", "ProtoCLI/0.1.0")
        .call()
        .map_err(|e| format!("Paper API error: {}", e))?;

    let data: PaperBuilds = resp
        .into_json()
        .map_err(|e| format!("Paper parse error: {}", e))?;
    let build = data
        .builds
        .last()
        .ok_or_else(|| format!("No builds found for version {}", version))?;

    let jar_name = &build.downloads.application.name;
    let url = format!(
        "https://api.papermc.io/v2/projects/paper/versions/{}/builds/{}/downloads/{}",
        version, build.build, jar_name
    );

    sp.update(&format!("Downloading Paper {}...", version));
    download_with_progress(&url, dest, sp).map_err(|e| format!("Download failed: {}", e))?;
    Ok(())
}

fn download_fabric_jar(version: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    sp.update("Fetching Fabric versions...");

    #[derive(Deserialize)]
    struct FabricLoader {
        loader: FabricLoaderVersion,
    }
    #[derive(Deserialize)]
    struct FabricLoaderVersion {
        version: String,
    }

    let loader: Vec<FabricLoader> = ureq::get("https://meta.fabricmc.net/v2/versions/loader")
        .set("User-Agent", "ProtoCLI/0.1.0")
        .call()
        .map_err(|e| format!("Fabric loader error: {}", e))?
        .into_json()
        .map_err(|e| format!("Fabric loader parse error: {}", e))?;

    let loader_ver = &loader
        .first()
        .ok_or("No Fabric loader versions")?
        .loader
        .version;

    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/server/jar",
        version, loader_ver
    );

    sp.update(&format!("Downloading Fabric server {}...", version));
    download_with_progress(&url, dest, sp).map_err(|e| format!("Download failed: {}", e))?;
    Ok(())
}

// ─── Create command ─────────────────────────────────────────────────────────

fn create() {
    use dialoguer::{Confirm, Input, Select};

    println!("{}", proto_banner());
    println!(
        "{}\n",
        "Minecraft Server Creator".style(Theme::HEADER)
    );

    let loaders = &[
        "vanilla (Mojang official)",
        "paper (optimized Bukkit fork)",
        "fabric (mod loader)",
        "forge (mod loader - instructions only)",
        "spigot (instructions only)",
        "neoforge (instructions only)",
        "pumpkinmc (Rust server)",
    ];

    let loader_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select server loader")
        .items(loaders)
        .default(1)
        .interact()
        .unwrap_or(0);

    let loader = loaders[loader_idx]
        .split_whitespace()
        .next()
        .unwrap_or("vanilla");
    let needs_manual = matches!(loader, "forge" | "spigot" | "neoforge" | "pumpkinmc");

    let default_version = match loader {
        "vanilla" | "fabric" | "forge" | "spigot" | "neoforge" => "1.21.1".to_string(),
        "paper" => fetch_latest_paper_version().unwrap_or_else(|_| "1.21.1".to_string()),
        _ => "1.21.1".to_string(),
    };

    let version: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Minecraft version")
        .default(default_version)
        .interact_text()
        .unwrap();

    let server_name: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Server name")
        .default("My Proto Server".to_string())
        .interact_text()
        .unwrap();

    let folder_name = server_name.replace(' ', "_").to_lowercase();
    println!(
        "\n{} {}\n",
        "◆".style(Theme::ACCENT),
        "Server Configuration".style(Theme::HEADER)
    );

    let online_mode = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Online mode? (cracked = false)")
        .default(true)
        .interact()
        .unwrap_or(true);

    let whitelist = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Enable whitelist?")
        .default(false)
        .interact()
        .unwrap_or(false);

    let motd: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Server MOTD")
        .default("A Proto-powered Minecraft server!".to_string())
        .interact_text()
        .unwrap();

    let world_types = &["default", "flat", "amplified", "largebiomes"];
    let world_type_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("World type")
        .items(world_types)
        .default(0)
        .interact()
        .unwrap_or(0);

    let max_players: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Max players")
        .default("20".to_string())
        .interact_text()
        .unwrap();

    let difficulties = &["peaceful", "easy", "normal", "hard"];
    let diff_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Difficulty")
        .items(difficulties)
        .default(2)
        .interact()
        .unwrap_or(2);

    let game_modes = &["survival", "creative", "adventure", "spectator"];
    let gm_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Default game mode")
        .items(game_modes)
        .default(0)
        .interact()
        .unwrap_or(0);

    let pvp = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Enable PvP?")
        .default(true)
        .interact()
        .unwrap_or(true);

    let spawn_protection: String =
        Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Spawn protection radius (0 = disabled)")
            .default("16".to_string())
            .interact_text()
            .unwrap();

    let view_distance: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("View distance")
        .default("10".to_string())
        .interact_text()
        .unwrap();

    let rcon_password: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("RCON password (empty = disabled)")
        .allow_empty(true)
        .default(String::new())
        .interact_text()
        .unwrap();

    let has_rcon = !rcon_password.is_empty();

    println!();
    println!("{}", divider());
    println!(
        "  Server: {}",
        server_name.style(Theme::ACCENT)
    );
    println!(
        "  Loader: {} {}",
        loader.style(Theme::ACCENT),
        version.style(Theme::MUTED)
    );
    println!(
        "  MOTD:   {}",
        motd.style(Theme::MUTED)
    );
    println!(
        "  Dir:    {}",
        folder_name.style(Theme::MUTED)
    );
    println!("{}", divider());
    println!();

    let proceed = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Create server with these settings?")
        .default(true)
        .interact()
        .unwrap_or(true);

    if !proceed {
        println!("{}", "Aborted.".style(Theme::MUTED));
        return;
    }

    let sp = Spinner::new("Creating server directory...");
    let path = PathBuf::from(&folder_name);

    if path.exists() {
        sp.fail(&format!("Directory '{}' already exists.", folder_name));
        return;
    }
    std::fs::create_dir_all(&path).unwrap();

    if needs_manual {
        sp.done("Server directory ready");
        write_server_properties(
            &path,
            server_name,
            online_mode,
            whitelist,
            motd,
            world_types[world_type_idx],
            &max_players,
            difficulties[diff_idx],
            game_modes[gm_idx],
            pvp,
            &spawn_protection,
            &view_distance,
            has_rcon,
            &rcon_password,
        );
        accept_eula(&path);
        write_start_script(&path, "server.jar", loader);
        println!(
            "\n{}",
            warn(&format!("{} server must be set up manually.", loader))
        );
        println!(
            "  Download the {} server jar for version {} and place it as 'server.jar' in the folder.",
            loader, version
        );
        println!("  Then run: cd {} && ./start.sh", folder_name);
        return;
    }

    sp.update("Downloading server jar...");
    let jar_dest = path.join("server.jar");

    match download_server_jar(loader, &version, &jar_dest, &sp) {
        Ok(_) => {
            sp.done(&format!("Downloaded {}.jar", loader));
        }
        Err(e) => {
            sp.fail(&e);
            let _ = std::fs::remove_dir_all(&path);
            return;
        }
    }

    write_server_properties(
        &path,
        server_name,
        online_mode,
        whitelist,
        motd,
        world_types[world_type_idx],
        &max_players,
        difficulties[diff_idx],
        game_modes[gm_idx],
        pvp,
        &spawn_protection,
        &view_distance,
        has_rcon,
        &rcon_password,
    );
    accept_eula(&path);
    write_start_script(&path, "server.jar", loader);

    println!();
    println!("{}", success("Server created successfully!"));
    println!();
    println!(
        "  {}",
        format!("cd {}", folder_name).style(Theme::ACCENT)
    );
    println!(
        "  {}",
        "./start.sh".style(Theme::ACCENT)
    );
    println!();
    println!(
        "  {}  start  logs  restart  reboot  whitelist  console",
        "Commands:".style(Theme::MUTED)
    );
}

// ─── Ping command ───────────────────────────────────────────────────────────

fn ping(ip: &str) {
    let (host, port) = parse_ip(ip);

    let sp = Spinner::new(&format!("Pinging {}:{}...", host, port));

    let addr = match resolve_addr(&host, port) {
        Some(a) => a,
        None => {
            sp.fail(&format!("Could not resolve {}", host));
            return;
        }
    };

    match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(_) => {
            sp.done(&format!(
                "{}:{}",
                host.style(Theme::SUCCESS).bold(),
                port
            ));
            println!(
                "\n{} {}:{} is {}",
                success(""),
                host.style(Theme::ACCENT),
                port,
                "ONLINE".green().bold()
            );
        }
        Err(_) => {
            sp.fail(&format!(
                "{}:{}",
                host.style(Theme::ERROR),
                port
            ));
            println!(
                "\n{} {}:{} is {}",
                error(""),
                host.style(Theme::ACCENT),
                port,
                "OFFLINE".red().bold()
            );
        }
    }
}

// ─── Status command ─────────────────────────────────────────────────────────

fn status(ip: &str) {
    let (host, port) = parse_ip(ip);

    let sp = Spinner::new(&format!("Querying {}:{}...", host, port));

    let addr = match resolve_addr(&host, port) {
        Some(a) => a,
        None => {
            sp.fail(&format!("Could not resolve {}", host));
            return;
        }
    };

    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(s) => s,
        Err(_) => {
            sp.fail(&format!("{} is offline", host));
            println!(
                "\n{} {}:{} is {}",
                error(""),
                host.style(Theme::ACCENT),
                port,
                "OFFLINE".red().bold()
            );
            return;
        }
    };

    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

    send_slp_handshake(&mut stream, &host, port);

    match read_slp_response(&mut stream) {
        Ok(json_str) => {
            sp.done(&format!(
                "{}:{}",
                host.style(Theme::SUCCESS).bold(),
                port
            ));
            display_server_info(&json_str);
        }
        Err(_) => {
            sp.fail("Server did not respond with valid data");
            println!(
                "\n{} Server may not support SLP or is not a Minecraft server.",
                warn("")
            );
        }
    }
}
