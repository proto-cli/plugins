use proto_plugin_sdk::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut length = 16usize;
    let mut no_symbols = false;
    let mut no_numbers = false;
    let mut count = 1usize;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-l" | "--length" => {
                if let Some(v) = args.get(i + 1) {
                    length = v.parse().unwrap_or(length);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            "--no-symbols" => {
                no_symbols = true;
                i += 1;
            }
            "--no-numbers" => {
                no_numbers = true;
                i += 1;
            }
            "-n" | "--count" => {
                if let Some(v) = args.get(i + 1) {
                    count = v.parse().unwrap_or(count);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    let upper = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let lower = "abcdefghijklmnopqrstuvwxyz";
    let digits = "0123456789";
    let symbols = "!@#$%^&*-_=+[]{};:,.<>?";

    let mut charset = String::from(upper) + lower;
    if !no_numbers {
        charset.push_str(digits);
    }
    if !no_symbols {
        charset.push_str(symbols);
    }

    let chars: Vec<u8> = charset.bytes().collect();

    fn rand_byte() -> u8 {
        let mut buf = [0u8; 1];
        if std::fs::File::open("/dev/urandom")
            .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut buf).map(|_| buf))
            .is_ok()
        {
            return buf[0];
        }
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        ((t ^ (t >> 32)) & 0xff) as u8
    }

    println!("{}", header("Generate Password"));
    println!("{}", divider());

    for _ in 0..count {
        let password: String = (0..length)
            .map(|_| {
                let b = rand_byte();
                chars[(b as usize) % chars.len()] as char
            })
            .collect();
        println!("  {}", password.style(Theme::VALUE));
    }

    println!(
        "\n  {} {} chars, charset: {}",
        muted(""),
        length.style(Theme::VALUE),
        charset.len().style(Theme::VALUE)
    );
    println!(
        "  {} {} password(s) generated",
        muted(""),
        count.style(Theme::VALUE)
    );
    println!();
}
