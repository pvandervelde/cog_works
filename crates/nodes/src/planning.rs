//! Planning node — work item decomposition and sub-work-item creation.
//!
//! The Planning node takes the interface stubs and architecture document and
//! decomposes the work item into an ordered sequence of sub-work-items. It:
//! 1. Calls the LLM to decompose the work into sub-tasks with explicit dependencies.
//! 2. Creates GitHub sub-issues via `IssueTracker::create_sub_issue`.
//! 3. Topologically sorts sub-work-items via `topological_sort_sub_work_items`.
//! 4. Writes the ordered plan to output artifacts.
//!
//! ## Output Artifacts
//!
//! | Slot | Content |
//! |------|---------|
//! | `sub_work_items` | Topologically sorted [`Vec<pipeline::SubWorkItem>`] (JSON) |
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/nodes.md` §PlanningNode.

use pipeline::IssueTracker;

use crate::gateway::LlmGateway;
use crate::node::{NodeExecutionResult, NodeInput, PipelineConfig};

/// Planning node — decomposes the work item into ordered sub-work-items.
///
/// See `docs/spec/interfaces/nodes.md` §PlanningNode.
pub struct PlanningNode;

impl PlanningNode {
    /// Executes the Planning node for `input`.
    ///
    /// # Arguments
    ///
    /// * `input` — Node inputs including `architecture_doc` and `interface_stubs`.
    /// * `gateway` — LLM gateway for work decomposition.
    /// * `issues` — Issue tracker for creating GitHub sub-issues.
    /// * `config` — Resolved pipeline configuration for this run.
    ///
    /// # Returns
    ///
    /// `NodeExecutionResult::Success` with the `sub_work_items` artifact, or
    /// `NodeExecutionResult::Failure`.
    ///
    /// See `docs/spec/interfaces/nodes.md` §PlanningNode.
    pub async fn execute(
        _input: NodeInput,
        _gateway: &LlmGateway,
        _issues: &dyn IssueTracker,
        _config: &PipelineConfig,
    ) -> NodeExecutionResult {
        todo!("See docs/spec/interfaces/nodes.md §PlanningNode")
    }
}
