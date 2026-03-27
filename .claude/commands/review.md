Run a multi-perspective review on a kubo pull request. Usage: /review <PR-number>

## Step 1: Fetch the PR diff

```bash
gh pr diff $ARGUMENTS
```

Also fetch the PR description for context:

```bash
gh pr view $ARGUMENTS --json title,body
```

## Step 2: Launch review agents in parallel

Send a **single message** with Task tool calls so they run concurrently. Each agent receives the PR title, body, and full diff.

1. **Security reviewer** (`security-reviewer` agent) — Docker command injection, volume mount path traversal, credential leaks, container privilege escalation, shell script safety.

2. **Correctness reviewer** (`correctness-reviewer` agent) — Container lifecycle state transitions, Docker CLI error handling, volume consistency, concurrent exec session safety, path handling edge cases.

3. **Code quality reviewer** (`code-quality-reviewer` agent) — Rust idioms, error handling with thiserror, crate boundary (kubo-core is library, kubo-cli is binary), test coverage, clippy compliance.

4. **Dockerfile reviewer** (`dockerfile-reviewer` agent) — Only if the diff touches files in `image/`. Image security, layer optimization, cross-platform ARM64/x86_64 correctness, entrypoint safety.

Each agent must end its response with exactly one verdict line:

```
VERDICT: APPROVE
VERDICT: APPROVE_WITH_NOTES
VERDICT: REQUEST_CHANGES
```

## Step 3: Synthesize verdicts

Combine all agent responses into a single review summary:

```
## Review Summary for PR #<N>

### Security
<verdict> — <key findings or "No issues">

### Correctness
<verdict> — <key findings or "No issues">

### Code Quality
<verdict> — <key findings or "No issues">

### Dockerfile (if applicable)
<verdict> — <key findings or "No issues">

### Overall
<APPROVE / APPROVE_WITH_NOTES / REQUEST_CHANGES>
<1-2 sentence summary>
```

## Step 4: Post as PR comment

```bash
gh pr comment $ARGUMENTS --body "<the review summary>"
```
