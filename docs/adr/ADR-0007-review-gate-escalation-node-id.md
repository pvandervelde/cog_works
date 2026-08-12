# ADR-0007: Review Gate Escalation `NodeId` Sourcing (Deferred)

**Status:** Proposed
**Date:** 2026-08-09
**Deciders:** Architecture

---

## Context

`aggregate_review_results` (`crates/pipeline/src/review.rs`) is the pure function that
folds the Quality, Architecture, and Security review-pass results into an
`AggregateReviewDecision`. When the rework budget is exhausted, it returns
`Escalate(EscalationReason)`, and must populate `EscalationReason.node_id`.

The original spec text (`docs/spec/interfaces/pipeline-execution.md` §`aggregate_review_results`)
described `node_id` as "the Code Generation node ID, passed in via `EscalationReason`
construction by the caller." However, `aggregate_review_results` has no `NodeId`
parameter: its signature takes three `ReviewResult`s plus the remediation counters, and
it constructs the complete `EscalationReason` itself rather than delegating that to a
caller. The function also isn't scoped to a single node instance — it aggregates
across three independent review passes — so there is no single obvious node identity
to source from within the function itself.

Threading the real Code Generation `NodeId` through would require either:

1. Adding a `NodeId` parameter to `aggregate_review_results`, which the orchestration
   layer would populate from the `PipelineGraph` (the same way `increment_rework_counter`
   receives the graph to look up `ReworkEdge::max_traversals`), or
2. Moving `EscalationReason` construction out of `aggregate_review_results` entirely and
   into the caller in the `nodes` crate, which already has graph context.

Both are reasonable, but the orchestration-layer wiring for the Review node (`nodes`
crate) is not yet implemented, so there is no concrete call site to validate either
design against yet.

## Decision

**Deferred.** Until the Review node's orchestration wiring is implemented and a
concrete `NodeId`-sourcing call site exists:

1. `aggregate_review_results` populates `EscalationReason.node_id` with a fixed
   placeholder literal, `"review-gate"` (see `REVIEW_GATE_NODE_ID` in
   `crates/pipeline/src/review.rs`), rather than the real Code Generation node ID.
2. The spec (`docs/spec/interfaces/pipeline-execution.md` §`aggregate_review_results`)
   documents this as the current, actual behavior rather than the originally intended
   caller-supplied behavior.
3. This ADR is the tracking artifact for the open decision so it does not fall through
   the cracks.

## Consequences

- **Positive**: Unblocks `aggregate_review_results` as a complete, pure, independently
  testable function without prematurely committing to how the `nodes` crate will wire
  graph context into the review gate.
- **Negative**: Escalation reports produced by `aggregate_review_results` today carry
  the synthetic `"review-gate"` node ID instead of the real Code Generation node that
  needed rework. A human reviewer reading an escalation report cannot yet identify
  the specific node from `EscalationReason.node_id` alone (other fields — the
  `description` listing — still carry the actionable finding content).
- **Follow-up required**: When the `nodes` crate wires the Review node's orchestration
  (PR 9 per `docs/spec/interfaces/pipeline-execution.md` Implementation Notes), this
  ADR must be updated to `Accepted` with one of the two options above selected, and
  `aggregate_review_results` (or its caller) updated to source the real `NodeId`.

---

## References

- `crates/pipeline/src/review.rs` — `aggregate_review_results`, `REVIEW_GATE_NODE_ID`
- `docs/spec/interfaces/pipeline-execution.md` §`aggregate_review_results` — function spec
- ADR-0004 — Graph-structured pipeline (relevant context on `PipelineGraph`/`NodeId`)
