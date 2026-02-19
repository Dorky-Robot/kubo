# kubo

Software on demand. State the outcome you want, kubo generates a pipeline to get it done.

kubo composes shell commands, API calls, and human stages into action chains — pipelines that suspend when they need human input and resume when humans reply. First time you ask for something, kubo generates the pipeline. Second time, it reuses it.

## How it works

```
you: "help me plan a trip with my friends"

kubo generates:
  curl travel-api/destinations
    → tao ask traveler felix "where do you want to go?"
    → curl flights-api/search --data {destination,dates}
    → tao ask traveler felix "book this flight?"
    → curl booking-api/reserve

kubo saves this as action chain: "plan-trip"
next time: "plan a trip" → reuses plan-trip
```

## Architecture

kubo is the orchestrator. [tao](https://github.com/dorky_robot/tao) is the runtime primitive.

```
kubo (orchestrator + browser UI)
  ├─ takes natural language intent
  ├─ generates pipeline DAGs (curls, shell commands, tao human stages)
  ├─ saves/reuses action chains as templates
  ├─ browser interface for humans to interact
  └─ uses tao as the "human interrupt" primitive
       └─ tao: suspend/resume, NDJSON pipes, actions, roles
```

### What makes this different

| Traditional workflow tools | kubo |
|---------------------------|------|
| Predefined nodes, drag-and-drop | State the outcome, pipeline generated |
| Fixed integrations | Any executable or curl is a stage |
| Always-on server | Suspend to SQLite, no process while waiting |
| Proprietary nodes | Shell scripts, any binary |
| Can't create new nodes at runtime | Creates new actions on the fly |
| Humans trigger workflows | Humans are stages inside workflows |

### Core concepts

- **Action chain** — A saved pipeline template. Generated from intent the first time, reused and adapted on repeat requests.
- **Stage** — One step in a pipeline: a shell command, an API call, or a human stage (via tao).
- **Human stage** — A point where the pipeline suspends and waits for a person to respond. Delivered via email, telegram, or the kubo browser UI.
- **Intent** — Natural language description of what you want done. kubo figures out the pipeline.

### Browser UI

- **Inbox** — Pending questions waiting for your input
- **Active chains** — Pipelines currently running, with stage-by-stage progress
- **Library** — Saved action chains you can reuse, share, or edit
- **New request** — Text box where you state what you want

## Project status

Early development. Not yet functional.

## Related projects

- **[tao](https://github.com/dorky_robot/tao)** — The human interrupt primitive. Treats humans as composable Unix programs.
- **[sipag](https://github.com/dorky_robot/sipag)** — Work dispatcher. Polls tao for suspended actions, runs Claude Code, opens PRs.
