use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

// ─── Theme ──────────────────────────────────────────────────────────────────

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const LABEL: owo_colors::Style = owo_colors::Style::new().bright_cyan();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn warn(s: &str) -> String { format!("{} {}", "⚠".style(Theme::WARN), s) }
fn divider() -> String { "─".repeat(50).dimmed().to_string() }
fn label_value(label: &str, value: &str) -> String {
    format!("  {} {}", format!("{}:", label).style(Theme::LABEL), value.style(Theme::VALUE))
}

struct Spinner {
    spinner: indicatif::ProgressBar,
}
impl Spinner {
    fn new(msg: &str) -> Self {
        let sp = indicatif::ProgressBar::new_spinner().with_message(msg.to_string())
            .with_style(indicatif::ProgressStyle::with_template("{spinner:.cyan} {msg}").unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]));
        sp.enable_steady_tick(Duration::from_millis(80));
        Self { spinner: sp }
    }
    fn done(&self, msg: &str) { self.spinner.finish_with_message(msg.to_string()); }
    fn fail(&self, msg: &str) {
        self.spinner.finish_with_message(format!("{} {}", "✗".style(Theme::ERROR), msg.style(Theme::ERROR)));
    }
    fn update(&self, msg: &str) { self.spinner.set_message(msg.to_string()); }
}

// ─── CLI ────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "mc", about = "Minecraft resource packs & server management")]
struct Cli {
    #[command(subcommand)]
    action: McAction,
}

#[derive(Subcommand, Debug, Clone)]
enum McAction {
    #[command(name = "resource_pack", about = "Minecraft resource pack utilities")]
    ResourcePack {
        #[command(subcommand)]
        action: ResourcePackAction,
    },
    #[command(name = "server", about = "Minecraft server management")]
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum ResourcePackAction {
    #[command(about = "Create a new resource pack")]
    Create {
        #[arg(long, default_value = "1.21.1", value_name = "VERSION")]
        version: String,
        #[arg(long, default_value = "Resource Pack", value_name = "NAME")]
        name: String,
        #[arg(long, default_value = "true", value_name = "BOOL", action = clap::ArgAction::Set, value_parser = clap::value_parser!(bool))]
        clean: bool,
    },
    #[command(about = "Fetch available Minecraft versions and stats")]
    Fetch,
    #[command(about = "Pack current folder into a resource pack zip")]
    Pack {
        #[arg(long, default_value = "true", value_name = "BOOL", action = clap::ArgAction::Set, value_parser = clap::value_parser!(bool))]
        branding: bool,
    },
    #[command(about = "Add an item, block, or entity asset to the pack")]
    Add {
        #[arg(required = true, value_name = "CATEGORY")]
        category: String,
        #[arg(required = true, value_name = "NAME")]
        name: String,
        #[arg(long, value_name = "PATH", help = "Path to a PNG texture to use")]
        png: Option<String>,
        #[arg(long, value_name = "N", help = "Texture resolution (default: 16 for 16x16)")]
        resolution: Option<u32>,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum ServerAction {
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
    match &cli.action {
        McAction::ResourcePack { action } => match action {
            ResourcePackAction::Create { version, name, clean } => rp_create(version, name, *clean),
            ResourcePackAction::Fetch => rp_fetch(),
            ResourcePackAction::Pack { branding } => rp_pack(*branding),
            ResourcePackAction::Add { category, name, png, resolution } => {
                let cwd = std::env::current_dir().unwrap_or_default();
                rp_add(&cwd, category, name, png.as_deref(), *resolution)
            }
        },
        McAction::Server { action } => match action {
            ServerAction::Create => srv_create(),
            ServerAction::Ping { ip } => srv_ping(ip),
            ServerAction::Status { ip } => srv_status(ip),
        },
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// RESOURCE PACK
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
struct VersionManifest { versions: Vec<VersionEntry> }
#[derive(Deserialize)]
struct VersionEntry {
    id: String,
    #[serde(rename = "type")]
    release_type: String,
    #[serde(rename = "releaseTime")]
    release_time: String,
    url: String,
}
#[derive(Deserialize)]
struct VersionInfo { downloads: Option<Downloads>, asset_index: Option<AssetIndex> }
#[derive(Deserialize)]
struct Downloads { client: Option<ClientDownload> }
#[derive(Deserialize)]
struct ClientDownload { url: String, size: u64 }
#[derive(Deserialize)]
struct AssetIndex { url: String }

fn rp_create(version: &str, name: &str, clean: bool) {
    let folder_name = name.replace(' ', "_").to_lowercase();
    let path = PathBuf::from(&folder_name);

    if path.exists() { eprintln!("{} Directory '{}' already exists.", error(""), folder_name); return; }

    let sp = Spinner::new(&format!("Creating resource pack '{}'...", name));
    if let Err(e) = std::fs::create_dir_all(&path) { sp.fail(&format!("Failed: {}", e)); return; }

    rp_write_pack_mcmeta(&path, name);
    if !clean { sp.update("Fetching version manifest..."); rp_download_assets(&path, version, &sp); }

    sp.done(&format!("Created '{}' (Minecraft {})", name, version));
    println!(); println!("{}", success(&format!("Resource pack '{}' ready!", name)));
    println!("  {}", format!("cd {}", folder_name).style(Theme::MUTED));
    println!("  {}", "proto mc resource_pack pack".style(Theme::MUTED));
}

fn rp_write_pack_mcmeta(path: &Path, name: &str) {
    let mcmeta = format!(r#"{{"pack":{{"pack_format":0,"description":"{}"}}}}"#, name);
    std::fs::write(path.join("pack.mcmeta"), mcmeta).unwrap();
}

fn rp_download_assets(path: &Path, version: &str, sp: &Spinner) {
    let vm = rp_fetch_version_manifest();
    let entry = vm.versions.iter().find(|v| v.id == version);
    let entry = match entry {
        Some(e) => e,
        None => { sp.fail(&format!("Version '{}' not found. Use 'fetch' to see available versions.", version)); std::process::exit(1); }
    };
    sp.update(&format!("Fetching version info for {}...", version));
    let info = rp_fetch_json::<VersionInfo>(&entry.url);
    let client_url = info.downloads.and_then(|d| d.client).map(|c| c.url);
    let client_url = match client_url {
        Some(u) => u,
        None => { sp.fail("No client download found for this version."); return; }
    };
    let temp_dir = std::env::temp_dir().join(format!("proto_mc_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let jar_path = temp_dir.join("client.jar");
    sp.update("Downloading client.jar...");
    rp_download_file(&client_url, &jar_path, sp);
    sp.update("Extracting assets from client.jar...");
    rp_extract_assets(&jar_path, path, sp);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn rp_fetch() {
    let sp = Spinner::new("Fetching version manifest...");
    let vm = rp_fetch_version_manifest();
    sp.done("Fetched version manifest");

    let release_count = vm.versions.iter().filter(|v| v.release_type == "release").count();
    let snapshot_count = vm.versions.iter().filter(|v| v.release_type == "snapshot").count();

    println!("\n{}", "Minecraft Versions".style(Theme::HEADER));
    println!("{}", label_value("Releases", &release_count.to_string()));
    println!("{}", label_value("Snapshots", &snapshot_count.to_string()));
    println!("{}", label_value("Total", &vm.versions.len().to_string()));
    println!("{}", label_value("Latest release", &vm.versions.iter().find(|v| v.release_type == "release").map(|v| v.id.as_str()).unwrap_or("N/A")));
    println!("{}", label_value("Latest snapshot", &vm.versions.iter().find(|v| v.release_type == "snapshot").map(|v| v.id.as_str()).unwrap_or("N/A")));

    println!("\n{}", "Recent Versions".style(Theme::HEADER));
    println!("{}", divider());
    for v in vm.versions.iter().take(15) {
        let type_icon = match v.release_type.as_str() { "release" => "●".green().to_string(), "snapshot" => "◉".yellow().to_string(), _ => "○".dimmed().to_string() };
        let date = &v.release_time[..10.min(v.release_time.len())];
        println!("  {} {:16} {}", type_icon, v.id.style(Theme::ACCENT), date.dimmed().to_string());
    }
    println!("{}", divider());
    println!("\n{}", "--clean false  to download full assets for a version".style(Theme::MUTED));
}

fn rp_pack(branding: bool) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mcmeta = cwd.join("pack.mcmeta");
    if !mcmeta.exists() { eprintln!("{} No pack.mcmeta found. Run this inside a resource pack folder.", error("")); return; }

    let folder_name = cwd.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "resource_pack".to_string());
    let zip_name = format!("{}.zip", folder_name);
    let sp = Spinner::new(&format!("Packing '{}'...", folder_name));
    if branding { sp.update("Adding proto branding..."); rp_add_branding(&mcmeta); }
    let output_path = cwd.parent().unwrap_or(&cwd).join(&zip_name);
    let _ = std::fs::remove_file(&output_path);
    sp.update("Creating archive...");
    let status = std::process::Command::new("zip").arg("-r").arg("-q").arg(output_path.to_string_lossy().as_ref()).arg(".").arg("-x").arg(&zip_name).arg("-x").arg("*.zip").current_dir(&cwd).status();
    match status { Ok(s) if s.success() => { sp.done(&format!("Packed: {}", output_path.to_string_lossy())); println!("{}", success("Resource pack ready to import!")); }, _ => { sp.fail("zip command failed. Is 'zip' installed?"); } }
}

fn rp_add_branding(mcmeta: &Path) {
    let content = std::fs::read_to_string(mcmeta).unwrap_or_default();
    if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
        if let Some(pack) = val.get_mut("pack") {
            if let Some(desc) = pack.get("description").and_then(|d| d.as_str()) {
                let branded = format!("{}  |  Made with Proto CLI ✦", desc);
                if let Some(obj) = pack.as_object_mut() { obj.insert("description".into(), serde_json::Value::String(branded)); }
            }
        }
        let _ = std::fs::write(mcmeta, serde_json::to_string_pretty(&val).unwrap_or(content));
    }
}

fn rp_fetch_version_manifest() -> VersionManifest {
    ureq::get("https://piston-meta.mojang.com/mc/game/version_manifest.json")
        .set("User-Agent", "ProtoCLI/0.2.0").call().expect("Failed to fetch version manifest")
        .into_json::<VersionManifest>().expect("Failed to parse version manifest")
}

fn rp_fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> T {
        ureq::get(url).set("User-Agent", "ProtoCLI/0.2.0").call().unwrap_or_else(|e| panic!("Failed to fetch {}: {}", url, e))
        .into_json::<T>().unwrap_or_else(|e| panic!("Failed to parse JSON from {}: {}", url, e))
}

fn rp_download_file(url: &str, dest: &Path, sp: &Spinner) {
    let resp = ureq::get(url).set("User-Agent", "ProtoCLI/0.2.0").call().expect("Failed to download file");
    let total = resp.header("Content-Length").and_then(|s| s.parse::<u64>().ok());
    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).expect("Failed to create temp file");
    let mut buf = [0u8; 8192];
    let mut downloaded: u64 = 0;
    loop {
        let n = std::io::Read::read(&mut reader, &mut buf).expect("Read error");
        if n == 0 { break; }
        std::io::Write::write_all(&mut file, &buf[..n]).expect("Write error");
        downloaded += n as u64;
        if let Some(t) = total { sp.update(&format!("Downloading client.jar ({:.1} / {:.1} MB)...", downloaded as f64 / 1_048_576.0, t as f64 / 1_048_576.0)); }
    }
}

fn rp_extract_assets(jar_path: &Path, dest: &Path, sp: &Spinner) {
    let file = std::fs::File::open(jar_path).expect("Failed to open client.jar");
    let mut archive = zip::ZipArchive::new(file).expect("Failed to read client.jar as zip");
    let assets_prefix = "assets/minecraft/";
    let assets_base = dest.join("assets").join("minecraft");
    std::fs::create_dir_all(&assets_base).unwrap();
    let mut count = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().to_string();
        if name.starts_with(assets_prefix) && !entry.is_dir() {
            let relative = name.strip_prefix(assets_prefix).unwrap();
            let out_path = assets_base.join(relative);
            if let Some(parent) = out_path.parent() { std::fs::create_dir_all(parent).unwrap(); }
            let mut out = std::fs::File::create(&out_path).unwrap();
            std::io::copy(&mut entry, &mut out).unwrap();
            count += 1;
            if count % 50 == 0 { sp.update(&format!("Extracting assets... ({} files)", count)); }
        }
    }
    sp.update(&format!("Extracted {} asset files", count));
}

fn rp_add(root: &Path, category: &str, name: &str, png_path: Option<&str>, resolution: Option<u32>) {
    let mcmeta = root.join("pack.mcmeta");
    if !mcmeta.exists() {
        eprintln!("{} No pack.mcmeta found. Run this inside a resource pack folder.", error(""));
        eprintln!("{} Create one with: {} {}", warn(""), "proto mc resource_pack create --name".style(Theme::ACCENT), "\"My Pack\"".style(Theme::ACCENT));
        return;
    }
    let safe_name = name.to_lowercase().replace([' ', '-'], "_");
    if safe_name.is_empty() { eprintln!("{} Invalid name.", error("")); return; }
    let sp = Spinner::new(&format!("Adding {} '{}'...", category, name));
    let tex_data = match png_path {
        Some(path) => { sp.update(&format!("Reading texture from {}...", path)); match std::fs::read(path) { Ok(data) => { if data.len() >= 8 && &data[..8] == b"\x89PNG\r\n\x1a\n" { Some(data) } else { sp.fail(&format!("{} is not a valid PNG", path)); return; } } Err(e) => { sp.fail(&format!("Cannot read {}: {}", path, e)); return; } } },
        None => None,
    };
    let tex_resolution = resolution.unwrap_or(16);
    let results = match category.to_lowercase().as_str() {
        "item" => rp_add_item(root, &safe_name, &tex_data, tex_resolution),
        "block" => rp_add_block(root, &safe_name, &tex_data, tex_resolution),
        "entity" => rp_add_entity(root, &safe_name, &tex_data, tex_resolution),
        "armor" => rp_add_armor(root, &safe_name, &tex_data, tex_resolution),
        "armor_layer" | "armour_layer" => rp_add_armor_layer(root, &safe_name, &tex_data, tex_resolution),
        "gui" => rp_add_gui(root, &safe_name, &tex_data, tex_resolution),
        "particle" => rp_add_particle(root, &safe_name, &tex_data, tex_resolution),
        "environment" | "env" => rp_add_environment(root, &safe_name, &tex_data, tex_resolution),
        _ => { sp.fail(&format!("Unknown category: '{}'", category)); eprintln!("\n{} Categories: item, block, entity, armor, armor_layer, gui, particle, environment", warn("")); return; }
    };
    match results {
        Ok(files) => {
            let res_info = if png_path.is_some() { "custom" } else { &format!("{}x{}", tex_resolution, tex_resolution) };
            sp.done(&format!("Added {} '{}' ({})", category, name, res_info));
            println!(); for f in files { println!("  {} {}", "✦".style(Theme::SUCCESS), f.style(Theme::MUTED)); }
        }
        Err(e) => { sp.fail(&e); }
    }
}

fn rp_write_texture(target: &Path, tex_data: &Option<Vec<u8>>, resolution: u32) -> std::io::Result<bool> {
    if target.exists() { return Ok(false); }
    let data = match tex_data { Some(d) => d.clone(), None => rp_generate_blank_png(resolution, resolution) };
    std::fs::write(target, &data)?; Ok(true)
}

fn rp_generate_blank_png(width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
    let mut ihdr_data = Vec::new();
    ihdr_data.extend_from_slice(&width.to_be_bytes());
    ihdr_data.extend_from_slice(&height.to_be_bytes());
    ihdr_data.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut ihdr_chunk = b"IHDR".to_vec();
    ihdr_chunk.extend_from_slice(&ihdr_data);
    let ihdr_crc = crc32fast::hash(&ihdr_chunk);
    out.extend_from_slice(&(ihdr_data.len() as u32).to_be_bytes());
    out.extend_from_slice(b"IHDR");
    out.extend_from_slice(&ihdr_data);
    out.extend_from_slice(&ihdr_crc.to_be_bytes());
    let row_size = 1 + (width as usize) * 4;
    let raw_size = (height as usize) * row_size;
    let raw: Vec<u8> = vec![0u8; raw_size];
    let mut zlib_data = Vec::new();
    { use flate2::write::ZlibEncoder; use flate2::Compression; use std::io::Write; let mut enc = ZlibEncoder::new(&mut zlib_data, Compression::best()); enc.write_all(&raw).unwrap(); enc.finish().unwrap(); }
    let mut idat_chunk = b"IDAT".to_vec();
    idat_chunk.extend_from_slice(&zlib_data);
    let idat_crc = crc32fast::hash(&idat_chunk);
    out.extend_from_slice(&(zlib_data.len() as u32).to_be_bytes());
    out.extend_from_slice(b"IDAT");
    out.extend_from_slice(&zlib_data);
    out.extend_from_slice(&idat_crc.to_be_bytes());
    let iend_crc = crc32fast::hash(b"IEND");
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(b"IEND");
    out.extend_from_slice(&iend_crc.to_be_bytes());
    out
}

fn rp_add_item(root: &Path, name: &str, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let mut created = Vec::new();
    let models_dir = root.join("assets/minecraft/models/item");
    std::fs::create_dir_all(&models_dir).map_err(|e| format!("mkdir: {}", e))?;
    let model = format!(r#"{{"parent":"minecraft:item/generated","textures":{{"layer0":"minecraft:item/{}"}}}}"#, name);
    let model_path = models_dir.join(format!("{}.json", name));
    std::fs::write(&model_path, &model).map_err(|e| format!("write: {}", e))?;
    created.push(format!("assets/minecraft/models/item/{}.json", name));
    let tex_dir = root.join("assets/minecraft/textures/item");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if rp_write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { created.push(format!("assets/minecraft/textures/item/{}.png", name)); }
    Ok(created)
}

fn rp_add_block(root: &Path, name: &str, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let mut created = Vec::new();
    let models_dir = root.join("assets/minecraft/models/block");
    std::fs::create_dir_all(&models_dir).map_err(|e| format!("mkdir: {}", e))?;
    let model = format!(r#"{{"parent":"minecraft:block/cube_all","textures":{{"all":"minecraft:block/{}"}}}}"#, name);
    let model_path = models_dir.join(format!("{}.json", name));
    std::fs::write(&model_path, &model).map_err(|e| format!("write: {}", e))?;
    created.push(format!("assets/minecraft/models/block/{}.json", name));
    let item_model = format!(r#"{{"parent":"minecraft:block/{}"}}"#, name);
    let item_dir = root.join("assets/minecraft/models/item");
    std::fs::create_dir_all(&item_dir).map_err(|e| format!("mkdir: {}", e))?;
    let item_path = item_dir.join(format!("{}.json", name));
    std::fs::write(&item_path, &item_model).map_err(|e| format!("write: {}", e))?;
    created.push(format!("assets/minecraft/models/item/{}.json", name));
    let blockstates_dir = root.join("assets/minecraft/blockstates");
    std::fs::create_dir_all(&blockstates_dir).map_err(|e| format!("mkdir: {}", e))?;
    let bs = format!(r#"{{"variants":{{"":{{"model":"minecraft:block/{}"}}}}}}"#, name);
    let bs_path = blockstates_dir.join(format!("{}.json", name));
    std::fs::write(&bs_path, bs).map_err(|e| format!("write: {}", e))?;
    created.push(format!("assets/minecraft/blockstates/{}.json", name));
    let tex_dir = root.join("assets/minecraft/textures/block");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if rp_write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { created.push(format!("assets/minecraft/textures/block/{}.png", name)); }
    Ok(created)
}

fn rp_add_entity(root: &Path, name: &str, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/entity");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if rp_write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { Ok(vec![format!("assets/minecraft/textures/entity/{}.png", name)]) } else { Ok(vec![]) }
}

fn rp_add_armor(root: &Path, name: &str, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let mut created = Vec::new();
    let tex_dir = root.join("assets/minecraft/textures/models/armor");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    for layer in &[1, 2] {
        let fname = format!("{}_layer_{}.png", name, layer);
        let tex_path = tex_dir.join(&fname);
        if rp_write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { created.push(format!("assets/minecraft/textures/models/armor/{}", fname)); }
    }
    Ok(created)
}

fn rp_add_armor_layer(root: &Path, name: &str, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/models/armor");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if rp_write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { return Ok(vec![format!("assets/minecraft/textures/models/armor/{}.png", name)]); }
    Ok(vec![])
}

fn rp_add_gui(root: &Path, name: &str, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/gui");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if rp_write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { return Ok(vec![format!("assets/minecraft/textures/gui/{}.png", name)]); }
    Ok(vec![])
}

fn rp_add_particle(root: &Path, name: &str, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/particle");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if rp_write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { return Ok(vec![format!("assets/minecraft/textures/particle/{}.png", name)]); }
    Ok(vec![])
}

fn rp_add_environment(root: &Path, name: &str, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/environment");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if rp_write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { return Ok(vec![format!("assets/minecraft/textures/environment/{}.png", name)]); }
    Ok(vec![])
}

// ═════════════════════════════════════════════════════════════════════════════
// SERVER
// ═════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
struct PaperVersions { versions: Vec<String> }
#[derive(Deserialize)]
struct PaperBuilds { builds: Vec<PaperBuild> }
#[derive(Deserialize)]
struct PaperBuild { build: u32, downloads: PaperDownloads }
#[derive(Deserialize)]
struct PaperDownloads { application: PaperApp }
#[derive(Deserialize)]
struct PaperApp { name: String }

// ─── Varint / SLP protocol ──────────────────────────────────────────────────

fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        if (value & !0x7F) == 0 { buf.push(value as u8); break; }
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
        if (byte & 0x80) == 0 { break; }
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
        if (byte & 0x80) == 0 { break; }
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
    if packet_id != 0 { return Err(()); }

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
    if let Some(desc) = data["description"].as_str() { return desc.to_string(); }
    if let Some(obj) = data["description"].as_object() {
        let mut parts = Vec::new();
        if let Some(text) = obj.get("text").and_then(|v| v.as_str()) { parts.push(text.to_string()); }
        if let Some(extra) = obj.get("extra").and_then(|v| v.as_array()) {
            for item in extra {
                if let Some(t) = item.get("text").and_then(|v| v.as_str()) { parts.push(t.to_string()); }
            }
        }
        return parts.join("");
    }
    String::new()
}

fn display_server_info(json_str: &str) {
    let data = match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(d) => d,
        Err(_) => { println!("\n{} {}", warn("Raw:"), json_str); return; }
    };

    println!("\n{} {}\n{}", "◆".style(Theme::ACCENT), "Server Status".style(Theme::HEADER), divider());

    let motd = extract_motd(&data);
    if !motd.is_empty() { println!("{}", label_value("MOTD", &motd)); }

    if let Some(ver) = data["version"]["name"].as_str() { println!("{}", label_value("Version", ver)); }
    if let Some(proto) = data["version"]["protocol"].as_u64() { println!("{}", label_value("Protocol", &proto.to_string())); }

    if let Some(players) = data["players"].as_object() {
        let online = players["online"].as_u64().unwrap_or(0);
        let max = players["max"].as_u64().unwrap_or(0);
        println!("{}", label_value("Players", &format!("{}/{}", online, max)));

        if let Some(sample) = players.get("sample") {
            if let Some(arr) = sample.as_array() {
                if !arr.is_empty() {
                    println!("\n{}", "Online:".style(Theme::HEADER));
                    for p in arr.iter().take(15) {
                        println!("  {} {}", "▶".style(Theme::ACCENT), p["name"].as_str().unwrap_or("?"));
                    }
                    if arr.len() > 15 { println!("  ... and {} more", arr.len() - 15); }
                }
            }
        }
    }

    if let Some(fav) = data.get("favicon") {
        if fav.as_str().map(|s| !s.is_empty()).unwrap_or(false) { println!("{}", label_value("Favicon", "yes")); }
    }

    if let Some(modinfo) = data.get("modinfo") {
        if let Some(mods) = modinfo.get("modList") {
            if let Some(arr) = mods.as_array() {
                if !arr.is_empty() {
                    println!("\n{} ({})", "Mods:".style(Theme::HEADER), arr.len());
                    for m in arr.iter().take(10) {
                        println!("  {} {}", "▶".style(Theme::ACCENT), m["modid"].as_str().unwrap_or("?"));
                    }
                }
            }
        }
    }

    println!("{}", divider());
}

// ─── Download with progress ─────────────────────────────────────────────────

fn srv_download_with_progress(url: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    let resp = ureq::get(url)
        .set("User-Agent", "ProtoCLI/0.2.0")
        .call()
        .map_err(|e| format!("HTTP error: {}", e))?;

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| format!("File create error: {}", e))?;

    let mut buf = [0u8; 65536];
    let mut total: u64 = 0;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Read error: {}", e))?;
        if n == 0 { break; }
        file.write_all(&buf[..n]).map_err(|e| format!("Write error: {}", e))?;
        total += n as u64;
        if total % (5 * 1_048_576) < 65536 {
            sp.update(&format!("Downloading... ({:.1} MB)", total as f64 / 1_048_576.0));
        }
    }
    Ok(())
}

// ─── RAM detection ──────────────────────────────────────────────────────────

fn detect_ram() -> String {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_memory();
    let total_gb = sys.total_memory() / 1_073_741_824;
    if total_gb >= 16 { "4G".into() } else if total_gb >= 8 { "2G".into() } else { "1G".into() }
}

// ─── Paper latest version ───────────────────────────────────────────────────

fn fetch_latest_paper_version() -> Result<String, String> {
    let resp = ureq::get("https://api.papermc.io/v2/projects/paper")
        .set("User-Agent", "ProtoCLI/0.2.0")
        .call()
        .map_err(|e| format!("Paper API error: {}", e))?;
    let data: PaperVersions = resp.into_json().map_err(|e| format!("Paper API parse error: {}", e))?;
    data.versions.last().cloned().ok_or_else(|| "No versions available".into())
}

// ─── Server file writers ────────────────────────────────────────────────────

fn write_server_properties(path: &Path, _name: String, online: bool, whitelist: bool, motd: String, world_type: &str, max_players: &str, difficulty: &str, game_mode: &str, pvp: bool, spawn_protection: &str, view_distance: &str, rcon: bool, rcon_pass: &str) {
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

fn write_start_script(path: &Path, jar: &str, _loader: &str) {
    let ram = detect_ram();
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
    if [ -f logs/latest.log ]; then tail -f logs/latest.log; else echo "No logs found."; fi
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
        let _ = std::fs::set_permissions(path.join("start.sh"), std::fs::Permissions::from_mode(0o755));
    }
}

// ─── Server jar downloads ───────────────────────────────────────────────────

fn download_server_jar(loader: &str, version: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    match loader {
        "vanilla" => download_vanilla_jar(version, dest, sp),
        "paper" => download_paper_jar(version, dest, sp),
        "fabric" => download_fabric_jar(version, dest, sp),
        _ => Err(format!("Automatic download for {} is not supported yet.", loader)),
    }
}

fn download_vanilla_jar(version: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    let manifest_url = "https://piston-meta.mojang.com/mc/game/version_manifest.json";
    let resp = ureq::get(manifest_url).set("User-Agent", "ProtoCLI/0.2.0").call()
        .map_err(|e| format!("Failed to fetch version manifest: {}", e))?;

    #[derive(Deserialize)]
    struct Vm { versions: Vec<VmEntry> }
    #[derive(Deserialize)]
    struct VmEntry { id: String, url: String }

    let vm: Vm = resp.into_json().map_err(|e| format!("Parse error: {}", e))?;
    let entry = vm.versions.iter().find(|v| v.id == version)
        .ok_or_else(|| format!("Version {} not found", version))?;

    #[derive(Deserialize)]
    struct Vi { downloads: ViDl }
    #[derive(Deserialize)]
    struct ViDl { server: Option<ViSrv> }
    #[derive(Deserialize)]
    struct ViSrv { url: String, #[allow(dead_code)] size: u64 }

    let vi: Vi = ureq::get(&entry.url).set("User-Agent", "ProtoCLI/0.2.0").call()
        .map_err(|e| format!("Version info error: {}", e))?
        .into_json().map_err(|e| format!("Version parse error: {}", e))?;

    let url = &vi.downloads.server.ok_or_else(|| "No server download for this version".to_string())?.url;
    sp.update("Downloading vanilla server...");
    srv_download_with_progress(url, dest, sp).map_err(|e| format!("Download failed: {}", e))?;
    Ok(())
}

fn download_paper_jar(version: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    let builds_url = format!("https://api.papermc.io/v2/projects/paper/versions/{}/builds", version);
    let resp = ureq::get(&builds_url).set("User-Agent", "ProtoCLI/0.2.0").call()
        .map_err(|e| format!("Paper API error: {}", e))?;
    let data: PaperBuilds = resp.into_json().map_err(|e| format!("Paper parse error: {}", e))?;
    let build = data.builds.last().ok_or_else(|| format!("No builds found for version {}", version))?;
    let jar_name = &build.downloads.application.name;
    let url = format!("https://api.papermc.io/v2/projects/paper/versions/{}/builds/{}/downloads/{}", version, build.build, jar_name);
    sp.update(&format!("Downloading Paper {}...", version));
    srv_download_with_progress(&url, dest, sp).map_err(|e| format!("Download failed: {}", e))?;
    Ok(())
}

fn download_fabric_jar(version: &str, dest: &Path, sp: &Spinner) -> Result<(), String> {
    sp.update("Fetching Fabric versions...");

    #[derive(Deserialize)]
    struct FabricLoader { loader: FabricLoaderVersion }
    #[derive(Deserialize)]
    struct FabricLoaderVersion { version: String }

    let loader: Vec<FabricLoader> = ureq::get("https://meta.fabricmc.net/v2/versions/loader")
        .set("User-Agent", "ProtoCLI/0.2.0").call()
        .map_err(|e| format!("Fabric loader error: {}", e))?
        .into_json().map_err(|e| format!("Fabric loader parse error: {}", e))?;

    let loader_ver = &loader.first().ok_or("No Fabric loader versions")?.loader.version;
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{}/{}/server/jar", version, loader_ver);
    sp.update(&format!("Downloading Fabric server {}...", version));
    srv_download_with_progress(&url, dest, sp).map_err(|e| format!("Download failed: {}", e))?;
    Ok(())
}

// ─── Server commands ────────────────────────────────────────────────────────

fn srv_create() {
    use dialoguer::{Confirm, Input, Select};

    println!("{}", "    ⣀⡀".cyan());
    println!("{}", "⢠⣤⡀⣾⣿⣿⠀⣤⣤⡄".cyan());
    println!("{}", "⢿⣿⡇⠘⠛⠁⢸⣿⣿⠃".cyan());
    println!("{}", "⠈⣉⣤⣾⣿⣿⡆⠉⣴⣶⣶".cyan());
    println!("{}", "⣾⣿⣿⣿⣿⣿⣿⡀⠻⠟⠃".cyan());
    println!("{}", "⠙⠛⠻⢿⣿⣿⣿⡇".cyan());
    println!("{}", "    ⠈⠙⠋⠁".cyan());
    println!();
    println!("{}", "Minecraft Server Creator".style(Theme::HEADER));
    println!();

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

    let loader = loaders[loader_idx].split_whitespace().next().unwrap_or("vanilla");
    let needs_manual = matches!(loader, "forge" | "spigot" | "neoforge" | "pumpkinmc");

    let default_version = match loader {
        "vanilla" | "fabric" | "forge" | "spigot" | "neoforge" => "1.21.1".to_string(),
        "paper" => fetch_latest_paper_version().unwrap_or_else(|_| "1.21.1".to_string()),
        _ => "1.21.1".to_string(),
    };

    let version: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Minecraft version").default(default_version).interact_text().unwrap();
    let server_name: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Server name").default("My Proto Server".to_string()).interact_text().unwrap();
    let folder_name = server_name.replace(' ', "_").to_lowercase();

    println!("\n{} {}\n", "◆".style(Theme::ACCENT), "Server Configuration".style(Theme::HEADER));

    let online_mode = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Online mode? (cracked = false)").default(true).interact().unwrap_or(true);
    let whitelist = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Enable whitelist?").default(false).interact().unwrap_or(false);
    let motd: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Server MOTD").default("A Proto-powered Minecraft server!".to_string()).interact_text().unwrap();

    let world_types = &["default", "flat", "amplified", "largebiomes"];
    let world_type_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("World type").items(world_types).default(0).interact().unwrap_or(0);
    let max_players: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Max players").default("20".to_string()).interact_text().unwrap();

    let difficulties = &["peaceful", "easy", "normal", "hard"];
    let diff_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Difficulty").items(difficulties).default(2).interact().unwrap_or(2);
    let game_modes = &["survival", "creative", "adventure", "spectator"];
    let gm_idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Default game mode").items(game_modes).default(0).interact().unwrap_or(0);

    let pvp = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Enable PvP?").default(true).interact().unwrap_or(true);
    let spawn_protection: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Spawn protection radius (0 = disabled)").default("16".to_string()).interact_text().unwrap();
    let view_distance: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("View distance").default("10".to_string()).interact_text().unwrap();
    let rcon_password: String = Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("RCON password (empty = disabled)").allow_empty(true).default(String::new()).interact_text().unwrap();
    let has_rcon = !rcon_password.is_empty();

    println!();
    println!("{}", divider());
    println!("  Server: {}", server_name.style(Theme::ACCENT));
    println!("  Loader: {} {}", loader.style(Theme::ACCENT), version.style(Theme::MUTED));
    println!("  MOTD:   {}", motd.style(Theme::MUTED));
    println!("  Dir:    {}", folder_name.style(Theme::MUTED));
    println!("{}", divider());
    println!();

    let proceed = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Create server with these settings?").default(true).interact().unwrap_or(true);
    if !proceed { println!("{}", "Aborted.".style(Theme::MUTED)); return; }

    let sp = Spinner::new("Creating server directory...");
    let path = PathBuf::from(&folder_name);
    if path.exists() { sp.fail(&format!("Directory '{}' already exists.", folder_name)); return; }
    std::fs::create_dir_all(&path).unwrap();

    if needs_manual {
        sp.done("Server directory ready");
        write_server_properties(&path, server_name, online_mode, whitelist, motd, world_types[world_type_idx], &max_players, difficulties[diff_idx], game_modes[gm_idx], pvp, &spawn_protection, &view_distance, has_rcon, &rcon_password);
        accept_eula(&path);
        write_start_script(&path, "server.jar", loader);
        println!("\n{}", warn(&format!("{} server must be set up manually.", loader)));
        println!("  Download the {} server jar for version {} and place it as 'server.jar' in the folder.", loader, version);
        println!("  Then run: cd {} && ./start.sh", folder_name);
        return;
    }

    sp.update("Downloading server jar...");
    let jar_dest = path.join("server.jar");
    match download_server_jar(loader, &version, &jar_dest, &sp) {
        Ok(_) => { sp.done(&format!("Downloaded {}.jar", loader)); }
        Err(e) => { sp.fail(&e); let _ = std::fs::remove_dir_all(&path); return; }
    }

    write_server_properties(&path, server_name, online_mode, whitelist, motd, world_types[world_type_idx], &max_players, difficulties[diff_idx], game_modes[gm_idx], pvp, &spawn_protection, &view_distance, has_rcon, &rcon_password);
    accept_eula(&path);
    write_start_script(&path, "server.jar", loader);

    println!();
    println!("{}", success("Server created successfully!"));
    println!();
    println!("  {}", format!("cd {}", folder_name).style(Theme::ACCENT));
    println!("  {}", "./start.sh".style(Theme::ACCENT));
    println!();
    println!("  {}  start  logs  restart  reboot  whitelist  console", "Commands:".style(Theme::MUTED));
}

fn srv_ping(ip: &str) {
    let (host, port) = parse_ip(ip);
    let sp = Spinner::new(&format!("Pinging {}:{}...", host, port));
    let addr = match resolve_addr(&host, port) {
        Some(a) => a,
        None => { sp.fail(&format!("Could not resolve {}", host)); return; }
    };
    match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(_) => {
            sp.done(&format!("{}:{}", host.style(Theme::SUCCESS).bold(), port));
            println!("\n{} {}:{} is {}", success(""), host.style(Theme::ACCENT), port, "ONLINE".green().bold());
        }
        Err(_) => {
            sp.fail(&format!("{}:{}", host.style(Theme::ERROR), port));
            println!("\n{} {}:{} is {}", error(""), host.style(Theme::ACCENT), port, "OFFLINE".red().bold());
        }
    }
}

fn srv_status(ip: &str) {
    let (host, port) = parse_ip(ip);
    let sp = Spinner::new(&format!("Querying {}:{}...", host, port));
    let addr = match resolve_addr(&host, port) {
        Some(a) => a,
        None => { sp.fail(&format!("Could not resolve {}", host)); return; }
    };
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(s) => s,
        Err(_) => {
            sp.fail(&format!("{} is offline", host));
            println!("\n{} {}:{} is {}", error(""), host.style(Theme::ACCENT), port, "OFFLINE".red().bold());
            return;
        }
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    send_slp_handshake(&mut stream, &host, port);
    match read_slp_response(&mut stream) {
        Ok(json_str) => {
            sp.done(&format!("{}:{}", host.style(Theme::SUCCESS).bold(), port));
            display_server_info(&json_str);
        }
        Err(_) => {
            sp.fail("Server did not respond with valid data");
            println!("\n{} Server may not support SLP or is not a Minecraft server.", warn(""));
        }
    }
}
