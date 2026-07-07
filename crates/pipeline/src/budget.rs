//! Token cost budget enforcement for pipeline execution.
//!
//! This module provides the [`acquire_budget`] function and supporting types
//! for checking cost budgets before each node (or parallel batch of nodes)
//! is started. Budget enforcement is pure: no I/O, no shared state.
//!
//! ## Atomicity Contract — CRITICAL for Parallel Execution
//!
//! [`acquire_budget`] is a **pure function** with no internal synchronisation.
//! When two or more nodes execute concurrently, the same accumulated cost and
//! the same budget limit are visible to all callers. If each parallel node
//! calls `acquire_budget` independently without coordination, the combined
//! estimated cost is never checked holistically — two nodes each estimating
//! 40 % of the budget can both be approved even when their combined cost
//! would exceed 100 %.
//!
//! **The caller is therefore required to**:
//! 1. Hold a `Mutex` (or equivalent) for the duration of the
//!    `acquire_budget` call when driving parallel nodes.
//! 2. Update the accumulated cost immediately after receiving `Approved`,
//!    while still holding the lock.
//! 3. Release the lock only after the accumulated value is updated.
//!
//! This contract is enforced by convention, not by the type system. The
//! `nodes` crate's `PipelineExecutor` holds the shared `Mutex<TokenCost>`
//! across all parallel node budget checks.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/pipeline-execution.md` §Budget Enforcement for
//! the full contract, decision table, and atomicity requirements.

use serde::{Deserialize, Serialize};

use crate::{
    identifiers::{NodeId, SubWorkItemId},
    types::{CostBudget, TokenCost},
};

// ─── Cost breakdown ──────────────────────────────────────────────────────────

/// Per-node breakdown entry in a [`CostReport`].
///
/// Represents the total token cost attributed to a single node's executions
/// (including all retries and rework iterations) within the current run.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §CostReport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCostEntry {
    /// The node this cost entry belongs to.
    pub node_id: NodeId,
    /// Total token cost incurred by all execution attempts for this node.
    pub total_cost: TokenCost,
}

// ---------------------------------------------------------------------------

/// Per-sub-work-item breakdown entry in a [`CostReport`].
///
/// Represents the total cost attributed to all nodes that executed in the
/// context of a specific sub-work-item.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §CostReport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubWorkItemCostEntry {
    /// The sub-work-item this cost entry belongs to.
    pub sub_work_item_id: SubWorkItemId,
    /// Total token cost incurred by all nodes processing this sub-work-item.
    pub total_cost: TokenCost,
}

// ---------------------------------------------------------------------------

/// Full cost breakdown produced when a budget check is denied.
///
/// Included in [`BudgetAcquisition::Denied`] so that the caller can present
/// a complete cost breakdown in the escalation report and audit trail.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §CostReport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostReport {
    /// Per-node cost breakdown for all nodes that have executed in this run.
    ///
    /// Ordered by total cost descending (highest spender first).
    pub per_node: Vec<NodeCostEntry>,

    /// Per-sub-work-item cost breakdown.
    ///
    /// Empty if the current run has not yet produced any sub-work-items (i.e.
    /// the Planning node has not yet completed).
    pub per_sub_work_item: Vec<SubWorkItemCostEntry>,

    /// Total accumulated cost at the point the budget was exceeded.
    pub total: TokenCost,

    /// The configured budget limit that was exceeded.
    pub budget_limit: CostBudget,
}

// ─── Budget acquisition result ───────────────────────────────────────────────

/// Result of a [`acquire_budget`] call.
///
/// The caller must inspect this value before starting any node execution:
/// - On `Approved`, start the node and add `estimated` to the running
///   accumulator *while the lock is still held*.
/// - On `Denied`, emit a [`crate::execution::NextAction::HaltWithError`] with
///   [`crate::execution::PipelineError::BudgetExceeded`].
///
/// See `docs/spec/interfaces/pipeline-execution.md` §BudgetAcquisition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BudgetAcquisition {
    /// The estimated cost fits within the remaining budget; the node may proceed.
    ///
    /// `remaining` is the budget headroom *after* the estimated cost is reserved.
    /// The caller must update the running accumulator by `estimated` immediately
    /// after receiving this variant (while holding the parallel-execution lock).
    Approved {
        /// Remaining budget after the estimated cost is reserved.
        remaining: CostBudget,
    },

    /// The estimated cost would exceed the budget; the node must not proceed.
    ///
    /// The attached [`CostReport`] provides a full breakdown for the
    /// escalation report and audit trail.
    Denied(CostReport),
}

// ─── Pure business logic function ────────────────────────────────────────────

/// Checks whether the estimated cost for a new node invocation fits within
/// the configured budget.
///
/// Returns [`BudgetAcquisition::Approved`] if `accumulated + estimated < limit`,
/// or [`BudgetAcquisition::Denied`] with a full cost report otherwise.
///
/// ## Strict Inequality
///
/// The approval condition is **strict** (`<`, not `<=`). A node whose estimated
/// cost exactly equals the remaining headroom is denied. This is intentional:
/// it preserves a minimum of one budget unit of headroom against f64 rounding
/// accumulation. If you need to allow exact-budget use, document it explicitly
/// and use a per-project budget with a small safety margin instead.
///
/// ## Floating-Point Rounding
///
/// Both [`TokenCost`] and [`CostBudget`] wrap `f64`. Repeated addition of
/// small values accumulates rounding error: 100 additions of `0.001` may yield
/// `0.10000000000000001` rather than `0.1`. At the margins this can cause
/// borderline `Approved`/`Denied` decisions to flip. Callers should pad budget
/// limits by a small epsilon (e.g. round up to the nearest micro-dollar unit)
/// if sub-cent precision is required. The strict `<` check (see above) provides
/// a minimum guard against exact-boundary accumulation errors.
///
/// ## ⚠️ Atomicity Contract — CRITICAL
///
/// This function is **not thread-safe** by itself. When nodes execute in
/// parallel the caller **must** hold a `Mutex` (or equivalent) that covers
/// both the `acquire_budget` call and the subsequent update to the accumulated
/// cost accumulator. Releasing the lock before updating the accumulator creates
/// a TOCTOU race that allows concurrent overspend.
///
/// The `PipelineExecutor` in the `nodes` crate holds `Arc<Mutex<TokenCost>>`
/// for exactly this purpose. Every call site that drives parallel nodes must
/// use that shared mutex.
///
/// ## Lazy Report Construction
///
/// The `report` parameter is a `FnOnce() -> CostReport` closure rather than
/// an eagerly constructed [`CostReport`]. This avoids two `Vec` allocations on
/// every call (which is the hot path — the vast majority of checks are
/// `Approved`). The closure is only invoked when the check is `Denied` (~1 %
/// of calls in a healthy run). Callers should build the breakdown inside the
/// closure, not outside it.
///
/// ## Parameters
///
/// - `accumulated` — Current total cost recorded in
///   [`crate::PipelineState::cost_accumulator`] at the moment of the check.
/// - `estimated` — Expected cost for the node about to be started.
/// - `limit` — Budget ceiling from the node or pipeline configuration.
/// - `report` — Lazy closure producing the full cost breakdown if the check
///   fails. Only called on `Denied`; not called on `Approved`.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §acquire_budget`
#[must_use]
pub fn acquire_budget(
    accumulated: &TokenCost,
    estimated: &TokenCost,
    limit: &CostBudget,
    report: impl FnOnce() -> CostReport,
) -> BudgetAcquisition {
    let total = accumulated.as_f64() + estimated.as_f64();
    if total < limit.as_f64() {
        // total < limit.as_f64() (strict), so remaining_f64 > 0 and finite.
        // CostBudget::new requires > 0, which holds. The fallback to *limit
        // is unreachable in practice but avoids an unwrap() in production.
        let remaining_f64 = limit.as_f64() - total;
        let remaining = CostBudget::new(remaining_f64).unwrap_or(*limit);
        BudgetAcquisition::Approved { remaining }
    } else {
        BudgetAcquisition::Denied(report())
    }
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
