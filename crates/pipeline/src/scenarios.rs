//! Scenario satisfaction scoring.
//!
//! This module aggregates raw scenario trajectory results (produced by the
//! [`crate::ScenarioExecutor`] trait) into per-scenario scores and an overall
//! pass/fail determination.
//!
//! ## Explicit-Failure Scenarios
//!
//! Some scenarios are designed to verify that the system *fails safely* in a
//! given situation (e.g. a sensor disconnection causes a graceful stop rather
//! than undefined behaviour). These are marked with
//! `TrajectoryResult::expected_failure == true`. For these scenarios,
//! satisfaction means the expected failure was observed; if the failure was
//! *not* observed, the scenario is considered violated and reported in
//! `ScenarioSatisfactionResult::explicit_failure_violations`.
//!
//! ## Pure Business Logic
//!
//! No I/O. [`compute_satisfaction`] is deterministic for identical inputs.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/advanced-features.md` §Scenario Satisfaction for
//! the full contract and examples.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{domain_services::TrajectoryResult, types::SatisfactionScore};

// ─── Per-scenario score ───────────────────────────────────────────────────────

/// Pass/fail result for a single scenario, including a fractional satisfaction
/// score and explicit-failure detection.
///
/// One `PerScenarioScore` is produced for each distinct `scenario_id` found
/// across the trajectory results passed to [`compute_satisfaction`].
///
/// See `docs/spec/interfaces/advanced-features.md` §PerScenarioScore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerScenarioScore {
    /// Identifier matching [`crate::Scenario::id`].
    pub scenario_id: String,

    /// Count of trajectories that met all acceptance criteria.
    ///
    /// For explicit-failure scenarios this counts trajectories where the
    /// expected failure was observed (`TrajectoryResult::satisfied == true` in
    /// combination with `TrajectoryResult::expected_failure == true`).
    pub satisfied_trajectories: u32,

    /// Total number of trajectories executed for this scenario.
    pub total_trajectories: u32,

    /// Fraction of trajectories that were satisfied, in `[0.0, 1.0]`.
    ///
    /// Computed as `satisfied_trajectories / total_trajectories`. When
    /// `total_trajectories == 0` this is `1.0` (vacuously satisfied).
    pub score: SatisfactionScore,

    /// `true` when `score >= threshold` (where threshold was passed to
    /// [`compute_satisfaction`]).
    pub passed: bool,

    /// `true` when this is an explicit-failure scenario (at least one
    /// trajectory in the group had `expected_failure == true`).
    pub explicit_failure: bool,
}

// ─── Aggregate result ─────────────────────────────────────────────────────────

/// Aggregated result for all scenarios executed in one simulation pass.
///
/// Produced by [`compute_satisfaction`] from a flat slice of
/// [`TrajectoryResult`] values.
///
/// See `docs/spec/interfaces/advanced-features.md` §ScenarioSatisfactionResult.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSatisfactionResult {
    /// Per-scenario score breakdown; one entry per distinct `scenario_id`.
    pub per_scenario: Vec<PerScenarioScore>,

    /// Unweighted mean of all per-scenario scores, in `[0.0, 1.0]`.
    ///
    /// When `per_scenario` is empty this is `1.0` (vacuously satisfied).
    pub overall_score: SatisfactionScore,

    /// `true` when:
    /// - Every per-scenario `passed == true`, **and**
    /// - `explicit_failure_violations` is empty.
    pub passed: bool,

    /// Scenario IDs of expected-failure scenarios whose failure was *not*
    /// observed.
    ///
    /// An entry here means a scenario designed to verify graceful failure
    /// behaviour succeeded when it should have failed — a potential safety
    /// concern for safety-critical systems.
    pub explicit_failure_violations: Vec<String>,
}

// ─── Core function ────────────────────────────────────────────────────────────

/// Aggregates raw trajectory results into per-scenario scores and an overall
/// satisfaction determination.
///
/// # Arguments
///
/// * `trajectory_results` — flat slice of per-trajectory outcomes returned by
///   [`crate::ScenarioExecutor::execute_trajectory`].
/// * `threshold` — minimum per-scenario score required to consider a scenario
///   passed.
///
/// # Returns
///
/// A [`ScenarioSatisfactionResult`] with:
/// - One [`PerScenarioScore`] per distinct `scenario_id`.
/// - `overall_score` as the unweighted mean of per-scenario scores.
/// - `passed` iff all scenarios pass **and** no explicit-failure violations
///   are found.
///
/// # Empty Input
///
/// When `trajectory_results` is empty returns a result with `passed: true`
/// and `overall_score: 1.0` (vacuously satisfied — no scenarios to fail).
///
/// # Examples
///
/// ```no_run
/// use pipeline::scenarios::compute_satisfaction;
/// use pipeline::domain_services::TrajectoryResult;
/// use pipeline::types::SatisfactionScore;
///
/// let results = vec![
///     TrajectoryResult {
///         scenario_id: "sc-01".to_string(),
///         passed: true,
///         satisfaction_score: SatisfactionScore::new(1.0).unwrap(),
///         expected_failure: false,
///         diagnostics: pipeline::domain_services::Diagnostics::empty(),
///     },
///     TrajectoryResult {
///         scenario_id: "sc-01".to_string(),
///         passed: false,
///         satisfaction_score: SatisfactionScore::new(0.0).unwrap(),
///         expected_failure: false,
///         diagnostics: pipeline::domain_services::Diagnostics::empty(),
///     },
/// ];
/// let threshold = SatisfactionScore::new(0.5).unwrap();
/// let result = compute_satisfaction(&results, threshold);
/// // sc-01: 1/2 passed → overall score 0.5 → passed (0.5 >= 0.5)
/// assert!(result.passed);
/// ```
///
/// See `docs/spec/interfaces/advanced-features.md` §compute_satisfaction.
pub fn compute_satisfaction(
    _trajectory_results: &[TrajectoryResult],
    _threshold: SatisfactionScore,
) -> ScenarioSatisfactionResult {
    todo!("See docs/spec/interfaces/advanced-features.md §Scenario Satisfaction")
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Groups trajectory results by scenario_id for scoring.
///
/// Returns a map from scenario_id → (satisfied_count, total_count, has_explicit_failure).
#[allow(dead_code)]
fn group_by_scenario(_results: &[TrajectoryResult]) -> HashMap<String, (u32, u32, bool)> {
    todo!("See docs/spec/interfaces/advanced-features.md §compute_satisfaction internals")
}

#[cfg(test)]
#[path = "scenarios_tests.rs"]
mod tests;
