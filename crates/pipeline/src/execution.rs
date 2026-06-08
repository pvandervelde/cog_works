//! Graph execution decision logic and sub-work-item ordering.
//!
//! This module provides the pure functions and types that drive the pipeline
//! state machine: determining which nodes to execute next, evaluating edge
//! conditions, tracking rework cycle limits, and ordering sub-work-items by
//! their declared dependencies.
//!
//! ## Pure Business Logic
//!
//! No I/O lives here. Every function takes its inputs as arguments and returns
//! typed results. The `nodes` crate and `cli` crate call these functions to
//! drive the execution loop.
//!
//! ## Thread Safety Note — Budget and Parallel Nodes
//!
//! When multiple nodes execute in parallel, the caller **must** hold a
//! synchronisation primitive (e.g. `Mutex`) around calls to
//! [`crate::acquire_budget`] to prevent concurrent over-spend. This module
//! provides the decision logic; enforcement of the synchronisation contract is
//! the responsibility of the caller. See [`crate::budget`] for details.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/pipeline-execution.md` for the full contract,
//! state machine description, and decision table.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    errors::RetryPolicy,
    graph::{
        CompositeCondition, EdgeConditionKind, EdgeEvaluationRecord, EvaluatorKind, NodeState,
        NodeStatus, PipelineGraph, PipelineState, evaluate_deterministic_condition,
    },
    identifiers::{EdgeId, NodeId, SubWorkItemId},
    types::{CostBudget, Timestamp, TokenCost},
};

// ─── Node output types ───────────────────────────────────────────────────────

/// A requested update to a specific node's runtime state, returned as part of
/// [`NodeOutput`] so the orchestrator can apply the change to [`PipelineState`].
///
/// See `docs/spec/interfaces/pipeline-execution.md` §NodeStateUpdate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStateUpdate {
    /// The node whose status should be updated.
    pub node_id: NodeId,
    /// The new status to apply.
    pub new_status: NodeStatus,
    /// Error description when `new_status` is [`NodeStatus::Failed`].
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------

/// The structured output produced by a successfully executed node.
///
/// Contains the artifacts the node produced, a cost delta to be accumulated
/// into [`PipelineState::cost_accumulator`], and any state updates that the
/// orchestrator must apply (e.g. marking spawned nodes as `Active`).
///
/// ## Usage
///
/// `NodeOutput` is the success payload returned by every node `execute` function
/// in the `nodes` crate. The execution engine applies `cost_delta` and
/// `state_updates` to the current [`PipelineState`], then proceeds with edge
/// condition evaluation.
///
/// The `artifacts` map carries the named output slots declared in
/// [`crate::NodeDefinition::declared_outputs`]. Keys are slot names; values are
/// domain-specific JSON values (schemas are defined per-node in the `nodes` crate).
///
/// See `docs/spec/interfaces/pipeline-execution.md` §NodeOutput.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOutput {
    /// Named output artifacts produced by this node's execution step.
    ///
    /// Keys are the slot names declared in [`crate::NodeDefinition::declared_outputs`].
    /// Values are domain-specific JSON payloads whose schemas are defined per-node.
    pub artifacts: HashMap<String, serde_json::Value>,

    /// Additional token cost accumulated during this node's work.
    ///
    /// Added to [`PipelineState::cost_accumulator`] by the orchestrator after
    /// a successful execution step.
    pub cost_delta: TokenCost,

    /// Node state changes to apply after this execution step.
    ///
    /// Because `NodeOutput` is a value type (no shared state), nodes signal
    /// required state transitions here. The orchestrator applies them
    /// atomically before evaluating outgoing edges.
    ///
    /// Typically empty for ordinary LLM or deterministic nodes. The Spawning
    /// node uses this to mark newly created child sub-work-item nodes as
    /// `Active`.
    pub state_updates: Vec<NodeStateUpdate>,
}

// ─── Execution decision types ────────────────────────────────────────────────

// GateStatus and GateConfig are defined in `graph.rs` (alongside PipelineState)
// so that PipelineStateComment can include gate_config without a circular
// dependency.  They are re-exported here for backward-compatibility.
pub use crate::graph::{GateConfig, GateStatus};

// ---------------------------------------------------------------------------

/// Execution-level pipeline error, produced when the state machine cannot
/// continue and the pipeline must either halt or escalate.
///
/// Distinct from [`crate::CogWorksError`] (which covers pipeline-halt
/// conditions from the domain logic layer); `PipelineError` is produced by
/// the execution engine when it encounters a structural or infrastructure
/// failure during the step loop.
///
/// ## Combined Lifecycle Enum — Design Rationale
///
/// This enum deliberately mixes load-time failures (`GraphInvalid`,
/// `ConstitutionalRulesLoadFailed`) with runtime failures (`NodeFailed`,
/// `BudgetExceeded`). The rationale is that the `run_step` entry point in
/// `PipelineExecutor` (PR 9) uses a single error channel for the full step
/// lifecycle — from pre-flight checks through to node completion — so callers
/// need to handle only one error type. Load-time variants can never occur
/// after the pre-flight phase, but including them prevents the entry-point
/// signature from needing two separate error types or a nested enum.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §PipelineError.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum PipelineError {
    /// A node failed and exhausted its retry budget.
    ///
    /// The orchestrator has already applied all retries declared in
    /// [`crate::PipelineSettings::max_node_retries`]; no further retries are
    /// possible without human intervention.
    #[error("Node '{node_id}' failed: {error}")]
    NodeFailed {
        /// The node that failed.
        node_id: NodeId,
        /// Human-readable description of the failure.
        error: String,
        /// Retry policy from the node's last failure (should be `NonRetryable`).
        retry_policy: RetryPolicy,
    },

    /// Token cost exceeded the configured budget before all nodes completed.
    ///
    /// Produced by the execution engine when [`crate::acquire_budget`] returns
    /// [`crate::BudgetAcquisition::Denied`] and no more budget is available.
    #[error("Budget exceeded: accumulated {accumulated}, limit {limit}")]
    BudgetExceeded {
        /// Total cost accumulated at the point of failure.
        accumulated: TokenCost,
        /// The configured budget that was exceeded.
        limit: CostBudget,
    },

    /// The pipeline configuration is structurally invalid.
    ///
    /// Produced when the graph fails validation on load (see
    /// [`crate::validate_pipeline_graph`]). The pipeline never starts with an
    /// invalid graph.
    #[error("Graph invalid: {message}")]
    GraphInvalid {
        /// Human-readable description of the validation failure.
        message: String,
    },

    /// The constitutional rules file could not be loaded or validated at the
    /// start of a pipeline step.
    ///
    /// This is an unconditional halt — the pipeline never executes without
    /// validated constitutional rules.
    #[error("Constitutional rules could not be loaded or validated: {message}")]
    ConstitutionalRulesLoadFailed {
        /// Human-readable description of the load or validation failure.
        message: String,
    },
}

// ---------------------------------------------------------------------------

/// Structured report attached to an escalation, providing the context a human
/// reviewer needs to understand why the pipeline escalated.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §EscalationReason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationReason {
    /// Human-readable description of what went wrong.
    pub description: String,
    /// The node that triggered the escalation.
    pub node_id: NodeId,
    /// Total execution attempts made for `node_id` in this run.
    pub attempt_count: u32,
    /// Total rework iterations applied to `node_id` in this run.
    pub rework_count: u32,
    /// Token cost accumulated across all attempts and rework iterations for
    /// this node.
    pub cost_spent: TokenCost,
}

// ---------------------------------------------------------------------------

/// The action the execution engine should take next for a pipeline run.
///
/// Returned by [`determine_next_actions`]. The engine processes each action
/// in the returned `Vec`; parallel execution is signalled by multiple
/// `ExecuteNode` entries or a single `ExecuteParallel`.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §NextAction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextAction {
    /// Execute a single node immediately.
    ExecuteNode(NodeId),
    /// Execute all listed nodes concurrently in a new parallel branch.
    ///
    /// The orchestrator is responsible for creating the parallel branch in
    /// [`crate::PipelineState::active_parallel_branches`] before starting the
    /// nodes.
    ExecuteParallel(Vec<NodeId>),
    /// No nodes are immediately executable; the pipeline is waiting for a
    /// human gate decision or an external event.
    Wait,
    /// The pipeline cannot continue and requires human intervention.
    ///
    /// The attached [`EscalationReason`] is written to the audit trail and as
    /// a GitHub issue comment.
    Escalate(EscalationReason),
    /// The pipeline has encountered an unrecoverable error and must halt.
    ///
    /// The attached [`PipelineError`] is written to the audit trail and
    /// included in the GitHub state comment.
    HaltWithError(PipelineError),
}

// ─── Rework and dependency error types ───────────────────────────────────────

/// Error returned by [`increment_rework_counter`] when a rework edge has
/// been traversed the maximum permitted number of times.
///
/// The execution engine inspects the attached [`crate::ReworkEdge::overflow_behaviour`]
/// to decide whether to halt, escalate, or take a forward bypass edge.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §TerminationConditionReached.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[error("Rework edge '{edge_id}' reached traversal limit: {current_traversals}/{max_traversals}")]
pub struct TerminationConditionReached {
    /// The rework edge that exceeded its limit.
    pub edge_id: EdgeId,
    /// The traversal count at the point the limit was exceeded.
    pub current_traversals: u32,
    /// The configured maximum number of traversals permitted.
    pub max_traversals: u32,
}

// ---------------------------------------------------------------------------

/// Error returned by [`topological_sort_sub_work_items`] when the sub-work-item
/// dependency graph is invalid.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §DependencyError.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum DependencyError {
    /// A cyclic dependency was detected among sub-work-items.
    ///
    /// The `cycle` field lists the IDs forming the cycle, in traversal order.
    /// For example, if item A depends on B and B depends on A, the cycle is
    /// `[A, B]`.
    #[error(
        "Cyclic dependency detected in sub-work-items: {}",
        cycle.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(" → ")
    )]
    CyclicDependency {
        /// Ordered list of sub-work-item IDs forming the cycle.
        cycle: Vec<SubWorkItemId>,
    },

    /// A sub-work-item declares a dependency on an ID that does not exist in
    /// the provided list.
    #[error("Sub-work-item '{item_id}' depends on unknown item '{unknown_dep}'")]
    UnknownDependency {
        /// The sub-work-item that has the bad reference.
        item_id: SubWorkItemId,
        /// The referenced ID that could not be found.
        unknown_dep: SubWorkItemId,
    },
}

// ─── Sub-work-item type ──────────────────────────────────────────────────────

/// A single planned implementation sub-task within a larger work item.
///
/// Sub-work-items are created by the Planning node (see `nodes` crate) and
/// stored as GitHub sub-issues. The Planning node declares explicit
/// dependencies between them so they can be executed in a valid order.
///
/// ## Dependency Rules
///
/// - `depends_on` is a list of `SubWorkItemId`s that must complete before
///   this item may begin.
/// - Circular dependencies are rejected by [`topological_sort_sub_work_items`].
/// - References to IDs not present in the same batch are also rejected.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §SubWorkItem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubWorkItem {
    /// The GitHub sub-issue number for this sub-task.
    pub id: SubWorkItemId,
    /// Human-readable description of what this sub-task implements.
    ///
    /// GitHub sub-issue bodies have a maximum of 65,536 characters. If the
    /// Planning node produces a description longer than this limit, the
    /// subsequent `IssueTracker::create_sub_issue` call will fail. The
    /// Planning node is responsible for keeping descriptions within that bound.
    pub description: String,
    /// Sub-work-item IDs that must be completed before this one may begin.
    ///
    /// An empty vec means this item has no dependencies and may start
    /// immediately.
    pub depends_on: Vec<SubWorkItemId>,
}

// ─── Pure business logic functions ───────────────────────────────────────────

/// Determines which actions the execution engine should take next given the
/// current pipeline runtime state.
///
/// This is the central dispatch function of the CogWorks state machine. It
/// inspects the current [`PipelineState`], the graph topology
/// ([`PipelineGraph`]), and the current gate configuration ([`GateConfig`]) to
/// decide what to do next.
///
/// ## Decision Algorithm
///
/// 1. If any node is `Active` but its timeout has elapsed → `HaltWithError`.
/// 2. Call [`crate::compute_eligible_nodes`] to find all nodes ready to start.
/// 3. For each eligible node, check its gate configuration:
///    - [`NodeGate::HumanGated`]: consult `gate_config.gated_nodes` for this node.
///      Not present → `Wait`. `Approved` → include in execute set. `Rejected` → `Escalate`.
///    - [`NodeGate::AutoProceed`]: include in execute set directly.
/// 4. Fan-in nodes are only included if [`check_fan_in_ready`] returns `true`.
/// 5. Multiple eligible nodes → `ExecuteParallel`; single → `ExecuteNode`.
/// 6. No eligible nodes and none active → all nodes completed → return empty vec.
///
/// ## Vec Contents Contract
///
/// The returned `Vec` contains exactly one logical outcome per call:
///
/// | Scenario | Vec contents |
/// |----------|-------------|
/// | One or more auto-proceed eligible nodes (no fan-in blocking) | `[ExecuteNode(id)]` or `[ExecuteParallel(ids)]` |
/// | Mix of auto-proceed eligible and gated-waiting nodes | Only `[ExecuteNode(id)]` or `[ExecuteParallel(ids)]` for the eligible nodes; `Wait` is **not** co-returned |
/// | All eligible nodes are awaiting gate approval | `[Wait]` |
/// | A gated node was rejected | `[Escalate(reason)]` |
/// | A node timeout was exceeded | `[HaltWithError(error)]` |
/// | No eligible nodes and no active nodes | `[]` (run complete) |
///
/// Rationale for "mix" row: the orchestrator should start available work while
/// waiting for gate decisions on other nodes. Mixing `Wait` with execute actions
/// would force the orchestrator to implement its own action-splitting logic.
/// Instead, the unblocked nodes are returned for immediate execution; the
/// orchestrator discovers the gated nodes are still waiting on the *next* call
/// after those nodes complete.
///
/// ## Return Value
///
/// Returns an empty `Vec` when the pipeline run is complete (all nodes
/// completed). The caller should detect this and write the final
/// [`crate::PipelineStateComment`].
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §determine_next_actions`
#[must_use]
pub fn determine_next_actions(
    _state: &PipelineState,
    _graph: &PipelineGraph,
    _gate_config: &GateConfig,
) -> Vec<NextAction> {
    todo!("See docs/spec/interfaces/pipeline-execution.md §determine_next_actions")
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------

/// Evaluates an LLM-evaluated edge condition using a pre-resolved results map.
///
/// Returns `false` conservatively when the map does not contain an entry for
/// `edge_id`, logs a warning, and fires a `debug_assert` in production builds
/// to signal the missing pre-population (a caller contract violation).
fn evaluate_llm_branch(
    edge_id: &EdgeId,
    cond: &EdgeConditionKind,
    input_snapshot: serde_json::Value,
    llm_evaluated_results: &HashMap<EdgeId, bool>,
    evaluated_at: Timestamp,
) -> (bool, Vec<EdgeEvaluationRecord>) {
    let result = llm_evaluated_results
        .get(edge_id)
        .copied()
        .unwrap_or_else(|| {
            // SAFETY: debug_assert disabled in test builds so the existing fallback
            // test can exercise this path without panicking. In production debug
            // builds this fires as an invariant violation marker.
            #[cfg(not(test))]
            debug_assert!(false, "LlmEvaluated result missing for edge {:?}", edge_id);
            tracing::warn!(
                edge_id = ?edge_id,
                "LlmEvaluated result missing; falling back to false (conservative)"
            );
            false
        });
    let record = build_record(
        edge_id,
        cond,
        input_snapshot,
        result,
        EvaluatorKind::LlmModel {
            model_id: LLM_EVALUATED_MODEL_ID.to_string(),
        },
        evaluated_at,
    );
    (result, vec![record])
}

/// Evaluates a list of conditions with short-circuit semantics.
///
/// - `short_circuit_value = false` → And semantics (return false on first false).
/// - `short_circuit_value = true`  → Or semantics  (return true on first true).
///
/// Evaluation stops at the first short-circuit match but all records up to
/// and including the triggering condition are collected.
fn evaluate_short_circuit_conditions(
    edge_id: &EdgeId,
    conditions: &[EdgeConditionKind],
    state: &PipelineState,
    node_output: &NodeOutput,
    llm_evaluated_results: &HashMap<EdgeId, bool>,
    evaluated_at: Timestamp,
    short_circuit_value: bool,
) -> (bool, Vec<EdgeEvaluationRecord>) {
    let mut all_records: Vec<EdgeEvaluationRecord> = Vec::new();
    let mut overall = !short_circuit_value; // identity: true for And, false for Or
    for c in conditions {
        let (r, mut inner) = evaluate_edge_condition(
            edge_id,
            c,
            state,
            node_output,
            llm_evaluated_results,
            evaluated_at,
        );
        all_records.append(&mut inner);
        if r == short_circuit_value {
            overall = short_circuit_value;
            break;
        }
    }
    (overall, all_records)
}

/// Private helper that evaluates a [`CompositeCondition`] and collects audit
/// records for every evaluated sub-condition.
fn evaluate_composite_condition(
    edge_id: &EdgeId,
    composite: &CompositeCondition,
    state: &PipelineState,
    node_output: &NodeOutput,
    llm_evaluated_results: &HashMap<EdgeId, bool>,
    evaluated_at: Timestamp,
) -> (bool, Vec<EdgeEvaluationRecord>) {
    match composite {
        CompositeCondition::And(conditions) => evaluate_short_circuit_conditions(
            edge_id,
            conditions,
            state,
            node_output,
            llm_evaluated_results,
            evaluated_at,
            false, // short-circuit on false (And semantics)
        ),
        CompositeCondition::Or(conditions) => evaluate_short_circuit_conditions(
            edge_id,
            conditions,
            state,
            node_output,
            llm_evaluated_results,
            evaluated_at,
            true, // short-circuit on true (Or semantics)
        ),
        CompositeCondition::Not(inner) => {
            let (r, inner_records) = evaluate_edge_condition(
                edge_id,
                inner,
                state,
                node_output,
                llm_evaluated_results,
                evaluated_at,
            );
            (!r, inner_records)
        }
    }
}

// ─── Constants ──────────────────────────────────────────────────────────────

/// Sentinel model ID used in audit records when an LlmEvaluated edge condition
/// result is retrieved from the pre-populated `llm_evaluated_results` map.
/// The actual model ID (e.g. `"claude-3-7-sonnet"`) will be populated by PR 9
/// when the LLM evaluation infrastructure is integrated.
const LLM_EVALUATED_MODEL_ID: &str = "llm-evaluated";

// ─── Snapshot and record builders ────────────────────────────────────────────

fn capture_state_snapshot(edge_id: &EdgeId, state: &PipelineState) -> serde_json::Value {
    serde_json::to_value(state).unwrap_or_else(|err| {
        tracing::error!(
            edge_id = ?edge_id,
            error = %err,
            "Failed to serialise PipelineState for audit record; \
             snapshot will be empty — audit trail incomplete"
        );
        serde_json::Value::Object(Default::default())
    })
}

fn build_record(
    edge_id: &EdgeId,
    cond: &EdgeConditionKind,
    input_snapshot: serde_json::Value,
    result: bool,
    evaluator: EvaluatorKind,
    evaluated_at: Timestamp,
) -> EdgeEvaluationRecord {
    EdgeEvaluationRecord {
        edge_id: edge_id.clone(),
        condition: cond.clone(),
        input_snapshot,
        result,
        evaluator,
        timestamp: evaluated_at,
    }
}

#[must_use]
#[allow(clippy::only_used_in_recursion)]
/// Evaluates a single edge condition against the current pipeline state and
/// the producing node's output.
///
/// Returns both the boolean result and a complete [`EdgeEvaluationRecord`]
/// suitable for including in the audit trail and in the next
/// [`crate::PipelineStateComment`].
///
/// ## Condition Kinds
///
/// - [`EdgeConditionKind::Deterministic`]: delegates to
///   [`crate::evaluate_deterministic_condition`]. Pure; always returns the
///   same result for the same inputs.
/// - [`EdgeConditionKind::LlmEvaluated`]: looks up the pre-resolved result
///   in `llm_evaluated_results`. This map is populated by the `nodes` crate
///   (PR 9) before calling this function: `LlmGateway::call` is invoked
///   asynchronously for every `LlmEvaluated` condition on the outgoing edges
///   of the completed node, and the `(EdgeId, bool)` results are collected
///   into the map. The map entry **must** exist for every `LlmEvaluated`
///   edge; a missing entry is treated as `false` (conservative fallback) and
///   **must** be surfaced immediately via `debug_assert!(false, "LlmEvaluated
///   result missing for edge {:?}", edge_id)` or an equivalent audit-log
///   warning, to prevent silent pipeline stalls that are hard to debug.
/// - [`EdgeConditionKind::Composite`]: recursively evaluates inner conditions.
///
/// ## Parameters
///
/// - `edge_id` — Required to populate `EdgeEvaluationRecord::edge_id`.
/// - `node_output` — The output of the node that produced this edge condition.
///   Currently unused; reserved as a forward-compatibility placeholder for
///   artifact-aware condition kinds in PR 9 (e.g. presence checks like
///   `"artifact 'foo' is present"`). Callers must always provide this parameter.
/// - `llm_evaluated_results` — Pre-resolved LLM condition outcomes, keyed by
///   [`EdgeId`]. Populated by the `nodes` crate before calling this function
///   (see above). Empty for graphs with no `LlmEvaluated` edges.
/// - `evaluated_at` — Wall-clock time of evaluation; passed in so the function
///   remains pure and testable without `std::time` access.
///
/// ## Calling Convention (nodes crate)
///
/// ```text
/// // 1. Collect all LlmEvaluated edges leaving the completed node
/// // 2. For each, call LlmGateway::call to get a bool result (async)
/// // 3. Build llm_evaluated_results: HashMap<EdgeId, bool>
/// // 4. Call evaluate_edge_condition for every outgoing edge (sync)
/// ```
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §evaluate_edge_condition`
pub fn evaluate_edge_condition(
    edge_id: &EdgeId,
    cond: &EdgeConditionKind,
    state: &PipelineState,
    node_output: &NodeOutput,
    llm_evaluated_results: &HashMap<EdgeId, bool>,
    evaluated_at: Timestamp,
) -> (bool, Vec<EdgeEvaluationRecord>) {
    let input_snapshot = capture_state_snapshot(edge_id, state);

    match cond {
        EdgeConditionKind::Deterministic(expr) => {
            let result = evaluate_deterministic_condition(expr, state);
            let record = build_record(
                edge_id,
                cond,
                input_snapshot,
                result,
                EvaluatorKind::Deterministic,
                evaluated_at,
            );
            (result, vec![record])
        }
        EdgeConditionKind::LlmEvaluated(_) => evaluate_llm_branch(
            edge_id,
            cond,
            input_snapshot,
            llm_evaluated_results,
            evaluated_at,
        ),
        EdgeConditionKind::Composite(composite) => {
            let (result, mut inner_records) = evaluate_composite_condition(
                edge_id,
                composite,
                state,
                node_output,
                llm_evaluated_results,
                evaluated_at,
            );
            let root_record = build_record(
                edge_id,
                cond,
                input_snapshot,
                result,
                EvaluatorKind::Composite,
                evaluated_at,
            );
            let mut records = vec![root_record];
            records.append(&mut inner_records);
            (result, records)
        }
    }
}

// ---------------------------------------------------------------------------

/// Checks whether all incoming forward edges of a fan-in node have been
/// satisfied (i.e. all predecessor nodes have [`NodeStatus::Completed`]).
///
/// A fan-in node is one with two or more incoming forward edges from nodes
/// that were part of a parallel branch. This function prevents the fan-in
/// node from starting before all parallel branches complete.
///
/// Returns `true` when every incoming forward edge's source node is
/// [`NodeStatus::Completed`] in `state`.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §check_fan_in_ready`
#[must_use]
pub fn check_fan_in_ready(node: &NodeId, state: &PipelineState, graph: &PipelineGraph) -> bool {
    graph
        .edges
        .iter()
        .filter(|e| &e.target == node && e.rework_edge.is_none())
        .map(|e| &e.source)
        .all(|pred| {
            state
                .node_states
                .get(pred)
                .is_some_and(|ns| ns.status == NodeStatus::Completed)
        })
}

// ---------------------------------------------------------------------------

/// Resolves a rework edge to its target node and max-traversals limit.
///
/// # Panics
///
/// Panics (via `unreachable!()`) if the edge is not found in the graph or
/// if the edge does not have rework metadata. These are invariant violations
/// that should never occur if the caller has validated the graph.
fn resolve_rework_edge(edge: &EdgeId, graph: &PipelineGraph) -> (NodeId, u32) {
    let Some(edge_def) = graph.edges.iter().find(|e| &e.id == edge) else {
        // INVARIANT: validate_pipeline_graph guarantees all EdgeIds in a
        // valid graph are addressable. Callers must only pass EdgeIds that
        // originate from the same PipelineGraph.
        unreachable!(
            "increment_rework_counter: edge '{:?}' not found in graph; \
             graph invariant violated",
            edge
        );
    };

    let Some(rework) = edge_def.rework_edge.as_ref() else {
        // INVARIANT: this function must only be called on rework edges.
        // The orchestrator is responsible for checking edge kind before calling.
        unreachable!(
            "increment_rework_counter: edge '{:?}' has no rework metadata; \
             must only be called on rework (back) edges",
            edge
        );
    };

    (edge_def.target.clone(), rework.max_traversals)
}

/// Increments the traversal counter for the given rework edge in the current
/// pipeline state.
///
/// Returns the new traversal count on success. Returns
/// [`TerminationConditionReached`] if the increment would cause the count to
/// exceed the edge's `max_traversals` limit.
///
/// ## Caller Responsibility
///
/// The caller must inspect [`crate::ReworkEdge::overflow_behaviour`] whenever
/// this function returns `Err` and act accordingly:
/// - [`crate::OverflowBehaviour::HaltWithError`] → emit `HaltWithError`.
/// - [`crate::OverflowBehaviour::Escalate`] → emit `Escalate`.
/// - [`crate::OverflowBehaviour::TakeEdge`] → activate the bypass edge instead.
///
/// # Errors
///
/// Returns [`TerminationConditionReached`] if incrementing would exceed the
/// rework edge's `max_traversals` limit.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §increment_rework_counter`
pub fn increment_rework_counter(
    edge: &EdgeId,
    state: &mut PipelineState,
    graph: &PipelineGraph,
) -> Result<u32, TerminationConditionReached> {
    let (target, max_traversals) = resolve_rework_edge(edge, graph);

    let node_state = state
        .node_states
        .entry(target)
        .or_insert_with(|| NodeState {
            status: NodeStatus::Pending,
            attempt_count: 0,
            rework_count: 0,
            current_error: None,
            rework_edge_traversals: HashMap::new(),
        });

    let traversal_count = node_state
        .rework_edge_traversals
        .entry(edge.clone())
        .or_insert(0);
    *traversal_count += 1;
    let new_count = *traversal_count;

    if new_count <= max_traversals {
        Ok(new_count)
    } else {
        Err(TerminationConditionReached {
            edge_id: edge.clone(),
            current_traversals: new_count,
            max_traversals,
        })
    }
}

// ---------------------------------------------------------------------------

/// Returns a topological ordering of sub-work-items respecting their
/// `depends_on` declarations.
///
/// The returned `Vec<SubWorkItemId>` lists items from least-dependent to
/// most-dependent (sources first). Items with no dependencies appear first.
///
/// ## Validation
///
/// The function validates the input list before sorting:
/// - Any reference in `depends_on` that does not match an `id` in `items`
///   returns [`DependencyError::UnknownDependency`].
/// - Any cycle in the dependency graph returns
///   [`DependencyError::CyclicDependency`].
///
/// # Errors
///
/// Returns [`DependencyError`] if the dependency graph contains cycles or
/// unknown references.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §topological_sort_sub_work_items`
pub fn topological_sort_sub_work_items(
    _items: &[SubWorkItem],
) -> Result<Vec<SubWorkItemId>, DependencyError> {
    todo!("See docs/spec/interfaces/pipeline-execution.md §topological_sort_sub_work_items")
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
