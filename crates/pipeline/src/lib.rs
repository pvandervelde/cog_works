//! Core orchestration domain for CogWorks.
//!
//! This crate contains every domain concept, newtype identifier, shared primitive
//! type, and cross-cutting error type used throughout the pipeline. Infrastructure
//! crates implement the traits defined here; they never add domain rules.
//!
//! ## Architectural Layer
//!
//! **Business logic + port definitions.** This crate has no I/O dependencies.
//! It defines *what* is needed; infrastructure crates define *how* to supply it.
//!
//! ## Module Layout
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`identifiers`] | Newtype domain identifiers (`WorkItemId`, `NodeId`, etc.) |
//! | [`types`] | Shared value types (`TokenCount`, `CostBudget`, `Diagnostic`, etc.) |
//! | [`errors`] | Top-level error and retry-policy types |
//!
//! ## Specification
//!
//! See [`docs/spec/interfaces/shared-types.md`] for the full contract.

pub mod errors;
pub mod identifiers;
pub mod types;

// Re-export everything at the crate root for ergonomic usage by downstream crates.
pub use errors::{CogWorksError, RetryPolicy};
pub use identifiers::{
    ArtifactPath, BranchName, CommitSha, ContextPackId, DomainServiceName, EdgeId, InterfaceId,
    MilestoneId, NodeId, PipelineName, PipelineRunId, ProfileName, PullRequestId, RepositoryId,
    SkillName, SubWorkItemId, ToolName, WorkItemId,
};
pub use types::{
    AlignmentScore, ApiVersion, CostBudget, Diagnostic, DiagnosticCategory, DiagnosticSeverity,
    SatisfactionScore, Timestamp, TokenCost, TokenCount,
};
