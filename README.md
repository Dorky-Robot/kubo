# kubo

Isolated dev environments in Docker. Point it at a directory, get a fully loaded container.

```
kubo katulong    # mount ./katulong into a container, drop into zsh
kubo .           # mount current dir
```

The idea: run Claude Code (or anything) dangerously but safely — everything is isolated in a container while your project files stay mounted at `/work`.

## What's inside

The kubo image comes with:

- **Rust** (stable + clippy/rustfmt)
- **Node 22** (via fnm)
- **Go 1.24**
- **Claude Code** — plus `yolo` alias for `claude --dangerously-skip-permissions`
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
