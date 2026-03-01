//! Top-level error and retry-policy types for the CogWorks pipeline domain.
//!
//! [`CogWorksError`] covers conditions that halt or escalate the pipeline itself.
//! Component-level errors (e.g. [`crate::github`] failures, LLM call failures)
//! are defined in their respective modules.
//!
//! [`RetryPolicy`] is a cross-cutting concern: any error type that participates
//! in retry decisions must be able to produce a [`RetryPolicy`].
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/shared-types.md` §Error Types for the full contract.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ArtifactPath, CostBudget, TokenCost};

// ---------------------------------------------------------------------------
// Retry semantics
// ---------------------------------------------------------------------------

/// Whether an error condition is safe to retry and, if so, after what delay.
///
/// Returned by infrastructure error types to let the orchestrator decide
/// whether to re-invoke an operation without escalating.
///
/// ## Rules (from `docs/spec/constraints.md` §Error Handling)
///
/// - `Retryable` errors: API timeouts, transient rate-limit responses.
/// - `NonRetryable` errors: budget exceeded, invalid configuration, injection
///   detected, constitutional rules missing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetryPolicy {
    /// The operation may be retried.
    ///
    /// `after` optionally specifies the minimum delay before retrying (e.g.
    /// derived from `Retry-After` or `x-ratelimit-reset` response headers).
    Retryable {
        /// Minimum back-off before the next attempt. `None` means retry
        /// immediately or apply the caller's own back-off schedule.
        after: Option<Duration>,
    },
    /// The operation must not be retried; escalation or pipeline halt is required.
    NonRetryable,
}

// ---------------------------------------------------------------------------
// Pipeline-level errors
// ---------------------------------------------------------------------------

/// Errors that halt or escalate the pipeline itself.
///
/// These are distinct from per-component errors (GitHub API failure, LLM
/// provider failure) in that they represent conditions the pipeline cannot
/// recover from within its normal retry/rework budget.
///
/// ## Specification
///
/// See `docs/spec/interfaces/shared-types.md` §CogWorksError for the full
/// list of variants and when each is produced.
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum CogWorksError {
    /// The pipeline has been halted by an explicit decision (not a transient failure).
    ///
    /// Produced by: scope enforcer, injection guard, human-gate abort.
    #[error("Pipeline halted: {reason}")]
    PipelineHalt {
        /// Human-readable description of why the pipeline was halted.
        reason: String,
    },

    /// Accumulated token cost exceeded the configured budget before completion.
    ///
    /// Produced by: budget enforcement during parallel or sequential node execution.
    #[error("Cost budget exceeded: accumulated {accumulated}, limit {limit}")]
    BudgetExceeded {
        /// Total cost accumulated at the point of failure.
        accumulated: TokenCost,
        /// Configured budget that was exceeded.
        limit: CostBudget,
    },

    /// The constitutional rules check detected that external content contains
    /// text structured as a directive to CogWorks.
    ///
    /// The work item is placed in hold state; it is **not** automatically retried.
    #[error("Injection detected in '{source_document}': {offending_text}")]
    InjectionDetected {
        /// Label identifying the source document (e.g. `"issue body"`, file path).
        source_document: String,
        /// The specific text that triggered detection.
        offending_text: String,
    },

    /// The constitutional rules file could not be loaded or its content could
    /// not be validated.
    ///
    /// This error halts the pipeline unconditionally. There is no retry; a
    /// human must investigate and resolve the rules file before re-running.
    #[error("Constitutional rules could not be loaded or validated")]
    ConstitutionalRulesMissing,

    /// The pipeline attempted to write to or modify a protected path.
    ///
    /// Protected paths include: the constitutional rules file, prompt template
    /// directory, scenario specification directory, and Extension API schemas.
    #[error("Protected path violation: {path}")]
    ProtectedPathViolation {
        /// The artefact path that triggered the protection check.
        path: ArtifactPath,
    },

    /// The work item is outside the approved capability scope.
    ///
    /// Produced by: scope enforcer when a work item would require capabilities
    /// not present in the approved specification.
    #[error("Scope violation: {description}")]
    ScopeViolation {
        /// Description of the violated constraint.
        description: String,
    },

    /// The pipeline configuration or runtime configuration is invalid.
    ///
    /// Produced at load time; the pipeline never starts with an invalid config.
    #[error("Configuration error: {message}")]
    ConfigurationError {
        /// Description of the configuration problem.
        message: String,
    },
}
