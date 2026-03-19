//! Review aggregation logic for the multi-pass review gate.
//!
//! The CogWorks review gate runs three independent review passes — Quality,
//! Architecture, and Security — and then aggregates their findings via
//! [`aggregate_review_results`] to determine whether the pipeline may proceed,
//! must remediate, or must escalate.
//!
//! ## Decision Table
//!
//! | Quality | Architecture | Security | Remediation count vs. limit | Decision |
//! |---------|-------------|----------|----------------------------|----------|
//! | No blocking findings | No blocking findings | No blocking findings | Any | `Proceed` |
//! | Any blocking findings | Any | Any | < limit | `Remediate` |
//! | Any | Any blocking findings | Any | < limit | `Remediate` |
//! | Any | Any | Any blocking findings | < limit | `Remediate` |
//! | Any pass has blocking findings | Any | Any | ≥ limit | `Escalate` |
//!
//! The `limit` is typically the `max_rework_count` from the active
//! [`crate::ReworkEdge`] connecting the Review node back to the Code Generation
//! node.
//!
//! ## Pure Business Logic
//!
//! No I/O. [`aggregate_review_results`] is deterministic for identical inputs.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/pipeline-execution.md` §Review Aggregation for
//! the full contract and examples.

use serde::{Deserialize, Serialize};

use crate::{execution::EscalationReason, identifiers::ArtifactPath, types::DiagnosticSeverity};

// ─── Review types ─────────────────────────────────────────────────────────────

/// One of the three independent review passes in the CogWorks review gate.
///
/// Each pass focuses on a distinct quality dimension:
/// - [`Quality`]: correctness, test coverage, documentation completeness,
///   code style, and performance considerations.
/// - [`Architecture`]: adherence to architectural decisions (ADRs), clean
///   architecture boundaries, responsibility assignments (RDD), and
///   interface conformance.
/// - [`Security`]: OWASP Top-10 coverage, secrets handling, scope enforcement,
///   injection risks, and protected-path compliance.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §ReviewPass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReviewPass {
    /// Correctness, test coverage, documentation, style, and performance.
    Quality,
    /// ADR conformance, architecture boundaries, RDD responsibilities,
    /// interface contract compliance.
    Architecture,
    /// OWASP Top-10, secrets handling, scope enforcement, injection detection,
    /// protected-path compliance.
    Security,
}

impl std::fmt::Display for ReviewPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quality => write!(f, "Quality"),
            Self::Architecture => write!(f, "Architecture"),
            Self::Security => write!(f, "Security"),
        }
    }
}

// ---------------------------------------------------------------------------

/// A single finding emitted by a review pass.
///
/// Findings are collected from all three passes and aggregated by
/// [`aggregate_review_results`] to determine the overall review decision.
///
/// ## Severity
///
/// - [`DiagnosticSeverity::Blocking`]: the code generation node must remediate
///   this before the review will pass. Any blocking finding in any pass causes
///   a `Remediate` or `Escalate` decision.
/// - [`DiagnosticSeverity::Warning`]: should be addressed but does not block
///   progression on its own.
/// - [`DiagnosticSeverity::Informational`]: context only; never blocks
///   progression.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §ReviewFinding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    /// The review pass that produced this finding.
    pub pass: ReviewPass,

    /// Severity of this finding.
    pub severity: DiagnosticSeverity,

    /// Human-readable description of the problem and how to address it.
    ///
    /// Written in terms of the artifact content (not the LLM's internal state).
    /// Passed back to the Code Generation node as rework guidance.
    pub description: String,

    /// Repo-relative path of the artifact this finding relates to.
    ///
    /// `None` for findings that are not file-specific (e.g. a missing test
    /// file, or an overall architectural concern).
    pub location: Option<ArtifactPath>,
}

// ---------------------------------------------------------------------------

/// The complete output of one review pass.
///
/// Produced by the Review node in the `nodes` crate; consumed by
/// [`aggregate_review_results`].
///
/// See `docs/spec/interfaces/pipeline-execution.md` §ReviewResult.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// Which pass produced these findings.
    pub pass: ReviewPass,

    /// All findings from this pass, ordered by severity (blocking first).
    ///
    /// An empty `findings` vec means the pass found no issues.
    pub findings: Vec<ReviewFinding>,
}

impl ReviewResult {
    /// Returns `true` if this pass has at least one [`DiagnosticSeverity::Blocking`]
    /// finding.
    #[must_use]
    pub fn has_blocking(&self) -> bool {
        self.findings
            .iter()
            .any(|f| f.severity == DiagnosticSeverity::Blocking)
    }

    /// Returns an iterator over [`DiagnosticSeverity::Blocking`] findings.
    pub fn blocking_findings(&self) -> impl Iterator<Item = &ReviewFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity == DiagnosticSeverity::Blocking)
    }
}

// ─── Aggregated decision type ─────────────────────────────────────────────────

/// The overall review gate decision after aggregating all three passes.
///
/// Returned by [`aggregate_review_results`] and acted on by the execution
/// engine:
///
/// - `Proceed` → activate the outgoing forward edge to the Integration node.
/// - `Remediate` → activate the rework edge back to Code Generation, carrying
///   the blocking findings as guidance.
/// - `Escalate` → the rework budget is exhausted; write the escalation report
///   to GitHub and halt.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §AggregateReviewDecision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregateReviewDecision {
    /// All three passes found no blocking issues; the pipeline may continue.
    Proceed,

    /// At least one blocking finding exists and the rework budget allows
    /// another cycle.
    ///
    /// The attached findings are all blocking items from all passes, deduplicated
    /// and ordered by pass (Quality → Architecture → Security). These are passed
    /// back to the Code Generation node as the rework guidance payload.
    Remediate(Vec<ReviewFinding>),

    /// The rework budget is exhausted (`remediation_count >= limit`); escalate
    /// to a human reviewer with all remaining findings.
    Escalate(EscalationReason),
}

// ─── Pure business logic function ────────────────────────────────────────────

/// Aggregates findings from the three review passes into a single decision.
///
/// ## Decision Algorithm
///
/// 1. Collect all [`DiagnosticSeverity::Blocking`] findings from all three
///    passes into a single list.
/// 2. If the list is **empty** → return [`AggregateReviewDecision::Proceed`].
/// 3. If `remediation_count >= limit` → return `Escalate` with all blocking
///    findings described in the [`EscalationReason`].
/// 4. Otherwise → return `Remediate` with all blocking findings.
///
/// Non-blocking (warning and informational) findings are not returned in
/// `Remediate`; they are included in the audit trail by the Review node but
/// do not influence the gate decision.
///
/// ## Parameters
///
/// - `quality` — Result of the Quality review pass.
/// - `architecture` — Result of the Architecture review pass.
/// - `security` — Result of the Security review pass.
/// - `remediation_count` — Number of rework cycles already applied to the
///   Code Generation node in this run. Corresponds to
///   [`crate::NodeState::rework_count`] for the Code Generation node.
/// - `limit` — Maximum rework cycles permitted before escalation. Typically
///   derived from [`crate::ReworkEdge::max_traversals`] on the rework edge
///   connecting Review back to Code Generation.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §aggregate_review_results`
#[must_use]
pub fn aggregate_review_results(
    _quality: ReviewResult,
    _architecture: ReviewResult,
    _security: ReviewResult,
    _remediation_count: u32,
    _limit: u32,
) -> AggregateReviewDecision {
    todo!("See docs/spec/interfaces/pipeline-execution.md §aggregate_review_results")
}
