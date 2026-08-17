use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use serde::Deserialize;

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
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Self { spinner: sp }
    }
    fn done(&self, msg: &str) { self.spinner.finish_with_message(msg.to_string()); }
    fn fail(&self, msg: &str) {
        self.spinner.finish_with_message(format!("{} {}", "✗".style(Theme::ERROR), msg.style(Theme::ERROR)));
    }
    fn update(&self, msg: &str) { self.spinner.set_message(msg.to_string()); }
}

#[derive(Parser)]
#[command(name = "mc", about = "Minecraft resource pack utilities")]
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

fn main() {
    let cli = Cli::parse();
    match &cli.action {
        McAction::ResourcePack { action } => match action {
            ResourcePackAction::Create { version, name, clean } => create(version, name, *clean),
            ResourcePackAction::Fetch => fetch(),
            ResourcePackAction::Pack { branding } => pack(*branding),
            ResourcePackAction::Add { category, name, png, resolution } => add(category, name, png.as_deref(), *resolution),
        },
    }
}

fn create(version: &str, name: &str, clean: bool) {
    let folder_name = name.replace(' ', "_").to_lowercase();
    let path = std::path::PathBuf::from(&folder_name);

    if path.exists() { eprintln!("{} Directory '{}' already exists.", error(""), folder_name); return; }

    let sp = Spinner::new(&format!("Creating resource pack '{}'...", name));
    if let Err(e) = std::fs::create_dir_all(&path) { sp.fail(&format!("Failed: {}", e)); return; }

    write_pack_mcmeta(&path, name);
    if !clean { sp.update("Fetching version manifest..."); download_assets(&path, version, &sp); }

    sp.done(&format!("Created '{}' (Minecraft {})", name, version));
    println!(); println!("{}", success(&format!("Resource pack '{}' ready!", name)));
    println!("  {}", format!("cd {}", folder_name).style(Theme::MUTED));
    println!("  {}", "proto mc resource_pack pack".style(Theme::MUTED));
}

fn write_pack_mcmeta(path: &std::path::Path, name: &str) {
    let mcmeta = format!(r#"{{"pack":{{"pack_format":0,"description":"{}"}}}}"#, name);
    std::fs::write(path.join("pack.mcmeta"), mcmeta).unwrap();
}

fn download_assets(path: &std::path::Path, version: &str, sp: &Spinner) {
    let vm = fetch_version_manifest();
    let entry = vm.versions.iter().find(|v| v.id == version);
    let entry = match entry {
        Some(e) => e,
        None => { sp.fail(&format!("Version '{}' not found. Use 'fetch' to see available versions.", version)); std::process::exit(1); }
    };
    sp.update(&format!("Fetching version info for {}...", version));
    let info = fetch_json::<VersionInfo>(&entry.url);
    let client_url = info.downloads.and_then(|d| d.client).map(|c| c.url);
    let client_url = match client_url {
        Some(u) => u,
        None => { sp.fail("No client download found for this version."); return; }
    };
    let temp_dir = std::env::temp_dir().join(format!("proto_mc_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let jar_path = temp_dir.join("client.jar");
    sp.update(&format!("Downloading client.jar ({:.1} MB)...", 0.0));
    download_file(&client_url, &jar_path, sp);
    sp.update("Extracting assets from client.jar...");
    extract_assets(&jar_path, path, sp);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn fetch() {
    let sp = Spinner::new("Fetching version manifest...");
    let vm = fetch_version_manifest();
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

fn pack(branding: bool) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mcmeta = cwd.join("pack.mcmeta");
    if !mcmeta.exists() { eprintln!("{} No pack.mcmeta found. Run this inside a resource pack folder.", error("")); return; }

    let folder_name = cwd.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "resource_pack".to_string());
    let zip_name = format!("{}.zip", folder_name);
    let sp = Spinner::new(&format!("Packing '{}'...", folder_name));
    if branding { sp.update("Adding proto branding..."); add_branding(&mcmeta); }
    let output_path = cwd.parent().unwrap_or(&cwd).join(&zip_name);
    let _ = std::fs::remove_file(&output_path);
    sp.update("Creating archive...");
    let status = std::process::Command::new("zip").arg("-r").arg("-q").arg(output_path.to_string_lossy().as_ref()).arg(".").arg("-x").arg(&zip_name).arg("-x").arg("*.zip").current_dir(&cwd).status();
    match status { Ok(s) if s.success() => { sp.done(&format!("Packed: {}", output_path.to_string_lossy())); println!("{}", success("Resource pack ready to import!")); }, _ => { sp.fail("zip command failed. Is 'zip' installed?"); } }
}

fn add_branding(mcmeta: &std::path::Path) {
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

fn fetch_version_manifest() -> VersionManifest {
    ureq::get("https://piston-meta.mojang.com/mc/game/version_manifest.json")
        .set("User-Agent", "ProtoCLI/0.1.0").call().expect("Failed to fetch version manifest")
        .into_json::<VersionManifest>().expect("Failed to parse version manifest")
}

fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> T {
    ureq::get(url).set("User-Agent", "ProtoCLI/0.1.0").call().unwrap_or_else(|e| panic!("Failed to fetch {}: {}", url, e))
        .into_json::<T>().unwrap_or_else(|e| panic!("Failed to parse JSON from {}: {}", url, e))
}

fn download_file(url: &str, dest: &std::path::Path, sp: &Spinner) {
    let resp = ureq::get(url).set("User-Agent", "ProtoCLI/0.1.0").call().expect("Failed to download file");
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

fn extract_assets(jar_path: &std::path::Path, dest: &std::path::Path, sp: &Spinner) {
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

fn add(category: &str, name: &str, png_path: Option<&str>, resolution: Option<u32>) {
    let cwd = std::env::current_dir().unwrap_or_default();
    let mcmeta = cwd.join("pack.mcmeta");
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
        "item" => add_item(&cwd, &safe_name, &sp, &tex_data, tex_resolution),
        "block" => add_block(&cwd, &safe_name, &sp, &tex_data, tex_resolution),
        "entity" => add_entity(&cwd, &safe_name, &sp, &tex_data, tex_resolution),
        "armor" => add_armor(&cwd, &safe_name, &sp, &tex_data, tex_resolution),
        "armor_layer" | "armour_layer" => add_armor_layer(&cwd, &safe_name, &sp, &tex_data, tex_resolution),
        "gui" => add_gui(&cwd, &safe_name, &sp, &tex_data, tex_resolution),
        "particle" => add_particle(&cwd, &safe_name, &sp, &tex_data, tex_resolution),
        "environment" | "env" => add_environment(&cwd, &safe_name, &sp, &tex_data, tex_resolution),
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

fn write_texture(target: &std::path::Path, tex_data: &Option<Vec<u8>>, resolution: u32) -> std::io::Result<bool> {
    if target.exists() { return Ok(false); }
    let data = match tex_data { Some(d) => d.clone(), None => generate_blank_png(resolution, resolution) };
    std::fs::write(target, &data)?; Ok(true)
}

fn generate_blank_png(width: u32, height: u32) -> Vec<u8> {
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

fn add_item(root: &std::path::Path, name: &str, _sp: &Spinner, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
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
    if write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { created.push(format!("assets/minecraft/textures/item/{}.png", name)); }
    Ok(created)
}

fn add_block(root: &std::path::Path, name: &str, sp: &Spinner, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
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
    sp.update("Creating placeholder texture...");
    let tex_dir = root.join("assets/minecraft/textures/block");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { created.push(format!("assets/minecraft/textures/block/{}.png", name)); }
    Ok(created)
}

fn add_entity(root: &std::path::Path, name: &str, _sp: &Spinner, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/entity");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { Ok(vec![format!("assets/minecraft/textures/entity/{}.png", name)]) } else { Ok(vec![]) }
}

fn add_armor(root: &std::path::Path, name: &str, _sp: &Spinner, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let mut created = Vec::new();
    let tex_dir = root.join("assets/minecraft/textures/models/armor");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    for layer in &[1, 2] {
        let fname = format!("{}_layer_{}.png", name, layer);
        let tex_path = tex_dir.join(&fname);
        if write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { created.push(format!("assets/minecraft/textures/models/armor/{}", fname)); }
    }
    Ok(created)
}

fn add_armor_layer(root: &std::path::Path, name: &str, _sp: &Spinner, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/models/armor");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { return Ok(vec![format!("assets/minecraft/textures/models/armor/{}.png", name)]); }
    Ok(vec![])
}

fn add_gui(root: &std::path::Path, name: &str, _sp: &Spinner, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/gui");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { return Ok(vec![format!("assets/minecraft/textures/gui/{}.png", name)]); }
    Ok(vec![])
}

fn add_particle(root: &std::path::Path, name: &str, _sp: &Spinner, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/particle");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { return Ok(vec![format!("assets/minecraft/textures/particle/{}.png", name)]); }
    Ok(vec![])
}

fn add_environment(root: &std::path::Path, name: &str, _sp: &Spinner, tex_data: &Option<Vec<u8>>, resolution: u32) -> Result<Vec<String>, String> {
    let tex_dir = root.join("assets/minecraft/textures/environment");
    std::fs::create_dir_all(&tex_dir).map_err(|e| format!("mkdir: {}", e))?;
    let tex_path = tex_dir.join(format!("{}.png", name));
    if write_texture(&tex_path, tex_data, resolution).map_err(|e| format!("write: {}", e))? { return Ok(vec![format!("assets/minecraft/textures/environment/{}.png", name)]); }
    Ok(vec![])
}
