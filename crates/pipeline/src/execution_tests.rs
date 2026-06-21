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
        PipelineToolProfileConfig, ReworkEdge, ReworkSemantics, TimeoutSeconds, ValidationKind,
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
        activated_at: None,
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

        let (result, records) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);
        let record = records
            .first()
            .expect("must produce at least one audit record");

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

        let (result, _records) =
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

        let (result, records) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);
        let record = records
            .first()
            .expect("must produce at least one audit record");

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

        let (result, _records) =
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

        let (result, _records) =
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

        let (result, _records) =
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

        let (result, _records) =
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

        let (result, _records) =
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

        let (result, _records) =
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

        let (result, _records) =
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

        let (_result, records) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);
        let record = records
            .first()
            .expect("must produce at least one audit record");

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

        let (_result, records) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);
        let record = records
            .first()
            .expect("must produce at least one audit record");

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

        let (_result, records) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, fixed_ts);
        let record = records
            .first()
            .expect("must produce at least one audit record");

        assert_eq!(
            record.timestamp, fixed_ts,
            "EdgeEvaluationRecord.timestamp must equal the evaluated_at parameter, not a fresh now()"
        );
    }

    #[test]
    fn test_evaluate_edge_condition_composite_and_returns_inner_records() {
        // Composite And([false, unevaluated]) with short-circuit must still
        // return a record for the first (false) inner condition.
        // Audit constraint: every evaluated condition produces a record.
        let edge_id = eid("e-and-records");
        let state = state_with_active_node_a(); // node-a Active → first inner = false → short-circuit
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();
        let false_expr = Expression::new("node_states.node-a.status == 'Completed'").expect("expr");
        let true_expr = Expression::new("node_states.node-a.status == 'Completed'").expect("expr");

        let cond = EdgeConditionKind::Composite(CompositeCondition::And(vec![
            EdgeConditionKind::Deterministic(false_expr), // evaluates false → short-circuit
            EdgeConditionKind::Deterministic(true_expr),  // never evaluated
        ]));

        let (result, records) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(!result, "And([false, _]) must be false");
        // Root composite record + 1 inner record (short-circuit stops after first)
        assert!(
            records.len() >= 2,
            "must have root composite record plus at least one inner record; got {}",
            records.len()
        );
    }

    #[test]
    fn test_evaluate_edge_condition_composite_not_returns_inner_record() {
        // Composite Not must return the inner condition's record.
        let edge_id = eid("e-not-records");
        let state = state_with_completed_node_a();
        let output = make_output();
        let llm_results = HashMap::new();
        let ts = Timestamp::now();

        let cond = EdgeConditionKind::Composite(CompositeCondition::Not(Box::new(
            cond_true_when_node_a_completed(),
        )));

        let (result, records) =
            evaluate_edge_condition(&edge_id, &cond, &state, &output, &llm_results, ts);

        assert!(!result, "Not(true) must be false");
        // Root composite record + 1 inner record
        assert_eq!(
            records.len(),
            2,
            "Composite Not must produce root record + 1 inner record; got {}",
            records.len()
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

// ─── determine_next_actions ───────────────────────────────────────────────────

mod determine_next_actions_tests {
    use super::*;
    use crate::types::Timestamp;

    /// A fixed "now" timestamp used as the baseline in timeout tests.
    fn t0() -> Timestamp {
        Timestamp::from_utc(chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap())
    }

    /// A timestamp `secs` seconds AFTER `t0()`.
    fn t0_plus(secs: i64) -> Timestamp {
        Timestamp::from_utc(chrono::DateTime::from_timestamp(1_700_000_000 + secs, 0).unwrap())
    }

    /// Build a minimal graph with a single AutoProceed node and no edges.
    fn single_node_graph(id: &str) -> PipelineGraph {
        make_graph(vec![make_node(id)], vec![])
    }

    /// Build a graph with a single HumanGated node and no edges.
    fn single_gated_node_graph(id: &str) -> PipelineGraph {
        let mut node = make_node(id);
        node.gate = NodeGate::HumanGated;
        make_graph(vec![node], vec![])
    }

    /// Build a node with a specific timeout.
    fn make_node_with_timeout(id: &str, timeout_secs: u64) -> NodeDefinition {
        let mut n = make_node(id);
        n.timeout = Some(TimeoutSeconds(timeout_secs));
        n
    }

    /// Build a NodeState with `activated_at` set to a specific timestamp.
    fn make_active_state_at(activated_at: Timestamp) -> NodeState {
        NodeState {
            status: NodeStatus::Active,
            attempt_count: 1,
            rework_count: 0,
            current_error: None,
            rework_edge_traversals: HashMap::new(),
            activated_at: Some(activated_at),
        }
    }

    // ── Specification Tests (Tier 1) ─────────────────────────────────────────

    #[test]
    fn test_determine_next_actions_all_completed_returns_empty_vec() {
        // ASSERT: no eligible nodes and no active nodes → [] (run complete).
        // Vec-contents contract row: "No eligible + no active → []".
        // A stub returning [Wait] or [ExecuteNode] for a completed pipeline would fail.
        let graph = single_node_graph("a");
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Completed));
        let gate = GateConfig::default();
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert!(
            result.is_empty(),
            "all nodes Completed and none Active → must return empty vec (run complete)"
        );
    }

    #[test]
    fn test_determine_next_actions_single_autoproceed_eligible_returns_execute_node() {
        // ASSERT: single AutoProceed eligible node → [ExecuteNode(id)].
        // A stub always returning [] would fail.
        let graph = single_node_graph("a");
        let state = make_state(); // "a" is Pending → eligible
        let gate = GateConfig::default();
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert_eq!(result.len(), 1, "must return exactly one action");
        assert!(
            matches!(&result[0], NextAction::ExecuteNode(id) if *id == nid("a")),
            "single eligible AutoProceed node must produce ExecuteNode(a)"
        );
    }

    #[test]
    fn test_determine_next_actions_multiple_autoproceed_eligible_returns_execute_parallel() {
        // ASSERT: multiple AutoProceed eligible nodes → [ExecuteParallel(ids)].
        // A stub returning [ExecuteNode] for multi-eligible would fail.
        let graph = make_graph(vec![make_node("a"), make_node("b")], vec![]);
        let state = make_state(); // both Pending → both eligible
        let gate = GateConfig::default();
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert_eq!(result.len(), 1, "must return exactly one action");
        let ids = match &result[0] {
            NextAction::ExecuteParallel(ids) => ids.clone(),
            other => panic!("expected ExecuteParallel, got {other:?}"),
        };
        assert!(
            ids.contains(&nid("a")),
            "ExecuteParallel must include node a"
        );
        assert!(
            ids.contains(&nid("b")),
            "ExecuteParallel must include node b"
        );
    }

    #[test]
    fn test_determine_next_actions_human_gated_not_in_config_returns_wait() {
        // ASSERT: HumanGated eligible node absent from gate_config → [Wait].
        // A stub that ignores gate and proceeds would fail.
        let graph = single_gated_node_graph("gate-node");
        let state = make_state(); // gate-node is Pending → eligible
        let gate = GateConfig::default(); // empty — gate-node not present
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert_eq!(result.len(), 1, "must return exactly one action");
        assert!(
            matches!(&result[0], NextAction::Wait),
            "HumanGated node not in gate_config must return [Wait]"
        );
    }

    #[test]
    fn test_determine_next_actions_human_gated_approved_returns_execute_node() {
        // ASSERT: HumanGated eligible node with Approved status → [ExecuteNode].
        // A stub that always returns Wait for HumanGated would fail.
        let graph = single_gated_node_graph("gate-node");
        let state = make_state();
        let mut gate = GateConfig::default();
        gate.gated_nodes.insert(
            nid("gate-node"),
            GateStatus::Approved {
                approved_by: "reviewer".to_string(),
            },
        );
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert_eq!(result.len(), 1, "must return exactly one action");
        assert!(
            matches!(&result[0], NextAction::ExecuteNode(id) if *id == nid("gate-node")),
            "HumanGated node with Approved status must produce ExecuteNode"
        );
    }

    #[test]
    fn test_determine_next_actions_human_gated_rejected_returns_escalate() {
        // ASSERT: HumanGated eligible node with Rejected status → [Escalate].
        // A stub that returns Wait or ExecuteNode for Rejected would fail.
        let graph = single_gated_node_graph("gate-node");
        let state = make_state();
        let mut gate = GateConfig::default();
        gate.gated_nodes.insert(
            nid("gate-node"),
            GateStatus::Rejected {
                rejected_by: "reviewer".to_string(),
                reason: "quality insufficient".to_string(),
            },
        );
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert_eq!(result.len(), 1, "must return exactly one action");
        assert!(
            matches!(&result[0], NextAction::Escalate(_)),
            "HumanGated node with Rejected status must produce [Escalate]"
        );
    }

    #[test]
    fn test_determine_next_actions_escalate_carries_rejected_node_id() {
        // ASSERT: Escalate reason must reference the rejected node.
        // A stub populating EscalationReason with a wrong node_id would fail.
        let graph = single_gated_node_graph("gate-node");
        let state = make_state();
        let mut gate = GateConfig::default();
        gate.gated_nodes.insert(
            nid("gate-node"),
            GateStatus::Rejected {
                rejected_by: "reviewer".to_string(),
                reason: "quality insufficient".to_string(),
            },
        );
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        let reason = match &result[0] {
            NextAction::Escalate(r) => r,
            other => panic!("expected Escalate, got {other:?}"),
        };
        assert_eq!(
            reason.node_id,
            nid("gate-node"),
            "EscalationReason.node_id must match the rejected node"
        );
    }

    // ── Timeout Tests ────────────────────────────────────────────────────────

    #[test]
    fn test_determine_next_actions_active_node_timeout_elapsed_returns_halt() {
        // ASSERT: Active node activated_at t0, timeout 10s, now = t0+11 → [HaltWithError].
        // A stub that never checks timeout would fail.
        let timeout_secs = 10u64;
        let graph = make_graph(vec![make_node_with_timeout("timed", timeout_secs)], vec![]);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("timed"), make_active_state_at(t0()));
        let gate = GateConfig::default();
        let now = t0_plus(timeout_secs as i64 + 1); // elapsed > timeout

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert_eq!(result.len(), 1, "must return exactly one action");
        assert!(
            matches!(
                &result[0],
                NextAction::HaltWithError(PipelineError::NodeFailed { .. })
            ),
            "elapsed timeout must produce HaltWithError(NodeFailed)"
        );
    }

    #[test]
    fn test_determine_next_actions_active_node_at_timeout_boundary_returns_halt() {
        // ASSERT: now - activated_at == timeout exactly → still a timeout (> is wrong; rule is >).
        // Timeout detection rule: now - activated_at > timeout → halt. At boundary (==) → no halt.
        // This test verifies the boundary is EXCLUSIVE (>) not inclusive (>=).
        let timeout_secs = 10u64;
        let graph = make_graph(vec![make_node_with_timeout("timed", timeout_secs)], vec![]);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("timed"), make_active_state_at(t0()));
        let gate = GateConfig::default();
        let now = t0_plus(timeout_secs as i64); // elapsed == timeout exactly

        let result = determine_next_actions(&state, &graph, &gate, now);

        // Spec rule: now - activated_at > timeout → HaltWithError.
        // At the boundary (==), it's NOT greater → should NOT halt.
        assert!(
            !matches!(&result.first(), Some(NextAction::HaltWithError(_))),
            "elapsed == timeout (boundary) must NOT produce HaltWithError; rule is strictly >"
        );
    }

    #[test]
    fn test_determine_next_actions_active_node_no_timeout_configured_no_halt() {
        // ASSERT: Active node with no node-level timeout and no pipeline default → no timeout halt.
        // A stub that halts unconditionally for Active nodes would fail.
        let graph = make_graph(vec![make_node("running")], vec![]); // no timeout
        let mut state = make_state();
        state
            .node_states
            .insert(nid("running"), make_active_state_at(t0()));
        let gate = GateConfig::default();
        let now = t0_plus(99999); // far future — still no timeout

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert!(
            !result
                .iter()
                .any(|a| matches!(a, NextAction::HaltWithError(_))),
            "Active node with None timeout must never produce HaltWithError"
        );
    }

    #[test]
    fn test_determine_next_actions_active_node_no_activated_at_no_halt() {
        // ASSERT: Active node with activated_at = None → no timeout check → no halt.
        // A stub that halts when activated_at is None would fail.
        let graph = make_graph(vec![make_node_with_timeout("running", 5)], vec![]);
        let mut state = make_state();
        // activated_at is None — timeout cannot be computed
        state
            .node_states
            .insert(nid("running"), make_node_state(NodeStatus::Active));
        let gate = GateConfig::default();
        let now = t0_plus(99999);

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert!(
            !result
                .iter()
                .any(|a| matches!(a, NextAction::HaltWithError(_))),
            "Active node with activated_at = None must not trigger timeout halt"
        );
    }

    #[test]
    fn test_determine_next_actions_node_timeout_overrides_pipeline_default() {
        // ASSERT: Node-level timeout takes precedence over the pipeline default.
        // Pipeline default = 100s, node-level = 5s; elapsed = 6s → halt uses node timeout.
        // A stub using only pipeline default would fail to halt here.
        let mut node = make_node("n");
        node.timeout = Some(TimeoutSeconds(5)); // node-level: 5s
        let mut graph = make_graph(vec![node], vec![]);
        graph.settings.default_timeout = Some(TimeoutSeconds(100)); // pipeline default: 100s
        let mut state = make_state();
        state
            .node_states
            .insert(nid("n"), make_active_state_at(t0()));
        let gate = GateConfig::default();
        let now = t0_plus(6); // 6s elapsed; > 5s (node) but < 100s (pipeline)

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert!(
            matches!(
                result.first(),
                Some(NextAction::HaltWithError(PipelineError::NodeFailed { .. }))
            ),
            "node-level timeout (5s) must override pipeline default (100s); 6s elapsed must halt"
        );
    }

    #[test]
    fn test_determine_next_actions_pipeline_default_timeout_applies_when_no_node_timeout() {
        // ASSERT: Pipeline-level default_timeout applies when node has no timeout set.
        // A stub ignoring pipeline default would fail to halt here.
        let graph_with_default = {
            let node = make_node("n"); // no node-level timeout
            let mut g = make_graph(vec![node], vec![]);
            g.settings.default_timeout = Some(TimeoutSeconds(10));
            g
        };
        let mut state = make_state();
        state
            .node_states
            .insert(nid("n"), make_active_state_at(t0()));
        let gate = GateConfig::default();
        let now = t0_plus(11); // elapsed > pipeline default

        let result = determine_next_actions(&state, &graph_with_default, &gate, now);

        assert!(
            matches!(
                result.first(),
                Some(NextAction::HaltWithError(PipelineError::NodeFailed { .. }))
            ),
            "pipeline default_timeout must apply when node has no timeout; elapsed 11s > 10s"
        );
    }

    // ── Fan-in Tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_determine_next_actions_fan_in_predecessor_not_completed_excluded() {
        // ASSERT: Fan-in node with an incomplete predecessor → excluded from execute set → [Wait].
        // Graph: a → c, b → c. b is Active (not Completed). c must not be executed.
        // A stub that ignores fan-in would include c and return ExecuteNode(c).
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "c"), make_edge("e2", "b", "c")];
        let graph = make_graph(nodes, edges);
        let mut state = make_state();
        // a is Completed, b is Active — c is eligible topologically but fan-in not ready
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Completed));
        state
            .node_states
            .insert(nid("b"), make_node_state(NodeStatus::Active));
        let gate = GateConfig::default();
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        // c must NOT be in any execute action
        for action in &result {
            match action {
                NextAction::ExecuteNode(id) => {
                    assert_ne!(
                        *id,
                        nid("c"),
                        "fan-in node c must not be executed when b is Active"
                    );
                }
                NextAction::ExecuteParallel(ids) => {
                    assert!(
                        !ids.contains(&nid("c")),
                        "fan-in node c must not be in ExecuteParallel when b is Active"
                    );
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_determine_next_actions_fan_in_all_predecessors_completed_included() {
        // ASSERT: Fan-in node with all predecessors Completed → included in execute set.
        // Graph: a → c, b → c. Both a and b Completed → c must execute.
        // A stub that always excludes fan-in nodes would fail.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "c"), make_edge("e2", "b", "c")];
        let graph = make_graph(nodes, edges);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Completed));
        state
            .node_states
            .insert(nid("b"), make_node_state(NodeStatus::Completed));
        let gate = GateConfig::default();
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        let executes_c = result.iter().any(|a| match a {
            NextAction::ExecuteNode(id) => *id == nid("c"),
            NextAction::ExecuteParallel(ids) => ids.contains(&nid("c")),
            _ => false,
        });
        assert!(
            executes_c,
            "fan-in node c must be in execute set when all predecessors are Completed"
        );
    }

    // ── Mix Tests ────────────────────────────────────────────────────────────

    #[test]
    fn test_determine_next_actions_mix_auto_and_gated_returns_only_execute_no_wait() {
        // ASSERT: Mix of AutoProceed eligible + HumanGated-waiting → only execute actions.
        // Wait must NOT be co-returned when execute actions exist.
        // Graph: two independent nodes: "auto" (AutoProceed) and "gated" (HumanGated, no gate entry).
        let auto_node = make_node("auto"); // AutoProceed
        let mut gated_node = make_node("gated");
        gated_node.gate = NodeGate::HumanGated;
        let graph = make_graph(vec![auto_node, gated_node], vec![]);
        let state = make_state(); // both Pending → both eligible
        let gate = GateConfig::default(); // "gated" not approved yet
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        let has_wait = result.iter().any(|a| matches!(a, NextAction::Wait));
        let has_execute = result.iter().any(|a| {
            matches!(a, NextAction::ExecuteNode(id) if *id == nid("auto"))
                || matches!(a, NextAction::ExecuteParallel(ids) if ids.contains(&nid("auto")))
        });

        assert!(
            !has_wait,
            "Wait must NOT be co-returned when auto-proceed execute actions are present"
        );
        assert!(
            has_execute,
            "AutoProceed eligible node must be included in execute set"
        );
    }

    // ── No-Eligible, Active-Running Tests ────────────────────────────────────

    #[test]
    fn test_determine_next_actions_no_eligible_active_nodes_returns_wait() {
        // ASSERT: No eligible nodes (all downstream blocked) but active nodes exist → [Wait].
        // Spec step 7: "No eligible and no active → []". Implicit: active but no eligible → [Wait].
        // A stub returning [] when active nodes exist would fail.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b")];
        let graph = make_graph(nodes, edges);
        let mut state = make_state();
        // a is Active; b has unmet predecessor (a not Completed) → no eligible nodes
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Active));
        let gate = GateConfig::default();
        let now = t0();

        let result = determine_next_actions(&state, &graph, &gate, now);

        assert_eq!(result.len(), 1, "must return exactly one action");
        assert!(
            matches!(&result[0], NextAction::Wait),
            "no eligible nodes with active nodes running must return [Wait]"
        );
    }

    // ── Halt Contains NodeFailed Variant ─────────────────────────────────────

    #[test]
    fn test_determine_next_actions_halt_error_carries_timed_out_node_id() {
        // ASSERT: HaltWithError(NodeFailed) must reference the timed-out node.
        // A stub populating NodeFailed with a wrong node_id would fail.
        let graph = make_graph(vec![make_node_with_timeout("slow", 5)], vec![]);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("slow"), make_active_state_at(t0()));
        let gate = GateConfig::default();
        let now = t0_plus(10);

        let result = determine_next_actions(&state, &graph, &gate, now);

        let node_id = match &result[0] {
            NextAction::HaltWithError(PipelineError::NodeFailed { node_id, .. }) => node_id.clone(),
            other => panic!("expected HaltWithError(NodeFailed), got {other:?}"),
        };
        assert_eq!(
            node_id,
            nid("slow"),
            "NodeFailed.node_id must identify the timed-out node"
        );
    }

    #[test]
    fn test_determine_next_actions_multiple_timed_out_nodes_reports_first_activated() {
        // ASSERT: When multiple nodes timeout simultaneously, the implementation
        // deterministically reports the first-activated node (earliest activated_at).
        // This ensures consistent audit trails and predictable test behaviour.
        let graph = make_graph(
            vec![
                make_node_with_timeout("slow-a", 5),
                make_node_with_timeout("slow-b", 5),
                make_node_with_timeout("slow-c", 5),
            ],
            vec![],
        );
        let mut state = make_state();
        // Activate nodes in reverse order: c, b, a
        // Verify we report 'a' (the earliest-activated) despite iteration order
        state
            .node_states
            .insert(nid("slow-c"), make_active_state_at(t0_plus(2)));
        state
            .node_states
            .insert(nid("slow-b"), make_active_state_at(t0_plus(1)));
        state
            .node_states
            .insert(nid("slow-a"), make_active_state_at(t0())); // earliest
        let gate = GateConfig::default();
        let now = t0_plus(10); // all three are timed out (10 > 5)

        let result = determine_next_actions(&state, &graph, &gate, now);

        let node_id = match &result[0] {
            NextAction::HaltWithError(PipelineError::NodeFailed { node_id, .. }) => node_id.clone(),
            other => panic!("expected HaltWithError(NodeFailed), got {other:?}"),
        };
        assert_eq!(
            node_id,
            nid("slow-a"),
            "When multiple nodes timeout, must report first-activated (slow-a at t0) for deterministic audit trail"
        );
    }
}
