//! Work-item classification logic and safety-critical module detection.
//!
//! This module provides two pure functions that operate on the
//! [`ClassificationResult`] produced by the Intake node:
//!
//! - [`apply_safety_override`] — cross-references affected modules against
//!   the [`SafetyCriticalRegistry`] and sets `safety_affecting = true` if any
//!   match. This ensures safety-critical changes are flagged even when the LLM
//!   classifier missed the significance.
//! - [`check_scope_threshold`] — validates that the work item's estimated
//!   scope does not exceed the configured single-work-item limit, preventing
//!   the pipeline from processing changes that are too large to review safely.
//!
//! ## Pure Business Logic
//!
//! Neither function performs I/O. Both are deterministic given the same inputs.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/pipeline-execution.md` §Classification for the
//! full contract, safety-override algorithm, and scope threshold rules.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::context::ClassificationResult;

// ─── Supporting types ────────────────────────────────────────────────────────

/// Registry of glob patterns that identify safety-critical modules.
///
/// Any [`ClassificationResult`] whose `affected_modules` contains a path
/// matching one or more of these patterns must have `safety_affecting` set to
/// `true`. Matching is performed by [`apply_safety_override`].
///
/// ## Pattern Syntax
///
/// Patterns follow the same glob syntax as `.gitignore` entries:
/// - `*` matches any sequence of characters within a single path component.
/// - `**` matches zero or more path components.
/// - `?` matches any single character.
///
/// ## Configuration Source
///
/// The registry is loaded from `.cogworks/safety-registry.toml` at pipeline
/// startup by the `PipelineConfigurationLoader` (see `knowledge` module). The
/// `cli` crate constructs this value from the loaded configuration.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §SafetyCriticalRegistry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SafetyCriticalRegistry {
    /// Glob patterns for module paths that are safety-critical.
    ///
    /// An empty registry means no modules are considered safety-critical by
    /// default. This is the correct default for projects without an explicit
    /// safety classification policy.
    pub patterns: Vec<String>,
}

impl SafetyCriticalRegistry {
    /// Creates an empty registry (no safety-critical modules).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a registry from the given list of glob patterns.
    ///
    /// Patterns are stored as-is; they are matched at call time by
    /// [`apply_safety_override`].
    pub fn from_patterns(patterns: Vec<String>) -> Self {
        Self { patterns }
    }
}

// ---------------------------------------------------------------------------

/// Error returned by [`check_scope_threshold`] when the estimated scope of a
/// work item exceeds the configured single-work-item limit.
///
/// When `check_scope_threshold` returns this error, the pipeline should
/// escalate with a message asking the author to split the work into smaller
/// pieces before requeueing with the `cogworks:run` label.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §EscalationResult.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[error(
    "Work item scope ({estimated_scope}) exceeds the configured threshold ({threshold}); \
     split into smaller work items before re-running."
)]
pub struct EscalationResult {
    /// The estimated scope magnitude of the work item (1 = trivial, 10 = very large).
    pub estimated_scope: u32,
    /// The configured maximum scope magnitude that a single work item may have.
    pub threshold: u32,
}

impl EscalationResult {
    /// Human-readable description of why the scope check failed.
    #[must_use]
    pub fn description(&self) -> String {
        format!(
            "Work item scope ({}) exceeds the configured threshold ({}); \
             split into smaller work items before re-running.",
            self.estimated_scope, self.threshold
        )
    }
}

// ─── Pure business logic functions ───────────────────────────────────────────

/// Checks whether any affected module in `result` is safety-critical and, if
/// so, sets `result.safety_affecting = true`.
///
/// The Intake node's LLM classifier sets `safety_affecting` based on the work
/// item description alone. This function provides a deterministic safety net
/// that checks the actual affected modules against the authoritative registry,
/// overriding a false-negative classification if needed.
///
/// ## Matching
///
/// For each `ArtifactPath` in `result.affected_modules`, the function tests
/// whether the path matches any glob pattern in `registry.patterns`. If any
/// match is found, `result.safety_affecting` is set to `true` regardless of
/// its current value (override is one-way: `false → true` only).
///
/// ## Return Value
///
/// Returns the (potentially modified) `ClassificationResult`. If no
/// safety-critical modules are affected, the result is returned unchanged.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §apply_safety_override`
#[must_use]
pub fn apply_safety_override(
    _result: ClassificationResult,
    _registry: &SafetyCriticalRegistry,
) -> ClassificationResult {
    todo!("See docs/spec/interfaces/pipeline-execution.md §apply_safety_override")
}

// ---------------------------------------------------------------------------

/// Validates that the work item's estimated scope is within the configured
/// single-work-item size limit.
///
/// CogWorks is designed to process focused, well-scoped work items. Items
/// that are too large cannot be reviewed safely and should be split by the
/// author before requeueing.
///
/// ## Decision Rule
///
/// - If `result.estimated_scope <= threshold`: return `Ok(result)` unchanged.
/// - If `result.estimated_scope > threshold`: return
///   `Err(EscalationResult { estimated_scope, threshold })`.
///
/// ## Threshold Source
///
/// The threshold is typically loaded from the active pipeline's
/// [`crate::knowledge::ToolProfile`] or from a project-level policy. The `cli`
/// crate is responsible for resolving and passing the correct value.
///
/// # Errors
///
/// Returns [`EscalationResult`] when the scope estimate exceeds `threshold`.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §check_scope_threshold`
pub fn check_scope_threshold(
    _result: ClassificationResult,
    _threshold: u32,
) -> Result<ClassificationResult, EscalationResult> {
    todo!("See docs/spec/interfaces/pipeline-execution.md §check_scope_threshold")
}
