//! Pipeline executor — step-function loop and infrastructure dependency holder.
//!
//! [`PipelineExecutor`] holds every infrastructure dependency as an `Arc<dyn Trait>`
//! and is constructed once at startup in `cli/src/main.rs`. All pipeline runs
//! in service mode share the same executor instance.
//!
//! [`run_step`] drives one step of the pipeline state machine:
//!
//! 1. Load and validate constitutional rules.
//! 2. Load and validate the pipeline graph.
//! 3. Reconstruct pipeline state from GitHub.
//! 4. Determine next actions.
//! 5. Dispatch — route action variants to the appropriate outcome path.
//! 6. Execute eligible nodes (with budget serialisation for parallel batches).
//! 7. Persist updated `PipelineStateComment` to GitHub.
//! 8. Return a `StepResult`; callers must loop to process subsequent steps.
//!
//! See `docs/spec/interfaces/nodes.md` §Pipeline Executor for the full contract.

use std::sync::Arc;

use pipeline::{
    AuditStore, CodeRepository, DomainServiceClient, EscalationReason, InterfaceRegistryLoader,
    IssueTracker, LlmProvider, NodeId, NodeOutput, PipelineConfigurationLoader, PipelineError,
    PullRequestManager, SummaryCache, ToolProfileStore, WorkItemId,
};
use tracing::instrument;

use crate::gateway::LlmGateway;
use crate::node::CliConfig;

// ─── StepResult ──────────────────────────────────────────────────────────────

/// The outcome of one invocation of [`run_step`].
///
/// The `cli` crate inspects `StepResult` to decide whether to loop (service
/// mode) or exit (single-shot mode).
///
/// See `docs/spec/interfaces/nodes.md` §StepResult.
#[derive(Debug)]
pub enum StepResult {
    /// A single node completed successfully in this step.
    Completed {
        /// The node that completed.
        node_id: NodeId,
        /// The node's output, ready to be applied to the pipeline state.
        output: NodeOutput,
    },
    /// Multiple nodes completed concurrently in this step (parallel branch).
    ///
    /// Used when an `ExecuteParallel` action fired multiple nodes. The caller
    /// must apply **all** results to the pipeline state before evaluating
    /// outgoing edges. Results are in completion order (not the order given
    /// to `ExecuteParallel`).
    CompletedParallel {
        /// Per-node results for every node in the parallel branch.
        results: Vec<(NodeId, NodeOutput)>,
    },
    /// A [`pipeline::NodeGate::HumanGated`] node is awaiting reviewer approval.
    Gated {
        /// The node awaiting approval.
        node_id: NodeId,
        /// Human-readable explanation of what the reviewer must decide.
        gate_reason: String,
    },
    /// The pipeline cannot continue without human intervention.
    ///
    /// An escalation comment has been posted on the work-item issue.
    Escalated(EscalationReason),
    /// The pipeline encountered an unrecoverable error.
    ///
    /// A failure comment has been posted on the work-item issue.
    Halted(PipelineError),
}

// ─── PipelineExecutor ────────────────────────────────────────────────────────

/// Holds all infrastructure dependencies for one pipeline service instance.
///
/// Constructed once at startup in `cli/src/main.rs` and shared across all
/// `run_step` calls. All fields are `Arc`-wrapped so they can be cloned across
/// concurrent tasks when processing parallel node batches.
///
/// ## Construction
///
/// Use [`PipelineExecutor::new`] to assemble the executor from pre-built
/// infrastructure instances. The `cli` crate wires up the concrete types.
///
/// See `docs/spec/interfaces/nodes.md` §PipelineExecutor.
pub struct PipelineExecutor {
    /// Issue tracker for fetching work items and posting state comments.
    issues: Arc<dyn IssueTracker>,
    /// Pull request manager for creating and reviewing PRs.
    pull_requests: Arc<dyn PullRequestManager>,
    /// Code repository for reading files and constitutional rules.
    code_repository: Arc<dyn CodeRepository>,
    /// Domain service client for validation and interface extraction.
    domain_service: Arc<dyn DomainServiceClient>,
    /// LLM gateway shared across all nodes in each step.
    llm_gateway: Arc<LlmGateway>,
    /// Audit store for recording every LLM call, state transition, and finding.
    audit: Arc<dyn AuditStore>,
    /// Summary cache for pyramid-style context assembly.
    summary_cache: Arc<dyn SummaryCache>,
    /// Interface registry loader for cross-domain constraint validation.
    interface_registry: Arc<dyn InterfaceRegistryLoader>,
    /// Pipeline configuration loader for resolving the active pipeline graph.
    config_loader: Arc<dyn PipelineConfigurationLoader>,
    /// Tool profile store for resolving node tool availability.
    tool_profile_store: Arc<dyn ToolProfileStore>,
}

impl PipelineExecutor {
    /// Constructs a `PipelineExecutor` from the given infrastructure instances.
    ///
    /// This is the only constructor. It is called once from `cli/src/main.rs`
    /// after all infrastructure crates have been initialised.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issues: Arc<dyn IssueTracker>,
        pull_requests: Arc<dyn PullRequestManager>,
        code_repository: Arc<dyn CodeRepository>,
        domain_service: Arc<dyn DomainServiceClient>,
        llm_provider: Arc<dyn LlmProvider>,
        audit: Arc<dyn AuditStore>,
        summary_cache: Arc<dyn SummaryCache>,
        interface_registry: Arc<dyn InterfaceRegistryLoader>,
        config_loader: Arc<dyn PipelineConfigurationLoader>,
        tool_profile_store: Arc<dyn ToolProfileStore>,
    ) -> Self {
        Self {
            issues,
            pull_requests,
            code_repository,
            domain_service,
            llm_gateway: Arc::new(LlmGateway::new(llm_provider)),
            audit,
            summary_cache,
            interface_registry,
            config_loader,
            tool_profile_store,
        }
    }
}

// ─── run_step ────────────────────────────────────────────────────────────────

/// Drives one step of the pipeline state machine for `work_item_id`.
///
/// ## Step Sequence
///
/// 1. **Load constitutional rules** — read from `config.approved_branch`.
///    Failure halts immediately (`StepResult::Halted`).
/// 2. **Load pipeline config** — resolve via `config.pipeline_name`.
///    Graph validation failure halts immediately.
/// 3. **Reconstruct state** — fetch the latest `PipelineStateComment` from GitHub.
/// 4. **Determine next actions** — call `determine_next_actions`.
/// 5. **Dispatch** — inspect actions:
///    - Empty vec → run complete; write final state comment; return `StepResult::Completed`.
///    - `Wait` → return `StepResult::Gated`.
///    - `Escalate` / `HaltWithError` → write comment; return `StepResult::Escalated` /
///      `StepResult::Halted`.
///    - `ExecuteNode` / `ExecuteParallel` → proceed to step 6.
/// 6. **Execute nodes** — for eligible nodes; serialise budget checks across
///    any parallel batch via `BudgetTracker`. For `ExecuteParallel`, all listed
///    nodes run concurrently within this single call. On any `NonRetryable`
///    failure, apply the retry/rework policy; escalate or halt as appropriate.
/// 7. **Persist state** — write updated `PipelineStateComment` to the issue
///    via `executor.issues.post_comment`. State includes contributions from all
///    nodes in the batch.
/// 8. **Return** — `StepResult::Completed { node_id, output }` for the node
///    that completed. For a parallel batch, one representative node is returned;
///    the caller **must loop** — calling `run_step` again will pick up the next
///    set of eligible nodes from the persisted state.
///
/// See `docs/spec/interfaces/nodes.md` §run_step for the detailed decision table.
#[instrument(skip_all, fields(work_item_id = %_work_item_id))]
pub async fn run_step(
    _executor: &PipelineExecutor,
    _work_item_id: WorkItemId,
    _config: &CliConfig,
) -> StepResult {
    todo!("See docs/spec/interfaces/nodes.md §run_step")
}
