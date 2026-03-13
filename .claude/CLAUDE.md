# kubo project instructions

## Architecture

kubo provides isolated development environments using Docker containers. Point it at a directory, and it mounts that directory into a container where you can dev freely without risking your host system.

```
kubo-core (Container management, Docker interaction)
  ↓
kubo-cli (CLI interface: "kubo ./myproject")
```

- **kubo-core** — Container lifecycle: create, start, exec, stop, remove. Uses Docker CLI under the hood. Containers are labeled `managed-by=kubo` for tracking.
- **kubo-cli** — CLI entry point. `kubo <dir>` opens an isolated shell, `kubo ls/stop/rm` manage containers.

## Design principles

- **Simple first.** `kubo .` should just work — mount the current dir and drop into a shell.
- **Containers are persistent.** A kubo container sticks around (stopped) until you explicitly remove it. Re-running `kubo <dir>` reattaches.
- **Host dir is /work.** The mounted directory appears at `/work` inside the container.
- **Docker labels for state.** No external database — container labels (`managed-by=kubo`, `kubo.host-path`) are the source of truth.
- **Shell out to docker.** MVP uses `docker` CLI, not a Rust Docker library. Keep it simple.

## Development rules

- `cargo test --workspace` must pass before push.
- `cargo fmt` and `cargo clippy` clean.
- No secrets in code or config files.
