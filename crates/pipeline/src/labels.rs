//! Pipeline label constants and parse/generate utilities.
//!
//! GitHub labels are the primary signalling mechanism between the pipeline and
//! the external world: operators trigger runs with `cogworks:run`, the pipeline
//! signals status with `cogworks:status:*` labels, and human reviewers respond
//! to gate labels.
//!
//! This module provides a typed [`PipelineLabel`] enum and two pure conversion
//! functions:
//!
//! - [`parse_label`] — parses a raw GitHub label string into a `PipelineLabel`,
//!   returning `None` for labels that are not CogWorks pipeline labels.
//! - [`generate_label`] — serialises a `PipelineLabel` back to its canonical
//!   GitHub label string. Round-trip guarantee:
//!   `parse_label(generate_label(l)) == Some(l)`.
//!
//! ## Usage
//!
//! The pipeline's event loop receives [`crate::GitHubEvent::LabelApplied`] events
//! where the `label` field is a raw GitHub string. Call [`parse_label`] to
//! convert it to a typed value before pattern-matching on pipeline behaviour.
//!
//! ## Architectural Layer
//!
//! **Business logic (pure functions).** No I/O.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/context.md` §Pipeline Labels.

use serde::{Deserialize, Serialize};

// ─── Pipeline label enum ──────────────────────────────────────────────────────

/// A typed representation of a GitHub label that has significance for the
/// CogWorks pipeline.
///
/// The canonical string for each variant is defined by [`generate_label`].
///
/// See `docs/spec/interfaces/context.md` §PipelineLabel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineLabel {
    // ── Trigger labels ────────────────────────────────────────────────────
    /// `"cogworks:run"` — starts the pipeline for the labelled issue.
    Run,

    /// `"cogworks:restart"` — restarts the pipeline from the last checkpoint.
    Restart,

    // ── Status labels ─────────────────────────────────────────────────────
    /// `"cogworks:status:running"` — pipeline is currently executing.
    Running,

    /// `"cogworks:status:done"` — pipeline completed successfully.
    Done,

    /// `"cogworks:status:failed"` — pipeline failed and requires attention.
    Failed,

    /// `"cogworks:status:escalated"` — pipeline escalated to a human because
    /// retry/rework budgets were exhausted or a non-retryable error occurred.
    Escalated,

    // ── Gate labels ───────────────────────────────────────────────────────
    /// `"cogworks:gate:pending"` — a human-gated node is awaiting approval.
    HumanGatePending,

    /// `"cogworks:gate:approved"` — a human reviewer approved a gated node.
    HumanGateApproved,

    /// `"cogworks:gate:rejected"` — a human reviewer rejected a gated node.
    HumanGateRejected,

    // ── Processing lock ───────────────────────────────────────────────────
    /// `"cogworks:lock"` — mutual-exclusion lock applied while the pipeline
    /// holds the issue. Prevents concurrent pipeline runs on the same issue.
    Lock,

    // ── Security and safety ───────────────────────────────────────────────
    /// `"cogworks:security-hold"` — injection or scope violation detected;
    /// the pipeline has halted and the issue requires human investigation.
    SecurityHold,

    /// `"cogworks:safety-critical"` — the issue touches safety-critical
    /// modules. Human approval is required before any PR can be merged.
    SafetyCritical,
}

// ─── Conversion functions ─────────────────────────────────────────────────────

/// Parses a raw GitHub label string into a [`PipelineLabel`].
///
/// Returns `None` for any label string that is not a known CogWorks pipeline
/// label. Non-CogWorks labels (e.g. `"bug"`, `"enhancement"`) are silently
/// ignored at the call site.
///
/// # Round-trip guarantee
///
/// `parse_label(&generate_label(label))` always returns `Some(label)`.
///
/// **Infallible. Pure.**
///
/// See `docs/spec/interfaces/context.md` §parse_label.
pub fn parse_label(s: &str) -> Option<PipelineLabel> {
    match s {
        "cogworks:run" => Some(PipelineLabel::Run),
        "cogworks:restart" => Some(PipelineLabel::Restart),
        "cogworks:status:running" => Some(PipelineLabel::Running),
        "cogworks:status:done" => Some(PipelineLabel::Done),
        "cogworks:status:failed" => Some(PipelineLabel::Failed),
        "cogworks:status:escalated" => Some(PipelineLabel::Escalated),
        "cogworks:gate:pending" => Some(PipelineLabel::HumanGatePending),
        "cogworks:gate:approved" => Some(PipelineLabel::HumanGateApproved),
        "cogworks:gate:rejected" => Some(PipelineLabel::HumanGateRejected),
        "cogworks:lock" => Some(PipelineLabel::Lock),
        "cogworks:security-hold" => Some(PipelineLabel::SecurityHold),
        "cogworks:safety-critical" => Some(PipelineLabel::SafetyCritical),
        _ => None,
    }
}

// ---------------------------------------------------------------------------

/// Returns the canonical GitHub label string for a [`PipelineLabel`].
///
/// The returned `&'static str` requires no allocation. CogWorks assumes these
/// labels exist on the GitHub repository (case-sensitive); it does not create
/// them automatically.
///
/// # Round-trip guarantee
///
/// `parse_label(generate_label(label))` always returns `Some(label)`.
///
/// **Infallible. Pure.**
///
/// See `docs/spec/interfaces/context.md` §generate_label.
pub fn generate_label(label: &PipelineLabel) -> &'static str {
    match label {
        PipelineLabel::Run => "cogworks:run",
        PipelineLabel::Restart => "cogworks:restart",
        PipelineLabel::Running => "cogworks:status:running",
        PipelineLabel::Done => "cogworks:status:done",
        PipelineLabel::Failed => "cogworks:status:failed",
        PipelineLabel::Escalated => "cogworks:status:escalated",
        PipelineLabel::HumanGatePending => "cogworks:gate:pending",
        PipelineLabel::HumanGateApproved => "cogworks:gate:approved",
        PipelineLabel::HumanGateRejected => "cogworks:gate:rejected",
        PipelineLabel::Lock => "cogworks:lock",
        PipelineLabel::SecurityHold => "cogworks:security-hold",
        PipelineLabel::SafetyCritical => "cogworks:safety-critical",
    }
}
#[cfg(test)]
#[path = "labels_tests.rs"]
mod tests;
