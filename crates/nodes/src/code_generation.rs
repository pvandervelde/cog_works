//! Code Generation node — code generation with domain service feedback loop.
//!
//! The Code Generation node iterates over sub-work-items (one per execution),
//! generating code against the interface stubs from the Interface Design node. It:
//! 1. Enforces scenario holdout before assembling context.
//! 2. Budget-checks before each LLM call via `BudgetTracker::try_acquire`.
//! 3. Calls the LLM to generate code for the current sub-work-item.
//! 4. Validates generated code via the domain service.
//! 5. Writes file artifacts to output.
//!
//! ## Output Artifacts
//!
//! | Slot | Content |
//! |------|---------|
//! | `generated_files` | Map of repo-relative paths to file content (JSON) |
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/nodes.md` §CodeGenerationNode.

use pipeline::DomainServiceClient;

use crate::gateway::LlmGateway;
use crate::node::{BudgetTracker, ContextAssembler, NodeExecutionResult, NodeInput};

/// Code Generation node — generates code for one sub-work-item.
///
/// See `docs/spec/interfaces/nodes.md` §CodeGenerationNode.
pub struct CodeGenerationNode;

impl CodeGenerationNode {
    /// Executes the Code Generation node for `input`.
    ///
    /// # Arguments
    ///
    /// * `input` — Node inputs including `sub_work_items` and `interface_stubs`.
    /// * `gateway` — LLM gateway for code generation calls.
    /// * `context` — Context assembler (scenario holdout is enforced internally).
    /// * `domain_svc` — Domain service for build, test, and lint validation.
    /// * `budget` — Budget tracker; `try_acquire` is called before each LLM call.
    ///
    /// # Returns
    ///
    /// `NodeExecutionResult::Success` with `generated_files` artifact, or
    /// `NodeExecutionResult::Failure`.
    ///
    /// See `docs/spec/interfaces/nodes.md` §CodeGenerationNode.
    pub async fn execute(
        _input: NodeInput,
        _gateway: &LlmGateway,
        _context: &ContextAssembler,
        _domain_svc: &dyn DomainServiceClient,
        _budget: &BudgetTracker,
    ) -> NodeExecutionResult {
        todo!("See docs/spec/interfaces/nodes.md §CodeGenerationNode")
    }
}
