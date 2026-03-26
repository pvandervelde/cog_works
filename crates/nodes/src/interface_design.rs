//! Interface Design node — interface stub generation and domain service validation.
//!
//! The Interface Design node takes the architecture document and produces typed
//! interface stubs (type definitions, trait signatures, function stubs). It:
//! 1. Assembles context including the current interface definitions.
//! 2. Calls the LLM to generate interface stubs.
//! 3. Validates stubs via the domain service (`DomainServiceClient::validate`).
//! 4. Runs alignment; updates the traceability matrix (LLM-semantic path only).
//!
//! ## Output Artifacts
//!
//! | Slot | Content |
//! |------|---------|
//! | `interface_stubs` | Map of file paths to stub content (JSON) |
//! | `traceability_matrix` | Updated [`pipeline::TraceabilityMatrix`] (JSON) |
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/nodes.md` §InterfaceDesignNode.

use pipeline::{CodeRepository, DomainServiceClient};

use crate::gateway::LlmGateway;
use crate::node::{ContextAssembler, NodeExecutionResult, NodeInput};

/// Interface Design node — generates typed interface stubs.
///
/// See `docs/spec/interfaces/nodes.md` §InterfaceDesignNode.
pub struct InterfaceDesignNode;

impl InterfaceDesignNode {
    /// Executes the Interface Design node for `input`.
    ///
    /// # Arguments
    ///
    /// * `input` — Node inputs including the `architecture_doc` artifact.
    /// * `gateway` — LLM gateway for stub generation and semantic alignment.
    /// * `context` — Context assembler for building the node's context package.
    /// * `domain_svc` — Domain service for validating generated interface stubs.
    /// * `github` — Repository access for reading existing interface definitions.
    ///
    /// # Returns
    ///
    /// `NodeExecutionResult::Success` with `interface_stubs` and updated
    /// `traceability_matrix` artifacts, or `NodeExecutionResult::Failure`.
    ///
    /// See `docs/spec/interfaces/nodes.md` §InterfaceDesignNode.
    pub async fn execute(
        _input: NodeInput,
        _gateway: &LlmGateway,
        _context: &ContextAssembler,
        _domain_svc: &dyn DomainServiceClient,
        _github: &dyn CodeRepository,
    ) -> NodeExecutionResult {
        todo!("See docs/spec/interfaces/nodes.md §InterfaceDesignNode")
    }
}
