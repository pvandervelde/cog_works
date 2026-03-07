//! Core orchestration domain for CogWorks.
//!
//! This crate contains every domain concept, newtype identifier, shared primitive
//! type, and cross-cutting error type used throughout the pipeline. Infrastructure
//! crates implement the traits defined here; they never add domain rules.
//!
//! ## Architectural Layer
//!
//! **Business logic + trait definitions.** This crate has no I/O dependencies.
//! It defines *what* is needed; infrastructure crates define *how* to supply it.
//!
//! ## Module Layout
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`identifiers`] | Newtype domain identifiers (`WorkItemId`, `NodeId`, etc.) |
//! | [`types`] | Shared value types (`TokenCount`, `CostBudget`, `Diagnostic`, etc.) |
//! | [`errors`] | Top-level error and retry-policy types |
//! | [`graph`] | Pipeline graph model and runtime state types |
//! | [`github`] | GitHub traits: `EventSource`, `IssueTracker`, `PullRequestManager`, `CodeRepository`, `ProjectBoard` and their data types |
//! | [`templates`] | `TemplateEngine` trait |
//! | [`audit`] | `AuditStore` trait, `AuditEvent` enum, `PipelineSummary` |
//!
//! ## Specification
//!
//! See [`docs/spec/interfaces/shared-types.md`] for shared types.
//! See [`docs/spec/interfaces/pipeline-graph.md`] for graph model types.
//! See [`docs/spec/interfaces/github-traits.md`] for GitHub trait contracts.

pub mod audit;
pub mod errors;
pub mod github;
pub mod graph;
pub mod identifiers;
pub mod templates;
pub mod types;

// Re-export everything at the crate root for ergonomic usage by downstream crates.
pub use audit::{
    AuditEvent, AuditStore, AuditStoreError, CostSnapshot, InjectionDetectionRecord, LlmCallRecord,
    PipelineOutcome, PipelineSummary, ScopeViolationRecord, StateTransitionRecord,
    ValidationRecord,
};
pub use errors::{CogWorksError, RetryPolicy};
pub use github::{
    CodeRepository, DirectoryEntry, DirectoryEntryKind, EventSource, EventSourceError, FileContent,
    GitHubEvent, GitHubOperationError, Issue, IssueState, IssueTracker, Label, Milestone,
    ProjectBoard, PullRequest, PullRequestFilter, PullRequestManager, QueueEventConfig,
    ReviewDecision, ReviewStatus, SubIssue, TypedLink, TypedLinkKind, WebhookConfig,
};
pub use graph::{
    compute_eligible_nodes, evaluate_deterministic_condition, topological_sort,
    validate_pipeline_graph, CompositeCondition, CycleError, EdgeConditionKind, EdgeDefinition,
    EdgeEvaluationRecord, EvaluationMode, EvaluatorKind, Expression, GraphValidationError,
    NaturalLanguageCondition, NodeDefinition, NodeGate, NodeState, NodeStatus, NodeType,
    OverflowBehaviour, PipelineConfiguration, PipelineGraph, PipelineSettings, PipelineState,
    PipelineStateComment, PipelineToolProfileConfig, ReworkEdge, ReworkSemantics, SchemaVersion,
    TimeoutSeconds, ValidationKind,
};
pub use identifiers::{
    ArtifactPath, BlobSha, BranchName, CommitSha, ContextPackId, DomainServiceName, EdgeId,
    InterfaceId, MilestoneId, NodeId, PipelineName, PipelineRunId, ProfileName, PullRequestId,
    RepositoryId, SkillName, SubWorkItemId, ToolName, WorkItemId,
};
pub use templates::{TemplateEngine, TemplateError};
pub use types::{
    AlignmentScore, ApiVersion, CostBudget, Diagnostic, DiagnosticCategory, DiagnosticSeverity,
    SatisfactionScore, Timestamp, TokenCost, TokenCount,
};
