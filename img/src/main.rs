use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgb, Rgba, imageops};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::io::Read;
use std::path::{Path, PathBuf};

struct Theme;
impl Theme {
    const HEADER: Style = Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD);
    const ACCENT: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    const MUTED: Style = Style::new().fg(Color::DarkGray);
    const SUCCESS: Style = Style::new().fg(Color::Green);
    const ERROR: Style = Style::new().fg(Color::Red);
    const WARN: Style = Style::new().fg(Color::Yellow);
    const VALUE: Style = Style::new().fg(Color::White);
    const SELECTED: Style = Style::new().fg(Color::Black).bg(Color::Cyan);
    const HOT: Style = Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD);
}

#[derive(Debug, Clone, PartialEq)]
enum Panel {
    Files,
    Operations,
    Params,
}

#[derive(Debug, Clone)]
struct ImageFile {
    name: String,
    path: PathBuf,
    w: u32,
    h: u32,
}

#[derive(Debug, Clone)]
enum Operation {
    Resize,
    Crop,
    Rotate,
    FlipH,
    FlipV,
    Grayscale,
    Sepia,
    Blur,
    Brightness,
    Contrast,
    Pixelate,
    Invert,
    MemeOverlay,
    SpeechBubble,
    ThugLife,
    Drakeno,
    ThisIsFine,
}

impl Operation {
    fn label(&self) -> &str {
        match self {
            Operation::Resize => "Resize",
            Operation::Crop => "Crop",
            Operation::Rotate => "Rotate",
            Operation::FlipH => "Flip Horizontal",
            Operation::FlipV => "Flip Vertical",
            Operation::Grayscale => "Grayscale",
            Operation::Sepia => "Sepia Tone",
            Operation::Blur => "Blur",
            Operation::Brightness => "Brightness",
            Operation::Contrast => "Contrast",
            Operation::Pixelate => "Pixelate",
            Operation::Invert => "Invert Colors",
            Operation::MemeOverlay => "Meme Overlay",
            Operation::SpeechBubble => "Speech Bubble",
            Operation::ThugLife => "Thug Life Glasses",
            Operation::Drakeno => "Drake No (reject)",
            Operation::ThisIsFine => "This Is Fine ☕",
        }
    }

    fn category(&self) -> &str {
        match self {
            Operation::Resize | Operation::Crop | Operation::Rotate
            | Operation::FlipH | Operation::FlipV => "Transform",
            Operation::Grayscale | Operation::Sepia | Operation::Blur
            | Operation::Brightness | Operation::Contrast | Operation::Pixelate
            | Operation::Invert => "Effects",
            Operation::MemeOverlay | Operation::SpeechBubble | Operation::ThugLife
            | Operation::Drakeno | Operation::ThisIsFine => "Meme",
        }
    }

    fn needs_param(&self) -> bool {
        matches!(
            self,
            Operation::Resize | Operation::Crop | Operation::Rotate
                | Operation::Blur | Operation::Brightness | Operation::Contrast
                | Operation::Pixelate | Operation::MemeOverlay | Operation::SpeechBubble
        )
    }

    fn all() -> Vec<Self> {
        vec![
            Operation::Resize,
            Operation::Crop,
            Operation::Rotate,
            Operation::FlipH,
            Operation::FlipV,
            Operation::Grayscale,
            Operation::Sepia,
            Operation::Blur,
            Operation::Brightness,
            Operation::Contrast,
            Operation::Pixelate,
            Operation::Invert,
            Operation::MemeOverlay,
            Operation::SpeechBubble,
            Operation::ThugLife,
            Operation::Drakeno,
            Operation::ThisIsFine,
        ]
    }
}

struct App {
    files: Vec<ImageFile>,
    file_state: ListState,
    operations: Vec<Operation>,
    op_state: ListState,
    active: Panel,
    status_msg: String,
    input_buf: String,
    input_mode: bool,
    input_prompt: String,
}

impl App {
    fn new(images: Vec<ImageFile>) -> Self {
        let mut fs = ListState::default();
        let mut os = ListState::default();
        if !images.is_empty() {
            fs.select(Some(0));
        }
        os.select(Some(0));
        Self {
            files: images,
            file_state: fs,
            operations: Operation::all(),
            op_state: os,
            active: Panel::Files,
            status_msg: String::new(),
            input_buf: String::new(),
            input_mode: false,
            input_prompt: String::new(),
        }
    }
}

fn scan_images(dir: &Path) -> Vec<ImageFile> {
    let exts = ["png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff"];
    let mut images = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if exts.contains(&ext.to_lowercase().as_str()) {
                        let name = p.file_name().unwrap().to_string_lossy().to_string();
                        let (w, h) = match image::image_dimensions(&p) {
                            Ok(d) => d,
                            Err(_) => (0, 0),
                        };
                        images.push(ImageFile {
                            name,
                            path: p,
                            w,
                            h,
                        });
                    }
                }
            }
        }
    }
    images.sort_by(|a, b| a.name.cmp(&b.name));
    images
}

fn apply_operation(img: &mut DynamicImage, op: &Operation, param: &str) -> Result<String, String> {
    match op {
        Operation::Resize => {
            let dims: Vec<u32> = param
                .split('x')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if dims.len() != 2 {
                return Err("Format: WxH (e.g. 800x600)".into());
            }
            *img = img.resize(dims[0], dims[1], imageops::FilterType::Lanczos3);
            Ok(format!("Resized to {}x{}", dims[0], dims[1]))
        }
        Operation::Crop => {
            let parts: Vec<i64> = param
                .split(|c: char| c == ',' || c == 'x')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if parts.len() != 4 {
                return Err("Format: X,Y,Width,Height".into());
            }
            let sub = img.crop(
                parts[0] as u32,
                parts[1] as u32,
                parts[2] as u32,
                parts[3] as u32,
            );
            *img = sub;
            Ok("Cropped".into())
        }
        Operation::Rotate => {
            let deg: f32 = param.parse().map_err(|_| "Enter degrees (90, 180, 270)")?;
            match deg as u32 {
                90 => *img = img.rotate90(),
                180 => *img = img.rotate180(),
                270 => *img = img.rotate270(),
                _ => *img = img.rotate90(),
            }
            Ok(format!("Rotated {}°", deg))
        }
        Operation::FlipH => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = ImageBuffer::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let px = rgba.get_pixel(x, y);
                    out.put_pixel(w - 1 - x, y, *px);
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok("Flipped horizontally".into())
        }
        Operation::FlipV => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = ImageBuffer::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let px = rgba.get_pixel(x, y);
                    out.put_pixel(x, h - 1 - y, *px);
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok("Flipped vertically".into())
        }
        Operation::Grayscale => {
            *img = img.grayscale();
            Ok("Converted to grayscale".into())
        }
        Operation::Sepia => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = ImageBuffer::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let px = rgba.get_pixel(x, y);
                    let r = px[0] as f64;
                    let g = px[1] as f64;
                    let b = px[2] as f64;
                    let sr = (r * 0.393 + g * 0.769 + b * 0.189).min(255.0) as u8;
                    let sg = (r * 0.349 + g * 0.686 + b * 0.168).min(255.0) as u8;
                    let sb = (r * 0.272 + g * 0.534 + b * 0.131).min(255.0) as u8;
                    out.put_pixel(x, y, Rgba([sr, sg, sb, px[3]]));
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok("Sepia applied".into())
        }
        Operation::Blur => {
            let sigma: f32 = param.parse().unwrap_or(3.0);
            *img = img.blur(sigma);
            Ok(format!("Blurred (σ={})", sigma))
        }
        Operation::Brightness => {
            let val: i32 = param.parse().map_err(|_| "Enter brightness (-255..255)")?;
            *img = img.brighten(val);
            Ok(format!("Brightness adjusted by {}", val))
        }
        Operation::Contrast => {
            let val: f32 = param.parse().map_err(|_| "Enter contrast (-255..255)")?;
            *img = img.adjust_contrast(val);
            Ok(format!("Contrast adjusted by {}", val))
        }
        Operation::Pixelate => {
            let block: u32 = param.parse().unwrap_or(8);
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = rgba.clone();
            for by in (0..h).step_by(block as usize) {
                for bx in (0..w).step_by(block as usize) {
                    let px = *rgba.get_pixel(bx, by);
                    for dy in 0..block.min(h - by) {
                        for dx in 0..block.min(w - bx) {
                            out.put_pixel(bx + dx, by + dy, px);
                        }
                    }
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok(format!("Pixelated (block={})", block))
        }
        Operation::Invert => {
            img.invert();
            Ok("Inverted".into())
        }
        Operation::MemeOverlay => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = rgba.clone();
            // Top banner: black background with white "text" placeholder
            let banner_h = h / 6;
            for y in 0..banner_h {
                for x in 0..w {
                    out.put_pixel(x, y, Rgba([0, 0, 0, 220]));
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok(format!("Meme overlay added ({}px banner)", banner_h))
        }
        Operation::SpeechBubble => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = rgba.clone();
            // Draw a white speech bubble at bottom-left
            let bw = w / 3;
            let bh = h / 4;
            let bx = w / 10;
            let by = h - bh - h / 10;
            for y in by..(by + bh).min(h) {
                for x in bx..(bx + bw).min(w) {
                    let in_rect = y > by + 4 && y < by + bh - 4 && x > bx + 4 && x < bx + bw - 4;
                    if in_rect {
                        out.put_pixel(x, y, Rgba([255, 255, 255, 240]));
                    }
                }
            }
            // Triangle tail
            for i in 0..20u32 {
                let tx = bx + bw / 3 + i;
                let ty = by + bh - 1 + i / 2;
                if tx < w && ty < h {
                    out.put_pixel(tx, ty, Rgba([255, 255, 255, 240]));
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok("Speech bubble added".into())
        }
        Operation::ThugLife => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = rgba.clone();
            // Draw pixel "thug life" glasses
            let gw = w / 3;
            let gh = h / 12;
            let gx = w / 4;
            let gy = h / 3;
            for y in gy..(gy + gh).min(h) {
                for x in gx..(gx + gw).min(w) {
                    out.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
            // Bridge
            let bridge_y = gy + gh / 2;
            let bridge_start = gx + gw;
            let bridge_end = (gx + gw + gw / 2).min(w);
            for x in bridge_start..bridge_end {
                if bridge_y < h {
                    out.put_pixel(x, bridge_y, Rgba([0, 0, 0, 255]));
                }
            }
            // Right lens
            let rx = gx + gw + gw / 2;
            for y in gy..(gy + gh).min(h) {
                for x in rx..(rx + gw).min(w) {
                    out.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok("Thug life glasses applied".into())
        }
        Operation::Drakeno => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = ImageBuffer::new(w, h);
            // Top half: original, bottom half: grayscale (drake rejecting)
            for y in 0..h {
                for x in 0..w {
                    if y < h / 2 {
                        out.put_pixel(x, y, *rgba.get_pixel(x, y));
                    } else {
                        let px = rgba.get_pixel(x, y);
                        let gray = (px[0] as f64 * 0.299 + px[1] as f64 * 0.587 + px[2] as f64 * 0.114) as u8;
                        out.put_pixel(x, y, Rgba([gray, gray, gray, px[3]]));
                    }
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok("Drake No effect (top=color, bottom=grayscale)".into())
        }
        Operation::ThisIsFine => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = ImageBuffer::new(w, h);
            // Apply warm orange tint and slight blur
            for y in 0..h {
                for x in 0..w {
                    let px = rgba.get_pixel(x, y);
                    let r = (px[0] as f64 * 1.15).min(255.0) as u8;
                    let g = (px[1] as f64 * 0.85).min(255.0) as u8;
                    let b = (px[2] as f64 * 0.6).min(255.0) as u8;
                    out.put_pixel(x, y, Rgba([r, g, b, px[3]]));
                }
            }
            *img = DynamicImage::ImageRgba8(out);
            Ok("This Is Fine 🔥 applied".into())
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(f.area());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(35), Constraint::Percentage(35)])
        .split(chunks[0]);

    render_file_list(f, app, main[0]);
    render_operations(f, app, main[1]);
    render_preview(f, app, main[2]);
    render_status_bar(f, app, chunks[1]);
}

fn render_file_list(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .files
        .iter()
        .enumerate()
        .map(|(i, img)| {
            let style = if app.file_state.selected() == Some(i) {
                Theme::SELECTED
            } else {
                Theme::VALUE
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", img.name), style),
                Span::styled(format!("{}×{}", img.w, img.h), Theme::MUTED),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(
            format!(" Images ({}) ", app.files.len()),
            Theme::ACCENT,
        ))
        .borders(Borders::ALL)
        .border_style(if app.active == Panel::Files {
            Theme::ACCENT
        } else {
            Theme::MUTED
        });

    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::SELECTED)
        .highlight_symbol("▶ ");

    let mut state = app.file_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_operations(f: &mut Frame, app: &mut App, area: Rect) {
    let items: Vec<ListItem> = app
        .operations
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let style = if app.op_state.selected() == Some(i) {
                Theme::SELECTED
            } else if op.category() == "Meme" {
                Theme::HOT
            } else if op.category() == "Effects" {
                Theme::WARN
            } else {
                Theme::VALUE
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", op.label()), style),
                Span::styled(
                    format!("[{}]", op.category()),
                    Theme::MUTED,
                ),
            ]))
        })
        .collect();

    let block = Block::default()
        .title(Span::styled(" Operations ", Theme::ACCENT))
        .borders(Borders::ALL)
        .border_style(if app.active == Panel::Operations {
            Theme::ACCENT
        } else {
            Theme::MUTED
        });

    let list = List::new(items)
        .block(block)
        .highlight_style(Theme::SELECTED)
        .highlight_symbol("▶ ");

    let mut state = app.op_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_preview(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(" Preview ", Theme::ACCENT))
        .borders(Borders::ALL)
        .border_style(Theme::ACCENT);

    if let Some(idx) = app.file_state.selected() {
        if let Some(img_file) = app.files.get(idx) {
            let hint = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!(" {} ", img_file.name),
                    Theme::HEADER,
                )),
                Line::from(Span::styled(
                    format!(" {}×{} px ", img_file.w, img_file.h),
                    Theme::MUTED,
                )),
                Line::from(""),
                Line::from(Span::styled(" p  preview (kitty icat)", Theme::ACCENT)),
                Line::from(Span::styled(" Enter  apply operation", Theme::MUTED)),
                Line::from(Span::styled(" Tab  switch panel", Theme::MUTED)),
            ];
            let para = Paragraph::new(hint).block(block);
            f.render_widget(para, area);
        }
    } else {
        let empty = vec![
            Line::from(""),
            Line::from(Span::styled(" Select an image", Theme::MUTED)),
        ];
        let para = Paragraph::new(empty).block(block);
        f.render_widget(para, area);
    }
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::styled(" ESC quit ", Theme::SELECTED),
        Span::styled(" ←→ navigate ", Theme::MUTED),
        Span::styled(" Enter apply ", Theme::SUCCESS),
    ];
    if app.input_mode {
        spans.push(Span::styled(
            format!(" {}:{} ", app.input_prompt, app.input_buf),
            Theme::WARN,
        ));
    }
    let status = Line::from(spans);
    let block = Block::default().borders(Borders::ALL).border_style(Theme::MUTED);
    let para = Paragraph::new(status).block(block);
    f.render_widget(para, area);
}

fn run_operation_prompt(app: &mut App) -> Option<(Operation, String)> {
    let idx = app.op_state.selected()?;
    let op = app.operations.get(idx)?.clone();
    if op.needs_param() {
        let prompt = match &op {
            Operation::Resize => "WxH (e.g. 800x600)",
            Operation::Crop => "X,Y,Width,Height",
            Operation::Rotate => "Degrees (90, 180, 270)",
            Operation::Blur => "Sigma (e.g. 3.0)",
            Operation::Brightness => "Value (-255..255)",
            Operation::Contrast => "Value (-255..255)",
            Operation::Pixelate => "Block size (e.g. 8)",
            Operation::MemeOverlay => "Top text",
            Operation::SpeechBubble => "Bubble text",
            _ => "Value",
        };
        Some((op, prompt.to_string()))
    } else {
        Some((op, String::new()))
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let images = scan_images(&dir);
    if images.is_empty() {
        eprintln!("✗ No images found in {}", dir.display());
        eprintln!("  Supported: png, jpg, jpeg, gif, bmp, webp, tiff");
        std::process::exit(1);
    }

    let mut app = App::new(images);

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if app.input_mode {
                    match key.code {
                        KeyCode::Esc => {
                            app.input_mode = false;
                            app.input_buf.clear();
                        }
                        KeyCode::Enter => {
                            let param = app.input_buf.trim().to_string();
                            app.input_buf.clear();
                            app.input_mode = false;
                            if let Some(idx) = app.file_state.selected() {
                                let file = &app.files[idx];
                                let op_idx = app.op_state.selected().unwrap_or(0);
                                let op = app.operations[op_idx].clone();
                                match apply_operation(
                                    &mut image::open(&file.path).unwrap(),
                                    &op,
                                    &param,
                                ) {
                                    Ok(msg) => {
                                        let mut img = image::open(&file.path).unwrap();
                                        if let Ok(msg) = apply_operation(&mut img, &op, &param) {
                                            let out = file.path.with_file_name(format!(
                                                "{}_{}{}",
                                                file.path.file_stem().unwrap().to_string_lossy(),
                                                op.label().to_lowercase().replace(' ', "_"),
                                                file.path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default()
                                            ));
                                            img.save(&out).unwrap();
                                            app.status_msg = format!("{} → {}", msg, out.file_name().unwrap().to_string_lossy());
                                        }
                                    }
                                    Err(e) => {
                                        app.status_msg = format!("✗ {}", e);
                                    }
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            app.input_buf.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input_buf.pop();
                        }
                        _ => {}
                    }
                    continue;
                }

                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => {
                        app.active = match &app.active {
                            Panel::Files => Panel::Operations,
                            Panel::Operations => Panel::Files,
                            Panel::Params => Panel::Files,
                        };
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        match &app.active {
                            Panel::Files => {
                                let i = app.file_state.selected().unwrap_or(0).saturating_sub(1);
                                app.file_state.select(Some(i));
                            }
                            Panel::Operations => {
                                let i = app.op_state.selected().unwrap_or(0).saturating_sub(1);
                                app.op_state.select(Some(i));
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        match &app.active {
                            Panel::Files => {
                                let i = (app.file_state.selected().unwrap_or(0) + 1)
                                    .min(app.files.len().saturating_sub(1));
                                app.file_state.select(Some(i));
                            }
                            Panel::Operations => {
                                let i = (app.op_state.selected().unwrap_or(0) + 1)
                                    .min(app.operations.len().saturating_sub(1));
                                app.op_state.select(Some(i));
                            }
                            _ => {}
                        }
                    }
                    KeyCode::Enter => {
                        if let Some((op, prompt)) = run_operation_prompt(&mut app) {
                            if prompt.is_empty() {
                                // No param needed, apply directly
                                if let Some(idx) = app.file_state.selected() {
                                    let file = &app.files[idx];
                                    let mut img = match image::open(&file.path) {
                                        Ok(i) => i,
                                        Err(e) => {
                                            app.status_msg = format!("✗ {}", e);
                                            continue;
                                        }
                                    };
                                    match apply_operation(&mut img, &op, "") {
                                        Ok(msg) => {
                                            let out = file.path.with_file_name(format!(
                                                "{}_{}{}",
                                                file.path.file_stem().unwrap().to_string_lossy(),
                                                op.label().to_lowercase().replace(' ', "_"),
                                                file.path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default()
                                            ));
                                            img.save(&out).unwrap();
                                            app.status_msg = format!("{} → {}", msg, out.file_name().unwrap().to_string_lossy());
                                        }
                                        Err(e) => app.status_msg = format!("✗ {}", e),
                                    }
                                }
                            } else {
                                app.input_mode = true;
                                app.input_prompt = prompt;
                                app.input_buf.clear();
                            }
                        }
                    }
                    KeyCode::Char('p') => {
                        if let Some(idx) = app.file_state.selected() {
                            let file = &app.files[idx];
                            // Exit TUI, show with kitty icat, wait for key, re-enter TUI
                            disable_raw_mode().ok();
                            execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
                            terminal.show_cursor().ok();

                            // Try kitten icat first, then kitty +kitten icat, then xdg-open
                            let icat = ["kitten icat", "kitty +kitten icat"];
                            let mut shown = false;
                            for cmd in &icat {
                                let parts: Vec<&str> = cmd.split_whitespace().collect();
                                if std::process::Command::new(parts[0])
                                    .args(&parts[1..])
                                    .arg(file.path.to_str().unwrap_or(""))
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .status()
                                    .map(|s| s.success())
                                    .unwrap_or(false)
                                {
                                    shown = true;
                                    break;
                                }
                            }
                            if !shown {
                                // Fallback: just print file info
                                println!("\n  Preview: {}", file.path.display());
                                println!("  {}×{} px\n", file.w, file.h);
                            }

                            println!("  Press Enter to return...");
                            let mut buf = [0u8; 1];
                            let _ = std::io::stdin().read(&mut buf);

                            // Re-enter TUI
                            enable_raw_mode().ok();
                            execute!(terminal.backend_mut(), EnterAlternateScreen).ok();
                            terminal.hide_cursor().ok();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
