//! Pipeline graph model and runtime state.
//!
//! This module defines all data structures that describe a pipeline graph
//! (nodes, edges, conditions, configuration) and the runtime state captured
//! at each node boundary.
//!
//! ## Pure Data Module
//!
//! No I/O lives here. Functions operate on values passed in as arguments.
//! All types implement [`serde::Serialize`] and [`serde::Deserialize`] for
//! persistence to GitHub issue comments.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/pipeline-graph.md` for the full contract.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    CostBudget, EdgeId, NodeId, PipelineName, PipelineRunId, ProfileName, Timestamp, TokenCost,
    WorkItemId,
};

// ─── Auxiliary scalar types ────────────────────────────────────────────────

/// A boolean expression evaluated deterministically against [`PipelineState`].
///
/// The expression language is a simple predicate evaluated by the graph
/// execution engine. Format is defined in
/// `docs/spec/interfaces/pipeline-graph.md §Expression Language`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Expression(String);

impl Expression {
    /// Creates an [`Expression`] from a raw string, returning `None` if empty.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let v = value.into();
        if v.is_empty() {
            None
        } else {
            Some(Self(v))
        }
    }

    /// Returns the raw expression string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A natural-language description of a condition evaluated by an LLM.
///
/// The LLM decides `true`/`false` by reasoning against this description
/// applied to the node's output.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NaturalLanguageCondition(String);

impl NaturalLanguageCondition {
    /// Creates a [`NaturalLanguageCondition`], returning `None` if empty.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let v = value.into();
        if v.is_empty() {
            None
        } else {
            Some(Self(v))
        }
    }

    /// Returns the raw condition description.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Timeout expressed as whole seconds for serialisation compatibility.
///
/// Use `From<std::time::Duration>` / `Into<std::time::Duration>` for conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimeoutSeconds(pub u64);

impl From<std::time::Duration> for TimeoutSeconds {
    fn from(d: std::time::Duration) -> Self {
        Self(d.as_secs())
    }
}

impl From<TimeoutSeconds> for std::time::Duration {
    fn from(t: TimeoutSeconds) -> Self {
        std::time::Duration::from_secs(t.0)
    }
}

// ─── Graph structure ────────────────────────────────────────────────────────

/// Classification of a pipeline node by its execution characteristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// Node that invokes an LLM to produce structured output.
    Llm,
    /// Node whose logic is fully deterministic (no LLM call).
    Deterministic,
    /// Node that spawns child sub-work-items from the current work item.
    Spawning,
}

/// Whether a node proceeds automatically or requires human approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeGate {
    /// The pipeline resumes automatically after this node completes.
    AutoProceed,
    /// A human must approve the node output before the pipeline continues.
    HumanGated,
}

/// Kind of validation applied to a node's output before edge evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationKind {
    /// No additional validation beyond the node's built-in output schema.
    None,
    /// Output is validated by the appropriate domain service.
    DomainService,
    /// Scenario execution is used to validate the output.
    Scenario,
}

/// How outgoing edges from a node are selected for activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvaluationMode {
    /// All edges whose conditions evaluate to `true` are activated (fan-out).
    AllMatching,
    /// The first edge (in declaration order) whose condition is `true` fires.
    FirstMatching,
    /// Exactly the edges listed in the node's explicit-edge list are activated.
    Explicit,
}

/// The complete definition of a single node as declared in the pipeline config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDefinition {
    /// Unique node identifier within the pipeline graph.
    pub id: NodeId,
    /// Execution type (LLM, deterministic, or spawning).
    pub node_type: NodeType,
    /// Names of input artifact slots this node consumes.
    ///
    /// The execution engine verifies all declared inputs are present before
    /// starting the node (see `docs/spec/constraints.md §Pipeline Graph`).
    pub declared_inputs: Vec<String>,
    /// Names of output artifact slots this node produces.
    pub declared_outputs: Vec<String>,
    /// Maximum wall-clock time allowed for this node to complete.
    ///
    /// `None` means no node-level timeout; the pipeline-level setting applies.
    pub timeout: Option<TimeoutSeconds>,
    /// Maximum token cost this node may accumulate.
    ///
    /// `None` means the node uses the pipeline-level cost budget.
    pub cost_budget: Option<CostBudget>,
    /// Gate type: auto-proceed or human approval required.
    pub gate: NodeGate,
    /// Validation applied to the node's output before edge evaluation begins.
    pub validation_kind: ValidationKind,
    /// When `true`, failure of this node cancels all concurrently active siblings.
    pub abort_siblings_on_failure: bool,
}

/// A composite edge condition combining inner conditions with boolean logic.
///
/// Uses `Box` for the `Not` variant to break the recursive type cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompositeCondition {
    /// All inner conditions must evaluate to `true`.
    And(Vec<EdgeConditionKind>),
    /// At least one inner condition must evaluate to `true`.
    Or(Vec<EdgeConditionKind>),
    /// The inner condition must evaluate to `false`.
    Not(Box<EdgeConditionKind>),
}

/// The condition guarding an edge — the criterion that must be satisfied for
/// the edge to fire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeConditionKind {
    /// A deterministic boolean expression evaluated against [`PipelineState`].
    Deterministic(Expression),
    /// A natural-language condition evaluated by an LLM against node output.
    LlmEvaluated(NaturalLanguageCondition),
    /// A composite of simpler conditions combined with boolean operators.
    Composite(CompositeCondition),
}

/// Semantics of traversal for a rework (back) edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReworkSemantics {
    /// The target node re-executes with the same input (identical retry).
    Retry,
    /// The target node re-executes with its input enriched by findings from the
    /// current node's output (guided rework).
    Rework,
}

/// What happens when a rework edge exceeds its `max_traversals` limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OverflowBehaviour {
    /// Stop the pipeline with a structured error.
    HaltWithError,
    /// Escalate to a human reviewer with a structured report.
    Escalate,
    /// Activate the specified forward edge instead of continuing the loop.
    TakeEdge(EdgeId),
}

/// Metadata for a directed edge that can loop back to an earlier node.
///
/// Every cycle in the graph must have at least one `ReworkEdge` with a finite
/// `max_traversals` — enforced by [`validate_pipeline_graph`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReworkEdge {
    /// Maximum number of times this edge may be traversed in a single run.
    ///
    /// Must be ≥ 1. [`validate_pipeline_graph`] will return
    /// [`GraphValidationError::InvalidMaxTraversals`] for any rework edge
    /// with `max_traversals == 0`.
    pub max_traversals: u32,
    /// Output artifact keys from the source node preserved and forwarded to
    /// the target node on every traversal.
    pub preserved_outputs: Vec<String>,
    /// Behaviour when `max_traversals` is exceeded.
    pub overflow_behaviour: OverflowBehaviour,
    /// Whether the target re-runs with the same input or enriched input.
    pub semantics: ReworkSemantics,
}

/// The complete definition of a directed edge in the pipeline configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDefinition {
    /// Unique edge identifier within the pipeline graph.
    pub id: EdgeId,
    /// The node this edge originates from.
    pub source: NodeId,
    /// The node this edge leads to.
    pub target: NodeId,
    /// Condition that must be satisfied for this edge to fire.
    pub condition: EdgeConditionKind,
    /// Rework semantics; present only for back-edges (cycle edges).
    ///
    /// Forward-only edges have `None`. The graph validator rejects cycles that
    /// contain no edge with `rework_edge: Some(_)`.
    pub rework_edge: Option<ReworkEdge>,
}

/// Pipeline-level execution settings applied when no node-level override exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSettings {
    /// Default wall-clock timeout applied to nodes without their own timeout.
    pub default_timeout: Option<TimeoutSeconds>,
    /// Default cost budget applied to nodes without their own budget.
    pub default_cost_budget: Option<CostBudget>,
    /// Maximum retries for any node before the pipeline escalates.
    pub max_node_retries: u32,
}

/// A complete, validated pipeline graph with all structural metadata.
///
/// Produced by [`validate_pipeline_graph`]. This is the runtime representation
/// loaded from a configuration file after validation succeeds.
///
/// ## Loading Sequence
///
/// Always deserialise → validate → use. A deserialised `PipelineGraph` is
/// **not** guaranteed valid. Call [`validate_pipeline_graph`] before passing
/// this value to any execution logic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineGraph {
    /// Ordered list of node definitions.
    pub nodes: Vec<NodeDefinition>,
    /// Ordered list of edge definitions.
    pub edges: Vec<EdgeDefinition>,
    /// Per-node edge evaluation mode overrides.
    ///
    /// Nodes absent from this map use [`EvaluationMode::FirstMatching`].
    pub evaluation_modes: HashMap<NodeId, EvaluationMode>,
    /// Per-node explicit edge lists, used when [`EvaluationMode::Explicit`] is active.
    ///
    /// Only consulted when `evaluation_modes[node_id] == EvaluationMode::Explicit`.
    /// Nodes absent from this map that have `Explicit` mode produce a
    /// [`GraphValidationError`] at validation time.
    pub explicit_edge_lists: HashMap<NodeId, Vec<EdgeId>>,
    /// Pipeline-level execution settings.
    pub settings: PipelineSettings,
    /// Tool-profile overrides scoped to this pipeline.
    ///
    /// Stored here (not on [`PipelineConfiguration`]) so that two pipelines
    /// in the same configuration file with identically named nodes do not
    /// share override entries.
    pub tool_profiles: PipelineToolProfileConfig,
}

/// Tool-profile overrides declared in a pipeline configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineToolProfileConfig {
    /// The tool profile applied by default to all nodes.
    pub default_profile: ProfileName,
    /// Per-node overrides of the default tool profile.
    pub node_overrides: HashMap<NodeId, ProfileName>,
}

/// The full content of a `.cogworks/pipeline.toml` configuration file.
///
/// A single file may declare multiple named pipelines; `cli` selects the
/// active pipeline by [`PipelineName`] at startup.
///
/// ## Loading Sequence
///
/// Always deserialise → validate each [`PipelineGraph`] → use. Call
/// [`validate_pipeline_graph`] on every graph in `pipelines` before starting
/// a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PipelineConfiguration {
    /// All named pipeline graphs declared in the configuration.
    ///
    /// Each [`PipelineGraph`] carries its own tool-profile overrides.
    pub pipelines: HashMap<PipelineName, PipelineGraph>,
}

// ─── Runtime state ──────────────────────────────────────────────────────────

/// Execution phase of a single node within a pipeline run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node has not yet been started in this run.
    Pending,
    /// Node is currently executing.
    Active,
    /// Node completed successfully and its outputs are available.
    Completed,
    /// Node failed and has exhausted its retry budget.
    Failed,
    /// Node output is awaiting human review before the pipeline can continue.
    HumanGated,
}

/// All mutable runtime state associated with a single node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    /// Current execution phase of this node.
    pub status: NodeStatus,
    /// Total number of execution attempts, including the first.
    pub attempt_count: u32,
    /// Number of times this node has been re-executed due to rework feedback.
    pub rework_count: u32,
    /// Error description from the most recent failed attempt, if any.
    pub current_error: Option<String>,
    /// Per-rework-edge traversal counts for cycle-termination enforcement.
    ///
    /// Keys are [`EdgeId`]s of rework edges connected to this node.
    /// Values are the number of times each has been traversed in this run.
    pub rework_edge_traversals: HashMap<EdgeId, u32>,
}

/// The complete runtime state of a pipeline run at a single point in time.
///
/// Updated atomically at every node boundary and persisted via
/// [`PipelineStateComment`] to GitHub.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    /// Identifies the pipeline run this state belongs to.
    pub run_id: PipelineRunId,
    /// Per-node runtime state, keyed by [`NodeId`].
    pub node_states: HashMap<NodeId, NodeState>,
    /// Sets of nodes currently executing in parallel.
    ///
    /// Each inner `Vec` is one concurrent branch. Empty when no parallel
    /// execution is in progress.
    pub active_parallel_branches: Vec<Vec<NodeId>>,
    /// Total token cost accumulated so far in this run (USD).
    ///
    /// Starts at [`TokenCost::zero()`] when a run begins. Compare against
    /// the configured [`CostBudget`] using [`CostBudget::is_exceeded_by`].
    pub cost_accumulator: TokenCost,
}

/// Which component performed a given edge-condition evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvaluatorKind {
    /// A deterministic boolean expression evaluator.
    Deterministic,
    /// An LLM model.
    LlmModel {
        /// Identifier of the specific model used (e.g. `"claude-3-7-sonnet"`).
        model_id: String,
    },
    /// A composite condition whose inner evaluators are listed separately.
    Composite,
}

/// Audit record for a single edge-condition evaluation.
///
/// Every evaluation is recorded regardless of outcome, satisfying
/// `docs/spec/constraints.md §Edge condition evaluation is audited`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeEvaluationRecord {
    /// The edge whose condition was evaluated.
    pub edge_id: EdgeId,
    /// The condition definition that was applied.
    pub condition: EdgeConditionKind,
    /// Snapshot of the [`PipelineState`] used as evaluation input.
    ///
    /// Stored as [`serde_json::Value`] rather than a pre-serialised string to
    /// avoid double-escaping when this record is embedded in
    /// [`PipelineStateComment`] (also JSON). Keeps the persisted comment
    /// human-readable and directly queryable.
    pub input_snapshot: serde_json::Value,
    /// Whether the condition evaluated to `true`.
    pub result: bool,
    /// The component that performed the evaluation.
    pub evaluator: EvaluatorKind,
    /// Wall-clock time at which the evaluation was performed.
    pub timestamp: Timestamp,
}

/// Schema version token for [`PipelineStateComment`].
///
/// Deserialisation via `serde` automatically rejects any version string that
/// is not a known value (currently only `"1"`). This is enforced at the serde
/// boundary via `#[serde(try_from = "String")]`, so no additional runtime
/// validation is required by callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SchemaVersion(String);

impl SchemaVersion {
    /// The current (and only known) schema version.
    pub const CURRENT: &'static str = "1";

    /// Returns the current schema version.
    pub fn current() -> Self {
        Self(Self::CURRENT.to_string())
    }

    /// Returns the version string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SchemaVersion {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "1" => Ok(Self(value)),
            other => Err(format!(
                "Unknown PipelineStateComment schema version {other:?}; expected \"1\""
            )),
        }
    }
}

impl From<SchemaVersion> for String {
    fn from(v: SchemaVersion) -> Self {
        v.0
    }
}

/// Serialisable snapshot written to a GitHub issue comment at every node boundary.
///
/// ## Source of Truth Contract
///
/// This struct MUST contain enough information to fully reconstruct the pipeline
/// execution state with no other persistent source. The working directory is
/// a performance optimisation; its loss must not require a pipeline restart
/// (see `docs/spec/constraints.md §Pipeline state is recoverable from GitHub`).
///
/// On resume after interruption, the executor loads the most recent
/// `PipelineStateComment` from GitHub and reconstructs [`PipelineState`] from it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStateComment {
    /// Schema version for forward-compatibility.
    ///
    /// Deserialisation fails automatically for any version string that is
    /// not `"1"` (enforced by [`SchemaVersion`]'s `TryFrom` impl).
    pub schema_version: SchemaVersion,
    /// Identifies the pipeline run this comment belongs to.
    pub pipeline_run_id: PipelineRunId,
    /// The GitHub Issue number this pipeline run is processing.
    pub work_item_id: WorkItemId,
    /// Full pipeline runtime state at the time this comment was written.
    pub state: PipelineState,
    /// SHA-256 hex digest of the pipeline configuration used for this run.
    ///
    /// Compared on resume to detect configuration drift. A mismatch must
    /// cause an escalation rather than a silent state corruption.
    pub graph_hash: String,
    /// Wall-clock time this comment was authored.
    pub written_at: Timestamp,
}

// ─── Error types ────────────────────────────────────────────────────────────

/// Returned by [`topological_sort`] when the graph contains a directed cycle
/// among the non-rework (forward) edges.
///
/// This is distinct from [`GraphValidationError::UnterminatedCycle`]: a
/// `CycleError` indicates the forward-edge subgraph is not a DAG (a hard
/// configuration error). `UnterminatedCycle` indicates a loop exists but has
/// no rework edge with a finite `max_traversals` (also a configuration error,
/// but detected by [`validate_pipeline_graph`] which translates any
/// `CycleError` result appropriately).
///
/// A cycle is only valid if every path around it passes through at least one
/// edge with `rework_edge: Some(_)` specifying a finite `max_traversals`.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("Cycle detected: {}", cycle.iter().map(NodeId::as_str).collect::<Vec<_>>().join(" → "))]
pub struct CycleError {
    /// The sequence of node IDs forming the detected cycle.
    pub cycle: Vec<NodeId>,
}

/// A single structural violation found by [`validate_pipeline_graph`].
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum GraphValidationError {
    /// The graph contains no nodes.
    #[error("Pipeline graph is empty")]
    EmptyGraph,

    /// A cycle exists without any rework edge providing a termination condition.
    ///
    /// Produced when [`topological_sort`] detects a forward-edge cycle that
    /// [`validate_pipeline_graph`] determines is missing a terminating rework
    /// edge. See also [`CycleError`] for the lower-level sort error.
    #[error("Cycle through {nodes:?} has no rework edge — infinite execution is possible")]
    UnterminatedCycle {
        /// Node IDs forming the unterminated cycle.
        nodes: Vec<NodeId>,
    },

    /// A rework edge declares `max_traversals == 0`, which would make the
    /// loop immediately enter overflow on the first traversal.
    ///
    /// `max_traversals` must be ≥ 1.
    #[error("Rework edge '{edge}' has max_traversals = 0; must be ≥ 1")]
    InvalidMaxTraversals {
        /// The rework edge with the invalid traversal count.
        edge: EdgeId,
    },

    /// A node has no incoming or outgoing edges (unreachable or dead-end).
    #[error("Node '{node}' is an orphan (no connected edges)")]
    OrphanNode {
        /// The orphaned node identifier.
        node: NodeId,
    },

    /// Two nodes share the same identifier.
    #[error("Duplicate node ID: '{id}'")]
    DuplicateNodeId {
        /// The duplicated identifier.
        id: NodeId,
    },

    /// Two edges share the same identifier.
    #[error("Duplicate edge ID: '{id}'")]
    DuplicateEdgeId {
        /// The duplicated identifier.
        id: EdgeId,
    },

    /// An edge references a node that is not declared in the graph.
    #[error("Edge '{edge}' references unknown node '{node}'")]
    UnknownNode {
        /// The edge that contains the bad reference.
        edge: EdgeId,
        /// The node ID that could not be resolved.
        node: NodeId,
    },
}

// ─── Pure business logic functions ──────────────────────────────────────────

/// Returns the forward-edge topological ordering of node IDs (sources first).
///
/// Rework (back) edges are excluded from the sort traversal; the result
/// represents the primary execution order only.
///
/// # Errors
///
/// Returns [`CycleError`] if the graph contains a directed cycle among the
/// non-rework edges (which would indicate a configuration error).
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-graph.md §topological_sort`
pub fn topological_sort(
    _nodes: &[NodeDefinition],
    _edges: &[EdgeDefinition],
) -> Result<Vec<NodeId>, CycleError> {
    todo!("See docs/spec/interfaces/pipeline-graph.md §topological_sort")
}

/// Evaluates a deterministic [`Expression`] against the current [`PipelineState`].
///
/// Returns `true` if the condition is satisfied, `false` otherwise. Pure;
/// no side effects.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-graph.md §evaluate_deterministic_condition`
#[must_use]
pub fn evaluate_deterministic_condition(_expr: &Expression, _state: &PipelineState) -> bool {
    todo!("See docs/spec/interfaces/pipeline-graph.md §evaluate_deterministic_condition")
}

/// Validates a [`PipelineGraph`] for structural correctness before any
/// node executes.
///
/// Checks: non-empty graph, unique IDs, valid edge references, no orphan
/// nodes, no unterminated cycles.
///
/// # Errors
///
/// Returns `Err(Vec<GraphValidationError>)` listing every violation found.
/// Returns `Ok(())` only when the graph is fully valid.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-graph.md §validate_pipeline_graph`
pub fn validate_pipeline_graph(_graph: &PipelineGraph) -> Result<(), Vec<GraphValidationError>> {
    todo!("See docs/spec/interfaces/pipeline-graph.md §validate_pipeline_graph")
}

/// Returns the set of nodes eligible to execute next given the current state.
///
/// A node is eligible when all of the following hold:
/// - Its [`NodeStatus`] is [`NodeStatus::Pending`].
/// - All upstream nodes (via non-rework forward edges) have status
///   [`NodeStatus::Completed`].
/// - All its [`NodeDefinition::declared_inputs`] are available in the artifact
///   store (checked by the caller before starting the node).
///
/// Gate status is **not** evaluated here; the caller is responsible for
/// checking [`NodeGate`] before actually starting eligible nodes.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-graph.md §compute_eligible_nodes`
#[must_use]
pub fn compute_eligible_nodes(_state: &PipelineState, _graph: &PipelineGraph) -> Vec<NodeId> {
    todo!("See docs/spec/interfaces/pipeline-graph.md §compute_eligible_nodes")
}
