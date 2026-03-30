# Releasing kubo

## What changed

- Fix diwa in container: use load-dynamic ONNX loading with Microsoft's official shared library

## Steps

### 1. Bump the version

Edit `Cargo.toml` in the workspace root — change `version` under `[workspace.package]`:

```toml
[workspace.package]
version = "0.5.12"
```

### 2. Make sure it builds

```bash
cargo build --release
cargo test --workspace
cargo clippy --workspace
```

### 3. Commit and tag

```bash
git add -A
git commit -m "0.5.12: add tunnels CLI, update delta + gh, use buildx"
git tag v0.5.12
git push origin main --tags
```

### 4. Create the GitHub release

```bash
gh release create v0.5.12 --title "v0.5.12" --notes "- Add tunnels CLI to container image
- Switch to docker buildx build
- Update delta to 0.19.1
- Update GitHub CLI to 2.89.0"
```

The release workflow (`.github/workflows/release.yml`) will automatically:
- Build binaries for all 4 targets (x86_64/aarch64 x linux/macos)
- Upload them to the release
- Update the Homebrew tap formula at `Dorky-Robot/homebrew-tap`

### 5. Verify

After the workflow completes (~5 min):

```bash
# Check the release has all 4 assets
gh release view v0.5.12 --repo Dorky-Robot/kubo

# Check the tap was updated
brew update && brew upgrade kubo

# Rebuild containers
kubo refresh
```
