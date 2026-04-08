# Releasing kubo

## What changed

- Fix `kubo upgrade` failing with `Permission denied (os error 13)` on Homebrew installs. The non-sudo upgrade branch now `unlink`s the destination before copying, so a `0555`-mode file (Homebrew's default install permission) doesn't break the copy. As a side benefit, this also handles macOS's "can't open a running Mach-O binary for write" case — the running process keeps mapping the old (now unlinked) inode while `dest` is repointed at a fresh one.

## Steps

### 1. Bump the version

Edit `Cargo.toml` in the workspace root — change `version` under `[workspace.package]`:

```toml
[workspace.package]
version = "0.5.26"
```

### 2. Make sure it builds

```bash
cargo build --release
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

### 3. Commit and tag

```bash
git add Cargo.toml crates/kubo-cli/src/main.rs RELEASE.md
git commit -m "0.5.26: fix kubo upgrade EACCES on Homebrew installs"
git tag v0.5.26
git push origin main
git push origin v0.5.26
```

### 4. Create the GitHub release

```bash
gh release create v0.5.26 --title "v0.5.26" --notes "- Fix \`kubo upgrade\` failing with Permission denied on Homebrew installs
- Non-sudo upgrade path now unlinks the destination before copying, so 0555-mode files (Homebrew's default) and currently-running Mach-O binaries on macOS both work"
```

The release workflow (`.github/workflows/release.yml`) will automatically:
- Build binaries for all 4 targets (x86_64/aarch64 x linux/macos)
- Upload them to the release
- Update the Homebrew tap formula at `Dorky-Robot/homebrew-tap`

### 5. Verify

After the workflow completes (~2 min):

```bash
# Check the release has all 4 assets
gh release view v0.5.26 --repo Dorky-Robot/kubo

# Upgrade via brew (the broken self-upgrade in older versions can't bootstrap us here)
brew update && brew upgrade dorky-robot/tap/kubo

# Future upgrades from 0.5.26+ should now work via:
kubo upgrade
```
