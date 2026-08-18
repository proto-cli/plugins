use proto_plugin_sdk::*;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};
use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let file = match args.first() {
        Some(f) => f.clone(),
        None => {
            eprintln!("  {} Usage: render-md <FILE>", error(""));
            return;
        }
    };

    let content = match std::fs::read_to_string(&file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  {} Cannot read {}: {}", error(""), file, e);
            return;
        }
    };

    println!("{}", header("Render Markdown"));
    println!("{}", divider());

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(&content, options);

    render(parser);
    println!();
}

fn render<'a>(parser: impl Iterator<Item = Event<'a>>) {
    let mut in_code_block = false;
    #[allow(unused_assignments)]
    let mut code_lang = None;
    let mut out = io::stdout();

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading(level, _, _) => {
                    let _ = match level {
                        HeadingLevel::H1 => write!(out, "{}", "".bold()),
                        HeadingLevel::H2 => write!(out, "{}", "".bold()),
                        HeadingLevel::H3 => write!(out, "{}", "".bold()),
                        _ => write!(out, ""),
                    };
                }
                Tag::Paragraph => {
                    let _ = write!(out, "  ");
                }
                Tag::CodeBlock(kind) => {
                    in_code_block = true;
                    code_lang = match kind {
                        CodeBlockKind::Fenced(lang) if !lang.is_empty() => {
                            Some(lang.to_string())
                        }
                        _ => None,
                    };
                    if let Some(ref lang) = code_lang {
                        println!(
                            "  {} {}",
                            lang.style(Theme::MUTED),
                            "─".repeat(40).style(Theme::MUTED)
                        );
                    }
                }
                Tag::List(_) => {}
                Tag::Item => {
                    let _ = write!(out, "  ");
                }
                Tag::Emphasis => {
                    let _ = write!(out, "\x1b[3m");
                }
                Tag::Strong => {
                    let _ = write!(out, "\x1b[1m");
                }
                Tag::Strikethrough => {
                    let _ = write!(out, "\x1b[9m");
                }
                Tag::Link { .. } | Tag::Image { .. } => {}
                _ => {}
            },
            Event::End(end_tag) => match end_tag {
                Tag::Heading(_, _, _) | Tag::Paragraph => println!(),
                Tag::CodeBlock(_) => {
                    in_code_block = false;
                    println!("  {}", "─".repeat(46).style(Theme::MUTED));
                }
                Tag::List(_) => println!(),
                Tag::Emphasis => {
                    let _ = write!(out, "\x1b[23m");
                }
                Tag::Strong => {
                    let _ = write!(out, "\x1b[22m");
                }
                Tag::Strikethrough => {
                    let _ = write!(out, "\x1b[29m");
                }
                _ => {}
            },
            Event::Text(text) | Event::Code(text) => {
                if in_code_block {
                    print!("{}", text.style(Theme::VALUE));
                } else {
                    print!("{}", text);
                }
            }
            Event::SoftBreak => print!(" "),
            Event::HardBreak => println!(),
            Event::Rule => println!("  {}", "─".repeat(46).style(Theme::MUTED)),
            Event::TaskListMarker(checked) => {
                if checked {
                    print!("[x] ");
                } else {
                    print!("[ ] ");
                }
            }
            Event::Html(html) => {
                print!("{}", html.style(Theme::MUTED));
            }
            _ => {}
        }
        let _ = out.flush();
    }
}
