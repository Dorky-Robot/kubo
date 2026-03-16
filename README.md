# kubo

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

## Port forwarding

Containers use host networking — any port your app binds to is accessible on the host immediately. If you use [tunnels](https://github.com/Dorky-Robot/tunnels) to expose local ports via Cloudflare:

```bash
# Inside kubo
work > cd frontend
frontend main > npm run dev    # starts on port 3000

# On the host (separate terminal) — works because of host networking
tunnels route add app.dorkyrobot.com 3000 --tunnel prod
```

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

The kubo image comes with:

- **Claude Code** — plus `yolo` (passes all flags: `yolo --resume`, `yolo -p "fix the tests"`)
- **Rust** (stable + clippy/rustfmt)
- **Node 22** (via fnm)
- **Go 1.24**
- **GitHub CLI** (gh)
- **Build essentials** (gcc, pkg-config, libssl-dev, libsqlite3-dev)
- **Terminal tools**: fzf, ripgrep, fd, bat, eza, jq, htop, tmux
- **Zsh** with oh-my-zsh, autosuggestions, and syntax highlighting

Your host `~/.ssh`, `~/.config/gh`, and git identity are passed through (read-only where appropriate).

## Commands

```
kubo <dir>                    open dir in a container
kubo <name>                   attach to a named kubo
kubo new <name> <dirs...>     create a named kubo with multiple dirs
kubo add <name> <dirs...>     add dirs to an existing kubo
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
