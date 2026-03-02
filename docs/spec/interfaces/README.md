# Interface Design — Specification Index

This directory contains the canonical interface specifications produced by the
interface design phase of CogWorks development. Every source-code stub in the
workspace has a corresponding entry here.

**Coders must consult these specs.** Stubs enforce compile-time correctness;
specs describe **why** and **what** the types and traits mean.

---

## Workspace Structure

```
Cargo.toml                       (workspace root — 7 members)
crates/
  pipeline/                      # Domain types + business logic + all trait definitions
  nodes/                         # Node implementations + LLM gateway + PipelineExecutor
  github/                        # GitHub adapter (github-bot-sdk)
  llm/                           # Anthropic LLM provider adapter
  extension-api/                 # Domain service client (Extension API protocol)
  listener/                      # Trigger event sources (webhook + cloud queue)
  cli/                           # Entry point — composition root + observability wiring
docs/spec/interfaces/
  README.md                      — this file
  shared-types.md                — domain identifiers, value types, error types (PR 1)
  pipeline-graph.md              — pipeline graph model and runtime state (PR 2)
  github-traits.md               — GitHub + EventSource traits and supporting types (PR 3)
  domain-traits.md               — domain service, LLM, scenario, skill traits (PR 4)
  security.md                    — constitutional layer, injection detection (PR 5)
  context.md                     — context assembly, pack loading, label parsing (PR 6)
  pipeline-execution.md          — state machine, budget enforcement, review (PR 7)
  advanced-features.md           — alignment, traceability, observability, skills (PR 8)
  nodes.md                       — LLM gateway + all node + PipelineExecutor (PR 9)
  infrastructure.md              — concrete infrastructure implementation contracts (PR 10)
```

---

## Crate Dependency Graph

```
              ┌──────────────────────────────────────────────────────────────────┐
              │                         pipeline                                 │
              │   domain types · business logic · all trait definitions          │
              └──────────────────┬───────────────────────────────────────────────┘
                                 │ depended on by all crates below
         ┌───────────────────────┼─────────────────────────────────────────────┐
         │                       │                                             │
    ┌────▼───────┐   ┌───────────▼──────────────────────────────────────────┐  │
    │   nodes    │   │           infrastructure crates                      │  │
    │            │   │  github · llm · extension-api · listener             │  │
    └────────────┘   └──────────────────────────────────────────────────────┘  │
         │                                    │                                │
         └───────────────────────┬────────────┘                                │
                                 │                                             │
                            ┌────▼─────────────────────────────────────────────▼┐
                            │                      cli                          │
                            │   composition root · observability · trigger mode │
                            └───────────────────────────────────────────────────┘
```

**Architectural rules:**

- `pipeline` has **no I/O dependencies** (no tokio, reqwest, std::fs, std::process).
- Infrastructure crates implement `pipeline` traits. They do not add domain rules.
- `nodes` orchestrates calls between `pipeline` logic and infrastructure traits.
- `cli` is the only crate that imports all others and constructs concrete instances.

---

## Hexagonal Architecture Map

| Component | Layer | Purpose |
|-----------|-------|---------|
| `pipeline` (types, business logic) | Core domain | Domain rules, computations, type definitions |
| `pipeline` (trait definitions) | Port | Abstractions the domain needs from external systems |
| `github`, `llm`, `extension-api`, `listener` | Adapter | Implements domain ports against real APIs |
| `nodes` | Application/orchestration | Sequences port calls for each pipeline step |
| `cli` | Composition root | Wires ports to adapters; configures observability |

---

## Shared Type Registry

See [`docs/spec/shared-registry.md`](../shared-registry.md) for the live catalog
of every reusable type, trait, and pattern with its source location.
