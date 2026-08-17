use std::fmt;

use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;

// ---------------------------------------------------------------------------
// Theme helpers (mirrors proto's palette)
// ---------------------------------------------------------------------------

struct Theme;

impl Theme {
    fn header(s: &str) -> String {
        format!("{}", s.bold().bright_blue())
    }
    fn accent(s: &str) -> String {
        format!("{}", s.bold().bright_cyan())
    }
    fn muted(s: &str) -> String {
        format!("{}", s.dimmed())
    }
}

fn divider() {
    println!("{}", Theme::muted(&"─".repeat(50)));
}

// ---------------------------------------------------------------------------
// Clap CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "convert",
    about = "Convert between units of measurement",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Run a unit conversion")]
    Run {
        /// Value + unit to convert (e.g. "6m", "10.5km", "500ms", "2GB")
        #[arg(required = true, value_name = "VALUE")]
        input: String,

        /// Target unit (e.g. "ft", "km/h"). If omitted, show all conversions in the same category.
        #[arg(value_name = "TO")]
        to: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Unit model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitCategory {
    Time,
    Length,
    Weight,
    Digital,
    Temperature,
    Speed,
    Volume,
}

impl fmt::Display for UnitCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Time => write!(f, "Time"),
            Self::Length => write!(f, "Length"),
            Self::Weight => write!(f, "Weight"),
            Self::Digital => write!(f, "Digital"),
            Self::Temperature => write!(f, "Temperature"),
            Self::Speed => write!(f, "Speed"),
            Self::Volume => write!(f, "Volume"),
        }
    }
}

#[derive(Debug, Clone)]
struct UnitDef {
    symbol: &'static str,
    category: UnitCategory,
    /// Factor to convert 1 of this unit into the category's base unit.
    /// For temperature this is unused (handled via formula).
    factor: f64,
}

fn all_units() -> Vec<UnitDef> {
    let mut u = Vec::new();

    // ── Time (base: seconds) ────────────────────────────────────────────
    for (sym, f) in &[
        ("ns", 1e-9),
        ("μs", 1e-6),
        ("ms", 1e-3),
        ("s", 1.0),
        ("min", 60.0),
        ("h", 3_600.0),
        ("d", 86_400.0),
        ("w", 604_800.0),
        ("mo", 2_629_746.0), // avg month
        ("y", 31_556_952.0), // avg year
    ] {
        u.push(UnitDef { symbol: sym, category: UnitCategory::Time, factor: *f });
    }

    // ── Length (base: metres) ───────────────────────────────────────────
    for (sym, f) in &[
        ("nm", 1e-9),
        ("μm", 1e-6),
        ("mm", 1e-3),
        ("cm", 1e-2),
        ("m", 1.0),
        ("km", 1_000.0),
        ("in", 0.0254),
        ("ft", 0.3048),
        ("yd", 0.9144),
        ("mi", 1_609.344),
    ] {
        u.push(UnitDef { symbol: sym, category: UnitCategory::Length, factor: *f });
    }

    // ── Weight (base: grams) ────────────────────────────────────────────
    for (sym, f) in &[
        ("mg", 0.001),
        ("g", 1.0),
        ("kg", 1_000.0),
        ("t", 1_000_000.0),
        ("oz", 28.3495),
        ("lb", 453.592),
    ] {
        u.push(UnitDef { symbol: sym, category: UnitCategory::Weight, factor: *f });
    }

    // ── Digital (base: bytes) ───────────────────────────────────────────
    for (sym, f) in &[
        ("b", 0.125),  // bit
        ("B", 1.0),
        ("KB", 1_000.0),
        ("MB", 1_000_000.0),
        ("GB", 1_000_000_000.0),
        ("TB", 1_000_000_000_000.0),
        ("PB", 1_000_000_000_000_000.0),
    ] {
        u.push(UnitDef { symbol: sym, category: UnitCategory::Digital, factor: *f });
    }

    // ── Temperature (formula-based, factor is placeholder) ──────────────
    for sym in &["°C", "°F", "K"] {
        u.push(UnitDef { symbol: sym, category: UnitCategory::Temperature, factor: 0.0 });
    }

    // ── Speed (base: m/s) ──────────────────────────────────────────────
    for (sym, f) in &[
        ("m/s", 1.0),
        ("km/h", 0.277778),
        ("mph", 0.44704),
        ("kn", 0.514444),
    ] {
        u.push(UnitDef { symbol: sym, category: UnitCategory::Speed, factor: *f });
    }

    // ── Volume (base: litres) ──────────────────────────────────────────
    for (sym, f) in &[
        ("mL", 0.001),
        ("L", 1.0),
        ("gal", 3.78541),
        ("qt", 0.946353),
        ("pt", 0.473176),
        ("cup", 0.236588),
    ] {
        u.push(UnitDef { symbol: sym, category: UnitCategory::Volume, factor: *f });
    }

    u
}

/// Find a unit definition by symbol (exact match).
fn find_unit(sym: &str) -> Option<UnitDef> {
    all_units().into_iter().find(|u| u.symbol == sym)
}

// ---------------------------------------------------------------------------
// Parsing input like "10.5km" → (10.5, "km")
// ---------------------------------------------------------------------------

fn parse_input(raw: &str) -> Result<(f64, &str), String> {
    let raw = raw.trim();
    // Find where the number ends and the unit begins.
    let split = raw
        .char_indices()
        .find(|&(_, c)| !c.is_ascii_digit() && c != '.' && c != '+' && c != '-')
        .map(|(i, _)| i)
        .or_else(|| {
            // whole string is a number – no unit
            None
        });

    let (num_str, unit_str) = match split {
        Some(i) => (&raw[..i], &raw[i..]),
        None => (raw, ""),
    };

    let value: f64 = num_str.parse().map_err(|_| format!("invalid number: {num_str}"))?;
    if unit_str.is_empty() {
        return Err("missing unit (e.g. 10.5km, 3s)".into());
    }
    Ok((value, unit_str))
}

// ---------------------------------------------------------------------------
// Temperature helpers
// ---------------------------------------------------------------------------

fn temp_to_base(symbol: &str, value: f64) -> f64 {
    // base = Celsius
    match symbol {
        "°C" => value,
        "°F" => (value - 32.0) * 5.0 / 9.0,
        "K" => value - 273.15,
        _ => unreachable!(),
    }
}

fn base_to_temp(symbol: &str, celsius: f64) -> f64 {
    match symbol {
        "°C" => celsius,
        "°F" => celsius * 9.0 / 5.0 + 32.0,
        "K" => celsius + 273.15,
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

fn convert(value: f64, from: &UnitDef, to: &UnitDef) -> f64 {
    if from.category == UnitCategory::Temperature && to.category == UnitCategory::Temperature {
        let base = temp_to_base(from.symbol, value);
        return base_to_temp(to.symbol, base);
    }
    // standard factor-based conversion
    let base_value = value * from.factor;
    base_value / to.factor
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

fn format_value(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let abs = v.abs();
    if abs >= 1e15 || (abs != 0.0 && abs < 1e-9) {
        format!("{v:.6e}")
    } else if abs >= 1_000_000.0 {
        format!("{v:.2}")
    } else if abs >= 100.0 {
        format!("{v:.4}")
    } else if abs >= 1.0 {
        format!("{v:.6}")
    } else {
        format!("{v:.8}")
    }
    .trim_end_matches('0')
    .trim_end_matches('.')
    .into()
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

fn print_converted(value: f64, from_sym: &str, to_sym: &str, result: f64) {
    println!(
        "{} {} = {} {}",
        Theme::accent(&format_value(value)),
        Theme::header(from_sym),
        Theme::accent(&format_value(result)),
        Theme::header(to_sym),
    );
}

fn print_category_list(value: f64, from: &UnitDef) {
    println!();
    println!(
        "{} {} conversions",
        Theme::header("◆"),
        Theme::header(&from.category.to_string()),
    );
    divider();

    let all = all_units();
    let units: Vec<&UnitDef> = all.iter().filter(|u| u.category == from.category).collect();

    for target in &units {
        if target.symbol == from.symbol {
            println!(
                "  {} {} {}",
                Theme::muted("▸"),
                Theme::accent(&format!("{} (base)", from.symbol)),
                Theme::muted("(you are here)"),
            );
            continue;
        }
        let result = convert(value, from, target);
        println!(
            "  {} {} {} {} {}",
            Theme::muted("▸"),
            Theme::accent(&format_value(result)),
            Theme::header(target.symbol),
            Theme::muted("←"),
            Theme::muted(&format!("{} {}", format_value(value), from.symbol)),
        );
    }
    println!();
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { input, to } => {
            let (value, from_sym) = match parse_input(&input) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{} {e}", Theme::header("error:"));
                    std::process::exit(1);
                }
            };

            let from = match find_unit(from_sym) {
                Some(u) => u,
                None => {
                    eprintln!("{} unknown unit '{from_sym}'", Theme::header("error:"));
                    eprintln!(
                        "{} supported units: {}",
                        Theme::muted("hint:"),
                        Theme::muted(
                            &all_units()
                                .iter()
                                .map(|u| u.symbol)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    );
                    std::process::exit(1);
                }
            };

            match to {
                Some(target_sym) => {
                    let to = match find_unit(&target_sym) {
                        Some(u) => u,
                        None => {
                            eprintln!(
                                "{} unknown unit '{target_sym}'",
                                Theme::header("error:")
                            );
                            std::process::exit(1);
                        }
                    };

                    if from.category != to.category {
                        eprintln!(
                            "{} cannot convert {} ({}) to {} ({})",
                            Theme::header("error:"),
                            from.symbol,
                            from.category,
                            to.symbol,
                            to.category,
                        );
                        std::process::exit(1);
                    }

                    let result = convert(value, &from, &to);
                    println!();
                    print_converted(value, from.symbol, to.symbol, result);
                    println!();
                }
                None => {
                    print_category_list(value, &from);
                }
            }
        }
    }
}
