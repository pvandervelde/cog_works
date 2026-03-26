//! Review node — multi-pass Quality/Architecture/Security review gate.
//!
//! The Review node runs three independent LLM-driven review passes and
//! aggregates their findings. It:
//! 1. Runs Quality, Architecture, and Security passes (one `LlmGateway::call` each).
//! 2. Validates cross-domain constraints via `ConstraintValidator`.
//! 3. Aggregates results via `aggregate_review_results`.
//! 4. Records findings to the audit trail.
//! 5. Writes the aggregate review decision to output artifacts.
//!
//! ## Output Artifacts
//!
//! | Slot | Content |
//! |------|---------|
//! | `review_decision` | [`pipeline::AggregateReviewDecision`] (JSON) |
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/nodes.md` §ReviewNode.

use pipeline::{AuditStore, LoadedContextPacks};

use crate::gateway::LlmGateway;
use crate::node::{ConstraintValidator, NodeExecutionResult, NodeInput};

/// Review node — runs the three-pass review gate.
///
/// See `docs/spec/interfaces/nodes.md` §ReviewNode.
pub struct ReviewNode;

impl ReviewNode {
    /// Executes the Review node for `input`.
    ///
    /// # Arguments
    ///
    /// * `input` — Node inputs including `generated_files` and `interface_stubs`.
    /// * `gateway` — LLM gateway for each review pass.
    /// * `constraint_validator` — Validates cross-domain interface constraints.
    /// * `packs` — Loaded context packs providing review guidance.
    /// * `audit` — Audit store for recording `AuditEvent::ValidationResult` entries.
    ///
    /// # Returns
    ///
    /// `NodeExecutionResult::Success` with the `review_decision` artifact, or
    /// `NodeExecutionResult::Failure`.
    ///
    /// See `docs/spec/interfaces/nodes.md` §ReviewNode.
    pub async fn execute(
        _input: NodeInput,
        _gateway: &LlmGateway,
        _constraint_validator: &ConstraintValidator,
        _packs: &LoadedContextPacks,
        _audit: &dyn AuditStore,
    ) -> NodeExecutionResult {
        todo!("See docs/spec/interfaces/nodes.md §ReviewNode")
    }
}
