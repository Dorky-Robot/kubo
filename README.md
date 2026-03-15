# kubo

Run Claude Code with full autonomy in an isolated container. No risk to your host system.

```
kubo myproject
yolo    # claude --dangerously-skip-permissions, safe inside the container
```

kubo mounts your project into a Docker container with a complete dev stack. Claude can install packages, modify system files, run arbitrary commands — all sandboxed. Your project files stay synced at `/work/<project>/`, everything else is disposable.

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

## Persistent volumes

Each container gets named Docker volumes (`{name}-home` and `{name}-work`) that survive container recreates and image upgrades. Installed tools, shell history, and configuration persist across sessions. To clean up volumes when removing a container:

```bash
kubo rm myproject --volumes
```

## Export & import

Package a container into a portable `.kubo` archive and restore it elsewhere:

```bash
# Export a container
kubo export myproject
kubo export myproject -o ./backup.kubo

# Import on another machine
kubo import myproject.kubo
kubo import myproject.kubo -n new-name -d ./local/path
```

## Auto-updates

The Docker image is embedded in the kubo binary. When you upgrade kubo:

1. Image files change → new image hash baked into the binary
2. `kubo myproject` detects the mismatch → rebuilds image automatically
3. Existing containers on the old image → recreated with the new image

No manual `docker build` or `kubo rm` needed. Just upgrade and go.

You can also update a specific container to the latest image without waiting for reattach:

```bash
kubo update myproject
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
kubo update <name>            update container to latest image
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
