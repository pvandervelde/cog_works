# ADR-0004: Graph-Structured Pipeline

**Status:** Accepted
**Date:** 2026-02-25
**Deciders:** CogWorks maintainers
**Supersedes:** Portions of REQ-PIPE-003 (fixed linear sequence)

---

## Context

The original CogWorks pipeline is a fixed 7-stage linear sequence:

```
Intake → Architecture → Interface Design → Planning → Code Generation → Review → Integration
```

This model is simple and sufficient for the most common case — implementing a feature end-to-end — but it constrains the system in several ways:

1. **No conditional bypasses.** A documentation-only change still traverses Architecture and Interface Design stages that add no value. There is no way to skip stages based on work item classification.
2. **No parallel execution.** User documentation and implementation planning are independent once the spec is approved, but the linear model forces them to run sequentially.
3. **Fixed rework routing.** When review fails, the pipeline always loops back to Code Generation. If the failure is architectural (wrong decomposition, wrong interface), routing back to Architecture or Interface Design would be more effective than re-generating code against a flawed spec.
4. **No extensibility.** Teams cannot add domain-specific processing steps (binary size checks, deployment manifest generation, refactoring analysis) without modifying the orchestrator.
5. **No follow-up work generation.** The pipeline produces PRs but has no mechanism to identify and create follow-up issues (refactoring opportunities, tech debt, documentation gaps) during a pipeline run.

These constraints become more significant as CogWorks is adopted across multiple domains and repository types.

---

## Decision

CogWorks will evolve from a fixed linear pipeline to a **configurable directed graph of nodes**. The existing 7 stages become node definitions in the default graph configuration — not hardcoded orchestrator behaviour. The orchestrator becomes a generic graph executor.

Key properties:

- **Nodes** are typed processing steps: LLM nodes (invoke LLM with prompt template), deterministic nodes (run script or domain service), or spawning nodes (create follow-up GitHub Issues).
- **Edges** connect nodes and may have conditions: deterministic expressions, LLM-evaluated natural-language conditions, or composites (AND/OR/NOT).
- **Cycles** are permitted (for retry and rework loops) but every cycle MUST have an explicit termination condition (max traversals, cost budget, deterministic exit).
- **The default pipeline** is the existing 7-stage linear sequence, ensuring backward compatibility for repositories with no pipeline configuration.
- **Configuration** is per-repository via `.cogworks/pipeline.toml`. Multiple named pipelines may be defined (e.g., `feature`, `bugfix`, `documentation-only`), with selection driven by the Intake node's classification output.
- **The orchestrator is generic.** It knows how to traverse a graph, evaluate edge conditions, manage a working directory, and report progress. It does not contain logic for any specific node type.

The graph model also introduces:

- **Pipeline working directory**: A persistent git worktree per pipeline run where intermediate artifacts accumulate across nodes (specs, interfaces, plans, code). Domain services continue to manage their own working copies for toolchain operations.
- **Parallel fan-out/fan-in**: Nodes whose inputs are all available may execute concurrently, with a synchronisation point at downstream fan-in nodes.
- **Pipeline cancellation and resume**: Running pipelines can be cancelled gracefully; failed pipelines can resume from the failed node using state persisted to GitHub.
- **Shift work boundary**: A configurable point in the pipeline after which CogWorks proceeds autonomously, making the human/autonomous boundary explicit.
- **Reference exemplars**: Context assembly can include files from external repositories as read-only reference patterns.

---

## Consequences

### Positive

- **Conditional bypasses** — Simple work items skip unnecessary stages. A documentation change goes Intake → Planning → Code Generation → Review → Integration with no Architecture or Interface Design.
- **Adaptive rework** — Review failures route to the appropriate stage (Architecture if structural, Interface Design if API is wrong, Code Generation if implementation is wrong) based on LLM-evaluated edge conditions.
- **Parallel execution** — Independent nodes execute concurrently, improving throughput for complex pipelines.
- **Extensibility** — Teams add domain-specific nodes (binary size check, licence scan, deployment manifest) by configuring new nodes in `pipeline.toml` without modifying CogWorks.
- **Follow-up work generation** — Spawning nodes create issues for refactoring, tech debt, and documentation gaps discovered during a pipeline run.
- **Backward compatible** — Repositories with no `pipeline.toml` get the default linear pipeline. No existing workflow breaks.

### Negative

- **Increased orchestrator complexity** — Graph traversal, cycle detection with termination guarantees, parallel execution with shared cost budget, and edge condition evaluation are significantly more complex than a linear loop.
- **Configuration surface area** — `pipeline.toml` is a new configuration file that can be misconfigured. Cycles without termination, orphan nodes, or contradictory edge conditions are all possible.
- **Non-determinism in routing** — LLM-evaluated edge conditions introduce non-determinism into pipeline routing. Two runs of the same work item might take different paths. Mitigated by recording all edge evaluations in the audit trail and requiring deterministic fallbacks.
- **Testing complexity** — The combinatorial space of graph configurations is much larger than a linear pipeline. Requires thorough configuration validation and conformance testing.

---

## Implementation Notes

- The graph executor MUST compute execution order via topological sorting of the graph with cycle-aware traversal.
- Edge conditions that require LLM evaluation MUST be recorded in the audit trail with prompt, response, and decision.
- LLM-evaluated edge conditions MUST have a deterministic fallback for when the LLM is unavailable or ambiguous.
- The persistent working directory is an orchestrator-level construct; domain services still manage their own clones via the Extension API context.
- The Pipeline Configuration file (`.cogworks/pipeline.toml`) MUST be validated at load time: no orphan nodes, all edge targets exist, every cycle has a termination condition, at least one terminal node is reachable from the start node.
- The shift work boundary is a per-classification default that sets node gate policies; individual nodes can still override.
