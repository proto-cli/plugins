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

fn divider() -> String {
    "─".repeat(40).style(Theme::MUTED).to_string()
}

fn muted(msg: &str) -> String {
    format!("{}", msg.style(Theme::MUTED))
}

fn success(msg: &str) -> String {
    format!("{} {}", "✔".style(Theme::SUCCESS), msg)
}

fn error(msg: &str) -> String {
    format!("{} {}", "✗".style(Theme::ERROR), msg)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut text = String::new();
    let mut out: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                if let Some(v) = args.get(i + 1) {
                    out = Some(v.clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                text = args[i].clone();
                i += 1;
            }
        }
    }

    if text.is_empty() {
        eprintln!("  {} Usage: qr <TEXT> [-o FILE]", error(""));
        return;
    }

    println!("{}", header("QR Code"));
    println!("{}", divider());

    let code = match qrcode::QrCode::new(&text) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  {} QR error: {}", error(""), e);
            return;
        }
    };

    let width = code.width();
    let modules = code.to_colors();

    println!(
        "  {} {} ({}x{} modules)\n",
        muted("Encoding:"),
        text.chars().take(50).collect::<String>().style(Theme::VALUE),
        width,
        width,
    );

    for y in 0..width {
        print!("  ");
        for x in 0..width {
            let dark = match modules[(y * width + x) as usize] {
                qrcode::Color::Dark => true,
                qrcode::Color::Light => false,
            };
            if dark {
                print!("{}", "  ".on_black().black());
            } else {
                print!("  ");
            }
        }
        println!();
    }
    println!();
    println!(
        "  {} {} characters encoded",
        muted(""),
        text.len().style(Theme::VALUE)
    );

    if let Some(path) = out {
        render_png(&code, &path);
    }
}

fn render_png(code: &qrcode::QrCode, path: &str) {
    let width = code.width() as u32;
    let scale = 8u32;
    let img_size = (width + 8) * scale;
    let mut img = image::RgbImage::new(img_size, img_size);

    for y in 0..img_size {
        for x in 0..img_size {
            let mx = (x / scale) as i32 - 4;
            let my = (y / scale) as i32 - 4;
            let dark = if mx < 0 || my < 0 || mx >= width as i32 || my >= width as i32 {
                false
            } else {
                matches!(
                    code.to_colors()[my as usize * code.width() + mx as usize],
                    qrcode::Color::Dark
                )
            };
            img.put_pixel(
                x,
                y,
                image::Rgb(if dark { [0, 0, 0] } else { [255, 255, 255] }),
            );
        }
    }

    match img.save(path) {
        Ok(_) => println!("  {} PNG saved to {}", success(""), path),
        Err(e) => eprintln!("  {} PNG save error: {}", error(""), e),
    }
}
