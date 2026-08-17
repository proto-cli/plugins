# Proto CLI Plugins

Official plugins for the [proto CLI](https://github.com/proto-cli/proto) - your friendly protogen CLI companion.

## Installation

```bash
# Install a plugin
proto plugins add mc-server

# Install a namespaced plugin
proto plugins add @proto/mc-server

# List installed plugins
proto plugins list

# Update all plugins
proto plugins update

# Remove a plugin
proto plugins remove mc-server
```

## Available Plugins

| Plugin | Description |
|---|---|
| `mc-server` | Minecraft server management |
| `mc` | Minecraft resource packs & servers |
| `ai` | AI chat, changelog generation, error explainer |
| `discord` | Discord bot & quest utilities |
| `docker` | Docker container management |
| `status` | Network monitoring tools |
| `app` | Project diagnostics & cleanup |
| `pr` | PR preparation & checkout |
| `git-extras` | Extended git utilities |
| `encrypt` | Encode, decode, hash cryptographic values |
| `media` | Image & video compression |
| `download` | Video & music download via yt-dlp |
| `memo` | Location-aware scratchpad notes |
| `todo` | Simple todo list manager |
| `focus` | Pomodoro focus timer |
| `convert` | Unit conversion |
| `gen-pass` | Secure password generator |
| `qr` | QR code generator |
| `search-docs` | Documentation search |
| `reader` | Syntax-highlighted file reader |
| `cert` | TLS certificate inspection |
| `dns` | DNS record lookup |
| `battery` | Battery health diagnostics |
| `ports` | Listening ports dashboard |
| `kill-heavy` | Process killer |
| `clean-cache` | Cache cleaning |
| `audit-deps` | Dependency vulnerability scanning |
| `tree-view` | Folder tree display |
| `readme` | README generator |
| `webhook` | Webhook listener |
| `copy-ctx` | Clipboard context bundler |
| `secret` | Secret scanning |
| `share` | File upload to temporary host |
| `share-session` | Terminal session sharing |
| `asciicast` | Terminal recording |
| `render-md` | Markdown renderer |
| `color-palette` | ANSI color palette display |
| `port-forward` | SSH port forwarding |
| `local-s3` | Local S3-compatible server |
| `dedupe` | Duplicate file finder |

## Creating Plugins

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to create your own plugin.

## Plugin Structure

Each plugin is a standalone Rust binary that proto spawns as a subprocess. Plugins declare their commands via a `plugin.toml` manifest.

```
plugin-name/
├── Cargo.toml
├── plugin.toml
└── src/
    └── main.rs
```

## License

MIT
