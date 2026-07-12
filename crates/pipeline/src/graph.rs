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

use std::collections::{HashMap, HashSet, VecDeque};

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
        if v.is_empty() { None } else { Some(Self(v)) }
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
        if v.is_empty() { None } else { Some(Self(v)) }
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
    /// Wall-clock time at which this node transitioned to [`NodeStatus::Active`].
    ///
    /// Set by the orchestrator when the node first starts executing. Used by
    /// [`crate::determine_next_actions`] to detect elapsed timeouts without
    /// requiring any I/O inside that pure function.
    ///
    /// `None` means the node has not yet been activated in this run.
    pub activated_at: Option<crate::Timestamp>,
}

/// Human-gate status for a specific node within an active pipeline run.
///
/// Tracks whether a human has approved or rejected a gated node's output.
///
/// Persisted as part of [`PipelineStateComment`] so that gate decisions
/// survive process restarts without requiring a re-approval.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §[`GateStatus`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateStatus {
    /// The node completed successfully and is waiting for human review.
    AwaitingApproval,
    /// A human reviewer approved this node's output; the pipeline may continue.
    Approved {
        /// The GitHub username of the approver.
        ///
        /// Raw `String` — no `GitHubUsername` newtype exists in the identifiers
        /// module. Values originate from the GitHub API `login` field and are
        /// expected to be non-empty strings conforming to GitHub's username
        /// constraints (alphanumeric + hyphens, 1–39 chars). No validation is
        /// performed here; the GitHub API is the authoritative source.
        approved_by: String,
    },
    /// A human reviewer rejected this node's output; the pipeline must not continue.
    Rejected {
        /// The GitHub username of the reviewer.
        ///
        /// Raw `String` — no `GitHubUsername` newtype exists in the identifiers
        /// module. Values originate from the GitHub API `login` field and are
        /// expected to be non-empty strings conforming to GitHub's username
        /// constraints (alphanumeric + hyphens, 1–39 chars). No validation is
        /// performed here; the GitHub API is the authoritative source.
        rejected_by: String,
        /// Human-readable explanation of the rejection.
        reason: String,
    },
}

// ---------------------------------------------------------------------------

/// Runtime gate state for all nodes in an active pipeline run.
///
/// Persisted inside [`PipelineStateComment`] so that gate approvals and
/// rejections survive process restarts.  On resume the executor reconstructs
/// this value from the latest state comment and passes it unchanged to
/// [`crate::determine_next_actions`].
///
/// See `docs/spec/interfaces/pipeline-execution.md` §[`GateConfig`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateConfig {
    /// Per-node gate status, keyed by the node that completed and was gated.
    ///
    /// Nodes absent from this map have never been gated (or were
    /// `AutoProceed` nodes).
    pub gated_nodes: HashMap<NodeId, GateStatus>,
}

// ---------------------------------------------------------------------------

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
    #[must_use]
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
    /// Gate approval and rejection records for all human-gated nodes in this run.
    ///
    /// Persisted alongside `state` so that gate decisions survive process
    /// restarts.  On resume, the executor passes this value directly to
    /// [`crate::determine_next_actions`].  An absent field deserialises as
    /// [`GateConfig::default`] (empty map), which is correct for fresh runs.
    #[serde(default)]
    pub gate_config: GateConfig,
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
#[error("Cycle detected involving: {}", cycle.iter().map(NodeId::as_str).collect::<Vec<_>>().join(", "))]
pub struct CycleError {
    /// Node IDs of nodes involved in the detected cycle.
    ///
    /// Collected in declaration order from the `nodes` slice, **not** in
    /// cycle-traversal order. The display message uses `", "` as separator
    /// rather than `" \u2192 "` to avoid implying a directed ordering.
    pub cycle: Vec<NodeId>,
}

/// A single structural violation found by [`validate_pipeline_graph`].
///
/// This enum is `#[non_exhaustive]` — future checks may add new variants
/// without requiring downstream match sites to be updated immediately.
#[non_exhaustive]
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

    /// A node has [`EvaluationMode::Explicit`] but is absent from [`PipelineGraph::explicit_edge_lists`].
    #[error("Node '{node}' has Explicit evaluation mode but no explicit-edge-list entry")]
    ExplicitModeWithoutEdgeList {
        /// The node whose explicit-edge-list entry is missing.
        node: NodeId,
    },

    /// A rework edge's [`OverflowBehaviour::TakeEdge`] names an edge that is
    /// not declared in the graph. The pipeline cannot use this overflow path.
    #[error(
        "Rework edge '{rework_edge}' overflow behaviour references unknown edge '{overflow_edge}'"
    )]
    UnknownOverflowEdge {
        /// The rework edge containing the bad overflow edge reference.
        rework_edge: EdgeId,
        /// The edge ID that could not be resolved.
        overflow_edge: EdgeId,
    },

    /// A node declares the same artifact slot name more than once in its
    /// `declared_inputs` or `declared_outputs`.
    ///
    /// Spec invariant: *"`declared_inputs` and `declared_outputs` must not
    /// contain duplicate names within the same node."*
    #[error("Node '{node}' has duplicate slot name '{slot}'")]
    DuplicateSlotName {
        /// The node containing the duplicate slot.
        node: NodeId,
        /// The duplicated slot name.
        slot: String,
    },
}

// ─── Pure business logic functions ──────────────────────────────────────────

/// Collects all node IDs from a slice into a [`HashSet`] for O(1) membership
/// tests.
///
/// Shared by [`topological_sort`] and [`validate_pipeline_graph`] to test
/// whether edge endpoints reference declared nodes.
fn make_node_id_set(nodes: &[NodeDefinition]) -> HashSet<&NodeId> {
    nodes.iter().map(|n| &n.id).collect()
}

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
    nodes: &[NodeDefinition],
    edges: &[EdgeDefinition],
) -> Result<Vec<NodeId>, CycleError> {
    // Build an index of all node IDs.
    let node_set = make_node_id_set(nodes);

    // Consider only forward (non-rework) edges.
    let forward_edges: Vec<&EdgeDefinition> =
        edges.iter().filter(|e| e.rework_edge.is_none()).collect();

    // Build adjacency list and in-degree map restricted to known nodes.
    let mut adjacency: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
    let mut in_degree: HashMap<&NodeId, usize> = HashMap::new();

    for node in nodes {
        adjacency.entry(&node.id).or_default();
        in_degree.entry(&node.id).or_insert(0);
    }

    for edge in &forward_edges {
        if node_set.contains(&edge.source) && node_set.contains(&edge.target) {
            adjacency
                .entry(&edge.source)
                .or_default()
                .push(&edge.target);
            *in_degree.entry(&edge.target).or_insert(0) += 1;
        }
    }

    // Initialise queue with all zero-in-degree nodes, sorted for stable order.
    let mut queue: VecDeque<&NodeId> = {
        let mut sources: Vec<&NodeId> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();
        sources.sort_by_key(|id| id.as_str());
        VecDeque::from(sources)
    };

    let mut result: Vec<NodeId> = Vec::with_capacity(nodes.len());

    while let Some(current) = queue.pop_front() {
        result.push(current.clone());

        if let Some(neighbours) = adjacency.get(current) {
            let mut sorted_neighbours = neighbours.clone();
            sorted_neighbours.sort_by_key(|id| id.as_str());
            for neighbour in sorted_neighbours {
                let deg = in_degree.entry(neighbour).or_insert(0);
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(neighbour);
                }
            }
        }
    }

    if result.len() < nodes.len() {
        // Collect the unprocessed nodes — they form the cycle.
        let processed: HashSet<&NodeId> = result.iter().collect();
        let cycle: Vec<NodeId> = nodes
            .iter()
            .map(|n| &n.id)
            .filter(|id| !processed.contains(id))
            .cloned()
            .collect();
        return Err(CycleError { cycle });
    }

    Ok(result)
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
pub fn evaluate_deterministic_condition(expr: &Expression, state: &PipelineState) -> bool {
    evaluate_condition_inner(expr.as_str(), state).unwrap_or(false)
}

/// Inner evaluator; returns `None` on any parse or navigation failure (→ false).
fn evaluate_condition_inner(expr: &str, state: &PipelineState) -> Option<bool> {
    // Find the operator: try " == " first, then " != ".
    let (lhs, op, rhs) = if let Some(pos) = expr.find(" == ") {
        (&expr[..pos], "==", &expr[pos + 4..])
    } else {
        let pos = expr.find(" != ")?;
        (&expr[..pos], "!=", &expr[pos + 4..])
    };

    // Navigate the JSON representation of PipelineState.
    let json_state = serde_json::to_value(state).ok()?;
    let json_val = navigate_json(&json_state, lhs)?;

    // Parse the RHS literal.
    let rhs = rhs.trim();
    let expected = parse_literal(rhs)?;

    let equal = *json_val == expected;
    Some(if op == "==" { equal } else { !equal })
}

/// Descend into a `serde_json::Value` using a dot-separated path.
/// Returns `None` if any segment is missing.
fn navigate_json<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = root;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

/// Parse a literal token into a `serde_json::Value`.
/// Supports single-quoted strings, double-quoted strings, and boolean literals.
fn parse_literal(raw: &str) -> Option<serde_json::Value> {
    if raw.len() >= 2
        && ((raw.starts_with('"') && raw.ends_with('"'))
            || (raw.starts_with('\'') && raw.ends_with('\'')))
    {
        // Strip the surrounding quote characters.
        // raw.len() >= 2 guarantees this slice never panics.
        let inner = &raw[1..raw.len() - 1];
        Some(serde_json::Value::String(inner.to_string()))
    } else if raw == "true" {
        Some(serde_json::Value::Bool(true))
    } else if raw == "false" {
        Some(serde_json::Value::Bool(false))
    } else {
        None
    }
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
pub fn validate_pipeline_graph(graph: &PipelineGraph) -> Result<(), Vec<GraphValidationError>> {
    let mut errors: Vec<GraphValidationError> = Vec::new();

    // Pre-compute shared lookup structures.
    let node_id_set = make_node_id_set(&graph.nodes);
    let edge_id_set: HashSet<&EdgeId> = graph.edges.iter().map(|e| &e.id).collect();

    check_empty_graph(graph, &mut errors);
    check_duplicate_node_ids(graph, &mut errors);
    check_duplicate_edge_ids(graph, &mut errors);
    check_unknown_node_references(graph, &node_id_set, &mut errors);
    check_orphan_nodes(graph, &mut errors);
    check_invalid_max_traversals(graph, &mut errors);
    check_unterminated_cycles(graph, &mut errors);
    check_explicit_mode_without_edge_list(graph, &mut errors);
    check_duplicate_slot_names(graph, &mut errors);
    check_unknown_overflow_edges(graph, &edge_id_set, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_empty_graph(graph: &PipelineGraph, errors: &mut Vec<GraphValidationError>) {
    if graph.nodes.is_empty() {
        errors.push(GraphValidationError::EmptyGraph);
    }
}

fn check_duplicate_node_ids(graph: &PipelineGraph, errors: &mut Vec<GraphValidationError>) {
    let mut seen: HashSet<&NodeId> = HashSet::new();
    for node in &graph.nodes {
        if !seen.insert(&node.id) {
            errors.push(GraphValidationError::DuplicateNodeId {
                id: node.id.clone(),
            });
        }
    }
}

fn check_duplicate_edge_ids(graph: &PipelineGraph, errors: &mut Vec<GraphValidationError>) {
    let mut seen: HashSet<&EdgeId> = HashSet::new();
    for edge in &graph.edges {
        if !seen.insert(&edge.id) {
            errors.push(GraphValidationError::DuplicateEdgeId {
                id: edge.id.clone(),
            });
        }
    }
}

fn check_unknown_node_references(
    graph: &PipelineGraph,
    node_id_set: &HashSet<&NodeId>,
    errors: &mut Vec<GraphValidationError>,
) {
    for edge in &graph.edges {
        if !node_id_set.contains(&edge.source) {
            errors.push(GraphValidationError::UnknownNode {
                edge: edge.id.clone(),
                node: edge.source.clone(),
            });
        }
        if !node_id_set.contains(&edge.target) {
            errors.push(GraphValidationError::UnknownNode {
                edge: edge.id.clone(),
                node: edge.target.clone(),
            });
        }
    }
}

fn check_orphan_nodes(graph: &PipelineGraph, errors: &mut Vec<GraphValidationError>) {
    let mut connected: HashSet<&NodeId> = HashSet::new();
    for edge in &graph.edges {
        connected.insert(&edge.source);
        connected.insert(&edge.target);
    }
    for node in &graph.nodes {
        if !connected.contains(&node.id) {
            errors.push(GraphValidationError::OrphanNode {
                node: node.id.clone(),
            });
        }
    }
}

fn check_invalid_max_traversals(graph: &PipelineGraph, errors: &mut Vec<GraphValidationError>) {
    for edge in &graph.edges {
        if let Some(rework) = &edge.rework_edge
            && rework.max_traversals == 0
        {
            errors.push(GraphValidationError::InvalidMaxTraversals {
                edge: edge.id.clone(),
            });
        }
    }
}

fn check_unterminated_cycles(graph: &PipelineGraph, errors: &mut Vec<GraphValidationError>) {
    if let Err(cycle_error) = topological_sort(&graph.nodes, &graph.edges) {
        errors.push(GraphValidationError::UnterminatedCycle {
            nodes: cycle_error.cycle,
        });
    }
}

fn check_explicit_mode_without_edge_list(
    graph: &PipelineGraph,
    errors: &mut Vec<GraphValidationError>,
) {
    for (node_id, mode) in &graph.evaluation_modes {
        if matches!(mode, EvaluationMode::Explicit)
            && !graph.explicit_edge_lists.contains_key(node_id)
        {
            errors.push(GraphValidationError::ExplicitModeWithoutEdgeList {
                node: node_id.clone(),
            });
        }
    }
}

fn check_duplicate_slot_names(graph: &PipelineGraph, errors: &mut Vec<GraphValidationError>) {
    for node in &graph.nodes {
        // Inputs and outputs are validated independently. A slot name that
        // appears in both lists is permitted: pass-through nodes legitimately
        // read and write the same artifact under the same name.
        for slots in [&node.declared_inputs, &node.declared_outputs] {
            let mut seen: HashSet<&str> = HashSet::new();
            for slot in slots {
                if !seen.insert(slot.as_str()) {
                    errors.push(GraphValidationError::DuplicateSlotName {
                        node: node.id.clone(),
                        slot: slot.clone(),
                    });
                }
            }
        }
    }
}

fn check_unknown_overflow_edges(
    graph: &PipelineGraph,
    edge_id_set: &HashSet<&EdgeId>,
    errors: &mut Vec<GraphValidationError>,
) {
    for edge in &graph.edges {
        if let Some(rework) = &edge.rework_edge
            && let OverflowBehaviour::TakeEdge(ref overflow_edge_id) = rework.overflow_behaviour
            && !edge_id_set.contains(overflow_edge_id)
        {
            errors.push(GraphValidationError::UnknownOverflowEdge {
                rework_edge: edge.id.clone(),
                overflow_edge: overflow_edge_id.clone(),
            });
        }
    }
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
pub fn compute_eligible_nodes(state: &PipelineState, graph: &PipelineGraph) -> Vec<NodeId> {
    let mut eligible: Vec<NodeId> = Vec::new();

    for node in &graph.nodes {
        // Skip nodes that are not Pending.
        let status = state
            .node_states
            .get(&node.id)
            .map_or(NodeStatus::Pending, |ns| ns.status);
        if status != NodeStatus::Pending {
            continue;
        }

        // Collect all forward-edge predecessors.
        let all_predecessors_completed = graph
            .edges
            .iter()
            .filter(|e| e.target == node.id && e.rework_edge.is_none())
            .all(|e| {
                state
                    .node_states
                    .get(&e.source)
                    .is_some_and(|ns| ns.status == NodeStatus::Completed)
            });

        if all_predecessors_completed {
            eligible.push(node.id.clone());
        }
    }

    eligible
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
