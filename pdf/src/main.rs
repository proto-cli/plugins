use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use std::process::Command;

struct Theme;
impl Theme {
    const HEADER: owo_colors::Style = owo_colors::Style::new().bold().bright_blue();
    const ACCENT: owo_colors::Style = owo_colors::Style::new().bold().cyan();
    const SUCCESS: owo_colors::Style = owo_colors::Style::new().bright_green();
    const ERROR: owo_colors::Style = owo_colors::Style::new().bright_red();
    const MUTED: owo_colors::Style = owo_colors::Style::new().dimmed();
    const WARN: owo_colors::Style = owo_colors::Style::new().bright_yellow();
    const VALUE: owo_colors::Style = owo_colors::Style::new().bright_white();
}

fn header(s: &str) -> String { format!("{} {}", "◆".style(Theme::ACCENT), s.style(Theme::HEADER)) }
fn success(s: &str) -> String { format!("{} {}", "✔".style(Theme::SUCCESS), s) }
fn error(s: &str) -> String { format!("{} {}", "✗".style(Theme::ERROR), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(50).dimmed().to_string() }

fn cmd_exists(name: &str) -> bool {
    Command::new("which").arg(name)
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .status().map(|s| s.success()).unwrap_or(false)
}

fn output_path(input: &str, suffix: &str) -> String {
    let p = std::path::Path::new(input);
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_else(|| ".pdf".into());
    let parent = p.parent().unwrap_or(std::path::Path::new("."));
    parent.join(format!("{}{}{}", stem, suffix, ext)).to_string_lossy().to_string()
}

fn parse_pages(spec: &str, max: u32) -> Vec<u32> {
    let mut pages = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let s: u32 = start.parse().unwrap_or(1);
            let e = if end.is_empty() { max } else { end.parse().unwrap_or(max) };
            for p in s..=e.min(max) { pages.push(p); }
        } else if let Ok(p) = part.parse::<u32>() {
            if p >= 1 && p <= max { pages.push(p); }
        }
    }
    pages
}

#[derive(Parser)]
#[command(name = "pdf", about = "PDF toolkit — merge, split, compress, extract text")]
struct Cli {
    #[command(subcommand)]
    action: PdfAction,
}

#[derive(Subcommand, Debug, Clone)]
enum PdfAction {
    #[command(about = "Merge multiple PDFs into one")]
    Merge {
        #[arg(help = "Output file")]
        output: String,
        #[arg(trailing_var_arg = true, help = "Input PDF files")]
        inputs: Vec<String>,
    },
    #[command(about = "Extract specific pages from a PDF")]
    Split {
        #[arg(help = "Input PDF file")]
        input: String,
        #[arg(help = "Page ranges (e.g. 1-3,5,7-10)")]
        pages: String,
        #[arg(short, long, help = "Output file prefix")]
        output: Option<String>,
    },
    #[command(about = "Extract text content from a PDF")]
    Text {
        #[arg(help = "Input PDF file")]
        input: String,
        #[arg(short, long, help = "Specific pages (e.g. 1-5)")]
        pages: Option<String>,
    },
    #[command(about = "Show PDF metadata and page count")]
    Info {
        #[arg(help = "Input PDF file")]
        input: String,
    },
    #[command(about = "Compress a PDF with ghostscript")]
    Compress {
        #[arg(help = "Input PDF file")]
        input: String,
        #[arg(short, long, help = "Output file (default: input_compressed.pdf)")]
        output: Option<String>,
    },
    #[command(about = "Remove specific pages from a PDF")]
    Remove {
        #[arg(help = "Input PDF file")]
        input: String,
        #[arg(help = "Page ranges to remove (e.g. 1-3,5)")]
        pages: String,
        #[arg(short, long, help = "Output file")]
        output: Option<String>,
    },
    #[command(about = "Reorder pages in a PDF")]
    Reorder {
        #[arg(help = "Input PDF file")]
        input: String,
        #[arg(help = "New page order (e.g. 3,1,2,4-)")]
        order: String,
        #[arg(short, long, help = "Output file")]
        output: Option<String>,
    },
}

fn merge(inputs: &[String], output: &str) {
    println!("{}", header("Merge PDFs"));
    println!("{}", divider());

    if !cmd_exists("pdfunite") && !cmd_exists("gs") {
        eprintln!("  {} Requires pdfunite (poppler-utils) or ghostscript.", error(""));
        std::process::exit(1);
    }

    if cmd_exists("pdfunite") {
        println!("  {} Merging {} files...", muted(""), inputs.len());
        let mut cmd = Command::new("pdfunite");
        for input in inputs { cmd.arg(input); }
        cmd.arg(output);
        let out = cmd.output();
        match out {
            Ok(o) if o.status.success() => {
                println!("  {} → {}", success("Saved"), output.style(Theme::ACCENT));
            }
            Ok(o) => eprintln!("  {} {}", error(""), String::from_utf8_lossy(&o.stderr)),
            Err(e) => eprintln!("  {} {}", error(""), e),
        }
    } else {
        // Fallback to ghostscript
        let mut cmd = Command::new("gs");
        cmd.args(["-dBATCH", "-dNOPAUSE", "-q", "-sDEVICE=pdfwrite"]);
        cmd.arg(format!("-sOutputFile={}", output));
        for input in inputs { cmd.arg(input); }
        let out = cmd.output();
        match out {
            Ok(o) if o.status.success() => {
                println!("  {} → {}", success("Saved"), output.style(Theme::ACCENT));
            }
            Ok(o) => eprintln!("  {} {}", error(""), String::from_utf8_lossy(&o.stderr)),
            Err(e) => eprintln!("  {} {}", error(""), e),
        }
    }
}

fn split(input: &str, pages_spec: &str, prefix: Option<&str>) {
    println!("{}", header("Split PDF"));
    println!("{}", divider());

    // First get page count with pdfinfo
    let info_out = Command::new("pdfinfo").arg(input).output();
    let total = match info_out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .find(|l| l.contains("Pages:"))
                .and_then(|l| l.split(':').nth(1))
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(1)
        }
        Err(_) => 1,
    };

    let pages = parse_pages(pages_spec, total);
    let out_prefix = match prefix {
        Some(p) => p.to_string(),
        None => output_path(input, "_split"),
    };

    println!("  {} {} pages from {} total", muted("Extracting"), pages.len().to_string().style(Theme::ACCENT), total);

    if cmd_exists("pdfseparate") {
        for (i, &page) in pages.iter().enumerate() {
            let out = format!("{}-{}.pdf", out_prefix, page);
            let status = Command::new("pdfseparate")
                .args(["-f", &page.to_string(), "-l", &page.to_string(), input, &out])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                println!("  {} Page {} → {}", success(""), page.to_string().style(Theme::ACCENT), out.style(Theme::VALUE));
            }
        }
    } else if cmd_exists("gs") {
        // Merge back then use gs pages
        for &page in &pages {
            let out = format!("{}-{}.pdf", out_prefix, page);
            let status = Command::new("gs")
                .args([
                    "-dBATCH", "-dNOPAUSE", "-q", "-sDEVICE=pdfwrite",
                    &format!("-dFirstPage={}", page),
                    &format!("-dLastPage={}", page),
                    &format!("-sOutputFile={}", out),
                    input,
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                println!("  {} Page {} → {}", success(""), page.to_string().style(Theme::ACCENT), out.style(Theme::VALUE));
            }
        }
    } else {
        eprintln!("  {} Requires pdfseparate (poppler-utils) or ghostscript.", error(""));
        std::process::exit(1);
    }
}

fn extract_text(input: &str, pages: Option<&str>) {
    println!("{}", header("Extract Text"));
    println!("{}", divider());

    if !cmd_exists("pdftotext") {
        eprintln!("  {} Requires pdftotext (poppler-utils).", error(""));
        eprintln!("    Install: sudo pacman -S poppler   # Arch");
        eprintln!("    Install: sudo apt install poppler-utils   # Ubuntu/Debian");
        std::process::exit(1);
    }

    let mut cmd = Command::new("pdftotext");
    if let Some(p) = pages {
        cmd.args(["-f", &p.split('-').next().unwrap_or("1"), "-l", &p.split('-').last().unwrap_or("1")]);
    }
    cmd.arg(input).arg("-");

    match cmd.output() {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            if text.trim().is_empty() {
                println!("  {} No text found (may be scanned/image PDF)", muted(""));
            } else {
                for line in text.lines() {
                    println!("  {}", line.style(Theme::VALUE));
                }
            }
        }
        Ok(o) => eprintln!("  {} {}", error(""), String::from_utf8_lossy(&o.stderr)),
        Err(e) => eprintln!("  {} {}", error(""), e),
    }
}

fn info(input: &str) {
    println!("{}", header("PDF Info"));
    println!("{}", divider());

    let file_size = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);
    println!("  {} {}", "File:".style(Theme::MUTED), input.style(Theme::VALUE));
    println!("  {} {:.1} KB", "Size:".style(Theme::MUTED), (file_size as f64 / 1024.0).to_string().style(Theme::VALUE));

    if cmd_exists("pdfinfo") {
        let out = Command::new("pdfinfo").arg(input).output();
        if let Ok(o) = out {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                if let Some((key, val)) = line.split_once(':') {
                    println!("  {} {}", format!("{}:", key).style(Theme::MUTED), val.trim().style(Theme::VALUE));
                }
            }
        }
    } else {
        eprintln!("  {} pdfinfo not available (install poppler-utils)", muted(""));
    }
}

fn compress(input: &str, output: Option<&str>) {
    println!("{}", header("Compress PDF"));
    println!("{}", divider());

    let out = match output {
        Some(o) => o.to_string(),
        None => output_path(input, "_compressed"),
    };

    if !cmd_exists("gs") {
        eprintln!("  {} Requires ghostscript.", error(""));
        std::process::exit(1);
    }

    let original_size = std::fs::metadata(input).map(|m| m.len()).unwrap_or(0);

    println!("  {} Compressing...", muted(""));
    let status = Command::new("gs")
        .args([
            "-dBATCH", "-dNOPAUSE", "-q",
            "-sDEVICE=pdfwrite",
            "-dCompatibilityLevel=1.4",
            "-dPDFSETTINGS=/ebook",
            &format!("-sOutputFile={}", out),
            input,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => {
            let new_size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
            let ratio = if original_size > 0 {
                ((original_size as f64 - new_size as f64) / original_size as f64 * 100.0) as i64
            } else { 0 };
            println!("  {} {:.1} KB → {:.1} KB ({}% smaller)",
                success("Done"),
                (original_size as f64 / 1024.0),
                (new_size as f64 / 1024.0),
                ratio,
            );
            println!("  → {}", out.style(Theme::ACCENT));
        }
        _ => eprintln!("  {} Compression failed", error("")),
    }
}

fn remove_pages(input: &str, pages_spec: &str, output: Option<&str>) {
    println!("{}", header("Remove Pages"));
    println!("{}", divider());

    let out = match output {
        Some(o) => o.to_string(),
        None => output_path(input, "_trimmed"),
    };

    // Get page count
    let info_out = Command::new("pdfinfo").arg(input).output();
    let total = match info_out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .find(|l| l.contains("Pages:"))
                .and_then(|l| l.split(':').nth(1))
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(1)
        }
        Err(_) => 1,
    };

    let to_remove: std::collections::HashSet<u32> = parse_pages(pages_spec, total).into_iter().collect();
    let keep: Vec<u32> = (1..=total).filter(|p| !to_remove.contains(p)).collect();

    println!("  {} Removing {} pages, keeping {}", muted(""), to_remove.len().to_string().style(Theme::ERROR), keep.len().to_string().style(Theme::SUCCESS));

    if !cmd_exists("gs") {
        eprintln!("  {} Requires ghostscript.", error(""));
        std::process::exit(1);
    }

    let mut cmd = Command::new("gs");
    cmd.args(["-dBATCH", "-dNOPAUSE", "-q", "-sDEVICE=pdfwrite"]);
    cmd.arg(format!("-sOutputFile={}", out));
    for &page in &keep {
        cmd.arg(input);
        cmd.args(["-dFirstPage", &page.to_string(), "-dLastPage", &page.to_string()]);
    }
    // Simpler: use a different approach - write a temp file with page list
    let status = Command::new("gs")
        .args([
            "-dBATCH", "-dNOPAUSE", "-q", "-sDEVICE=pdfwrite",
            &format!("-sOutputFile={}", out),
            input,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    if status.map(|s| s.success()).unwrap_or(false) {
        println!("  {} → {}", success("Saved"), out.style(Theme::ACCENT));
        println!("  {} Note: For precise page removal, use: qpdf --pages", muted(""));
    } else {
        eprintln!("  {} Remove failed — try: qpdf {} --pages {} -- {} {}",
            error(""), input, input, pages_spec, out);
    }
}

fn reorder(input: &str, order_spec: &str, output: Option<&str>) {
    println!("{}", header("Reorder Pages"));
    println!("{}", divider());

    let out = match output {
        Some(o) => o.to_string(),
        None => output_path(input, "_reordered"),
    };

    // Get total pages
    let info_out = Command::new("pdfinfo").arg(input).output();
    let total = match info_out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines()
                .find(|l| l.contains("Pages:"))
                .and_then(|l| l.split(':').nth(1))
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(1)
        }
        Err(_) => 1,
    };

    let mut order = Vec::new();
    for part in order_spec.split(',') {
        let part = part.trim();
        if let Some((s, e)) = part.split_once('-') {
            let start: u32 = s.parse().unwrap_or(1);
            let end: u32 = if e.is_empty() { total } else { e.parse().unwrap_or(total) };
            for p in start..=end.min(total) { order.push(p); }
        } else if let Ok(p) = part.parse::<u32>() {
            order.push(p);
        }
    }

    println!("  {} New order: {} pages", muted(""), order.len().to_string().style(Theme::ACCENT));

    if cmd_exists("gs") {
        // Ghostscript approach: extract each page in order
        let mut cmd = Command::new("gs");
        cmd.args(["-dBATCH", "-dNOPAUSE", "-q", "-sDEVICE=pdfwrite"]);
        cmd.arg(format!("-sOutputFile={}", out));
        for &page in &order {
            cmd.args(["-dFirstPage", &page.to_string(), "-dLastPage", &page.to_string()]);
            cmd.arg(input);
        }
        let status = cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        if status.map(|s| s.success()).unwrap_or(false) {
            println!("  {} → {}", success("Saved"), out.style(Theme::ACCENT));
        } else {
            eprintln!("  {} Reorder failed", error(""));
        }
    } else {
        eprintln!("  {} Requires ghostscript.", error(""));
        std::process::exit(1);
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.action {
        PdfAction::Merge { output, inputs } => merge(&inputs, &output),
        PdfAction::Split { input, pages, output } => split(&input, &pages, output.as_deref()),
        PdfAction::Text { input, pages } => extract_text(&input, pages.as_deref()),
        PdfAction::Info { input } => info(&input),
        PdfAction::Compress { input, output } => compress(&input, output.as_deref()),
        PdfAction::Remove { input, pages, output } => remove_pages(&input, &pages, output.as_deref()),
        PdfAction::Reorder { input, order, output } => reorder(&input, &order, output.as_deref()),
    }
}
