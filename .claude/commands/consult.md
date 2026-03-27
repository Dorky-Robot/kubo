Consult the masters — review the kubo codebase through the lens of great software engineers.

## Phase 1: Map the Codebase

Thoroughly explore kubo's full project structure:

1. **Rust source** — all `.rs` files in `crates/kubo-core/src/` and `crates/kubo-cli/src/`
2. **Docker image** — `image/Dockerfile`, `image/entrypoint.sh`, `image/zshrc`, clipboard scripts
3. **Configuration** — `Cargo.toml` (workspace and per-crate), `CLAUDE.md`
4. **Scripts** — `scripts/release.sh`, `scripts/refresh.sh`, `install.sh`
5. **Build** — `crates/kubo-core/build.rs`

Read ALL source files. kubo is small enough to read completely — every module, every script.

## Phase 2: Launch Review Agents in Parallel

Send a single message with 6 Task tool calls so they run concurrently. Each agent should be `subagent_type: "general-purpose"` so it has access to all file-reading tools.

**Shared context for every agent prompt:**
```
You are reviewing **kubo**, a Rust CLI that creates isolated Docker dev environments.
- Workspace: kubo-core (library: container lifecycle, image management) and kubo-cli (binary: clap CLI)
- kubo shells out to `docker` CLI via std::process::Command (no Rust Docker library)
- State tracked via Docker labels, persistent volumes for /home/dev and /work
- Image context (Dockerfile, entrypoint, scripts) embedded in the binary at build time

Read ALL source files in crates/ and image/ before forming your review.
Report your top 5 findings ranked by impact. For each finding, cite the specific file and line.
Do NOT suggest changes that would reduce capabilities or fight Rust idioms.
```

### Agent 1: Rich Hickey — Simplicity & Data Orientation

Review kubo for complecting, accidental complexity, and data-over-abstraction opportunities. Key areas:
- Is `Container` (1100+ lines) complecting too many concerns? Should container creation, exec, mounts, and export/import be separate?
- Are Docker labels (the state model) simple data, or is the label-parsing logic entangled with business logic?
- Is the `std::process::Command` construction simple or accidentally complex?

### Agent 2: Joe Armstrong — Fault Tolerance & Isolation

Review kubo for failure handling and process isolation. Key areas:
- What happens when Docker commands fail mid-lifecycle (create succeeded but start failed)?
- Are exec sessions isolated from each other? What if one crashes?
- Is the deferred-mount-update mechanism resilient to process death?
- What happens when `kubo update` fails partway through rebuilding?

### Agent 3: Leslie Lamport — State Machines & Temporal Reasoning

Review kubo for state machine clarity. Key areas:
- Can you enumerate all container states and valid transitions?
- Are there impossible states that the code doesn't prevent (e.g., a container with `managed-by=kubo` but no `kubo.host-path` label)?
- Are Docker label updates atomic? What interleavings are possible?
- What invariants should always hold, and are they enforced?

### Agent 4: Sandi Metz — Practical Object Design

Review kubo for single responsibility and ease of change. Key areas:
- `container.rs` is 1100+ lines — where are the natural seams?
- `main.rs` is 530+ lines — is the CLI doing too much?
- Does the dependency direction (cli → core) hold consistently?
- What would break if you needed to support podman alongside docker?

### Agent 5: Kent Beck — Simple Design & Courage

Review kubo for YAGNI, test gaps, and bold simplifications. Key areas:
- Is the export/import feature pulling its weight, or is it premature?
- Are there configuration options nobody uses?
- What tests are missing? What edge cases aren't covered?
- What's the boldest simplification you'd make?

### Agent 6: Eric Evans — Domain-Driven Design

Review kubo for ubiquitous language and bounded contexts. Key areas:
- Do code names match the domain? (e.g., is "container" the right term, or is "environment" or "workspace" more accurate?)
- Are mount, volume, and path concepts clearly separated?
- Is the image-building concern properly bounded from container management?
- Would a domain expert (DevOps engineer) recognize the terminology?

## Phase 3: Distill

Wait for all six agents to complete. Then:

1. **Cross-reference** — findings multiple agents agree on are highest signal.
2. **Filter** — discard findings that would add abstraction without payoff or fight Rust idioms.
3. **Rank** — order by impact on maintainability, correctness, and developer experience.

## Phase 4: Build the Execution Plan

Create a phased plan grouped by tier:
- **Tier 1: Critical fixes** — correctness, safety
- **Tier 2: Type safety & cleanup** — dead code, stringly-typed fixes
- **Tier 3: Structural improvements** — decomposition of large files
- **Tier 4: Architectural evolution** — cross-cutting changes

## Phase 5: Present Plan and Get Feedback

**STOP HERE and present the plan to the user before doing any implementation.**

Ask the user how to proceed:
- **Execute all** — implement every tier
- **Execute Tier 1-2 only** — critical fixes and cleanup only
- **Let me adjust first** — user modifies the plan

## Phase 6: Execute

Implement approved tiers. After each phase: run `cargo test --workspace`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`. Commit after each phase.

## Phase 7: Ship

Run `/ship-it` to create a PR with the full review cycle.
