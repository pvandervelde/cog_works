//! Spawning node — child work-item creation.
//!
//! The Spawning node is used when a work item must be decomposed into entirely
//! separate GitHub issues (as opposed to sub-work-items within the same issue).
//! It:
//! 1. Classifies the spawnable tasks via an LLM call.
//! 2. Creates GitHub sub-issues via `IssueTracker::create_sub_issue`.
//! 3. Emits `NodeStateUpdate` entries marking spawned child nodes as `Active`.
//! 4. Writes the `Vec<WorkItemId>` of spawned issues to output artifacts.
//!
//! ## Output Artifacts
//!
//! | Slot | Content |
//! |------|---------|
//! | `spawned_work_items` | [`Vec<pipeline::WorkItemId>`] (JSON) |
//!
//! ## Node State Updates
//!
//! The [`pipeline::NodeOutput::state_updates`] field carries
//! [`pipeline::NodeStateUpdate`] entries that mark newly spawned child-node
//! slots as `Active`. The executor applies these to `PipelineState` before
//! edge evaluation.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/nodes.md` §SpawningNode.

use pipeline::IssueTracker;

use crate::gateway::LlmGateway;
use crate::node::{NodeExecutionResult, NodeInput};

/// Spawning node — creates child work items from the current work item.
///
/// See `docs/spec/interfaces/nodes.md` §SpawningNode.
pub struct SpawningNode;

impl SpawningNode {
    /// Executes the Spawning node for `input`.
    ///
    /// # Arguments
    ///
    /// * `input` — Node inputs including the work item classification.
    /// * `gateway` — LLM gateway for identifying spawnable sub-tasks.
    /// * `issues` — Issue tracker for creating the spawned GitHub sub-issues.
    ///
    /// # Returns
    ///
    /// `NodeExecutionResult::Success` with `spawned_work_items` artifact and
    /// `state_updates` marking child nodes as `Active`, or
    /// `NodeExecutionResult::Failure`.
    ///
    /// See `docs/spec/interfaces/nodes.md` §SpawningNode.
    pub async fn execute(
        _input: NodeInput,
        _gateway: &LlmGateway,
        _issues: &dyn IssueTracker,
    ) -> NodeExecutionResult {
        todo!("See docs/spec/interfaces/nodes.md §SpawningNode")
    }
}
