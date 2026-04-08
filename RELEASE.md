# Releasing kubo

## What changed

- Mount host `~/.claude/{skills,agents,CLAUDE.md}` read-only into kubo containers so sandbox Claude inherits the same skills (implement-mode, diwa, kubo, tunnels, …) and global instructions as the host. Mounted at `/kubo-host/claude/*` and symlinked into `/home/dev/.claude` from the entrypoint to avoid shadowing the persistent home volume's `~/.claude` (sessions, projects, memory).

## Steps

### 1. Bump the version

Edit `Cargo.toml` in the workspace root — change `version` under `[workspace.package]`:

```toml
[workspace.package]
version = "0.5.25"
```

### 2. Make sure it builds

```bash
cargo build --release
cargo test --workspace
cargo clippy --workspace
```

### 3. Commit and tag

```bash
git add Cargo.toml crates/kubo-core/src/container.rs image/entrypoint.sh RELEASE.md
git commit -m "0.5.25: mount host ~/.claude skills + CLAUDE.md into containers"
git tag v0.5.25
git push origin main --tags
```

### 4. Create the GitHub release

```bash
gh release create v0.5.25 --title "v0.5.25" --notes "- Mount host ~/.claude/{skills,agents,CLAUDE.md} read-only into kubo containers
- Sandbox Claude inside a kubo now inherits the same skills and global rules as the host
- Mounted at /kubo-host/claude/* and symlinked into /home/dev/.claude by the entrypoint
- Persistent home volume's ~/.claude (sessions, projects, memory) is preserved untouched"
```

The release workflow (`.github/workflows/release.yml`) will automatically:
- Build binaries for all 4 targets (x86_64/aarch64 x linux/macos)
- Upload them to the release
- Update the Homebrew tap formula at `Dorky-Robot/homebrew-tap`

### 5. Verify

After the workflow completes (~5 min):

```bash
# Check the release has all 4 assets
gh release view v0.5.25 --repo Dorky-Robot/kubo

# Check the tap was updated
brew update && brew upgrade kubo

# Rebuild containers
kubo refresh
```
