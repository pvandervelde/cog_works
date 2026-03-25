//! Integration node — pull request creation and safety gating.
//!
//! The Integration node is the final step in the default pipeline. It takes
//! all generated artifacts and creates (or updates) a pull request. It:
//! 1. Creates or updates a pull request via `PullRequestManager::create_pull_request`.
//! 2. Posts review findings as inline review comments.
//! 3. Applies safety-approval labels if `ClassificationResult::safety_affecting`.
//! 4. Writes the `PullRequestId` to output artifacts.
//!
//! ## Output Artifacts
//!
//! | Slot | Content |
//! |------|---------|
//! | `pull_request_id` | [`pipeline::PullRequestId`] (JSON) |
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/nodes.md` §IntegrationNode.

use pipeline::PullRequestManager;

use crate::node::{NodeExecutionResult, NodeInput, WorkingCopyManager};

/// Integration node — creates the pull request and applies safety gates.
///
/// See `docs/spec/interfaces/nodes.md` §IntegrationNode.
pub struct IntegrationNode;

impl IntegrationNode {
    /// Executes the Integration node for `input`.
    ///
    /// # Arguments
    ///
    /// * `input` — Node inputs including `generated_files` and `review_decision`.
    /// * `pr_manager` — Pull request manager for creating/updating the PR.
    /// * `working_copy` — Working copy manager for branch and file operations.
    ///
    /// # Returns
    ///
    /// `NodeExecutionResult::Success` with the `pull_request_id` artifact, or
    /// `NodeExecutionResult::Failure`.
    ///
    /// See `docs/spec/interfaces/nodes.md` §IntegrationNode.
    pub async fn execute(
        _input: NodeInput,
        _pr_manager: &dyn PullRequestManager,
        _working_copy: &WorkingCopyManager,
    ) -> NodeExecutionResult {
        todo!("See docs/spec/interfaces/nodes.md §IntegrationNode")
    }
}
