use chrono::{DateTime, Datelike, Duration, Local, Timelike, Weekday};
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
fn warn(s: &str) -> String { format!("{} {}", "⚠".style(Theme::WARN), s) }
fn muted(s: &str) -> String { format!("{}", s.style(Theme::MUTED)) }
fn divider() -> String { "─".repeat(55).dimmed().to_string() }

const DOW: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

#[derive(Debug, Clone, Default)]
struct CronJob {
    min: String,
    hour: String,
    dom: String,
    mon: String,
    dow: String,
    command: String,
}

fn parse_crontab() -> Vec<CronJob> {
    let out = Command::new("crontab").arg("-l").output();
    let Ok(out) = out else { return Vec::new() };
    let content = String::from_utf8_lossy(&out.stdout);
    let mut jobs = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("@") { continue; }
        let fields: Vec<&str> = t.splitn(6, char::is_whitespace).collect();
        if fields.len() < 6 { continue; }
        jobs.push(CronJob {
            min: fields[0].into(),
            hour: fields[1].into(),
            dom: fields[2].into(),
            mon: fields[3].into(),
            dow: fields[4].into(),
            command: fields[5..].join(" "),
        });
    }
    jobs
}

fn expand(field: &str, range: (u32, u32), allow_dow_names: bool) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    let parse = |s: &str| -> Option<u32> {
        if allow_dow_names && s.len() == 3 {
            if let Some(idx) = DOW.iter().position(|d| d.eq_ignore_ascii_case(s)) {
                return Some(idx as u32);
            }
        }
        s.parse::<u32>().ok().filter(|v| *v >= range.0 && *v <= range.1)
    };

    for part in field.split(',') {
        if part == "*" {
            out.extend(range.0..=range.1);
            continue;
        }
        let (base, step) = match part.split_once('/') {
            Some((b, s)) => (b, s.parse::<u32>().ok().unwrap_or(1).max(1)),
            None => (part, 1),
        };
        match parse(base) {
            Some(v) => out.extend((v..=range.1).step_by(step as usize)),
            None => {
                if base == "*" {
                    out.extend((range.0..=range.1).step_by(step as usize));
                } else if let Some((lo, hi)) = base.split_once('-') {
                    let (Some(l), Some(h)) = (parse(lo), parse(hi)) else { return None };
                    out.extend((l..=h.min(range.1)).step_by(step as usize));
                } else {
                    return None;
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    Some(out)
}

fn next_runs(job: &CronJob, now: DateTime<Local>, count: usize) -> Vec<DateTime<Local>> {
    let Some(mins) = expand(&job.min, (0, 59), false) else { return Vec::new() };
    let Some(hours) = expand(&job.hour, (0, 23), false) else { return Vec::new() };
    let Some(doms) = expand(&job.dom, (1, 31), false) else { return Vec::new() };
    let Some(mons) = expand(&job.mon, (1, 12), false) else { return Vec::new() };
    let Some(dows) = expand(&job.dow, (0, 6), true) else { return Vec::new() };

    let mut runs = Vec::new();
    let mut cursor = now;
    let mut guard = 0u32;
    while runs.len() < count && guard < 1_000_000 {
        guard += 1;
        cursor = cursor + Duration::minutes(1);
        if !mins.contains(&cursor.minute()) { continue; }
        if !hours.contains(&cursor.hour()) { continue; }
        if !mons.contains(&(cursor.month() as u32)) { continue; }
        let dow = cursor.weekday().num_days_from_sunday();
        let dom = cursor.day();
        let dow_ok = dows.contains(&dow);
        let dom_ok = if doms.contains(&dom) { true } else { false };
        let day_ok = if job.dow == "*" && job.dom == "*" {
            dom_ok
        } else if job.dow == "*" {
            dom_ok
        } else if job.dom == "*" {
            dow_ok
        } else {
            dow_ok || dom_ok
        };
        if !day_ok { continue; }
        runs.push(cursor);
    }
    runs
}

fn last_field<'a>(f: &'a str) -> &'a str {
    f.split(',').last().unwrap_or(f)
}

fn humanize_schedule(job: &CronJob) -> String {

    let time = if job.min == "*" && job.hour == "*" {
        "every minute".to_string()
    } else if job.hour == "*" {
        format!("every hour at minute {}", last_field(&job.min))
    } else if job.min == "*" {
        format!("every minute of hour {}", last_field(&job.hour))
    } else {
        let h: u32 = last_field(&job.hour).parse().unwrap_or(0);
        let m: u32 = last_field(&job.min).parse().unwrap_or(0);
        format!("at {:02}:{:02}", h, m)
    };

    let mut days = Vec::new();
    if job.dom != "*" {
        days.push(format!("day {} of month", job.dom.replace(',', " or ")));
    }
    if job.dow != "*" {
        let names: Vec<String> = job
            .dow
            .split(',')
            .map(|d| {
                d.parse::<u32>()
                    .ok()
                    .and_then(|n| DOW.get(n as usize).copied())
                    .unwrap_or(d)
                    .to_lowercase()
            })
            .collect();
        days.push(format!("on {}", names.join(" & ")));
    }
    if job.mon != "*" {
        days.push(format!("in month {}", job.mon));
    }

    match days.len() {
        0 => time,
        1 => format!("{} ({})", time, days[0]),
        _ => format!("{} ({})", time, days.join(", ")),
    }
}

fn list_jobs() {
    let jobs = parse_crontab();
    if jobs.is_empty() {
        println!("  {} No crontab entries ({} also empty)", muted(""), muted("crontab -l"));
        return;
    }
    println!("{} {}", header("Cron Jobs"), muted(&format!("({} tracked)", jobs.len())));
    println!("{}", divider());
    let now = Local::now();
    for (i, job) in jobs.iter().enumerate() {
        println!();
        println!(
            "  {} {}",
            format!("{:>3}.", i + 1).style(Theme::MUTED),
            humanize_schedule(job).style(Theme::ACCENT)
        );
        let cmd_preview: String = if job.command.chars().count() > 70 {
            job.command.chars().take(67).collect::<String>() + "…"
        } else {
            job.command.clone()
        };
        println!("  {} {}", "     ↳".dimmed(), cmd_preview.style(Theme::VALUE));
        let runs = next_runs(job, now, 3);
        for r in runs {
            let in_what = r - now;
            let label = if in_what.num_minutes() < 1 {
                "now".into()
            } else if in_what.num_minutes() < 60 {
                format!("in {}m", in_what.num_minutes())
            } else if in_what.num_hours() < 48 {
                format!("in {}h", in_what.num_hours())
            } else {
                format!("in {}d", in_what.num_days())
            };
            println!(
                "                          {}  {}",
                r.format("%a %b %d %H:%M").to_string().style(Theme::SUCCESS),
                label.dimmed()
            );
        }
    }
    println!();
    println!("{}", divider());
}

fn validate() {
    let jobs = parse_crontab();
    let mut ok = 0;
    let mut bad = 0;
    for job in &jobs {
        let v: Vec<Option<Vec<u32>>> = vec![
            expand(&job.min, (0, 59), false),
            expand(&job.hour, (0, 23), false),
            expand(&job.dom, (1, 31), false),
            expand(&job.mon, (1, 12), false),
            expand(&job.dow, (0, 6), true),
        ];
        if v.iter().all(|x| x.is_some()) {
            ok += 1;
        } else {
            bad += 1;
            println!("  {} {}", error(""), job.command.style(Theme::VALUE));
            for (f, valid) in zip_labels(job) {
                if !valid {
                    println!("      {} field: {}", "·".dimmed(), f.style(Theme::WARN));
                }
            }
        }
    }
    println!("  {} {} valid, {} invalid", "Σ".style(Theme::ACCENT), ok.to_string().style(Theme::SUCCESS), bad.to_string().style(Theme::ERROR));
}

fn zip_labels(job: &CronJob) -> Vec<(String, bool)> {
    vec![
        (format!("min {} (0-59)", job.min), expand(&job.min, (0, 59), false).is_some()),
        (format!("hour {} (0-23)", job.hour), expand(&job.hour, (0, 23), false).is_some()),
        (format!("dom {} (1-31)", job.dom), expand(&job.dom, (1, 31), false).is_some()),
        (format!("month {} (1-12)", job.mon), expand(&job.mon, (1, 12), false).is_some()),
        (format!("dow {} (0-6)", job.dow), expand(&job.dow, (0, 6), true).is_some()),
    ]
}

fn print_help() {
    println!("{} Cron Scheduler", header("proto"));
    println!("{}", divider());
    println!();
    println!("  USAGE:");
    println!("    proto cron            List jobs with next-run times");
    println!("    proto cron validate   Validate current crontab syntax");
    println!("    proto cron edit       Edit crontab in your editor");
    println!("    proto cron --help     Show this help");
    println!();
    println!("  EXAMPLES:");
    println!("    proto cron");
    println!("    proto cron validate");
    println!("    crontab -e   # create jobs");
    println!();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        None => list_jobs(),
        Some("--help") | Some("-h") | Some("help") => print_help(),
        Some("validate") => validate(),
        Some("edit") => {
            if let Ok(status) = Command::new("crontab").arg("-e").status() {
                let _ = status;
            } else {
                eprintln!("  {} crontab not available", error(""));
            }
        }
        Some(cmd) => {
            eprintln!("  {} Unknown command: {}", error(""), cmd);
            print_help();
            std::process::exit(1);
        }
    }
}