//! Adversarial test suite for `execution.rs` pure business-logic stubs.
//!
//! Covers three functions:
//! - `check_fan_in_ready`        — 5 tests
//! - `evaluate_edge_condition`   — 13 tests
//! - `increment_rework_counter`  — 6 tests
//!
//! ## Phase: RED
//!
//! All tests compile but will **panic** at runtime because the three target
//! functions are `todo!()` stubs. This is the expected RED state in TDD. The
//! tests will turn GREEN once the implementations land.
//!
//! ## Assertions covered (from `docs/spec/assertions.md`)
//!
//! ASSERT-PSM-003 / ASSERT-PSM-004 / ASSERT-PSM-005 are exercised indirectly
//! through the fan-in and rework-counter primitives. The direct coverage for
//! those assertions lives in the integration-level `determine_next_actions`
//! tests (future work, tracked separately).
//!
//! ## Test naming
//!
//! `test_{function}_{scenario}_{expected}` per project convention.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use std::collections::HashMap;

use super::*;
use crate::{
    graph::{
        CompositeCondition, EdgeDefinition, EvaluatorKind, Expression, NaturalLanguageCondition,
        NodeDefinition, NodeGate, NodeState, NodeType, OverflowBehaviour, PipelineSettings,
        PipelineToolProfileConfig, ReworkEdge, ReworkSemantics, ValidationKind,
    },
    identifiers::{PipelineRunId, ProfileName},
};

// ─── Shared test helpers ──────────────────────────────────────────────────────

fn nid(s: &str) -> NodeId {
    NodeId::new(s).expect("test helper: node id must not be empty")
}

fn eid(s: &str) -> EdgeId {
    EdgeId::new(s).expect("test helper: edge id must not be empty")
}

/// Empty [`PipelineState`] — nodes absent from `node_states` are implicitly Pending.
fn make_state() -> PipelineState {
    PipelineState {
        run_id: PipelineRunId::new_random(),
        node_states: HashMap::new(),
        active_parallel_branches: vec![],
        cost_accumulator: TokenCost::zero(),
    }
}

/// [`NodeState`] with the given status and otherwise-zero counters.
fn make_node_state(status: NodeStatus) -> NodeState {
    NodeState {
        status,
        attempt_count: 1,
        rework_count: 0,
        current_error: None,
        rework_edge_traversals: HashMap::new(),
    }
}

/// Minimal [`NodeDefinition`] (Deterministic, no gates, no declared slots).
fn make_node(id: &str) -> NodeDefinition {
    NodeDefinition {
        id: nid(id),
        node_type: NodeType::Deterministic,
        declared_inputs: vec![],
        declared_outputs: vec![],
        timeout: None,
        cost_budget: None,
        gate: NodeGate::AutoProceed,
        validation_kind: ValidationKind::None,
        abort_siblings_on_failure: false,
    }
}

/// Minimal forward (non-rework) [`EdgeDefinition`].
fn make_edge(id: &str, src: &str, tgt: &str) -> EdgeDefinition {
    EdgeDefinition {
        id: eid(id),
        source: nid(src),
        target: nid(tgt),
        condition: EdgeConditionKind::Deterministic(
            Expression::new("status == 'active'").expect("test expr"),
        ),
        rework_edge: None,
    }
}

/// Back-edge (rework) [`EdgeDefinition`] with a specified `max_traversals`.
fn make_rework_edge_def(id: &str, src: &str, tgt: &str, max_traversals: u32) -> EdgeDefinition {
    EdgeDefinition {
        id: eid(id),
        source: nid(src),
        target: nid(tgt),
        condition: EdgeConditionKind::Deterministic(
            Expression::new("status == 'active'").expect("test expr"),
        ),
        rework_edge: Some(ReworkEdge {
            max_traversals,
            preserved_outputs: vec![],
            overflow_behaviour: OverflowBehaviour::HaltWithError,
            semantics: ReworkSemantics::Retry,
        }),
    }
}

/// Minimal valid [`PipelineGraph`] wrapping the provided nodes and edges.
fn make_graph(nodes: Vec<NodeDefinition>, edges: Vec<EdgeDefinition>) -> PipelineGraph {
    PipelineGraph {
        nodes,
        edges,
        evaluation_modes: HashMap::new(),
        explicit_edge_lists: HashMap::new(),
        settings: PipelineSettings {
            default_timeout: None,
            default_cost_budget: None,
            max_node_retries: 3,
        },
        tool_profiles: PipelineToolProfileConfig {
            default_profile: ProfileName::new("default").expect("test profile name"),
            node_overrides: HashMap::new(),
        },
    }
}

/// Minimal [`NodeOutput`] with no artifacts and zero cost.
fn make_output() -> NodeOutput {
    NodeOutput {
        artifacts: HashMap::new(),
        cost_delta: TokenCost::zero(),
        state_updates: vec![],
    }
}

// ─── check_fan_in_ready ───────────────────────────────────────────────────────

mod check_fan_in_ready_tests {
    use super::*;

    #[test]
    fn test_check_fan_in_ready_no_predecessors_returns_true() {
        // Entry node: no incoming forward edges → trivially ready.
        // Spec: "Returns true trivially for a node with no forward-edge predecessors."
        // A stub returning `false` unconditionally would fail this test.
        let state = make_state();
        let graph = make_graph(vec![make_node("a")], vec![]);

        assert!(
            check_fan_in_ready(&nid("a"), &state, &graph),
            "entry node with no predecessor edges must always be ready"
        );
    }

    #[test]
    fn test_check_fan_in_ready_all_completed_returns_true() {
        // A → C (forward) and B → C (forward): both A and B are Completed → C is ready.
        // A stub always returning `false` would fail here.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "c"), make_edge("e2", "b", "c")];
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Completed));
        state
            .node_states
            .insert(nid("b"), make_node_state(NodeStatus::Completed));
        let graph = make_graph(nodes, edges);

        assert!(
            check_fan_in_ready(&nid("c"), &state, &graph),
            "fan-in node must be ready when every predecessor is Completed"
        );
    }

    #[test]
    fn test_check_fan_in_ready_one_incomplete_returns_false() {
        // A → C (forward) and B → C (forward): A is Active, B is Completed → C is NOT ready.
        // A stub always returning `true` would fail here.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "c"), make_edge("e2", "b", "c")];
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Active));
        state
            .node_states
            .insert(nid("b"), make_node_state(NodeStatus::Completed));
        let graph = make_graph(nodes, edges);

        assert!(
            !check_fan_in_ready(&nid("c"), &state, &graph),
            "fan-in node must not be ready when at least one predecessor is not Completed"
        );
    }

    #[test]
    fn test_check_fan_in_ready_all_pending_returns_false() {
        // A → C (forward) and B → C (forward): neither A nor B have a state entry
        // (absent entries are Pending). Pending ≠ Completed → C is not ready.
        // A stub that returns `true` for nodes absent from state would fail.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "c"), make_edge("e2", "b", "c")];
        let state = make_state(); // A and B are absent → implicitly Pending
        let graph = make_graph(nodes, edges);

        assert!(
            !check_fan_in_ready(&nid("c"), &state, &graph),
            "fan-in node must not be ready when all predecessors are Pending"
        );
    }

    #[test]
    fn test_check_fan_in_ready_rework_edges_ignored() {
        // B →rework→ A: the only incoming edge to A is a rework (back) edge.
        // Rework edges must be excluded from the predecessor set.
        // A with no forward predecessors must return `true` even though B is Active.
        // An implementation that counts rework predecessors would return `false` here.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_rework_edge_def("rw1", "b", "a", 2)];
        let mut state = make_state();
        // B is Active (not Completed) — if the rework edge were counted the result
        // would be false, exposing the bug.
        state
            .node_states
            .insert(nid("b"), make_node_state(NodeStatus::Active));
        let graph = make_graph(nodes, edges);

        assert!(
            check_fan_in_ready(&nid("a"), &state, &graph),
            "rework-edge predecessors must be excluded; A has no forward predecessors so it is ready"
        );
    }
}

// ─── evaluate_edge_condition ──────────────────────────────────────────────────

mod evaluate_edge_condition_tests {
    use super::*;

    /// State where `node-a` has status `Completed`.
    fn state_with_completed_node_a() -> PipelineState {
        let mut s = make_state();
        s.node_states
            .insert(nid("node-a"), make_node_state(NodeStatus::Completed));
        s
    }

    /// State where `node-a` has status `Active` (not Completed).
    fn state_with_active_node_a() -> PipelineState {
        let mut s = make_state();
        s.node_states
            .insert(nid("node-a"), make_node_state(NodeStatus::Active));
        s
    }

    /// State where both `node-a` (Completed) and `node-b` (Active) exist.
    fn state_with_two_nodes() -> PipelineState {
        let mut s = make_state();
        s.node_states
            .insert(nid("node-a"), make_node_state(NodeStatus::Completed));
        s.node_states
            .insert(nid("node-b"), make_node_state(NodeStatus::Active));
        s
    }

    /// A Deterministic condition that evaluates to `true` when `node-a` is Completed.
    fn cond_true_when_node_a_completed() -> EdgeConditionKind {
        EdgeConditionKind::Deterministic(
            Expression::new("node_states.node-a.status == 'Completed'").expect("valid expr"),
        )
    }

    /// A Deterministic condition that evaluates to `false` when `node-a` is Active.
    fn cond_false_when_node_a_not_completed() -> EdgeConditionKind {
        // Returns false when node-a is Active (Active ≠ Completed).
        EdgeConditionKind::Deterministic(
            Expression::new("node_states.node-a.status == 'Completed'").expect("valid expr"),
        )
    }

    /// A Deterministic condition that evaluates to `false` against `node-b` (Active).
    fn cond_false_node_b() -> EdgeConditionKind {
        EdgeConditionKind::Deterministic(
            Expression::new("node_states.node-b.status == 'Completed'").expect("valid expr"),
        )
    }

    #[test]
    fn test_evaluate_edge_condition_deterministic_true_produces_record() {
        // Deterministic condition that is satisfied → result is true, record.evaluator is Deterministic.
        // A stub returning (false, _) or producing an LlmModel evaluator would fail.
        let edge_id = eid("e1");
        let cond = cond_true_when_node_a_completed();
        let state = state_with_completed_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();

        let (result, record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(result, "satisfied Deterministic condition must return true");
        assert!(
            matches!(record.evaluator, EvaluatorKind::Deterministic),
            "Deterministic condition must produce an EvaluatorKind::Deterministic record"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_deterministic_false_produces_record() {
        // Deterministic condition that is NOT satisfied → result is false.
        // A stub always returning `true` would fail.
        let edge_id = eid("e1");
        let cond = cond_false_when_node_a_not_completed();
        let state = state_with_active_node_a(); // Active ≠ Completed
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();

        let (result, _record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            !result,
            "unsatisfied Deterministic condition must return false"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_llm_evaluated_present_returns_value() {
        // LlmEvaluated key present in map with value `true` → result is `true`.
        // A stub ignoring the map and returning false would fail.
        let edge_id = eid("e-llm");
        let cond = EdgeConditionKind::LlmEvaluated(
            NaturalLanguageCondition::new("output quality is acceptable").expect("valid cond"),
        );
        let state = make_state();
        let output = make_output();
        let mut llm_results: HashMap<EdgeId, bool> = HashMap::new();
        llm_results.insert(eid("e-llm"), true);
        let ts = Timestamp::now();

        let (result, record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            result,
            "LlmEvaluated must return the pre-resolved true value from the map"
        );
        assert!(
            matches!(record.evaluator, EvaluatorKind::LlmModel { .. }),
            "LlmEvaluated condition must produce an EvaluatorKind::LlmModel record"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_llm_evaluated_absent_returns_false() {
        // LlmEvaluated key MISSING → conservative fallback: false.
        // Spec: "a missing entry is treated as false (conservative fallback)."
        // A stub returning true by default would fail.
        let edge_id = eid("e-llm-missing");
        let cond = EdgeConditionKind::LlmEvaluated(
            NaturalLanguageCondition::new("output quality is acceptable").expect("valid cond"),
        );
        let state = make_state();
        let output = make_output();
        let llm_results: HashMap<EdgeId, bool> = HashMap::new(); // key absent
        let ts = Timestamp::now();

        let (result, _record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            !result,
            "LlmEvaluated with missing map key must fall back to false (conservative)"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_composite_and_all_true_returns_true() {
        // Composite And([true, true]) → true.
        // A stub returning false for any Composite would fail.
        let edge_id = eid("e-and");
        // node-a is Completed; both inner conditions check node-a == Completed → both true.
        let state = state_with_completed_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();
        let completed_expr =
            Expression::new("node_states.node-a.status == 'Completed'").expect("expr");

        let cond = EdgeConditionKind::Composite(CompositeCondition::And(vec![
            EdgeConditionKind::Deterministic(completed_expr.clone()),
            EdgeConditionKind::Deterministic(completed_expr),
        ]));

        let (result, _record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            result,
            "Composite And with all true branches must evaluate to true"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_composite_and_one_false_returns_false() {
        // Composite And([true, false]) → false.
        // node-a is Completed (inner-1 true), node-b is Active (inner-2 false).
        // A stub always returning true for And would fail.
        let edge_id = eid("e-and");
        let state = state_with_two_nodes();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();

        let cond = EdgeConditionKind::Composite(CompositeCondition::And(vec![
            // node-a == Completed → true
            EdgeConditionKind::Deterministic(
                Expression::new("node_states.node-a.status == 'Completed'").expect("expr"),
            ),
            // node-b == Completed → false (node-b is Active)
            cond_false_node_b(),
        ]));

        let (result, _record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            !result,
            "Composite And with one false branch must evaluate to false"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_composite_or_one_true_returns_true() {
        // Composite Or([false, true]) → true.
        // node-a is Completed (inner-2 true), node-b is Active (inner-1 false).
        // A stub always returning false for Or would fail.
        let edge_id = eid("e-or");
        let state = state_with_two_nodes();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();

        let cond = EdgeConditionKind::Composite(CompositeCondition::Or(vec![
            // node-b == Completed → false (node-b is Active)
            cond_false_node_b(),
            // node-a == Completed → true
            EdgeConditionKind::Deterministic(
                Expression::new("node_states.node-a.status == 'Completed'").expect("expr"),
            ),
        ]));

        let (result, _record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            result,
            "Composite Or with at least one true branch must evaluate to true"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_composite_or_all_false_returns_false() {
        // Composite Or([false, false]) → false.
        // Both inner conditions check node-a == Completed but node-a is Active.
        // A stub returning true for Or would fail.
        let edge_id = eid("e-or");
        let state = state_with_active_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();
        let false_expr = Expression::new("node_states.node-a.status == 'Completed'").expect("expr");

        let cond = EdgeConditionKind::Composite(CompositeCondition::Or(vec![
            EdgeConditionKind::Deterministic(false_expr.clone()),
            EdgeConditionKind::Deterministic(false_expr),
        ]));

        let (result, _record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            !result,
            "Composite Or with all false branches must evaluate to false"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_composite_not_inverts_true_to_false() {
        // Composite Not(true) → false.
        // Inner condition is satisfied (node-a Completed); Not must invert it.
        // A stub returning the inner value unchanged would fail.
        let edge_id = eid("e-not");
        let state = state_with_completed_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();

        let cond = EdgeConditionKind::Composite(CompositeCondition::Not(Box::new(
            cond_true_when_node_a_completed(), // inner is true
        )));

        let (result, _record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            !result,
            "Composite Not must invert a true inner condition to false"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_composite_not_inverts_false_to_true() {
        // Composite Not(false) → true.
        // Inner condition is not satisfied (node-a Active ≠ Completed); Not must invert it.
        // A stub returning false unchanged would fail.
        let edge_id = eid("e-not");
        let state = state_with_active_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();

        let cond = EdgeConditionKind::Composite(CompositeCondition::Not(Box::new(
            cond_false_when_node_a_not_completed(), // inner is false
        )));

        let (result, _record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(
            result,
            "Composite Not must invert a false inner condition to true"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_record_contains_input_snapshot() {
        // EdgeEvaluationRecord.input_snapshot must equal serde_json::to_value(state).
        // Spec constraint: every evaluation is recorded with a state snapshot.
        // A stub storing Null or an empty object would fail.
        let edge_id = eid("e-snap");
        let cond = cond_true_when_node_a_completed();
        let state = state_with_completed_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();
        let expected_snapshot =
            serde_json::to_value(&state).expect("PipelineState must be serializable");

        let (_result, record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert_eq!(
            record.input_snapshot, expected_snapshot,
            "EdgeEvaluationRecord.input_snapshot must be the JSON-serialised PipelineState"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_record_contains_edge_id() {
        // EdgeEvaluationRecord.edge_id must match the edge_id parameter.
        // A stub hardcoding a different EdgeId or leaving it blank would fail.
        let edge_id = eid("e-audit-id");
        let cond = cond_true_when_node_a_completed();
        let state = state_with_completed_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();

        let (_result, record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert_eq!(
            record.edge_id, edge_id,
            "EdgeEvaluationRecord.edge_id must match the edge_id argument"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_record_timestamp_matches_evaluated_at() {
        // EdgeEvaluationRecord.timestamp must equal the evaluated_at argument.
        // Spec: "evaluated_at — Wall-clock time of evaluation; passed in so the function
        //        remains pure and testable without std::time access."
        // A stub calling Timestamp::now() internally would produce a different value.
        let edge_id = eid("e-ts");
        let cond = cond_true_when_node_a_completed();
        let state = state_with_completed_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        // Use a fixed Timestamp derived from a known UTC instant.
        let fixed_ts =
            Timestamp::from_utc(chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap());

        let (_result, record) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, fixed_ts);

        assert_eq!(
            record.timestamp, fixed_ts,
            "EdgeEvaluationRecord.timestamp must equal the evaluated_at parameter, not a fresh now()"
        );
    }
}

// ─── increment_rework_counter ─────────────────────────────────────────────────

mod increment_rework_counter_tests {
    use super::*;

    /// Returns a graph with a single rework edge `rw1` from `a` → `b`
    /// with the given `max_traversals`.
    fn graph_with_rework(max_traversals: u32) -> PipelineGraph {
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_rework_edge_def("rw1", "a", "b", max_traversals)];
        make_graph(nodes, edges)
    }

    /// Returns a state where node `b` already has `count` traversals for edge `rw1`.
    fn state_with_traversal_count(count: u32) -> PipelineState {
        let mut state = make_state();
        let mut ns = make_node_state(NodeStatus::Pending);
        ns.rework_edge_traversals.insert(eid("rw1"), count);
        state.node_states.insert(nid("b"), ns);
        state
    }

    #[test]
    fn test_increment_rework_counter_first_traversal_returns_one() {
        // No prior traversal count for `rw1`; first call returns Ok(1).
        // Spec: "Increments rework_edge_traversals[edge] (starting from 0 if absent)."
        // A stub returning Ok(0) or Err would fail.
        let graph = graph_with_rework(3);
        let mut state = make_state(); // node `b` absent from node_states

        let result = increment_rework_counter(&eid("rw1"), &mut state, &graph);

        assert_eq!(result.unwrap(), 1, "first traversal must return Ok(1)");
    }

    #[test]
    fn test_increment_rework_counter_increments_existing_count() {
        // Existing traversal count is 2; increment returns Ok(3).
        // A stub returning the old count unchanged (Ok(2)) would fail.
        let graph = graph_with_rework(5);
        let mut state = state_with_traversal_count(2);

        let result = increment_rework_counter(&eid("rw1"), &mut state, &graph);

        assert_eq!(result.unwrap(), 3, "counter at 2 must be incremented to 3");
    }

    #[test]
    fn test_increment_rework_counter_at_limit_returns_ok() {
        // Traversal count is max_traversals - 1; increment reaches max exactly → Ok(max).
        // Spec: "Returns Ok(new_count) if new_count <= max_traversals."
        // A stub treating == max as an error would fail.
        let max = 3u32;
        let graph = graph_with_rework(max);
        let mut state = state_with_traversal_count(max - 1); // count = 2

        let result = increment_rework_counter(&eid("rw1"), &mut state, &graph);

        assert_eq!(
            result.unwrap(),
            max,
            "counter reaching max_traversals exactly must return Ok(max), not Err"
        );
    }

    #[test]
    fn test_increment_rework_counter_over_limit_returns_err() {
        // Traversal count is already at max; increment exceeds limit → Err.
        // Spec: "Returns Err(TerminationConditionReached) if new_count > max_traversals."
        // A stub always returning Ok would fail.
        let max = 3u32;
        let graph = graph_with_rework(max);
        let mut state = state_with_traversal_count(max); // count already AT max

        let result = increment_rework_counter(&eid("rw1"), &mut state, &graph);

        assert!(
            result.is_err(),
            "incrementing beyond max_traversals must return Err(TerminationConditionReached)"
        );
    }

    #[test]
    fn test_increment_rework_counter_err_contains_correct_fields() {
        // TerminationConditionReached must carry the correct edge_id, current_traversals,
        // and max_traversals. A stub populating these with default/zero values would fail.
        let max = 3u32;
        let graph = graph_with_rework(max);
        let mut state = state_with_traversal_count(max); // will exceed on increment

        let err = increment_rework_counter(&eid("rw1"), &mut state, &graph).unwrap_err();

        assert_eq!(
            err.edge_id,
            eid("rw1"),
            "TerminationConditionReached.edge_id must match"
        );
        assert_eq!(
            err.current_traversals,
            max + 1,
            "TerminationConditionReached.current_traversals must be the incremented count"
        );
        assert_eq!(
            err.max_traversals, max,
            "TerminationConditionReached.max_traversals must match the edge configuration"
        );
    }

    #[test]
    fn test_increment_rework_counter_mutates_state() {
        // After a successful increment the mutation must be persisted in `state`.
        // Spec: "Finds/creates the NodeState for the TARGET node of that edge."
        // A stub that returns Ok(1) without actually updating state would fail.
        let graph = graph_with_rework(5);
        let mut state = make_state(); // target node `b` has no NodeState yet

        let _ = increment_rework_counter(&eid("rw1"), &mut state, &graph);

        let ns = state
            .node_states
            .get(&nid("b"))
            .expect("target node `b` must have a NodeState after increment");
        let count = ns
            .rework_edge_traversals
            .get(&eid("rw1"))
            .copied()
            .expect("rework_edge_traversals must contain the edge entry after increment");
        assert_eq!(
            count, 1,
            "rework_edge_traversals must reflect the incremented count"
        );
    }
}
