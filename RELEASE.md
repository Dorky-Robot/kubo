# Releasing kubo

## What changed

- Extract the upgrade binary-swap into a pure `replace_binary(src, dest)` function and lock in the 0.5.26 fix with two regression tests:
  - `replace_binary_overwrites_readonly_target` — proves the production function handles the Homebrew `0555` install case.
  - `naive_copy_fails_on_readonly_target` — reproduces the original `os error 13` against the bare `std::fs::copy` primitive, so any future refactor that drops the `unlink` will be caught immediately and explains *why* the unlink is load-bearing.

## Steps

### 1. Bump the version

Edit `Cargo.toml` in the workspace root — change `version` under `[workspace.package]`:

```toml
[workspace.package]
version = "0.5.27"
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
git add Cargo.toml crates/kubo-cli/Cargo.toml crates/kubo-cli/src/main.rs RELEASE.md
git commit -m "0.5.27: TDD regression tests for kubo upgrade Homebrew fix"
git tag v0.5.27
git push origin main
git push origin v0.5.27
```

### 4. Create the GitHub release

```bash
gh release create v0.5.27 --title "v0.5.27" --notes "- Extract upgrade binary-swap into a pure \`replace_binary\` function with TDD regression tests
- New tests pin both halves of the 0.5.26 fix: production function works on a 0555 dest, and the broken primitive (\`std::fs::copy\` alone) is proven to still fail so the unlink can never silently regress"
```

### 5. Verify

After the workflow completes (~2 min):

```bash
# Check the release has all 4 assets
gh release view v0.5.27 --repo Dorky-Robot/kubo

# Upgrade via brew (older self-upgrade is still broken; this is one-time)
brew update && brew upgrade dorky-robot/tap/kubo

# Future upgrades from 0.5.26+ now work via:
kubo upgrade
```
