# kubo

Run Claude Code with full autonomy in an isolated container. No risk to your host system.

```
kubo myproject
yolo    # claude --dangerously-skip-permissions, inside the container
```

kubo mounts a directory into a Docker container with a complete dev stack. Claude can install packages, modify system files, run arbitrary commands — all sandboxed. Your project files stay synced at `/work`, everything else is disposable.

## Why

Claude Code is most useful when you let it run without guardrails — `--dangerously-skip-permissions` lets it edit files, run commands, and install tools without asking. But doing that on your host machine is risky. kubo gives Claude a full Ubuntu environment to go wild in, while keeping your host safe.

```bash
kubo .       # mount current dir into a container, drop into zsh
yolo         # let claude loose
```

## What's inside

The kubo image comes with:

- **Claude Code** — plus `yolo` alias for `claude --dangerously-skip-permissions`
- **Rust** (stable + clippy/rustfmt)
- **Node 22** (via fnm)
- **Go 1.24**
- **GitHub CLI** (gh)
- **Oh My Zsh** with autosuggestions and syntax highlighting
- **Terminal tools**: fzf, ripgrep, fd, bat, eza, jq, htop, tmux

Your host `~/.ssh`, `~/.config/gh`, and git identity are passed through (read-only).

## Usage

```bash
kubo <dir>          # open dir in an isolated container
kubo ls             # list kubo containers
kubo stop <name>    # stop a container
kubo rm <name>      # remove a container
kubo build          # build or rebuild the kubo image
```

First run builds the Docker image (takes a few minutes). After that, containers start instantly.

Containers persist — `exit` the shell and `kubo <dir>` again to reattach. The container keeps its state (installed packages, etc.) until you `kubo rm` it.

## Install

```bash
cargo install --path crates/kubo-cli
```

Requires Docker.
