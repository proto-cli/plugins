use clap::{Parser, Subcommand};
use proto_plugin_sdk::*;

#[derive(Parser)]
#[command(name = "discord", about = "Discord bot project management")]
struct Cli {
    #[command(subcommand)]
    action: DiscordAction,
}

#[derive(Subcommand, Debug, Clone)]
enum DiscordAction {
    #[command(about = "Discord bot project management")]
    Bot {
        #[command(subcommand)]
        action: BotAction,
    },
    #[command(about = "Quest completion injector (WIP)")]
    Quest,
}

#[derive(Subcommand, Debug, Clone)]
enum BotAction {
    #[command(about = "Create a new Discord bot project with interactive setup")]
    Create {
        #[arg(long, value_name = "LANG", help = "Language: python, rust, javascript, typescript, csharp, cpp")]
        language: Option<String>,
        #[arg(long, value_name = "TEMPLATE", help = "Template: slash_command, prefix, none, repeater, counter")]
        template: Option<String>,
        #[arg(long, value_name = "NAME", help = "Bot project name")]
        name: Option<String>,
    },
}

fn proto_banner() -> String {
    use proto_plugin_sdk::OwoColorize;
    format!("{} {}", "◆".style(Theme::ACCENT), "Proto Discord".style(Theme::HEADER))
}

fn main() {
    let cli = Cli::parse();
    match &cli.action {
        DiscordAction::Bot { action } => bot(action),
        DiscordAction::Quest => quest(),
    }
}

fn bot(action: &BotAction) {
    match action {
        BotAction::Create { language, template, name } => bot_create(language.as_deref(), template.as_deref(), name.as_deref()),
    }
}

fn bot_create(cli_lang: Option<&str>, cli_tmpl: Option<&str>, cli_name: Option<&str>) {
    use dialoguer::{Confirm, Input, Select};

    println!("{}", proto_banner());
    println!("{}\n", "Discord Bot Creator".style(Theme::HEADER));

    let languages = &[
        "python     (discord.py)", "rust       (serenity)", "javascript (discord.js)",
        "typescript (discord.js)", "csharp     (Discord.Net)", "cpp        (DPP / instructions)",
    ];
    let lang_keys = &["python", "rust", "javascript", "typescript", "csharp", "cpp"];
    let lang_idx = if let Some(l) = cli_lang {
        lang_keys.iter().position(|k| *k == l).unwrap_or_else(|| {
            Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
                .with_prompt("Language").items(languages).default(0).interact().unwrap_or(0)
        })
    } else {
        Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Language").items(languages).default(0).interact().unwrap_or(0)
    };
    let lang = lang_keys[lang_idx];

    let templates = &[
        "slash_command  (modern, interaction-based)", "prefix         (classic !command style)",
        "repeater       (echoes messages back)", "counter        (simple counting bot)",
        "none           (bare minimum skeleton)",
    ];
    let tmpl_keys = &["slash_command", "prefix", "repeater", "counter", "none"];
    let tmpl_idx = if let Some(t) = cli_tmpl {
        tmpl_keys.iter().position(|k| *k == t).unwrap_or_else(|| {
            Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
                .with_prompt("Template").items(templates).default(0).interact().unwrap_or(0)
        })
    } else {
        Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Template").items(templates).default(0).interact().unwrap_or(0)
    };
    let tmpl = tmpl_keys[tmpl_idx];

    let bot_name: String = if let Some(n) = cli_name {
        n.to_string()
    } else {
        Input::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Bot name").default("MyProtoBot".into()).interact_text().unwrap()
    };

    let folder = bot_name.replace(' ', "-").to_lowercase();

    println!("\n{}", divider());
    println!("{}", label_value("Name", &bot_name));
    println!("{}", label_value("Language", lang));
    println!("{}", label_value("Template", tmpl));
    println!("{}", label_value("Folder", &folder));
    println!("{}", divider());
    println!();

    let proceed = Confirm::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Create bot project?").default(true).interact().unwrap_or(true);
    if !proceed {
        println!("{}", "Aborted.".style(Theme::MUTED));
        return;
    }

    let pkg_mgr = if matches!(lang, "javascript" | "typescript") {
        let mgrs = &["npm", "pnpm", "bun"];
        let idx = Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Package manager").items(mgrs).default(0).interact().unwrap_or(0);
        mgrs[idx].to_string()
    } else {
        String::new()
    };

    let sp = Spinner::new(&format!("Creating {}...", folder));
    std::fs::create_dir_all(&folder).unwrap();
    let path = std::path::Path::new(&folder);
    let env = write_env_template(path);

    let files = match lang {
        "python" => write_python(path, tmpl),
        "rust" => write_rust(path, &bot_name, tmpl),
        "javascript" => write_js(path, &bot_name, tmpl, &pkg_mgr, false),
        "typescript" => write_js(path, &bot_name, tmpl, &pkg_mgr, true),
        "csharp" => write_csharp(path, &bot_name, tmpl),
        "cpp" => write_cpp(path, &bot_name, tmpl),
        _ => vec![],
    };

    sp.done(&format!("Created '{}'", bot_name));
    println!();
    for f in &files { println!("  {} {}", "✦".style(Theme::SUCCESS), f.style(Theme::MUTED)); }
    println!("  {} {}", "✦".style(Theme::SUCCESS), env.style(Theme::MUTED));
    println!("\n{}", divider());
    println!("{}", "Next steps:".style(Theme::HEADER));
    println!("  cd {}", folder.style(Theme::ACCENT));
    match lang {
        "python" => println!("  pip install -r requirements.txt"),
        "rust" => println!("  cargo build"),
        "javascript" | "typescript" => {
            if lang == "typescript" { println!("  npm install && npm run build"); }
            else { println!("  npm install"); }
        }
        "csharp" => println!("  dotnet restore && dotnet run"),
        "cpp" => println!("  cmake -B build && cmake --build build"),
        _ => {}
    }
    println!("\n  {} Put your bot token in {}", "1.".dimmed(), ".env".style(Theme::ACCENT));
    println!("  {} Fill in the application ID and start coding!", "2.".dimmed());
}

fn quest() {
    println!();
    println!("{}", proto_banner());
    println!("{}", "Quest Injector".style(Theme::HEADER));
    println!("\n{} {}", "⏳".style(Theme::WARN), "This feature is a work in progress.".style(Theme::MUTED));
    println!("{} Inject quest completion into the Discord client process.\n", "  ".dimmed());
}

fn write_env_template(root: &std::path::Path) -> String {
    let content = "DISCORD_TOKEN=your_bot_token_here\nDISCORD_APP_ID=your_application_id_here\n";
    let path = root.join(".env.example");
    std::fs::write(&path, content).unwrap();
    ".env.example".into()
}

fn write_python(root: &std::path::Path, tmpl: &str) -> Vec<String> {
    let mut files = Vec::new();
    let code = match tmpl {
        "slash_command" => PY_SLASH, "prefix" => PY_PREFIX, "repeater" => PY_REPEATER,
        "counter" => PY_COUNTER, _ => PY_NONE,
    };
    std::fs::write(root.join("main.py"), code).unwrap();
    files.push("main.py".into());
    std::fs::write(root.join("requirements.txt"), "discord.py\npython-dotenv\n").unwrap();
    files.push("requirements.txt".into());
    files
}

fn write_rust(root: &std::path::Path, name: &str, tmpl: &str) -> Vec<String> {
    let mut files = Vec::new();
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let code = match tmpl {
        "slash_command" => RS_SLASH, "prefix" => RS_PREFIX, "repeater" => RS_REPEATER,
        "counter" => RS_COUNTER, _ => RS_NONE,
    };
    std::fs::write(src.join("main.rs"), code).unwrap();
    files.push("src/main.rs".into());
    let cargo = format!(r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
serenity = "0.12"
tokio = {{ version = "1", features = ["full"] }}
dotenvy = "0.15"
"#, name.to_lowercase().replace(' ', "_"));
    std::fs::write(root.join("Cargo.toml"), cargo).unwrap();
    files.push("Cargo.toml".into());
    files
}

fn write_js(root: &std::path::Path, name: &str, tmpl: &str, pkg_mgr: &str, ts: bool) -> Vec<String> {
    let mut files = Vec::new();
    let ext = if ts { "ts" } else { "js" };
    let src_dir = if ts {
        let d = root.join("src");
        std::fs::create_dir_all(&d).unwrap();
        d
    } else { root.to_path_buf() };

    let code = match tmpl {
        "slash_command" if ts => TS_SLASH, "slash_command" => JS_SLASH,
        "prefix" if ts => TS_PREFIX, "prefix" => JS_PREFIX,
        "repeater" if ts => TS_REPEATER, "repeater" => JS_REPEATER,
        "counter" if ts => TS_COUNTER, "counter" => JS_COUNTER,
        _ if ts => TS_NONE, _ => JS_NONE,
    };
    std::fs::write(src_dir.join(format!("index.{}", ext)), code).unwrap();
    files.push(format!("index.{}", ext));

    let deps = r#""discord.js": "^14.16.0""#;
    let dev_deps = if ts { r#""typescript": "^5.0.0", "ts-node": "^10.0.0""# } else { "" };
    let pkg = if ts {
        format!(r#"{{
  "name": "{}",
  "version": "1.0.0",
  "main": "dist/index.js",
  "scripts": {{"build": "tsc", "start": "node dist/index.js", "dev": "ts-node src/index.ts"}},
  "dependencies": {{ {} }},
  "devDependencies": {{ {} }}
}}
"#, name.to_lowercase().replace(' ', "-"), deps, dev_deps)
    } else {
        format!(r#"{{
  "name": "{}",
  "version": "1.0.0",
  "main": "index.js",
  "scripts": {{"start": "node index.js"}},
  "dependencies": {{ {} }}
}}
"#, name.to_lowercase().replace(' ', "-"), deps)
    };
    std::fs::write(root.join("package.json"), &pkg).unwrap();
    files.push("package.json".into());

    if ts {
        let tsconfig = r#"{"compilerOptions":{"target":"ES2022","module":"commonjs","outDir":"dist","rootDir":"src","strict":true,"esModuleInterop":true,"skipLibCheck":true},"include":["src"]}
"#;
        std::fs::write(root.join("tsconfig.json"), tsconfig).unwrap();
        files.push("tsconfig.json".into());
    }

    if !pkg_mgr.is_empty() {
        let install_cmd = if pkg_mgr == "bun" {
            std::process::Command::new("bun").arg("install").current_dir(root).status()
        } else if pkg_mgr == "pnpm" {
            std::process::Command::new("pnpm").arg("install").current_dir(root).status()
        } else {
            std::process::Command::new("npm").arg("install").current_dir(root).status()
        };
        if install_cmd.map(|s| s.success()).unwrap_or(false) {
            files.push(format!("(dependencies installed via {})", pkg_mgr));
        }
    }
    files
}

fn write_csharp(root: &std::path::Path, name: &str, tmpl: &str) -> Vec<String> {
    let mut files = Vec::new();
    let safe = name.replace(' ', "");
    let code = match tmpl {
        "slash_command" => CS_SLASH, "prefix" => CS_PREFIX, "repeater" => CS_REPEATER, _ => CS_NONE,
    };
    std::fs::write(root.join("Program.cs"), code).unwrap();
    files.push("Program.cs".into());
    let csproj = format!(r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework></PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Discord.Net" Version="3.15.0" />
    <PackageReference Include="DotNetEnv" Version="3.0.0" />
  </ItemGroup>
</Project>
"#);
    std::fs::write(root.join(format!("{}.csproj", safe)), csproj).unwrap();
    files.push(format!("{}.csproj", safe));
    files
}

fn write_cpp(root: &std::path::Path, name: &str, tmpl: &str) -> Vec<String> {
    let mut files = Vec::new();
    let code = match tmpl { "slash_command" => CPP_SLASH, "prefix" => CPP_PREFIX, _ => CPP_NONE };
    std::fs::write(root.join("main.cpp"), code).unwrap();
    files.push("main.cpp".into());
    let cmake = format!(r#"cmake_minimum_required(VERSION 3.16)
project({} LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 20)
add_executable(bot main.cpp)
target_link_libraries(bot PRIVATE dpp)
"#, name.to_lowercase().replace(' ', "_"));
    std::fs::write(root.join("CMakeLists.txt"), cmake).unwrap();
    files.push("CMakeLists.txt".into());
    let readme = "Dependencies: libdpp (https://dpp.dev)\nInstall: vcpkg install dpp or see dpp.dev/install.html\n";
    std::fs::write(root.join("SETUP.md"), readme).unwrap();
    files.push("SETUP.md".into());
    files
}

// ─── Templates ───────────────────

const PY_SLASH: &str = r#"
import os, discord
from discord import app_commands
from dotenv import load_dotenv
load_dotenv()

class MyBot(discord.Client):
    def __init__(self):
        intents = discord.Intents.default()
        super().__init__(intents=intents)
        self.tree = app_commands.CommandTree(self)
    async def setup_hook(self):
        await self.tree.sync()

bot = MyBot()

@bot.tree.command(name="ping", description="Replies with pong")
async def ping(interaction: discord.Interaction):
    await interaction.response.send_message("Pong!")

@bot.event
async def on_ready():
    print(f"Logged in as {bot.user}")

bot.run(os.getenv("DISCORD_TOKEN"))
"#;

const PY_PREFIX: &str = r#"
import os, discord
from discord.ext import commands
from dotenv import load_dotenv
load_dotenv()

bot = commands.Bot(command_prefix="!", intents=discord.Intents.default())

@bot.command()
async def ping(ctx):
    await ctx.send("Pong!")

@bot.event
async def on_ready():
    print(f"Logged in as {bot.user}")

bot.run(os.getenv("DISCORD_TOKEN"))
"#;

const PY_REPEATER: &str = r#"
import os, discord
from dotenv import load_dotenv
load_dotenv()

class Repeater(discord.Client):
    async def on_ready(self):
        print(f"Logged in as {self.user}")
    async def on_message(self, msg):
        if msg.author == self.user:
            return
        await msg.channel.send(msg.content)

Repeater(intents=discord.Intents.all()).run(os.getenv("DISCORD_TOKEN"))
"#;

const PY_COUNTER: &str = r#"
import os, discord
from discord.ext import commands
from dotenv import load_dotenv
load_dotenv()

bot = commands.Bot(command_prefix="!", intents=discord.Intents.default())
count = 0

@bot.command()
async def count(ctx):
    global count; count += 1
    await ctx.send(f"Count: {count}")

@bot.event
async def on_ready():
    print(f"Counter bot ready: {bot.user}")

bot.run(os.getenv("DISCORD_TOKEN"))
"#;

const PY_NONE: &str = r#"
import os, discord
from dotenv import load_dotenv
load_dotenv()

class Bot(discord.Client):
    async def on_ready(self):
        print(f"Logged in as {self.user}")

Bot(intents=discord.Intents.default()).run(os.getenv("DISCORD_TOKEN"))
"#;

const RS_SLASH: &str = r#"
use serenity::all::*;
use serenity::model::prelude::*;
use std::env;

struct Handler;

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("Logged in as {}", ready.user.name);
        let _ = Command::create_global_command(&ctx.http, CreateCommand::new("ping")
            .description("Replies with pong")).await;
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(cmd) = interaction {
            if cmd.data.name == "ping" {
                let _ = cmd.create_interaction_response(&ctx.http, CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().content("Pong!")
                )).await;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let mut client = Client::builder(&token, GatewayIntents::default())
        .event_handler(Handler).await.unwrap();
    client.start().await.unwrap();
}
"#;

const RS_PREFIX: &str = r#"
use serenity::all::*;
use serenity::model::prelude::*;
use std::env;

struct Handler;

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Logged in as {}", ready.user.name);
    }
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            let _ = msg.channel_id.say(&ctx.http, "Pong!").await;
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let mut client = Client::builder(&token, GatewayIntents::default())
        .event_handler(Handler).await.unwrap();
    client.start().await.unwrap();
}
"#;

const RS_REPEATER: &str = r#"
use serenity::all::*;
use std::env;

struct Handler;
#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Logged in as {}", ready.user.name);
    }
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot { return; }
        let _ = msg.channel_id.say(&ctx.http, &msg.content).await;
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let mut client = Client::builder(&token, GatewayIntents::all())
        .event_handler(Handler).await.unwrap();
    client.start().await.unwrap();
}
"#;

const RS_COUNTER: &str = r#"
use serenity::all::*;
use std::{env, sync::atomic::{AtomicU64, Ordering}};

struct Handler { count: AtomicU64 }
#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Counter bot ready: {}", ready.user.name);
    }
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!count" {
            let n = self.count.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = msg.channel_id.say(&ctx.http, format!("Count: {n}")).await;
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let mut client = Client::builder(&token, GatewayIntents::default())
        .event_handler(Handler { count: AtomicU64::new(0) }).await.unwrap();
    client.start().await.unwrap();
}
"#;

const RS_NONE: &str = r#"
use serenity::all::*;
use std::env;

struct Handler;
#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Logged in as {}", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let mut client = Client::builder(&token, GatewayIntents::default())
        .event_handler(Handler).await.unwrap();
    client.start().await.unwrap();
}
"#;

const JS_SLASH: &str = r#"
const { Client, GatewayIntentBits, REST, Routes, SlashCommandBuilder } = require("discord.js");
require("dotenv").config();
const client = new Client({ intents: [GatewayIntentBits.Guilds] });
client.on("ready", async () => {
    console.log(`Logged in as ${client.user.tag}`);
    const rest = new REST({ version: "10" }).setToken(process.env.DISCORD_TOKEN);
    await rest.put(Routes.applicationCommands(process.env.DISCORD_APP_ID), {
        body: [new SlashCommandBuilder().setName("ping").setDescription("Pong!")],
    });
});
client.on("interactionCreate", async (i) => {
    if (!i.isChatInputCommand()) return;
    if (i.commandName === "ping") await i.reply("Pong!");
});
client.login(process.env.DISCORD_TOKEN);
"#;

const JS_PREFIX: &str = r#"
const { Client, GatewayIntentBits } = require("discord.js");
require("dotenv").config();
const client = new Client({ intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildMessages, GatewayIntentBits.MessageContent] });
client.on("ready", () => console.log(`Logged in as ${client.user.tag}`));
client.on("messageCreate", async (msg) => {
    if (msg.author.bot) return;
    if (msg.content === "!ping") await msg.reply("Pong!");
});
client.login(process.env.DISCORD_TOKEN);
"#;

const JS_REPEATER: &str = r#"
const { Client, GatewayIntentBits } = require("discord.js");
require("dotenv").config();
const client = new Client({ intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildMessages, GatewayIntentBits.MessageContent] });
client.on("ready", () => console.log(`Repeater: ${client.user.tag}`));
client.on("messageCreate", (msg) => {
    if (msg.author.bot) return;
    msg.channel.send(msg.content);
});
client.login(process.env.DISCORD_TOKEN);
"#;

const JS_COUNTER: &str = r#"
const { Client, GatewayIntentBits } = require("discord.js");
require("dotenv").config();
let count = 0;
const client = new Client({ intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildMessages, GatewayIntentBits.MessageContent] });
client.on("ready", () => console.log(`Counter: ${client.user.tag}`));
client.on("messageCreate", async (msg) => {
    if (msg.author.bot) return;
    if (msg.content === "!count") { count++; await msg.reply(`Count: ${count}`); }
});
client.login(process.env.DISCORD_TOKEN);
"#;

const JS_NONE: &str = r#"
const { Client, GatewayIntentBits } = require("discord.js");
require("dotenv").config();
const client = new Client({ intents: [GatewayIntentBits.Guilds] });
client.on("ready", () => console.log(`Logged in as ${client.user.tag}`));
client.login(process.env.DISCORD_TOKEN);
"#;

const TS_SLASH: &str = r#"
import { Client, GatewayIntentBits, REST, Routes, SlashCommandBuilder, ChatInputCommandInteraction } from "discord.js";
import "dotenv/config";
const client = new Client({ intents: [GatewayIntentBits.Guilds] });
client.on("ready", async () => {
    console.log(`Logged in as ${client.user?.tag}`);
    const rest = new REST({ version: "10" }).setToken(process.env.DISCORD_TOKEN!);
    await rest.put(Routes.applicationCommands(process.env.DISCORD_APP_ID!), {
        body: [new SlashCommandBuilder().setName("ping").setDescription("Pong!")],
    });
});
client.on("interactionCreate", async (i: ChatInputCommandInteraction) => {
    if (i.commandName === "ping") await i.reply("Pong!");
});
client.login(process.env.DISCORD_TOKEN);
"#;

const TS_PREFIX: &str = r#"
import { Client, GatewayIntentBits } from "discord.js";
import "dotenv/config";
const client = new Client({ intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildMessages, GatewayIntentBits.MessageContent] });
client.on("ready", () => console.log(`Logged in as ${client.user?.tag}`));
client.on("messageCreate", async (msg) => {
    if (msg.author.bot) return;
    if (msg.content === "!ping") await msg.reply("Pong!");
});
client.login(process.env.DISCORD_TOKEN);
"#;

const TS_REPEATER: &str = r#"
import { Client, GatewayIntentBits } from "discord.js";
import "dotenv/config";
const client = new Client({ intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildMessages, GatewayIntentBits.MessageContent] });
client.on("ready", () => console.log(`Repeater: ${client.user?.tag}`));
client.on("messageCreate", (msg) => {
    if (msg.author.bot) return;
    msg.channel.send(msg.content);
});
client.login(process.env.DISCORD_TOKEN);
"#;

const TS_COUNTER: &str = r#"
import { Client, GatewayIntentBits } from "discord.js";
import "dotenv/config";
let count = 0;
const client = new Client({ intents: [GatewayIntentBits.Guilds, GatewayIntentBits.GuildMessages, GatewayIntentBits.MessageContent] });
client.on("ready", () => console.log(`Counter: ${client.user?.tag}`));
client.on("messageCreate", async (msg) => {
    if (msg.author.bot) return;
    if (msg.content === "!count") { count++; await msg.reply(`Count: ${count}`); }
});
client.login(process.env.DISCORD_TOKEN);
"#;

const TS_NONE: &str = r#"
import { Client, GatewayIntentBits } from "discord.js";
import "dotenv/config";
const client = new Client({ intents: [GatewayIntentBits.Guilds] });
client.on("ready", () => console.log(`Logged in as ${client.user?.tag}`));
client.login(process.env.DISCORD_TOKEN);
"#;

const CS_SLASH: &str = r#"
using Discord;
using Discord.Net;
using Discord.WebSocket;
using DotNetEnv;
Env.Load();
var client = new DiscordSocketClient();
client.Ready += async () => {
    Console.WriteLine($"Logged in as {client.CurrentUser}");
    var ping = new SlashCommandBuilder().WithName("ping").WithDescription("Pong!");
    await client.Rest.CreateGlobalCommand(ping.Build());
};
client.SlashCommandExecuted += async (cmd) => {
    if (cmd.Data.Name == "ping") await cmd.RespondAsync("Pong!");
};
await client.LoginAsync(TokenType.Bot, Environment.GetEnvironmentVariable("DISCORD_TOKEN"));
await client.StartAsync();
await Task.Delay(-1);
"#;

const CS_PREFIX: &str = r#"
using Discord;
using Discord.WebSocket;
using DotNetEnv;
Env.Load();
var config = new DiscordSocketConfig { GatewayIntents = GatewayIntents.AllUnprivileged | GatewayIntents.MessageContent };
var client = new DiscordSocketClient(config);
client.Ready += () => { Console.WriteLine($"Logged in as {client.CurrentUser}"); return Task.CompletedTask; };
client.MessageReceived += async (msg) => {
    if (msg.Author.IsBot) return;
    if (msg.Content == "!ping") await msg.Channel.SendMessageAsync("Pong!");
};
await client.LoginAsync(TokenType.Bot, Environment.GetEnvironmentVariable("DISCORD_TOKEN"));
await client.StartAsync();
await Task.Delay(-1);
"#;

const CS_REPEATER: &str = r#"
using Discord;
using Discord.WebSocket;
using DotNetEnv;
Env.Load();
var config = new DiscordSocketConfig { GatewayIntents = GatewayIntents.AllUnprivileged | GatewayIntents.MessageContent };
var client = new DiscordSocketClient(config);
client.Ready += () => { Console.WriteLine($"Repeater: {client.CurrentUser}"); return Task.CompletedTask; };
client.MessageReceived += async (msg) => {
    if (msg.Author.IsBot) return;
    await msg.Channel.SendMessageAsync(msg.Content);
};
await client.LoginAsync(TokenType.Bot, Environment.GetEnvironmentVariable("DISCORD_TOKEN"));
await client.StartAsync();
await Task.Delay(-1);
"#;

const CS_NONE: &str = r#"
using Discord;
using Discord.WebSocket;
using DotNetEnv;
Env.Load();
var config = new DiscordSocketConfig { GatewayIntents = GatewayIntents.AllUnprivileged };
var client = new DiscordSocketClient(config);
client.Ready += () => { Console.WriteLine($"Logged in as {client.CurrentUser}"); return Task.CompletedTask; };
await client.LoginAsync(TokenType.Bot, Environment.GetEnvironmentVariable("DISCORD_TOKEN"));
await client.StartAsync();
await Task.Delay(-1);
"#;

const CPP_SLASH: &str = r#"
#include <dpp/dpp.h>
#include <cstdlib>
int main() {
    const char* token = std::getenv("DISCORD_TOKEN");
    if (!token) { std::cerr << "Set DISCORD_TOKEN in .env\n"; return 1; }
    dpp::cluster bot(token, dpp::i_default_intents | dpp::i_message_content);
    bot.on_ready([&bot](const dpp::ready_t&) {
        std::cout << "Logged in as " << bot.me.username << "\n";
        bot.global_command_create(dpp::slashcommand("ping", "Pong!", bot.me.id));
    });
    bot.on_slashcommand([](const dpp::slashcommand_t& event) {
        if (event.command.get_command_name() == "ping") event.reply("Pong!");
    });
    bot.start(dpp::st_wait);
}
"#;

const CPP_PREFIX: &str = r#"
#include <dpp/dpp.h>
#include <cstdlib>
int main() {
    const char* token = std::getenv("DISCORD_TOKEN");
    if (!token) { std::cerr << "Set DISCORD_TOKEN in .env\n"; return 1; }
    dpp::cluster bot(token, dpp::i_default_intents | dpp::i_message_content);
    bot.on_ready([&bot](const dpp::ready_t&) {
        std::cout << "Logged in as " << bot.me.username << "\n";
    });
    bot.on_message_create([](const dpp::message_create_t& event) {
        if (event.msg.author.is_bot()) return;
        if (event.msg.content == "!ping") event.reply("Pong!");
    });
    bot.start(dpp::st_wait);
}
"#;

const CPP_NONE: &str = r#"
#include <dpp/dpp.h>
#include <cstdlib>
int main() {
    const char* token = std::getenv("DISCORD_TOKEN");
    if (!token) { std::cerr << "Set DISCORD_TOKEN in .env\n"; return 1; }
    dpp::cluster bot(token, dpp::i_default_intents | dpp::i_message_content);
    bot.on_ready([&bot](const dpp::ready_t&) {
        std::cout << "Logged in as " << bot.me.username << "\n";
    });
    bot.start(dpp::st_wait);
}
"#;
