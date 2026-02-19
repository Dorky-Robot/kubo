# kubo project instructions

## Architecture

kubo is an orchestrator that takes natural language intent and generates executable pipelines. It uses [tao](https://github.com/dorky_robot/tao) as the human interrupt primitive — tao handles suspend/resume, NDJSON pipes, and human-as-program semantics. kubo handles intent parsing, pipeline generation, action chain storage, and the browser UI.

```
kubo-core (action chains, stages, intent types)
  ↓
kubo-cli (CLI interface: "kubo 'plan a trip'")
kubo-web (browser UI: inbox, active chains, library)
```

- **kubo-core** — Core types: `Intent`, `Stage` (Shell | Human), `ActionChain`. No dependencies on tao internals — communicates via tao CLI.
- **kubo-cli** — CLI entry point. Parses intent, finds or generates action chain, executes via tao.
- **kubo-web** — Browser interface. Shows inbox (pending human stages), active chains, saved library, new request input.

## Design principles

- **tao is the runtime, kubo is the brain.** kubo generates pipelines, tao executes them. kubo never implements suspend/resume or channel delivery — that's tao's job.
- **Action chains are templates, not code.** A chain is a list of stages (shell commands + human stages) that can be parameterized and reused. Stored as JSON/TOML.
- **Generate first, reuse second.** First request generates a new chain via LLM. Subsequent similar requests match and reuse existing chains.
- **Shell commands are first class.** Any curl, jq, or executable can be a stage. No special integrations needed.
- **Humans are stages, not triggers.** A pipeline can ask a human something mid-flow. The pipeline suspends (via tao) and resumes when the human replies.

## Development rules

- `cargo test --workspace` must pass before push.
- `cargo fmt` and `cargo clippy` clean.
- No secrets in code or config files.
