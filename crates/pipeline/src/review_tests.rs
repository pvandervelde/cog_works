//! Adversarial test suite for `review.rs` — `aggregate_review_results` and the
//! `ReviewResult` contract it depends on.
//!
//! ## Phase: RED
//!
//! `aggregate_review_results` is a `todo!()` stub. Every test that calls it is
//! expected to **compile** cleanly but **panic** at runtime until the
//! implementation lands. `ReviewResult::has_blocking` / `blocking_findings` are
//! already implemented (not stubs) — the "Contract tests" section exercises
//! those directly and is expected to pass immediately.
//!
//! ## Assertions covered
//!
//! - ASSERT-REVIEW-001: a single `Blocking` finding anywhere prevents `Proceed`.
//! - ASSERT-REVIEW-002: only `Warning`/`Informational` findings → `Proceed`.
//! - ASSERT-REVIEW-004: blocking findings are fed back via `Remediate(Vec<ReviewFinding>)`.
//! - ASSERT-REVIEW-005: `remediation_count >= limit` with a blocking finding → `Escalate`.
//!
//! ## Spec gap
//!
//! `EscalationReason` (see `crates/pipeline/src/execution.rs`) has `node_id`,
//! `attempt_count`, `rework_count`, and `cost_spent` fields that `aggregate_review_results`
//! has no parameters to source. Tests in this file only assert on the
//! `description` field (which the spec text does define: "listing all blocking
//! findings"); the other fields are left unconstrained pending an architect
//! decision on where their values originate.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;

use super::{AggregateReviewDecision, ReviewFinding, ReviewPass, ReviewResult, aggregate_review_results};
use crate::types::DiagnosticSeverity;

// ─── Test helpers ────────────────────────────────────────────────────────────

fn finding(pass: ReviewPass, severity: DiagnosticSeverity, description: &str) -> ReviewFinding {
    ReviewFinding {
        pass,
        severity,
        description: description.to_string(),
        location: None,
    }
}

fn blocking(pass: ReviewPass, description: &str) -> ReviewFinding {
    finding(pass, DiagnosticSeverity::Blocking, description)
}

fn warning(pass: ReviewPass, description: &str) -> ReviewFinding {
    finding(pass, DiagnosticSeverity::Warning, description)
}

fn informational(pass: ReviewPass, description: &str) -> ReviewFinding {
    finding(pass, DiagnosticSeverity::Informational, description)
}

fn review_result(pass: ReviewPass, findings: Vec<ReviewFinding>) -> ReviewResult {
    ReviewResult { pass, findings }
}

fn empty_pass(pass: ReviewPass) -> ReviewResult {
    review_result(pass, vec![])
}

/// Maps a [`ReviewPass`] to its expected ordinal position in the aggregation
/// order (Quality → Architecture → Security), per the doc comment on
/// `AggregateReviewDecision::Remediate`.
fn pass_rank(pass: ReviewPass) -> u8 {
    match pass {
        ReviewPass::Quality => 0,
        ReviewPass::Architecture => 1,
        ReviewPass::Security => 2,
    }
}

fn severity_strategy() -> impl Strategy<Value = DiagnosticSeverity> {
    prop_oneof![
        Just(DiagnosticSeverity::Blocking),
        Just(DiagnosticSeverity::Warning),
        Just(DiagnosticSeverity::Informational),
    ]
}

fn review_result_from_severities(pass: ReviewPass, severities: &[DiagnosticSeverity]) -> ReviewResult {
    let findings = severities
        .iter()
        .enumerate()
        .map(|(i, sev)| finding(pass, *sev, &format!("{pass}-{i}")))
        .collect();
    review_result(pass, findings)
}

// ═══════════════════════════════════════════════════════════════════════════
// Contract tests — ReviewResult::has_blocking / blocking_findings
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_review_result_has_blocking_true_when_blocking_present() {
    let result = review_result(
        ReviewPass::Quality,
        vec![warning(ReviewPass::Quality, "w"), blocking(ReviewPass::Quality, "b")],
    );
    assert!(result.has_blocking());
}

#[test]
fn test_review_result_has_blocking_false_when_only_warning_and_informational() {
    let result = review_result(
        ReviewPass::Quality,
        vec![
            warning(ReviewPass::Quality, "w"),
            informational(ReviewPass::Quality, "i"),
        ],
    );
    assert!(!result.has_blocking());
}

#[test]
fn test_review_result_has_blocking_false_when_findings_empty() {
    let result = empty_pass(ReviewPass::Security);
    assert!(!result.has_blocking());
}

#[test]
fn test_review_result_blocking_findings_filters_only_blocking_severity() {
    let result = review_result(
        ReviewPass::Architecture,
        vec![
            blocking(ReviewPass::Architecture, "b1"),
            warning(ReviewPass::Architecture, "w"),
            blocking(ReviewPass::Architecture, "b2"),
            informational(ReviewPass::Architecture, "i"),
        ],
    );
    let descriptions: Vec<&str> = result
        .blocking_findings()
        .map(|f| f.description.as_str())
        .collect();
    assert_eq!(descriptions, vec!["b1", "b2"]);
}

#[test]
fn test_review_result_blocking_findings_empty_iterator_when_no_blocking() {
    let result = review_result(ReviewPass::Security, vec![warning(ReviewPass::Security, "w")]);
    assert_eq!(result.blocking_findings().count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 1: Specification tests
// ═══════════════════════════════════════════════════════════════════════════

/// ASSERT-REVIEW-002: no blocking findings anywhere → Proceed.
#[test]
fn test_aggregate_review_results_all_empty_findings_returns_proceed() {
    let decision = aggregate_review_results(
        empty_pass(ReviewPass::Quality),
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        0,
        3,
    );
    assert!(matches!(decision, AggregateReviewDecision::Proceed));
}

/// ASSERT-REVIEW-002: only Warning/Informational findings across all passes → Proceed.
#[test]
fn test_aggregate_review_results_all_warning_and_informational_returns_proceed() {
    let quality = review_result(
        ReviewPass::Quality,
        vec![warning(ReviewPass::Quality, "q-warn")],
    );
    let architecture = review_result(
        ReviewPass::Architecture,
        vec![informational(ReviewPass::Architecture, "a-info")],
    );
    let security = review_result(
        ReviewPass::Security,
        vec![
            warning(ReviewPass::Security, "s-warn"),
            informational(ReviewPass::Security, "s-info"),
        ],
    );

    let decision = aggregate_review_results(quality, architecture, security, 0, 3);

    assert!(matches!(decision, AggregateReviewDecision::Proceed));
}

#[test]
fn test_aggregate_review_results_blocking_in_quality_only_returns_remediate() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q-block")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        0,
        3,
    );
    assert!(matches!(decision, AggregateReviewDecision::Remediate(_)));
}

#[test]
fn test_aggregate_review_results_blocking_in_architecture_only_returns_remediate() {
    let architecture = review_result(
        ReviewPass::Architecture,
        vec![blocking(ReviewPass::Architecture, "a-block")],
    );
    let decision = aggregate_review_results(
        empty_pass(ReviewPass::Quality),
        architecture,
        empty_pass(ReviewPass::Security),
        0,
        3,
    );
    assert!(matches!(decision, AggregateReviewDecision::Remediate(_)));
}

#[test]
fn test_aggregate_review_results_blocking_in_security_only_returns_remediate() {
    let security = review_result(ReviewPass::Security, vec![blocking(ReviewPass::Security, "s-block")]);
    let decision = aggregate_review_results(
        empty_pass(ReviewPass::Quality),
        empty_pass(ReviewPass::Architecture),
        security,
        0,
        3,
    );
    assert!(matches!(decision, AggregateReviewDecision::Remediate(_)));
}

/// Blocking findings from all three passes must all be combined into one Remediate vec.
#[test]
fn test_aggregate_review_results_blocking_in_all_three_passes_combines_into_remediate() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let architecture = review_result(
        ReviewPass::Architecture,
        vec![blocking(ReviewPass::Architecture, "a")],
    );
    let security = review_result(ReviewPass::Security, vec![blocking(ReviewPass::Security, "s")]);

    let decision = aggregate_review_results(quality, architecture, security, 0, 3);

    match decision {
        AggregateReviewDecision::Remediate(findings) => assert_eq!(findings.len(), 3),
        other => panic!("expected Remediate, got {other:?}"),
    }
}

/// Warning/Informational findings must never appear in the Remediate payload,
/// even when mixed into the same pass as a blocking finding.
#[test]
fn test_aggregate_review_results_remediate_excludes_non_blocking_findings() {
    let quality = review_result(
        ReviewPass::Quality,
        vec![
            warning(ReviewPass::Quality, "w"),
            blocking(ReviewPass::Quality, "b"),
            informational(ReviewPass::Quality, "i"),
        ],
    );

    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        0,
        3,
    );

    match decision {
        AggregateReviewDecision::Remediate(findings) => {
            assert_eq!(findings.len(), 1, "only the blocking finding may be present");
            assert_eq!(findings[0].description, "b");
        }
        other => panic!("expected Remediate, got {other:?}"),
    }
}

/// Remediate ordering: Quality findings first, then Architecture, then Security.
#[test]
fn test_aggregate_review_results_remediate_orders_quality_before_architecture_before_security() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let architecture = review_result(
        ReviewPass::Architecture,
        vec![blocking(ReviewPass::Architecture, "a")],
    );
    let security = review_result(ReviewPass::Security, vec![blocking(ReviewPass::Security, "s")]);

    let decision = aggregate_review_results(quality, architecture, security, 0, 3);

    match decision {
        AggregateReviewDecision::Remediate(findings) => {
            let ranks: Vec<u8> = findings.iter().map(|f| pass_rank(f.pass)).collect();
            assert_eq!(ranks, vec![0, 1, 2], "expected Quality, Architecture, Security order");
        }
        other => panic!("expected Remediate, got {other:?}"),
    }
}

/// Boundary N: `remediation_count == limit` with a blocking finding → Escalate.
#[test]
fn test_aggregate_review_results_remediation_count_at_limit_returns_escalate() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        3,
        3,
    );
    assert!(matches!(decision, AggregateReviewDecision::Escalate(_)));
}

/// Boundary N+1: `remediation_count > limit` with a blocking finding → Escalate.
#[test]
fn test_aggregate_review_results_remediation_count_above_limit_returns_escalate() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        4,
        3,
    );
    assert!(matches!(decision, AggregateReviewDecision::Escalate(_)));
}

/// Boundary N-1: `remediation_count < limit` with a blocking finding → Remediate.
#[test]
fn test_aggregate_review_results_remediation_count_below_limit_returns_remediate() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        2,
        3,
    );
    assert!(matches!(decision, AggregateReviewDecision::Remediate(_)));
}

/// The `description` inside `EscalationReason` must list all blocking findings
/// (per spec text: "description listing all blocking findings").
#[test]
fn test_aggregate_review_results_escalate_description_lists_all_blocking_findings() {
    let quality = review_result(
        ReviewPass::Quality,
        vec![blocking(ReviewPass::Quality, "UNIQUE_QUALITY_MARKER")],
    );
    let architecture = review_result(
        ReviewPass::Architecture,
        vec![blocking(ReviewPass::Architecture, "UNIQUE_ARCHITECTURE_MARKER")],
    );
    let security = review_result(
        ReviewPass::Security,
        vec![blocking(ReviewPass::Security, "UNIQUE_SECURITY_MARKER")],
    );

    let decision = aggregate_review_results(quality, architecture, security, 5, 5);

    match decision {
        AggregateReviewDecision::Escalate(reason) => {
            assert!(
                reason.description.contains("UNIQUE_QUALITY_MARKER"),
                "description must mention the Quality blocking finding: {}",
                reason.description
            );
            assert!(
                reason.description.contains("UNIQUE_ARCHITECTURE_MARKER"),
                "description must mention the Architecture blocking finding: {}",
                reason.description
            );
            assert!(
                reason.description.contains("UNIQUE_SECURITY_MARKER"),
                "description must mention the Security blocking finding: {}",
                reason.description
            );
        }
        other => panic!("expected Escalate, got {other:?}"),
    }
}

/// ASSERT-REVIEW-001: a single blocking finding dominates regardless of how many
/// non-blocking findings exist elsewhere.
#[test]
fn test_aggregate_review_results_blocking_dominates_regardless_of_non_blocking_volume() {
    let many_warnings: Vec<ReviewFinding> = (0..50)
        .map(|i| warning(ReviewPass::Quality, &format!("warn-{i}")))
        .collect();
    let quality = review_result(ReviewPass::Quality, many_warnings);
    let architecture = review_result(
        ReviewPass::Architecture,
        (0..50)
            .map(|i| informational(ReviewPass::Architecture, &format!("info-{i}")))
            .collect(),
    );
    let mut security_findings: Vec<ReviewFinding> = (0..50)
        .map(|i| warning(ReviewPass::Security, &format!("s-warn-{i}")))
        .collect();
    security_findings.push(blocking(ReviewPass::Security, "the-one-blocker"));
    let security = review_result(ReviewPass::Security, security_findings);

    let decision = aggregate_review_results(quality, architecture, security, 0, 3);

    assert!(
        !matches!(decision, AggregateReviewDecision::Proceed),
        "a single blocking finding among 150 non-blocking findings must prevent Proceed"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 2: Adversarial / boundary / stub-killing tests
// ═══════════════════════════════════════════════════════════════════════════

/// Boundary: `remediation_count == limit == 0` with a blocking finding → Escalate
/// (0 >= 0 is true; the rework budget was never available in the first place).
#[test]
fn test_aggregate_review_results_zero_remediation_count_zero_limit_with_blocking_returns_escalate() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        0,
        0,
    );
    assert!(matches!(decision, AggregateReviewDecision::Escalate(_)));
}

#[test]
fn test_aggregate_review_results_zero_remediation_count_positive_limit_with_blocking_returns_remediate() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        0,
        1,
    );
    assert!(matches!(decision, AggregateReviewDecision::Remediate(_)));
}

/// Stub-killing: multiple blocking findings within the SAME pass must all be
/// included — a stub that only forwards the first blocking finding per pass fails.
#[test]
fn test_aggregate_review_results_multiple_blocking_findings_same_pass_all_included() {
    let quality = review_result(
        ReviewPass::Quality,
        vec![
            blocking(ReviewPass::Quality, "q1"),
            blocking(ReviewPass::Quality, "q2"),
            blocking(ReviewPass::Quality, "q3"),
        ],
    );
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        0,
        3,
    );

    match decision {
        AggregateReviewDecision::Remediate(findings) => assert_eq!(findings.len(), 3),
        other => panic!("expected Remediate with 3 findings, got {other:?}"),
    }
}

/// The description and location of each blocking finding must be preserved
/// verbatim through Remediate (kills stubs that reconstruct placeholder findings).
#[test]
fn test_aggregate_review_results_finding_description_and_location_preserved_through_remediate() {
    let quality = review_result(
        ReviewPass::Quality,
        vec![blocking(
            ReviewPass::Quality,
            "exact description text must survive aggregation",
        )],
    );
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        0,
        3,
    );

    match decision {
        AggregateReviewDecision::Remediate(findings) => {
            assert_eq!(findings.len(), 1);
            assert_eq!(
                findings[0].description,
                "exact description text must survive aggregation"
            );
            assert!(findings[0].location.is_none());
        }
        other => panic!("expected Remediate, got {other:?}"),
    }
}

/// Stub-killing: no blocking anywhere, but `remediation_count` already at/over the
/// limit — must still Proceed. Kills a stub that escalates purely on budget
/// exhaustion without checking for blocking findings first.
#[test]
fn test_aggregate_review_results_cannot_hardcode_escalate_when_no_blocking_present() {
    let decision = aggregate_review_results(
        empty_pass(ReviewPass::Quality),
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        100,
        1,
    );
    assert!(matches!(decision, AggregateReviewDecision::Proceed));
}

/// Stub-killing: blocking findings present, but `remediation_count` is nowhere
/// near the limit — must Remediate. Kills a stub that always escalates whenever
/// any blocking finding exists.
#[test]
fn test_aggregate_review_results_cannot_hardcode_escalate_when_blocking_present() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        0,
        1000,
    );
    assert!(matches!(decision, AggregateReviewDecision::Remediate(_)));
}

/// Stub-killing: blocking findings present AND the budget is exhausted — must
/// Escalate, not Remediate. Kills a stub that always remediates regardless of budget.
#[test]
fn test_aggregate_review_results_cannot_hardcode_remediate_when_limit_exhausted() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        1000,
        1000,
    );
    assert!(matches!(decision, AggregateReviewDecision::Escalate(_)));
}

/// Large-value boundary using values near `u32::MAX` to catch overflow bugs in
/// a naive `remediation_count + 1 >= limit` or similar off-by-one implementation.
#[test]
fn test_aggregate_review_results_u32_max_remediation_count_and_limit_equal_with_blocking_returns_escalate() {
    let quality = review_result(ReviewPass::Quality, vec![blocking(ReviewPass::Quality, "q")]);
    let decision = aggregate_review_results(
        quality,
        empty_pass(ReviewPass::Architecture),
        empty_pass(ReviewPass::Security),
        u32::MAX,
        u32::MAX,
    );
    assert!(matches!(decision, AggregateReviewDecision::Escalate(_)));
}

/// Relative order of multiple blocking findings within the same pass must be
/// preserved (stable ordering, not reversed or shuffled).
#[test]
fn test_aggregate_review_results_preserves_relative_order_within_same_pass() {
    let security = review_result(
        ReviewPass::Security,
        vec![
            blocking(ReviewPass::Security, "first"),
            warning(ReviewPass::Security, "ignored-warning"),
            blocking(ReviewPass::Security, "second"),
            blocking(ReviewPass::Security, "third"),
        ],
    );
    let decision = aggregate_review_results(
        empty_pass(ReviewPass::Quality),
        empty_pass(ReviewPass::Architecture),
        security,
        0,
        3,
    );

    match decision {
        AggregateReviewDecision::Remediate(findings) => {
            let descriptions: Vec<&str> = findings.iter().map(|f| f.description.as_str()).collect();
            assert_eq!(descriptions, vec!["first", "second", "third"]);
        }
        other => panic!("expected Remediate, got {other:?}"),
    }
}

/// Only the Security pass has a blocking finding — Remediate must contain
/// exactly that single finding, tagged with the Security pass.
#[test]
fn test_aggregate_review_results_only_security_pass_has_blocking_others_empty_returns_remediate_with_single_finding()
 {
    let security = review_result(ReviewPass::Security, vec![blocking(ReviewPass::Security, "s-only")]);
    let decision = aggregate_review_results(
        empty_pass(ReviewPass::Quality),
        empty_pass(ReviewPass::Architecture),
        security,
        0,
        3,
    );

    match decision {
        AggregateReviewDecision::Remediate(findings) => {
            assert_eq!(findings.len(), 1);
            assert!(matches!(findings[0].pass, ReviewPass::Security));
        }
        other => panic!("expected Remediate, got {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 3: Property-based tests
// ═══════════════════════════════════════════════════════════════════════════

proptest! {
    /// Tri-state decision invariant: for arbitrary combinations of findings and
    /// budget state, the decision matches the documented decision table exactly,
    /// and — when Remediate — the payload is complete (one entry per blocking
    /// finding) and correctly ordered (Quality before Architecture before Security).
    #[test]
    fn test_aggregate_review_results_decision_matches_blocking_presence_and_budget(
        quality_sevs in proptest::collection::vec(severity_strategy(), 0..6),
        architecture_sevs in proptest::collection::vec(severity_strategy(), 0..6),
        security_sevs in proptest::collection::vec(severity_strategy(), 0..6),
        remediation_count in 0u32..10,
        limit in 0u32..10,
    ) {
        let total_blocking = quality_sevs.iter().filter(|s| **s == DiagnosticSeverity::Blocking).count()
            + architecture_sevs.iter().filter(|s| **s == DiagnosticSeverity::Blocking).count()
            + security_sevs.iter().filter(|s| **s == DiagnosticSeverity::Blocking).count();

        let quality = review_result_from_severities(ReviewPass::Quality, &quality_sevs);
        let architecture = review_result_from_severities(ReviewPass::Architecture, &architecture_sevs);
        let security = review_result_from_severities(ReviewPass::Security, &security_sevs);

        let decision = aggregate_review_results(quality, architecture, security, remediation_count, limit);

        if total_blocking == 0 {
            prop_assert!(matches!(decision, AggregateReviewDecision::Proceed));
        } else if remediation_count < limit {
            match decision {
                AggregateReviewDecision::Remediate(findings) => {
                    prop_assert_eq!(findings.len(), total_blocking);
                    let mut last_rank = 0u8;
                    for f in &findings {
                        let rank = pass_rank(f.pass);
                        prop_assert!(rank >= last_rank, "findings must be ordered Quality -> Architecture -> Security");
                        last_rank = rank;
                    }
                }
                AggregateReviewDecision::Proceed => prop_assert!(false, "expected Remediate, got Proceed"),
                AggregateReviewDecision::Escalate(_) => prop_assert!(false, "expected Remediate, got Escalate"),
            }
        } else {
            prop_assert!(matches!(decision, AggregateReviewDecision::Escalate(_)));
        }
    }
}
