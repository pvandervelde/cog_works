//! Adversarial test suite for `budget.rs` — `acquire_budget`.
//!
//! ## Phase: RED
//!
//! All tests compile but will **panic** at runtime because `acquire_budget` is
//! a `todo!()` stub. This is the expected RED state in TDD. Tests turn GREEN
//! once the implementation lands.
//!
//! ## Assertions covered
//!
//! - ASSERT-BUDGET-001: strict `<` — a node whose estimated cost exactly equals
//!   the remaining headroom is **denied**.
//!
//! ## Atomicity Contract (documented here per TDD requirement)
//!
//! `acquire_budget` is a **pure function with no internal synchronisation**.
//! In a concurrent caller (e.g. `PipelineExecutor` driving parallel nodes) the
//! caller **MUST**:
//!   1. Hold a `Mutex` (or equivalent exclusive lock) for the **entire** duration
//!      of the `acquire_budget` call.
//!   2. Update the accumulated cost immediately after receiving `Approved`,
//!      **while still holding the lock**.
//!   3. Release the lock only after the accumulated value is updated.
//!
//! Releasing the lock before step 2 creates a TOCTOU race: two parallel callers
//! can each see the same (stale) accumulated value, both be approved, and their
//! combined cost can exceed the budget. The test
//! `test_acquire_budget_parallel_toctou_race_documents_atomicity_contract`
//! demonstrates this failure mode without concurrency (pure sequential simulation)
//! to document the requirement.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use crate::types::{CostBudget, TokenCost};

use super::{BudgetAcquisition, CostReport, acquire_budget};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn cost(v: f64) -> TokenCost {
    TokenCost::new(v).expect("test helper: cost must be finite and non-negative")
}

fn budget(v: f64) -> CostBudget {
    CostBudget::new(v).expect("test helper: budget must be finite and positive")
}

fn empty_report(limit: CostBudget) -> impl FnOnce() -> CostReport {
    move || CostReport {
        per_node: vec![],
        per_sub_work_item: vec![],
        total: TokenCost::zero(),
        budget_limit: limit,
    }
}

// ─── Tier 1: Specification Tests ─────────────────────────────────────────────

/// ASSERT-BUDGET-001 (positive path): accumulated + estimated strictly below limit
/// → must return Approved.
#[test]
fn test_acquire_budget_below_limit_returns_approved() {
    let accumulated = cost(0.30);
    let estimated = cost(0.10);
    let limit = budget(1.00);

    let result = acquire_budget(&accumulated, &estimated, &limit, empty_report(limit));

    assert!(
        matches!(result, BudgetAcquisition::Approved { .. }),
        "expected Approved when accumulated + estimated < limit"
    );
}

/// ASSERT-BUDGET-001 (boundary — exact equality is **denied**): when
/// `accumulated + estimated == limit` the check must return `Denied`.
///
/// This is the primary assertion for the strict `<` invariant.
#[test]
fn test_acquire_budget_exact_limit_returns_denied() {
    // accumulated(0.60) + estimated(0.40) == limit(1.00) → must be Denied
    let accumulated = cost(0.60);
    let estimated = cost(0.40);
    let limit = budget(1.00);

    let result = acquire_budget(&accumulated, &estimated, &limit, empty_report(limit));

    assert!(
        matches!(result, BudgetAcquisition::Denied(_)),
        "expected Denied when accumulated + estimated == limit (strict < required)"
    );
}

/// When `accumulated + estimated > limit` the check must return `Denied`.
#[test]
fn test_acquire_budget_over_limit_returns_denied() {
    let accumulated = cost(0.80);
    let estimated = cost(0.30);
    let limit = budget(1.00);

    let result = acquire_budget(&accumulated, &estimated, &limit, empty_report(limit));

    assert!(
        matches!(result, BudgetAcquisition::Denied(_)),
        "expected Denied when accumulated + estimated > limit"
    );
}

/// `remaining` in `Approved` must equal `limit - (accumulated + estimated)`.
#[test]
fn test_acquire_budget_approved_remaining_is_correct() {
    let accumulated = cost(0.20);
    let estimated = cost(0.10);
    let limit = budget(1.00);

    let result = acquire_budget(&accumulated, &estimated, &limit, empty_report(limit));

    let remaining = match result {
        BudgetAcquisition::Approved { remaining } => remaining,
        BudgetAcquisition::Denied(_) => panic!("expected Approved"),
    };

    let expected_remaining = limit.as_f64() - (accumulated.as_f64() + estimated.as_f64());
    assert!(
        (remaining.as_f64() - expected_remaining).abs() < 1e-9,
        "remaining was {:.9} but expected {:.9}",
        remaining.as_f64(),
        expected_remaining
    );
}

/// When `accumulated == 0` and `estimated == 0` and `limit > 0`, must approve
/// with `remaining == limit`.
#[test]
fn test_acquire_budget_zero_accumulated_and_estimated_approves_with_full_remaining() {
    let accumulated = TokenCost::zero();
    let estimated = TokenCost::zero();
    let limit = budget(5.00);

    let result = acquire_budget(&accumulated, &estimated, &limit, empty_report(limit));

    let remaining = match result {
        BudgetAcquisition::Approved { remaining } => remaining,
        BudgetAcquisition::Denied(_) => panic!("expected Approved for all-zero costs"),
    };

    assert!(
        (remaining.as_f64() - limit.as_f64()).abs() < 1e-9,
        "remaining should equal limit when all costs are zero"
    );
}

// ─── Tier 2: Adversarial / Boundary Tests ────────────────────────────────────

/// The `report` closure must NOT be called on `Approved` (lazy evaluation).
///
/// Verifies that the implementation does not eagerly invoke the report builder
/// on the hot (approved) path — confirmed by a side-effecting closure that
/// sets a flag when called.
#[test]
fn test_acquire_budget_report_closure_not_called_on_approved() {
    let accumulated = cost(0.10);
    let estimated = cost(0.10);
    let limit = budget(1.00);

    let mut called = false;
    let result = acquire_budget(&accumulated, &estimated, &limit, || {
        called = true;
        CostReport {
            per_node: vec![],
            per_sub_work_item: vec![],
            total: TokenCost::zero(),
            budget_limit: limit,
        }
    });

    assert!(
        matches!(result, BudgetAcquisition::Approved { .. }),
        "expected Approved"
    );
    assert!(
        !called,
        "report closure must not be called on Approved path"
    );
}

/// The `report` closure MUST be called exactly once on `Denied`.
#[test]
fn test_acquire_budget_report_closure_called_on_denied() {
    // accumulated(0.70) + estimated(0.40) > limit(1.00)
    let accumulated = cost(0.70);
    let estimated = cost(0.40);
    let limit = budget(1.00);

    let mut called_count = 0u32;
    let result = acquire_budget(&accumulated, &estimated, &limit, || {
        called_count += 1;
        CostReport {
            per_node: vec![],
            per_sub_work_item: vec![],
            total: accumulated,
            budget_limit: limit,
        }
    });

    assert!(
        matches!(result, BudgetAcquisition::Denied(_)),
        "expected Denied"
    );
    assert_eq!(
        called_count, 1,
        "report closure must be called exactly once on Denied"
    );
}

/// The `CostReport` returned inside `Denied` must be the one produced by the
/// `report` closure (i.e. the function must not substitute a different report).
#[test]
fn test_acquire_budget_denied_contains_report_from_closure() {
    let accumulated = cost(0.90);
    let estimated = cost(0.20);
    let limit = budget(1.00);

    // Use a sentinel budget_limit that differs from `limit` to verify the
    // report object is not reconstructed by the function itself.
    let sentinel_budget = budget(42.0);

    let result = acquire_budget(&accumulated, &estimated, &limit, || CostReport {
        per_node: vec![],
        per_sub_work_item: vec![],
        total: accumulated,
        budget_limit: sentinel_budget,
    });

    let report = match result {
        BudgetAcquisition::Denied(r) => r,
        BudgetAcquisition::Approved { .. } => panic!("expected Denied"),
    };

    assert_eq!(
        report.budget_limit.as_f64(),
        42.0,
        "Denied must carry the exact CostReport produced by the closure"
    );
}

/// One-unit-below-boundary: accumulated + estimated == limit - ε must be
/// Approved (confirms the boundary is exclusive from the approved side).
#[test]
fn test_acquire_budget_one_epsilon_below_limit_returns_approved() {
    // Use a value just below 1.0 for accumulated + estimated.
    // With f64 we use the next-smaller representable value.
    let sum_f64 = 1.0_f64 - f64::EPSILON;
    let accumulated = cost(0.0);
    let estimated = cost(sum_f64);
    let limit = budget(1.0);

    let result = acquire_budget(&accumulated, &estimated, &limit, empty_report(limit));

    assert!(
        matches!(result, BudgetAcquisition::Approved { .. }),
        "expected Approved when sum is just below limit"
    );
}

/// Documents the TOCTOU vulnerability when the atomicity contract is violated.
///
/// ## Atomicity Contract reminder
///
/// `acquire_budget` is NOT thread-safe. The caller MUST hold a Mutex for
/// the entire `acquire_budget` call AND the subsequent accumulator update.
/// This test simulates two parallel nodes checking without the mutex: both
/// see the same stale `accumulated`, both are approved, but their combined
/// cost exceeds the budget — which is the precise failure mode the contract
/// prevents.
///
/// **This test does NOT test concurrency.** It is a sequential simulation
/// that demonstrates the logical TOCTOU race to document the requirement.
#[test]
fn test_acquire_budget_parallel_toctou_race_documents_atomicity_contract() {
    // Budget: 1.00, accumulated so far: 0.70
    // Node A estimates 0.20 — combined 0.90 < 1.00 → Approved
    // Node B estimates 0.20 — combined 0.90 < 1.00 → Approved (sees same stale accumulator)
    // Actual combined: 0.70 + 0.20 + 0.20 = 1.10 > 1.00 — OVER BUDGET
    //
    // This documents WHY callers MUST hold a Mutex across the call + update.
    let accumulated = cost(0.70);
    let estimated_a = cost(0.20);
    let estimated_b = cost(0.20);
    let limit = budget(1.00);

    // Without mutex: both nodes call acquire_budget with the same `accumulated`.
    let result_a = acquire_budget(&accumulated, &estimated_a, &limit, empty_report(limit));
    let result_b = acquire_budget(&accumulated, &estimated_b, &limit, empty_report(limit));

    // Both are approved individually — this is the documented TOCTOU hazard.
    assert!(
        matches!(result_a, BudgetAcquisition::Approved { .. }),
        "Node A is approved (correct in isolation)"
    );
    assert!(
        matches!(result_b, BudgetAcquisition::Approved { .. }),
        "Node B is approved (correct in isolation) — TOCTOU race: combined cost exceeds budget"
    );

    // The combined cost IS over budget — demonstrating why the atomicity contract
    // requires the caller to hold a mutex across the call and the update.
    let combined = accumulated.as_f64() + estimated_a.as_f64() + estimated_b.as_f64();
    assert!(
        combined > limit.as_f64(),
        "combined cost {combined:.2} should exceed limit {:.2} — confirming TOCTOU hazard",
        limit.as_f64()
    );
}

/// When `accumulated` is zero and `estimated` is small, `Approved.remaining`
/// must be strictly less than the limit (not equal), because estimated > 0.
#[test]
fn test_acquire_budget_approved_remaining_less_than_limit_when_estimated_nonzero() {
    let accumulated = TokenCost::zero();
    let estimated = cost(0.01);
    let limit = budget(1.00);

    let result = acquire_budget(&accumulated, &estimated, &limit, empty_report(limit));

    let remaining = match result {
        BudgetAcquisition::Approved { remaining } => remaining,
        BudgetAcquisition::Denied(_) => panic!("expected Approved"),
    };

    assert!(
        remaining.as_f64() < limit.as_f64(),
        "remaining must be less than limit when estimated > 0"
    );
}
