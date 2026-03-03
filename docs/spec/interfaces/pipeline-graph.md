# Pipeline Graph Model — Interface Specification

**Architectural Layer**: Core domain (`pipeline` crate)
**Source file**: `crates/pipeline/src/graph.rs`
**Introduced in**: PR 2 (pipeline graph model)
**Depends on**: `docs/spec/interfaces/shared-types.md`

---

## Purpose

This document specifies all data structures that describe a pipeline graph and
its runtime execution state. These types are pure data — no I/O. All types
implement `Serialize`/`Deserialize` for writing and reading pipeline state
to/from GitHub issue comments.

Two categories of types live here:

1. **Graph structure** — how a pipeline is configured (nodes, edges, conditions).
2. **Runtime state** — what is happening during a run (node statuses, cost
   accumulation, traversal counts, audit records).

---

## Dependencies

- `NodeId`, `EdgeId`, `PipelineName`, `PipelineRunId`, `ProfileName`,
  `WorkItemId` — from `shared-types.md §Identifiers`
- `CostBudget`, `Timestamp` — from `shared-types.md §Value Types`
- `thiserror` — for error derives

---

## Auxiliary Scalar Types

### `Expression`

A string-based predicate evaluated deterministically against `PipelineState`.

#### Expression Language

An expression is a JSON-path-style boolean predicate. The execution engine
evaluates it against a JSON projection of `PipelineState`. Supported operators
and syntax are implementation-defined; the spec mandates only that:

- The expression is a non-empty string.
- Evaluation is pure (no side effects).
- Unknown field references evaluate to `false`.

**Constructor**: `Expression::new(value: impl Into<String>) -> Option<Expression>`
Returns `None` for empty strings.

---

### `NaturalLanguageCondition`

A non-empty string describing a condition in natural language, evaluated by an
LLM against the upstream node's output.

**Constructor**: `NaturalLanguageCondition::new(value: impl Into<String>) -> Option<NaturalLanguageCondition>`

---

### `TimeoutSeconds`

Timeout expressed as whole seconds. Wraps `u64` for serde compatibility (
`std::time::Duration` does not implement `Serialize`/`Deserialize` directly).

`From<std::time::Duration>` truncates to whole seconds.
`Into<std::time::Duration>` is lossless (whole seconds only).

---

## Graph Structure Types

### `NodeType`

| Variant | Meaning |
|---------|---------|
| `Llm` | Node calls an LLM; output is a structured JSON value matched to an output schema |
| `Deterministic` | Node executes a deterministic function; no LLM call |
| `Spawning` | Node decomposes the current work item into child sub-work-items |

---

### `NodeGate`

| Variant | Meaning |
|---------|---------|
| `AutoProceed` | Pipeline resumes automatically after the node completes |
| `HumanGated` | A human must approve the output before the pipeline continues; node enters `HumanGated` status |

---

### `ValidationKind`

| Variant | Meaning |
|---------|---------|
| `None` | No extra validation beyond the node's built-in output schema check |
| `DomainService` | Output is sent to the appropriate domain service for validation |
| `Scenario` | Scenario execution runs against the output |

---

### `EvaluationMode`

Controls how outgoing edges from a node are selected after the node completes.

| Variant | Behaviour |
|---------|-----------|
| `AllMatching` | Every edge whose condition is `true` fires (fan-out) |
| `FirstMatching` | First edge (declaration order) whose condition is `true` fires |
| `Explicit` | Only edges listed in an explicit-edges list fire |

Default: `FirstMatching` (nodes absent from `PipelineGraph::evaluation_modes`).

---

### `NodeDefinition`

The static declaration of a node in a pipeline configuration.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | `NodeId` | yes | Unique within the graph |
| `node_type` | `NodeType` | yes | Execution category |
| `declared_inputs` | `Vec<String>` | yes | Input artifact slot names |
| `declared_outputs` | `Vec<String>` | yes | Output artifact slot names |
| `timeout` | `Option<TimeoutSeconds>` | no | Node-level wall-clock timeout |
| `cost_budget` | `Option<CostBudget>` | no | Node-level cost cap |
| `gate` | `NodeGate` | yes | Auto-proceed or human-gated |
| `validation_kind` | `ValidationKind` | yes | Post-execution validation type |
| `abort_siblings_on_failure` | `bool` | yes | Cancel parallel siblings on failure |

**Invariant**: `declared_inputs` and `declared_outputs` must not contain
duplicate names within the same node.

---

### `EdgeConditionKind`

The condition type for a directed edge. Determines when the edge fires.

| Variant | Payload | Description |
|---------|---------|-------------|
| `Deterministic` | `Expression` | Pure expression evaluated against `PipelineState` |
| `LlmEvaluated` | `NaturalLanguageCondition` | LLM decides true/false against node output |
| `Composite` | `CompositeCondition` | Boolean combination of inner conditions |

---

### `CompositeCondition`

Recursive boolean combinator for `EdgeConditionKind` values.

| Variant | Logic |
|---------|-------|
| `And(Vec<EdgeConditionKind>)` | All inner conditions must be `true` |
| `Or(Vec<EdgeConditionKind>)` | At least one inner condition must be `true` |
| `Not(Box<EdgeConditionKind>)` | Inner condition must be `false` |

`Not` wraps in `Box` to break the recursive type structure.

---

### `ReworkSemantics`

| Variant | Behaviour |
|---------|-----------|
| `Retry` | Target re-executes with its original input unchanged |
| `Rework` | Target re-executes with its input enriched by findings from the current node |

---

### `OverflowBehaviour`

What happens when a rework edge's `max_traversals` is reached.

| Variant | Behaviour |
|---------|-----------|
| `HaltWithError` | Pipeline stops with `CogWorksError::PipelineHalt` |
| `Escalate` | Escalation report generated; human intervention required |
| `TakeEdge(EdgeId)` | The named forward edge fires instead of the loop |

---

### `ReworkEdge`

| Field | Type | Description |
|-------|------|-------------|
| `max_traversals` | `u32` | Hard upper bound on loop traversals in one run |
| `preserved_outputs` | `Vec<String>` | Output slot names forwarded on each traversal |
| `overflow_behaviour` | `OverflowBehaviour` | What happens when the limit is hit |
| `semantics` | `ReworkSemantics` | Retry vs. guided rework |

**Invariant**: `max_traversals` must be ≥ 1.

---

### `EdgeDefinition`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | `EdgeId` | yes | Unique within the graph |
| `source` | `NodeId` | yes | Originating node |
| `target` | `NodeId` | yes | Destination node |
| `condition` | `EdgeConditionKind` | yes | Firing criterion |
| `rework_edge` | `Option<ReworkEdge>` | no | Present only for back-edges (cycles) |

**Invariant**: If `source == target`, `rework_edge` must be `Some(_)`.

---

### `PipelineSettings`

Pipeline-level defaults applied to nodes with no override.

| Field | Type | Description |
|-------|------|-------------|
| `default_timeout` | `Option<TimeoutSeconds>` | Applied to nodes without `NodeDefinition::timeout` |
| `default_cost_budget` | `Option<CostBudget>` | Applied to nodes without `NodeDefinition::cost_budget` |
| `max_node_retries` | `u32` | Maximum retries before escalation |

---

### `PipelineGraph`

The complete pipeline graph as loaded and validated from configuration.

| Field | Type | Description |
|-------|------|-------------|
| `nodes` | `Vec<NodeDefinition>` | All node declarations |
| `edges` | `Vec<EdgeDefinition>` | All edge declarations |
| `evaluation_modes` | `HashMap<NodeId, EvaluationMode>` | Per-node overrides; absent nodes use `FirstMatching` |
| `explicit_edge_lists` | `HashMap<NodeId, Vec<EdgeId>>` | Explicit edge lists for nodes using `EvaluationMode::Explicit`; absent from map when not needed |
| `settings` | `PipelineSettings` | Pipeline-level defaults |
| `tool_profiles` | `PipelineToolProfileConfig` | Tool-profile overrides scoped to this pipeline |

**Invariant**: Only produced by `validate_pipeline_graph`. Never construct
directly in production code; always validate first.

**Note**: `tool_profiles` lives on `PipelineGraph` (not on `PipelineConfiguration`) so
that two pipelines within the same configuration file that happen to share a
node name do not collide on override entries.

**Deserialisation boundary**: `#[serde(deny_unknown_fields)]` is applied.
Always deserialise → validate → use; a deserialised value is not validated.

---

### `PipelineConfiguration`

The complete contents of `.cogworks/pipeline.toml`.

| Field | Type | Description |
|-------|------|-------------|
| `pipelines` | `HashMap<PipelineName, PipelineGraph>` | All named graphs; each carries its own `tool_profiles` |

**Loading sequence**: deserialise → call `validate_pipeline_graph` on every
graph → use. `#[serde(deny_unknown_fields)]` is applied.

---

### `PipelineToolProfileConfig`

| Field | Type | Description |
|-------|------|-------------|
| `default_profile` | `ProfileName` | Applied to all nodes without an override |
| `node_overrides` | `HashMap<NodeId, ProfileName>` | Per-node tool profile (scoped to the owning pipeline) |

---

## Runtime State Types

### `NodeStatus`

| Variant | Meaning |
|---------|---------|
| `Pending` | Not yet started |
| `Active` | Currently executing |
| `Completed` | Finished successfully |
| `Failed` | Last attempt failed; retry budget exhausted |
| `HumanGated` | Awaiting human approval |

---

### `NodeState`

Per-node mutable runtime state.

| Field | Type | Description |
|-------|------|-------------|
| `status` | `NodeStatus` | Current phase |
| `attempt_count` | `u32` | Total execution attempts (≥ 1 when started) |
| `rework_count` | `u32` | Number of rework-feedback re-executions |
| `current_error` | `Option<String>` | Last failure message |
| `rework_edge_traversals` | `HashMap<EdgeId, u32>` | Traversal count per rework edge |

---

### `PipelineState`

The complete mutable state of a pipeline run.

| Field | Type | Description |
|-------|------|-------------|
| `run_id` | `PipelineRunId` | Identifies the run |
| `node_states` | `HashMap<NodeId, NodeState>` | State per node |
| `active_parallel_branches` | `Vec<Vec<NodeId>>` | Currently executing parallel branches |
| `cost_accumulator` | `TokenCost` | Total cost accumulated so far (USD); starts at `TokenCost::zero()` |

**Invariant**: Mutations are atomic at node boundaries; partial updates
must not be persisted. Compare `cost_accumulator` against the configured
`CostBudget` using `CostBudget::is_exceeded_by`.

---

### `EvaluatorKind`

| Variant | Fields | Description |
|---------|--------|-------------|
| `Deterministic` | — | Pure expression evaluator |
| `LlmModel` | `model_id: String` | Specific LLM model |
| `Composite` | — | Boolean combinator |

---

### `EdgeEvaluationRecord`

Audit record for one edge-condition evaluation. Every evaluation is recorded
(see `docs/spec/constraints.md §Edge condition evaluation is audited`).

| Field | Type | Description |
|-------|------|-------------|
| `edge_id` | `EdgeId` | The edge evaluated |
| `condition` | `EdgeConditionKind` | The condition definition |
| `input_snapshot` | `serde_json::Value` | `PipelineState` snapshot as a JSON value (not a string) to avoid double-escaping |
| `result` | `bool` | Evaluation outcome |
| `evaluator` | `EvaluatorKind` | What evaluated it |
| `timestamp` | `Timestamp` | Wall-clock time of evaluation |

---

### `SchemaVersion`

Version token for `PipelineStateComment` forward-compatibility.

Deserialisation enforced at the serde boundary via `#[serde(try_from = "String")]`:
any value other than `"1"` causes deserialization to fail immediately. No
caller-side validation is required.

| Member | Description |
|--------|-------------|
| `CURRENT: &'static str` | `"1"` — the current version string |
| `current() -> SchemaVersion` | Returns the current version |
| `as_str() -> &str` | Returns the version string |

---

### `PipelineStateComment`

**This is the source of truth for pipeline state persistence.**

Written to a GitHub issue comment at every node boundary. Must be
self-contained (see `docs/spec/constraints.md §Pipeline state is recoverable
from GitHub`).

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | `SchemaVersion` | Serde-enforced version; deserialization fails for unknown values |
| `pipeline_run_id` | `PipelineRunId` | Run identifier |
| `work_item_id` | `WorkItemId` | GitHub issue being processed |
| `state` | `PipelineState` | Full runtime state |
| `graph_hash` | `String` | SHA-256 hex of the pipeline config; mismatch on resume → escalate |
| `written_at` | `Timestamp` | Authoring timestamp |

---

## Error Types

### `CycleError`

Produced by `topological_sort` when a directed cycle exists among non-rework
edges.

| Field | Description |
|-------|-------------|
| `cycle: Vec<NodeId>` | Sequence of node IDs forming the cycle |

---

### `GraphValidationError`

Single violation found by `validate_pipeline_graph`. Returned as a `Vec`.

| Variant | Fields | When |
|---------|--------|------|
| `EmptyGraph` | — | Graph has no nodes |
| `UnterminatedCycle` | `nodes: Vec<NodeId>` | Cycle with no rework-edge termination (see `CycleError`) |
| `OrphanNode` | `node: NodeId` | Node with no connected edges |
| `DuplicateNodeId` | `id: NodeId` | Two nodes share an ID |
| `DuplicateEdgeId` | `id: EdgeId` | Two edges share an ID |
| `UnknownNode` | `edge: EdgeId`, `node: NodeId` | Edge references undeclared node |
| `InvalidMaxTraversals` | `edge: EdgeId` | Rework edge has `max_traversals == 0` (must be ≥ 1) |

---

## Pure Business Logic Functions

### `topological_sort`

```rust
pub fn topological_sort(
    nodes: &[NodeDefinition],
    edges: &[EdgeDefinition],
) -> Result<Vec<NodeId>, CycleError>
```

Returns a topologically sorted ordering of node IDs with sources before sinks.
Rework (back) edges are excluded from the traversal.

**Algorithm**: Kahn's algorithm over the forward-edge subgraph.

**Errors**: `CycleError` if the forward-edge subgraph contains a directed cycle
(configuration error — every real cycle must use rework edges only).

---

### `evaluate_deterministic_condition`

```rust
pub fn evaluate_deterministic_condition(
    expr: &Expression,
    state: &PipelineState,
) -> bool
```

Evaluates an `Expression` against `PipelineState`. Pure — no side effects.
Unknown field references evaluate to `false`.

---

### `validate_pipeline_graph`

```rust
pub fn validate_pipeline_graph(
    graph: &PipelineGraph,
) -> Result<(), Vec<GraphValidationError>>
```

Validates a `PipelineGraph` for structural correctness.

**Checks performed** (in order):

1. Graph is non-empty (`EmptyGraph`).
2. All node IDs are unique (`DuplicateNodeId`).
3. All edge IDs are unique (`DuplicateEdgeId`).
4. All edge source/target references resolve to declared nodes (`UnknownNode`).
5. No orphan nodes (`OrphanNode`).
6. All rework edges have `max_traversals >= 1` (`InvalidMaxTraversals`).
7. No unterminated cycles — every cycle path must pass through ≥1 rework edge (`UnterminatedCycle`).

Returns `Ok(())` only when all checks pass. Returns `Err(vec)` containing
every violation found (all checks run; not short-circuited).

**Called at**: pipeline configuration load time, before any node executes.

---

### `compute_eligible_nodes`

```rust
pub fn compute_eligible_nodes(
    state: &PipelineState,
    graph: &PipelineGraph,
) -> Vec<NodeId>
```

Returns nodes eligible to execute next.

**A node is eligible when**:

1. Its `NodeStatus` is `Pending`.
2. All upstream nodes (via non-rework forward edges) have `NodeStatus::Completed`.
3. *(Artifact availability is verified by the caller, not this function.)*

**Not evaluated here**: gate status (`NodeGate`). Callers must check `gate`
before starting eligible nodes.

**Returns**: Empty `Vec` when no node is ready (run is waiting on parallel
branches, human gates, or LLM edge evaluation).

---

## Usage Examples

```rust
// Load and validate a pipeline graph (illustrative — no real I/O here)
let raw_graph: PipelineGraph = /* loaded by PipelineConfigurationLoader */;
validate_pipeline_graph(&raw_graph)?;

// Determine what to run next
let ready = compute_eligible_nodes(&current_state, &raw_graph);
for node_id in ready {
    let node_def = raw_graph.nodes.iter().find(|n| n.id == node_id).unwrap();
    match node_def.gate {
        NodeGate::AutoProceed => { /* start the node */ }
        NodeGate::HumanGated  => { /* wait for approval */ }
    }
}

// Evaluate a deterministic edge after node completion
let expr = Expression::new("state.nodes.review.status == 'Completed'").unwrap();
let fires = evaluate_deterministic_condition(&expr, &current_state);

// Persist state to GitHub
let comment = PipelineStateComment {
    schema_version: SchemaVersion::current(),
    pipeline_run_id: run_id,
    work_item_id,
    state: current_state.clone(),
    graph_hash: /* SHA-256 of config bytes */,
    written_at: Timestamp::now(),
};
let json = serde_json::to_string(&comment)?;
// post `json` as GitHub issue comment …
```

---

## Implementation Notes

- **Serialisation**: All types use `#[derive(Serialize, Deserialize)]`.
  Unknown schema versions are rejected automatically at deserialisation time
  by `SchemaVersion`'s `TryFrom<String>` impl — no additional runtime check
  is needed by callers.

- **Cost accumulation**: `PipelineState::cost_accumulator` is `TokenCost`
  (the accumulation type), initialised to `TokenCost::zero()`. The configured
  limit is a separate `CostBudget`. Compare them with `CostBudget::is_exceeded_by`.

- **Parallel budget safety**: When nodes execute concurrently, budget
  acquisition must be atomic. The execution engine (PR 7) holds a mutex across
  all concurrent `acquire_budget` calls. This function is pure data; it has no
  locking responsibilities.

- **`topological_sort` and cycles**: Rework edges are structurally permitted
  (they form the pipeline's feedback loops); only the forward-edge subgraph
  must be a DAG. Implementations must identify rework edges by checking
  `EdgeDefinition::rework_edge.is_some()`.

- **`EvaluationMode::Explicit` data**: The explicit edge lists are stored in
  `PipelineGraph::explicit_edge_lists`. If a node has `Explicit` mode but is
  absent from that map, `validate_pipeline_graph` must produce a validation
  error (this check will be added when the validator is implemented).
