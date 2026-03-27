Run the full kubo test and lint suite.

## Step 1: Run checks in parallel

Run all three checks concurrently:

```bash
cargo test --workspace
```

```bash
cargo fmt --all -- --check
```

```bash
cargo clippy --all-targets -- -D warnings
```

## Step 2: Report results

Summarize pass/fail for each check. If any failed, show the relevant error output and suggest fixes.

If all pass:
```
All kubo checks passed:
  - cargo test --workspace
  - cargo fmt --all -- --check
  - cargo clippy --all-targets -- -D warnings
```
