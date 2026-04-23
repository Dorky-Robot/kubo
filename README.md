# kubo

[![Discord](https://img.shields.io/discord/1483879594619568291?color=5865F2&label=Discord&logo=discord&logoColor=white)](https://dorkyrobot.com/discord)

Run Claude Code with full autonomy in an isolated container. No risk to your host system.

```
kubo myproject
yolo    # claude --dangerously-skip-permissions, safe inside the container
```

kubo mounts your project into a Docker container with a complete dev stack. Claude can install packages, modify system files, run arbitrary commands — all sandboxed. Your project files stay synced at `/work/<project>/`, and everything you install persists across updates.

## Why

Claude Code is most useful when you let it run without guardrails — `--dangerously-skip-permissions` lets it edit files, run commands, and install tools without asking. But doing that on your host machine is risky. kubo gives Claude a full dev environment to go wild in, while keeping your host safe.

## Quick start

```bash
# Install (macOS)
brew install Dorky-Robot/tap/kubo

# Open a project in an isolated container
kubo myproject

# Inside the container — let claude loose
yolo

# Resume a previous claude session
yolo --resume
```

## Multi-project workspaces

Mount multiple projects into a single kubo:

```bash
# Create a named kubo with several projects
kubo new fullstack ./frontend ./backend ./shared

# Add more projects later
kubo add fullstack ./docs ./infra

# Attach to it
kubo fullstack
```

Inside the container:

```
work > ls
backend/  docs/  frontend/  infra/  shared/

work > cd frontend
frontend main > yolo
```

## Staging with Cloudflare Tunnels

Containers use host networking and come with [cloudflared](https://github.com/cloudflare/cloudflared) built in. Spin up a dev server inside kubo and expose it to the internet in seconds:

```bash
# Inside kubo — start your app
work > cd frontend
frontend main > npm run dev    # starts on port 3000

# Quick ad-hoc tunnel (no config needed)
frontend main > cloudflared tunnel --url http://localhost:3000

# Or use tunnels (https://github.com/Dorky-Robot/tunnels) on the host
# for persistent subdomain routing — works because of host networking
tunnels route add app.dorkyrobot.com 3000 --tunnel prod
```

Your host's `~/.config/tunnels` is mounted read-only so tunnel tokens are available inside the container.

## Updates without data loss

Unlike vanilla Docker, kubo preserves your work across updates. Each container gets persistent volumes for `/home/dev` and `/work` — your shell history, Claude config, installed tools, and everything else survives when the container is rebuilt.

```bash
# Update a container — rebuilds image with latest tools, keeps all your data
kubo update myproject
```

This rebuilds the Docker image from scratch (fetching the latest versions of Claude Code, katulong, gh, etc.), recreates the container, and drops you right back in. Your `~/.claude` sessions, git repos, npm packages, and anything else you've set up are all still there.

To fully wipe a container and its data:

```bash
kubo rm myproject --volumes
```

## Auto-image management

The Docker image definition is embedded in the kubo binary. When you `brew upgrade kubo`:

1. New Dockerfile baked into the binary → image hash changes
2. Next `kubo myproject` detects the mismatch → rebuilds image automatically
3. You get the new image on your next attach — no extra steps

For on-demand updates (new tool versions without a kubo release):

```bash
kubo update myproject    # rebuild image + recreate container, keep data
kubo refresh             # rebuild image + update ALL running containers
```

## What's inside

**Dev stack:**

- **Claude Code** — plus `yolo` (passes all flags: `yolo --resume`, `yolo -p "fix the tests"`)
- **Rust** (stable + clippy/rustfmt), **Node 22** (fnm), **Go**
- **GitHub CLI** (gh)
- **Build essentials** (gcc, pkg-config, libssl-dev, libsqlite3-dev)
- **Terminal tools**: fzf, ripgrep, fd, bat, eza, jq, htop, tmux
- **Zsh** with oh-my-zsh, autosuggestions, and syntax highlighting

**[Dorky Robot](https://dorkyrobot.com) tools:**

- **[Katulong](https://github.com/Dorky-Robot/katulong)** — web terminal that lets you access your kubo sessions from any device (phone, tablet, another machine). Paste images from your device's clipboard into Claude Code sessions.
- **[Cloudflared](https://github.com/cloudflare/cloudflared)** — expose dev servers to the internet via Cloudflare Tunnels. Start an app inside kubo and share it instantly with `cloudflared tunnel --url http://localhost:3000`.
- **[Diwa](https://github.com/Dorky-Robot/diwa)** — turns git history into a searchable knowledge base. AI agents can query past decisions, patterns, and learnings with `diwa search repo "why..."`. Indexes are shared across kubos via the `~/.diwa` mount.
- **[Sipag](https://github.com/Dorky-Robot/sipag)** — autonomous PR agent. Picks up GitHub issues and opens pull requests using Claude Code. Runs inside kubo so it can't damage your host.

## Host config passthrough

kubo auto-detects tool configs on your host and mounts them into the container so credentials come with you. Only configs that exist are mounted — kubo works fine on machines without these tools.

| Host path | Purpose | Mode |
|---|---|---|
| `~/.ssh` | Git SSH keys | read-only |
| `~/.config/gh` | GitHub CLI auth | read-write |
| `~/.diwa` | [Diwa](https://github.com/Dorky-Robot/diwa) knowledge base and embeddings | read-write |
| `~/.config/tunnels` | [Tunnels](https://github.com/Dorky-Robot/tunnels) tokens and API keys | read-only |
| `~/.config/katulong` | Katulong instance config | read-only |
| `~/.config/yelo` | [Yelo](https://github.com/Dorky-Robot/yelo) S3/Glacier credentials | read-only |
| `~/.cloudflared` | Cloudflared auth certificate | read-only |
| `~/.katulong/uploads` | Clipboard bridge for image paste | read-write |

Git identity (`user.name`, `user.email`, signing key) is passed via environment variables so your commits inside the container are attributed correctly.

## Commands

```
kubo <dir>                    open dir in a container
kubo <name>                   attach to a named kubo
kubo new <name> <dirs...>     create a named kubo with multiple dirs
kubo add <name> <dirs...>     add dirs to an existing kubo
kubo detach <name> <dirs...>  remove dirs from a kubo
kubo update <name>            rebuild image + recreate container (keeps data)
kubo refresh                  rebuild image + update ALL containers
kubo export <name>            export container to a .kubo archive
kubo import <file>            import container from a .kubo archive
kubo ls                       list containers
kubo stop <name>              stop a container
kubo rm <name>                remove a container
kubo build                    force rebuild the image
kubo version                  show version and image hash
```

## Install

Requires Docker.

**macOS (Homebrew):**

```bash
brew install Dorky-Robot/tap/kubo
```

**Linux / macOS (script):**

```bash
curl -fsSL https://raw.githubusercontent.com/Dorky-Robot/kubo/main/install.sh | sh
```

**From source:**

```bash
cargo install --path crates/kubo-cli
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
