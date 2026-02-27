# System Overview

## What CogWorks Is

CogWorks is a deterministic orchestration system that receives GitHub Issues, executes a structured SDLC pipeline (classify → design → plan → implement → review), and produces Pull Requests with fully reviewed code. It coordinates LLM calls for reasoning tasks and conventional tooling for everything else.

## Design Philosophy

These principles are load-bearing — they constrain every architectural decision:

1. **Deterministic by default.** Every step that can be performed by conventional tooling MUST be. LLMs are invoked only for reasoning, synthesis, or natural language understanding. This controls cost and makes the system debuggable.

2. **Quality through iteration.** One implementation with maximum context, iterated based on structured feedback. Not many candidates in parallel.

3. **Structured I/O at every boundary.** Every LLM call has defined input and output schemas. Outputs are validated before the pipeline proceeds.

4. **Stateless and observable.** All durable state lives in GitHub. No hidden databases. The system can be fully understood by reading GitHub state.

5. **Follows the human process.** Design → contracts → plan → implement → review. Each node produces a human-recognisable artifact.

6. **Constitutional boundaries.** External content is data, not instructions. Non-overridable behavioral rules are loaded before any context assembly or LLM call. The boundary between trusted instructions and untrusted input is explicit and enforced.

## Execution Model: CLI-First

CogWorks is a **CLI-first** application. Each invocation is a stateless step function:

```
Read GitHub state → Determine next action → Execute one step → Write results → Exit
```

This design choice has several consequences:

- **Simplicity**: No connection pooling, no graceful shutdown, no in-memory state management
- **Testability**: Each invocation is independent and deterministic given the same GitHub state
- **Crash recovery**: Restart and re-invoke; the system reads GitHub state and picks up where it left off
- **Concurrency**: Multiple CLI invocations can run for different work items simultaneously
- **Future service mode**: Poll/webhook modes become thin wrappers that invoke the same step function in a loop or on events

### Trigger Modes (Current and Future)

| Mode | Phase | Description |
|------|-------|-------------|
| **CLI** | Phase 1 | Direct invocation: `cogworks process <issue-url>` |
| **Poll** | Phase 2+ | Periodic scan for trigger labels, invokes step function per work item |
| **Webhook** | Phase 3+ | GitHub App events via smee.io (dev) or direct (prod), invokes step function per event |

All three modes share the same core step function. The difference is only in how and when the step function is triggered.

### Service-Ready Boundaries

To ensure CLI-first doesn't block future service mode:

- **Component construction is explicit** — dependencies are injected via constructors, not resolved from global state. A service wrapper can construct long-lived components with connection pools.
- **No global mutable state** — all state flows through function parameters and return values.
- **Resource cleanup is explicit** — temporary directories, git clones, and other resources are cleaned up by the caller, not by `Drop` side effects that assume single-invocation lifetime.
- **Configuration is loaded once and passed** — not re-read on every operation. A service can load once at startup.

## System Context

```
┌──────────────┐          ┌───────────────────┐
│   Human      │  creates │   GitHub          │
│   Developer  │────────→ │   Issue           │
│              │  labels  │   (Work Item)     │
└──────────────┘  cogworks│                   │
                  :run    └────────┬──────────┘
                                   │ webhook / poll / CLI
                                   ▼
                          ┌───────────────────┐
                          │   CogWorks CLI    │
                          │   (Step Function) │
                          │                   │
                          │  Reads state      │
                          │  Executes action  │
                          │  Writes results   │
                          └──┬────┬───────┬───┘
                             │    │       │
              ┌──────────────┘    │       └──────────────┐
              ▼                   ▼                      ▼
    ┌───────────────────┐ ┌──────────────────┐  ┌───────────────────┐
    │   LLM Provider    │ │ Domain Services  │  │   GitHub API      │
    │   (Anthropic)     │ │ (External Procs) │  │   (Issues, PRs,   │
    │                   │ │                  │  │    Labels, Files) │
    └───────────────────┘ │ ┌──────────────┐ │  └───────────────────┘
              ▲           │ │ Rust Service │ │            ▲
              │           │ │ (firmware)   │ │            │
    ┌─────────┴─────────┐ │ ├──────────────┤ │  ┌─────────┴─────────┐
    │  Prompt Templates │ │ │ Future:      │ │  │  Repo Config      │
    │  Output Schemas   │ │ │ KiCad, etc.  │ │  │  (.cogworks/)     │
    │  Constitutional   │ │ └──────────────┘ │  │  Constraints      │
    │   Rules           │ └──────────────────┘  │  Interface Registry│
    │  (version-ctrl'd) │      ▲                │  Context Packs    │
    └───────────────────┘      │                │  ADRs, Standards  │
                    ┌──────────┴──────────┐     └───────────────────┘
                    │  Extension API      │
                    │  (Unix socket/HTTP) │
                    │  JSON messages      │
                    └─────────────────────┘
```

Domain services are external processes that communicate with CogWorks through the Extension API. CogWorks is domain-ignorant — it does not contain code for any specific domain. The Rust domain service ships alongside CogWorks as a reference implementation but uses the same Extension API as any third-party domain service.

## Pipeline Flow

The pipeline is a configurable directed graph of nodes. The default pipeline preserves the standard 7-node linear sequence. Repositories may define custom graphs in `.cogworks/pipeline.toml` with parallel fan-out, conditional edges, and rework loops.

### Default Pipeline

The default pipeline has two phases. Nodes 1–4 execute once per work item. Nodes 5–7 execute as a unit for each sub-work-item in dependency order.

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Per Work Item (once)                                                    │
│                                                                         │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐           │
│  │ 1. Task  │    │ 2. Arch  │    │ 3. Iface │    │ 4. Plan  │           │
│  │ Intake & │───→│ (Spec    │───→│ Design   │───→│ (Sub-WI  │           │
│  │ Classify │    │  Doc)    │    │ (Code)   │    │  Create) │           │
│  └──────────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘           │
│                       │PR             │PR              │Issues          │
│                       ▼               ▼                ▼                │
│                  [Gate]          [Gate]           [Gate]                │
└──────────────────────────────────────────┬──────────────────────────────┘
                                           │
                                           ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Per Sub-Work-Item (in dependency order, optionally parallel)            │
│                                                                         │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐           │
│  │ 5. Code  │───→│ Determin.│───→│ Scenario │───→│ 6. Review│───→PR     │
│  │ Generate │    │ Checks + │    │ Validate │    │ Gate     │           │
│  │          │    │ Tests    │    │          │    │          │           │
│  └────┬─────┘    └──────────┘    └────┬─────┘    └──────────┘           │
│       │ rework edge (max 5 traversals)           │                      │
│       └──────────────────────────────────────────┘                      │
└─────────────────────────────────────────────────────────────────────────┘
```

Scenario validation is optional — it only runs for sub-work-items that have applicable scenarios.

Each node gate is configurable: `auto-proceed` or `human-gated`. Safety-critical work items force human gates for all code-producing nodes.

### Custom Pipeline Graphs

Repositories may define custom pipeline graphs with:

- **Parallel fan-out**: Multiple nodes executing concurrently when their inputs are all available
- **Fan-in synchronisation**: A node waiting for all upstream parallel nodes to complete
- **Conditional edges**: Deterministic expressions, LLM-evaluated conditions, or boolean composites that control which downstream nodes activate
- **Rework loops**: Cycles with explicit termination conditions (maximum traversals, cost budget)
- **Spawning nodes**: Nodes that create derivative work items without blocking the pipeline

Multiple named pipelines can be defined in a single configuration file. The Intake node's classification output selects which pipeline to execute.

## Data Flow Across Nodes

Each node produces artifacts that flow into downstream nodes via the pipeline working directory or GitHub:

| Node | Input | Output | Storage |
|------|-------|--------|---------|
| 1. Intake | GitHub Issue | Classification result | Issue comment + labels |
| 2. Architecture | Classification + repo context + loaded Context Packs | Specification document (Markdown) | Pull Request |
| 3. Interface Design | Approved spec + repo context | Interface definition files (source code) | Pull Request |
| 4. Planning | Approved spec + approved interfaces | Sub-work-item issues with dependency graph | GitHub Issues |
| 5. Code Generation | Sub-work-item + spec + interfaces + prior SWI outputs | Implementation code + tests | Working directory → branch |
| 6. Review Gate | Generated code | Review results (pass/fail + findings) | Pipeline state (fed back to Code Gen or forward to Integration) |
| 7. Integration | Reviewed code | Pull Request | GitHub PR |

In custom pipeline graphs, node inputs and outputs are declared in the pipeline configuration and the orchestrator verifies all inputs are available before starting a node.

## Working Copy Management

CogWorks needs file access in two distinct modes:

1. **Lightweight reads** (context assembly, file existence checks): Use GitHub Contents API. No local clone needed. Suitable for reading individual files, directory listings, and configuration.

2. **Full toolchain operations** (validate, simulate, normalise, extract interfaces): Requires a local git clone. Domain services need real files on disk to invoke their toolchains.

Strategy:

- **Pipeline working directory**: Each pipeline run has a dedicated git worktree checked out from the target repository. Intermediate artifacts (specs, interface definitions, plans, generated code) are written to the working directory before being committed as PRs. The working directory persists across all nodes within a single pipeline run and is cleaned up on completion. The working directory is a **performance optimisation** — pipeline state is always recoverable from GitHub artifacts (PRs, issue comments) in case of failure.
- **Domain services manage their own working copies.** CogWorks provides repository information to domain services via the Extension API request envelope (`repository.path` and `repository.ref`). The `path` field is the local filesystem path to the repository root or a clone URL depending on deployment; the `ref` field is the git ref to validate against. Domain services handle cloning or checkout as needed. For co-located services (Unix socket), a shared filesystem path may be used; for remote services (HTTP), the domain service clones from the provided URL.
- **Shared libraries**: CogWorks publishes shared libraries that domain services can use for common operations: shallow clone management, branch creation, temporary directory lifecycle, commit/push. These are optional — domain services may implement their own.
- **Branch per artifact**: `cogworks/<work-item-number>/<node-slug>` (e.g., `cogworks/42/spec`, `cogworks/42/interfaces`, `cogworks/42/swi-3`)
- **Cleanup**: The pipeline working directory is removed when the pipeline run completes. Domain service temporary directories are removed after each domain service operation. Branches cleaned up after PR merge (standard GitHub settings).
