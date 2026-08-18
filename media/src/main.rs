use clap::{Subcommand, Parser};
use proto_plugin_sdk::*;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "media", about = "Media file optimization")]
struct Cli {
    #[command(subcommand)]
    action: MediaAction,
}

#[derive(Subcommand)]
enum MediaAction { Shrink { #[arg(value_name = "FILE|DIR", default_value = ".")] target: String } }

enum MediaResult { Ok(String), Unchanged(String), Skipped(String) }

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max - 1]) }
}

fn main() {
    let cli = Cli::parse();
    match cli.action { MediaAction::Shrink { target } => shrink(&target) }
}

fn shrink(target: &str) {
    let path = Path::new(target);
    if !path.exists() { eprintln!("{} Path not found: {}", error(""), target); return; }
    let files = if path.is_dir() { collect_media(path) } else { vec![path.to_path_buf()] };
    if files.is_empty() { println!("{} No media files found in {}.", warn(""), target); return; }
    println!("{}", header("Media Shrink")); println!("{}", divider());
    println!("  {}", label_value("Files", &files.len().to_string()));
    let mut total_before = 0u64; let mut total_after = 0u64; let mut processed = 0usize; let mut skipped = 0usize;
    for f in &files {
        let before = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
        let ext = f.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let name = f.file_name().unwrap_or_default().to_string_lossy().to_string();
        let result = match ext.as_str() {
            "png" => shrink_png(f), "jpg" | "jpeg" => shrink_jpeg(f), "webp" => shrink_webp(f),
            "mp4" | "mkv" | "mov" | "webm" | "avi" | "m4v" => shrink_video(f),
            _ => MediaResult::Skipped("unsupported extension".to_string()),
        };
        match result {
            MediaResult::Ok(delta) => { let after = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0); total_before += before; total_after += after; processed += 1; println!("  {} {:<28} {} → {}  {}", "✔".style(Theme::SUCCESS), truncate_str(&name, 28), format_size(before), format_size(after), delta.style(Theme::ACCENT)); }
            MediaResult::Unchanged(reason) | MediaResult::Skipped(reason) => { skipped += 1; println!("  {} {:<28} {}", "·".style(Theme::MUTED), truncate_str(&name, 28), reason.dimmed()); }
        }
    }
    println!(); println!("{}", divider());
    let saved = total_before.saturating_sub(total_after);
    if processed > 0 { let pct = if total_before > 0 { (saved as f64 / total_before as f64 * 100.0) as u32 } else { 0 }; println!("  {} {} file(s) optimized, {} saved ({}%)", success(""), processed, format_size(saved), pct); }
    else { println!("  {} Nothing to optimize.", warn("")); }
    if skipped > 0 { println!("  {}", format!("{} file(s) skipped (see above)", skipped).dimmed()); }
}

fn shrink_png(path: &Path) -> MediaResult {
    let before = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let status = if which("optipng") { run_command("optipng", &["-o2", path.to_str().unwrap_or("")]) }
    else if which("pngcrush") { run_command("pngcrush", &["-ow", "-reduce", "-q", path.to_str().unwrap_or("")]) }
    else { return MediaResult::Skipped("install optipng or pngcrush".to_string()); };
    if status.is_err() { return MediaResult::Skipped("tool failed".to_string()); }
    let after = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if after < before { MediaResult::Ok(format!("-{}", format_size(before - after))) } else { MediaResult::Unchanged("already optimal".to_string()) }
}

fn shrink_jpeg(path: &Path) -> MediaResult {
    if !which("jpegoptim") { return MediaResult::Skipped("install jpegoptim".to_string()); }
    let before = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let ok = run_command("jpegoptim", &["--strip-all", path.to_str().unwrap_or("")]);
    if ok.is_err() { return MediaResult::Skipped("tool failed".to_string()); }
    let after = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if after < before { MediaResult::Ok(format!("-{}", format_size(before - after))) } else { MediaResult::Unchanged("already optimal".to_string()) }
}

fn shrink_webp(path: &Path) -> MediaResult {
    if !which("cwebp") { return MediaResult::Skipped("install webp tools".to_string()); }
    let before = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let tmp = path.with_extension("proto.tmp.webp");
    let ok = run_command("cwebp", &["-lossless", "-quiet", "-q", "100", path.to_str().unwrap_or(""), "-o", tmp.to_str().unwrap_or("")]);
    if ok.is_err() { let _ = std::fs::remove_file(&tmp); return MediaResult::Skipped("tool failed".to_string()); }
    let after = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    if after > 0 && after < before { std::fs::rename(&tmp, path).ok(); MediaResult::Ok(format!("-{}", format_size(before - after))) }
    else { let _ = std::fs::remove_file(&tmp); MediaResult::Unchanged("already optimal".to_string()) }
}

fn shrink_video(path: &Path) -> MediaResult {
    if !which("ffmpeg") { return MediaResult::Skipped("install ffmpeg".to_string()); }
    let before = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let tmp = path.with_extension("proto.tmp.mp4");
    let ok = run_command("ffmpeg", &["-y", "-loglevel", "error", "-i", path.to_str().unwrap_or(""), "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-c:a", "copy", "-movflags", "+faststart", tmp.to_str().unwrap_or("")]);
    if ok.is_err() { let _ = std::fs::remove_file(&tmp); return MediaResult::Skipped("encode failed".to_string()); }
    let after = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
    if after > 0 && after < before { std::fs::rename(&tmp, path).ok(); MediaResult::Ok(format!("-{}", format_size(before - after))) }
    else { let _ = std::fs::remove_file(&tmp); MediaResult::Unchanged("already optimal".to_string()) }
}

fn collect_media(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new(); let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for entry in rd.flatten() { let path = entry.path(); if path.is_dir() { stack.push(path); } else if let Some(ext) = path.extension().map(|e| e.to_string_lossy().to_lowercase()) { if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "mp4" | "mkv" | "mov" | "webm" | "avi" | "m4v") { out.push(path); } } }
        }
    }
    out.sort(); out
}
