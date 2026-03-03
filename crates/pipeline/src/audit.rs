//! Audit store trait and audit event types for the CogWorks pipeline.
//!
//! Every LLM call, state transition, cost snapshot, edge evaluation, scope
//! check, and injection detection is recorded through the [`AuditStore`] trait.
//! The audit log is the primary post-hoc review tool for understanding why a
//! pipeline run behaved the way it did.
//!
//! ## Audit Guarantee
//!
//! The `github` infrastructure implementation writes audit events as
//! Markdown-formatted comments on the work-item issue. This means the full
//! audit trail is visible to reviewers in GitHub without requiring access to
//! internal systems.
//!
//! ## Architectural Layer
//!
//! Infrastructure crates (specifically `github`) implement [`AuditStore`];
//! the `pipeline` crate only emits [`AuditEvent`] values.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/github-traits.md` §AuditStore for the full
//! contract, retention policy, and formatting requirements.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    graph::{EdgeEvaluationRecord, NodeStatus},
    ArtifactPath, NodeId, PipelineRunId, TokenCost, TokenCount, WorkItemId,
};

// ─── Supporting types for AuditEvent variants ───────────────────────────────

/// Record of a single LLM API call made during pipeline execution.
///
/// One record is emitted per call; parallel node execution can produce
/// multiple records with the same `node_id` and `run_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCallRecord {
    /// Node that triggered this LLM call.
    pub node_id: NodeId,
    /// Model identifier string (e.g. `"claude-3-5-sonnet-20241022"`).
    pub model_id: String,
    /// Number of tokens in the prompt (input).
    pub prompt_tokens: TokenCount,
    /// Number of tokens in the completion (output).
    pub completion_tokens: TokenCount,
    /// Monetary cost of this call.
    pub cost: TokenCost,
    /// Wall-clock latency of the API call.
    pub latency: Duration,
    /// Whether the completion was validated against the output schema.
    pub schema_validated: bool,
    /// When the call was made (UTC).
    pub timestamp: DateTime<Utc>,
}

/// Record of a domain service or schema validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRecord {
    /// Node that triggered the validation.
    pub node_id: NodeId,
    /// The kind of validation performed (e.g. `"build"`, `"test"`, `"lint"`).
    pub validation_kind: String,
    /// Whether the validation passed.
    pub passed: bool,
    /// Structured diagnostic messages produced by the validation.
    ///
    /// Each entry is a human-readable string; structured `Diagnostic` types
    /// are defined in `crate::types` and serialised here as strings for
    /// portability across pipeline versions.
    pub diagnostics: Vec<String>,
    /// When the validation was performed (UTC).
    pub timestamp: DateTime<Utc>,
}

/// Record of a pipeline node state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransitionRecord {
    /// The node whose state changed.
    pub node_id: NodeId,
    /// The previous state of the node.
    pub from_status: NodeStatus,
    /// The new state of the node.
    pub to_status: NodeStatus,
    /// Human-readable reason for the transition (e.g. error message on failure).
    pub reason: Option<String>,
    /// When the transition occurred (UTC).
    pub timestamp: DateTime<Utc>,
}

/// Snapshot of accumulated cost at a point in time.
///
/// Emitted at each node boundary to provide cost visibility without requiring
/// the reviewer to sum all [`LlmCallRecord`]s manually.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSnapshot {
    /// Node at whose boundary this snapshot was taken.
    pub node_id: NodeId,
    /// Total accumulated cost of the pipeline run so far.
    pub accumulated: TokenCost,
    /// Configured cost budget for the run.
    pub budget: TokenCost,
    /// Whether the budget has been exceeded at this point.
    pub budget_exceeded: bool,
    /// When the snapshot was taken (UTC).
    pub timestamp: DateTime<Utc>,
}

/// Record of a detected prompt injection attempt.
///
/// Emitted whenever [`crate::security`] (PR 5) detects an injection pattern
/// in external content. Always treated as a non-retryable pipeline halt trigger.
///
/// Note: the `InjectionPattern` type will be refined in PR 5 (`security.rs`).
/// Until then, the pattern is recorded as a descriptive string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionDetectionRecord {
    /// The node during whose execution the injection was detected.
    pub node_id: NodeId,
    /// Label identifying the content source (e.g. `"issue_body"`, `"file:README.md"`).
    pub source_label: String,
    /// The text excerpt that triggered the detection.
    pub offending_text: String,
    /// Name of the injection pattern matched
    /// (e.g. `"PersonaOverride"`, `"InstructionInjection"`).
    pub pattern: String,
    /// When the detection occurred (UTC).
    pub timestamp: DateTime<Utc>,
}

/// Record of a scope violation detected during artefact or tool validation.
///
/// Note: the `ScopeViolation` type will be defined in full in PR 5
/// (`security.rs`). Until then, violations are recorded with a descriptive
/// string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeViolationRecord {
    /// The node during whose execution the violation was detected.
    pub node_id: NodeId,
    /// Repository-relative path of the artefact that violated scope.
    pub artifact_path: ArtifactPath,
    /// Human-readable description of the scope violation.
    pub description: String,
    /// The kind of violation (e.g. `"ProtectedPathViolation"`, `"UnauthorizedCapability"`).
    pub violation_kind: String,
    /// When the violation was detected (UTC).
    pub timestamp: DateTime<Utc>,
}

// ─── Audit event enum ────────────────────────────────────────────────────────

/// All observable events emitted by the pipeline for audit purposes.
///
/// Every variant is serialised and persisted by the [`AuditStore`]
/// implementation. The `github` infrastructure crate formats these as
/// Markdown comment blocks on the work-item issue.
///
/// ## Ordering
///
/// Events within a single pipeline run are timestamped monotonically (UTC).
/// The audit store must preserve insertion order when writing to GitHub.
///
/// ## Specification
///
/// See `docs/spec/interfaces/github-traits.md` §AuditEvent for per-variant
/// retention and formatting rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuditEvent {
    /// An LLM API call was made.
    LlmCall(LlmCallRecord),

    /// A validation step completed (domain service, build, test, or schema).
    Validation(ValidationRecord),

    /// A pipeline node changed state.
    StateTransition(StateTransitionRecord),

    /// A cost snapshot was captured at a node boundary.
    CostSnapshot(CostSnapshot),

    /// An edge condition was evaluated (deterministic or LLM-evaluated).
    EdgeEvaluation(EdgeEvaluationRecord),

    /// A prompt injection attempt was detected in external content.
    InjectionDetected(InjectionDetectionRecord),

    /// A scope violation was detected during artefact or tool validation.
    ScopeViolation(ScopeViolationRecord),
}

// ─── Pipeline summary ────────────────────────────────────────────────────────

/// Overall outcome of a completed (or halted) pipeline run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineOutcome {
    /// Every node completed successfully and the PR was opened.
    Completed,
    /// The pipeline was halted due to an unrecoverable error.
    Failed,
    /// The pipeline reached a human-gated node and is waiting for approval.
    HumanGated,
    /// The pipeline was escalated due to budget, rework-limit, or scope issues.
    Escalated,
}

/// Summary of a pipeline run written to the work-item issue at completion.
///
/// Written by the [`AuditStore`] implementation as a collapsible Markdown
/// section at the end of the run.
///
/// ## Specification
///
/// See `docs/spec/interfaces/github-traits.md` §PipelineSummary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSummary {
    /// The unique identifier for this pipeline run.
    pub run_id: PipelineRunId,
    /// The work item that triggered this run.
    pub work_item_id: WorkItemId,
    /// How the run ended.
    pub outcome: PipelineOutcome,
    /// Total accumulated LLM cost for the run.
    pub total_cost: TokenCost,
    /// Wall-clock duration of the run from first event to final state write.
    pub duration: Duration,
    /// Number of nodes that completed successfully.
    pub nodes_completed: u32,
    /// Number of nodes that failed (including retried nodes on their final attempt).
    pub nodes_failed: u32,
    /// Total number of rework iterations across all rework edges.
    pub total_rework_count: u32,
    /// Human-readable description of the terminal condition (error message or
    /// PR URL on success).
    pub terminal_message: String,
    /// When the pipeline run ended (UTC).
    pub completed_at: DateTime<Utc>,
}

// ─── Error type ─────────────────────────────────────────────────────────────

/// Errors returned by [`AuditStore`] operations.
#[derive(Debug, Error)]
pub enum AuditStoreError {
    /// The audit backend (e.g. GitHub API) is temporarily unavailable.
    ///
    /// Audit failures are non-fatal: the pipeline should log the error and
    /// continue. The event may be queued for retry.
    #[error("audit store unavailable: {message}")]
    Unavailable {
        /// Human-readable description of the availability failure.
        message: String,
    },

    /// An audit event could not be serialised for storage.
    #[error("audit event serialisation failed: {message}")]
    SerialisationError {
        /// Human-readable description of the serialisation failure.
        message: String,
    },
}

// ─── Trait ──────────────────────────────────────────────────────────────────

/// Persistent audit record storage.
///
/// All pipeline activity is emitted through this trait for post-hoc review.
/// Failures must not halt the pipeline — audit errors are logged at `WARN`
/// level and the pipeline continues.
///
/// ## Writing to GitHub
///
/// The `github` infrastructure implementation writes audit events as
/// Markdown-formatted comments on the work-item issue. The format is governed
/// by the template rules in `docs/spec/interfaces/github-traits.md`.
///
/// ## Specification
///
/// See `docs/spec/interfaces/github-traits.md` §AuditStore.
#[async_trait]
pub trait AuditStore: Send + Sync {
    /// Record a single audit event for the given pipeline run.
    ///
    /// The event is appended to the audit log in insertion order. Callers must
    /// not assume the event is visible in GitHub immediately after this call
    /// returns; batching is permitted by implementations.
    ///
    /// # Arguments
    ///
    /// * `run_id` — identifies the pipeline run this event belongs to.
    /// * `work_item_id` — the work item the run is acting on.
    /// * `event` — the event to record.
    ///
    /// # Errors
    ///
    /// - [`AuditStoreError::Unavailable`] — backend temporarily unreachable.
    /// - [`AuditStoreError::SerialisationError`] — event could not be serialised.
    ///
    /// Callers should log the error at `WARN` but must not propagate it up
    /// as a pipeline failure.
    async fn record_event(
        &self,
        run_id: PipelineRunId,
        work_item_id: WorkItemId,
        event: AuditEvent,
    ) -> Result<(), AuditStoreError>;

    /// Write a pipeline run summary to the work-item issue.
    ///
    /// Called once at the end of each pipeline step. The summary is formatted
    /// as a Markdown collapsible section appended to the issue.
    ///
    /// # Errors
    ///
    /// - [`AuditStoreError::Unavailable`] — backend temporarily unreachable.
    async fn write_summary(&self, summary: &PipelineSummary) -> Result<(), AuditStoreError>;
}
