//! Adversarial test suite for `graph.rs` pure business-logic functions.
//!
//! Tests are derived from `docs/spec/interfaces/pipeline-graph.md` and the
//! behavioral assertions in `docs/spec/assertions.md` (ASSERT-PSM-002,
//! ASSERT-PSM-008).
//!
//! All four functions are implemented; every test below is expected to pass (GREEN).
//!
//! ## Coverage targets
//! - `topological_sort`              ≥ 7 tests (Tier 1 + 2)
//! - `evaluate_deterministic_condition` ≥ 8 tests (Tier 1 + 2)
//! - `validate_pipeline_graph`       ≥ 12 tests (Tier 1 + 2)
//! - `compute_eligible_nodes`        ≥ 7 tests (Tier 1 + 2)

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use std::collections::HashMap;

use super::*;
use crate::{EdgeId, NodeId, PipelineRunId, ProfileName, TokenCost};

// ─── Test helpers ────────────────────────────────────────────────────────────

fn nid(s: &str) -> NodeId {
    NodeId::new(s).expect("test helper: node id must not be empty")
}

fn eid(s: &str) -> EdgeId {
    EdgeId::new(s).expect("test helper: edge id must not be empty")
}

/// Minimal valid [`NodeDefinition`] with Deterministic type and no gates.
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
fn make_rework_edge(id: &str, src: &str, tgt: &str, max_traversals: u32) -> EdgeDefinition {
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

/// Empty [`PipelineState`] — all nodes default to [`NodeStatus::Pending`].
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

// ─── topological_sort ────────────────────────────────────────────────────────

mod topological_sort_tests {
    use super::*;

    // ASSERT-PSM-002: sources must precede sinks so downstream nodes can be
    // activated in the correct order.

    #[test]
    fn test_topological_sort_empty_nodes_returns_empty_vec() {
        let result = topological_sort(&[], &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![]);
    }

    #[test]
    fn test_topological_sort_single_node_no_edges_returns_that_node() {
        let nodes = vec![make_node("a")];
        let result = topological_sort(&nodes, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![nid("a")]);
    }

    #[test]
    fn test_topological_sort_linear_chain_returns_sources_first() {
        // A → B → C — the canonical ordering requirement.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "b", "c")];

        let sorted = topological_sort(&nodes, &edges).expect("linear chain must not cycle");

        let pos = |id: &str| {
            sorted
                .iter()
                .position(|n| n == &nid(id))
                .expect("node must appear in sorted output")
        };
        assert!(pos("a") < pos("b"), "a must precede b");
        assert!(pos("b") < pos("c"), "b must precede c");
    }

    #[test]
    fn test_topological_sort_fan_out_source_before_both_targets() {
        // A → B and A → C: A must appear before both B and C.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "a", "c")];

        let sorted = topological_sort(&nodes, &edges).expect("fan-out must not cycle");

        let pos = |id: &str| {
            sorted
                .iter()
                .position(|n| n == &nid(id))
                .expect("node must appear in sorted output")
        };
        assert!(pos("a") < pos("b"), "source a must precede b");
        assert!(pos("a") < pos("c"), "source a must precede c");
    }

    #[test]
    fn test_topological_sort_fan_in_both_predecessors_before_shared_target() {
        // A → C and B → C: both A and B must appear before C.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "c"), make_edge("e2", "b", "c")];

        let sorted = topological_sort(&nodes, &edges).expect("fan-in must not cycle");

        let pos = |id: &str| {
            sorted
                .iter()
                .position(|n| n == &nid(id))
                .expect("node must appear in sorted output")
        };
        assert!(
            pos("a") < pos("c"),
            "predecessor a must precede shared target c"
        );
        assert!(
            pos("b") < pos("c"),
            "predecessor b must precede shared target c"
        );
    }

    #[test]
    fn test_topological_sort_rework_edge_excluded_forward_subgraph_succeeds() {
        // A →rework→ B (back-edge, excluded), B → C (forward).
        // Forward subgraph is B → C with A isolated. Must succeed without a cycle
        // and B must precede C.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![
            make_rework_edge("e-rework", "a", "b", 1),
            make_edge("e-fwd", "b", "c"),
        ];

        let sorted =
            topological_sort(&nodes, &edges).expect("rework back-edge must not cause CycleError");

        let pos_b = sorted
            .iter()
            .position(|n| n == &nid("b"))
            .expect("b must appear");
        let pos_c = sorted
            .iter()
            .position(|n| n == &nid("c"))
            .expect("c must appear");
        assert!(pos_b < pos_c, "b must precede c in the forward subgraph");
    }

    #[test]
    fn test_topological_sort_forward_cycle_returns_cycle_error() {
        // A → B and B → A: a directed forward cycle — no rework edges.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "b", "a")];

        let result = topological_sort(&nodes, &edges);

        assert!(result.is_err(), "forward cycle must return CycleError");
    }

    #[test]
    fn test_topological_sort_forward_cycle_error_names_cycle_nodes() {
        // CycleError::cycle must contain the node IDs involved in the cycle.
        // A stub returning any CycleError without the cycle nodes would fail this.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "b", "a")];

        let err = topological_sort(&nodes, &edges).unwrap_err();

        let names_at_least_one_cycle_member =
            err.cycle.contains(&nid("a")) || err.cycle.contains(&nid("b"));
        assert!(
            names_at_least_one_cycle_member,
            "CycleError.cycle must include nodes forming the cycle; got {:?}",
            err.cycle
        );
    }
}

// ─── evaluate_deterministic_condition ────────────────────────────────────────

mod evaluate_deterministic_condition_tests {
    use super::*;

    // Spec: "Unknown field references evaluate to false."
    // Spec: "Evaluation is pure (no side effects)."

    fn state_with_node_status(id: &str, status: NodeStatus) -> PipelineState {
        let mut s = make_state();
        s.node_states.insert(nid(id), make_node_state(status));
        s
    }

    fn state_with_node_error(id: &str, error: &str) -> PipelineState {
        let mut s = make_state();
        let mut ns = make_node_state(NodeStatus::Failed);
        ns.current_error = Some(error.to_string());
        s.node_states.insert(nid(id), ns);
        s
    }

    #[test]
    fn test_evaluate_deterministic_condition_equality_matching_value_returns_true() {
        // node_states["node-a"].status == "Completed" where status IS Completed.
        // A stub always returning false would fail here.
        let state = state_with_node_status("node-a", NodeStatus::Completed);
        let expr =
            Expression::new("node_states.node-a.status == \"Completed\"").expect("valid expr");

        assert!(evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_equality_differing_value_returns_false() {
        // node_states["node-a"].status == "Completed" where status is Active.
        // A stub always returning true would fail here.
        let state = state_with_node_status("node-a", NodeStatus::Active);
        let expr =
            Expression::new("node_states.node-a.status == \"Completed\"").expect("valid expr");

        assert!(!evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_unknown_path_returns_false() {
        // Spec mandates: unknown field reference → false.
        let state = make_state();
        let expr =
            Expression::new("completely_unknown_field.nested == \"value\"").expect("valid expr");

        assert!(!evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_deeply_nested_path_returns_correct_result() {
        // Three-level navigation: node_states → node-a → current_error.
        // Validates that the evaluator descends into nested JSON objects.
        let state = state_with_node_error("node-a", "timed out");
        let expr = Expression::new("node_states.node-a.current_error == \"timed out\"")
            .expect("valid expr");

        assert!(evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_inequality_different_values_returns_true() {
        // status is Active, expression checks != "Completed" → should be true.
        let state = state_with_node_status("node-a", NodeStatus::Active);
        let expr =
            Expression::new("node_states.node-a.status != \"Completed\"").expect("valid expr");

        assert!(evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_inequality_same_value_returns_false() {
        // status IS Active, expression checks != "Active" → should be false.
        // Kills a stub that always returns true for !=.
        let state = state_with_node_status("node-a", NodeStatus::Active);
        let expr = Expression::new("node_states.node-a.status != \"Active\"").expect("valid expr");

        assert!(!evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_boolean_true_literal_against_string_returns_false() {
        // The status field serialises as a string ("Completed"); comparing it with
        // the boolean literal `true` must be a type mismatch → false.
        // Verifies the evaluator parses `true` as a boolean, not a string.
        let state = state_with_node_status("node-a", NodeStatus::Completed);
        let expr = Expression::new("node_states.node-a.status == true").expect("valid expr");

        assert!(!evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_boolean_false_literal_against_string_returns_false() {
        // Same as above but with `false`.  A string value != false (type mismatch).
        let state = state_with_node_status("node-a", NodeStatus::Active);
        let expr = Expression::new("node_states.node-a.status == false").expect("valid expr");

        assert!(!evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_malformed_expression_no_operator_returns_false() {
        // Spec: "parse failure → false".
        let state = make_state();
        let expr =
            Expression::new("this_is_not_a_valid_expression_no_operator").expect("non-empty");

        assert!(!evaluate_deterministic_condition(&expr, &state));
    }

    #[test]
    fn test_evaluate_deterministic_condition_single_quoted_string_literal_matches() {
        // Spec supports single-quoted string literals in addition to double-quoted.
        let state = state_with_node_status("node-a", NodeStatus::Completed);
        let expr = Expression::new("node_states.node-a.status == 'Completed'").expect("valid expr");

        assert!(evaluate_deterministic_condition(&expr, &state));
    }
}

// ─── validate_pipeline_graph ─────────────────────────────────────────────────

mod validate_pipeline_graph_tests {
    use super::*;

    // Spec: all 10 checks must run — errors are collected, not short-circuited.

    #[test]
    fn test_validate_pipeline_graph_valid_linear_graph_returns_ok() {
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "b", "c")];
        let graph = make_graph(nodes, edges);

        assert!(validate_pipeline_graph(&graph).is_ok());
    }

    #[test]
    fn test_validate_pipeline_graph_valid_rework_cycle_returns_ok() {
        // A → B (forward), B → A (rework, max_traversals = 1).
        // Forward subgraph has no cycle; rework edge terminates the loop.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![
            make_edge("e-fwd", "a", "b"),
            make_rework_edge("e-rework", "b", "a", 1),
        ];
        let graph = make_graph(nodes, edges);

        assert!(validate_pipeline_graph(&graph).is_ok());
    }

    #[test]
    fn test_validate_pipeline_graph_empty_graph_returns_empty_graph_error() {
        let graph = make_graph(vec![], vec![]);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|e| matches!(e, GraphValidationError::EmptyGraph)),
            "expected EmptyGraph; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_duplicate_node_ids_returns_duplicate_node_id_error() {
        // Two nodes with the same id "a".
        let nodes = vec![make_node("a"), make_node("b"), make_node("a")];
        let edges = vec![make_edge("e1", "a", "b")];
        let graph = make_graph(nodes, edges);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors.iter().any(
                |e| matches!(e, GraphValidationError::DuplicateNodeId { id } if id == &nid("a"))
            ),
            "expected DuplicateNodeId for 'a'; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_duplicate_edge_ids_returns_duplicate_edge_id_error() {
        // Two edges with the same id "e1".
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![
            make_edge("e1", "a", "b"),
            make_edge("e1", "b", "c"), // same id
        ];
        let graph = make_graph(nodes, edges);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors.iter().any(
                |e| matches!(e, GraphValidationError::DuplicateEdgeId { id } if id == &eid("e1"))
            ),
            "expected DuplicateEdgeId for 'e1'; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_edge_with_undeclared_source_returns_unknown_node_error() {
        // Edge source "ghost" is not in the node list.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "ghost", "b")];
        let graph = make_graph(nodes, edges);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|e| matches!(e, GraphValidationError::UnknownNode { node, .. } if node == &nid("ghost"))),
            "expected UnknownNode for 'ghost'; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_edge_with_undeclared_target_returns_unknown_node_error() {
        // Edge target "phantom" is not in the node list.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "phantom")];
        let graph = make_graph(nodes, edges);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|e| matches!(e, GraphValidationError::UnknownNode { node, .. } if node == &nid("phantom"))),
            "expected UnknownNode for 'phantom'; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_orphan_node_returns_orphan_node_error() {
        // "orphan" has no edges; "a" and "b" are connected.
        let nodes = vec![make_node("a"), make_node("b"), make_node("orphan")];
        let edges = vec![make_edge("e1", "a", "b")];
        let graph = make_graph(nodes, edges);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors.iter().any(
                |e| matches!(e, GraphValidationError::OrphanNode { node } if node == &nid("orphan"))
            ),
            "expected OrphanNode for 'orphan'; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_rework_edge_zero_max_traversals_returns_invalid_max_traversals()
    {
        // max_traversals == 0 is explicitly forbidden (spec invariant: must be ≥ 1).
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![
            make_edge("e-fwd", "a", "b"),
            make_rework_edge("e-rework", "b", "a", 0), // invalid: 0
        ];
        let graph = make_graph(nodes, edges);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|e| matches!(e, GraphValidationError::InvalidMaxTraversals { edge } if edge == &eid("e-rework"))),
            "expected InvalidMaxTraversals for 'e-rework'; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_forward_cycle_returns_unterminated_cycle_error() {
        // A → B (forward), B → A (forward): a cycle with no rework edge.
        // Every cycle must pass through at least one rework edge — this one does not.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "b", "a")];
        let graph = make_graph(nodes, edges);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors
                .iter()
                .any(|e| matches!(e, GraphValidationError::UnterminatedCycle { .. })),
            "expected UnterminatedCycle; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_explicit_mode_without_edge_list_returns_error() {
        // Node "a" has EvaluationMode::Explicit in evaluation_modes but is
        // absent from explicit_edge_lists → ExplicitModeWithoutEdgeList.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b")];
        let mut graph = make_graph(nodes, edges);
        graph
            .evaluation_modes
            .insert(nid("a"), EvaluationMode::Explicit);
        // deliberately NOT inserting into explicit_edge_lists

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors.iter().any(
                |e| matches!(e, GraphValidationError::ExplicitModeWithoutEdgeList { node } if node == &nid("a"))
            ),
            "expected ExplicitModeWithoutEdgeList for 'a'; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_explicit_mode_with_edge_list_present_returns_ok() {
        // When explicit_edge_lists contains the node, the graph is valid.
        // Kills an implementation that always rejects Explicit mode.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b")];
        let mut graph = make_graph(nodes, edges);
        graph
            .evaluation_modes
            .insert(nid("a"), EvaluationMode::Explicit);
        graph.explicit_edge_lists.insert(nid("a"), vec![eid("e1")]);

        assert!(validate_pipeline_graph(&graph).is_ok());
    }

    #[test]
    fn test_validate_pipeline_graph_multiple_errors_both_collected_not_short_circuited() {
        // Duplicate node "a" (appears twice) AND an orphan node.
        // Both violations must appear in the returned error Vec, proving that
        // validation does not stop at the first error.
        let nodes = vec![
            make_node("a"),
            make_node("b"),
            make_node("a"), // duplicate
            make_node("orphan"),
        ];
        let edges = vec![make_edge("e1", "a", "b")];
        let graph = make_graph(nodes, edges);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        let has_duplicate = errors
            .iter()
            .any(|e| matches!(e, GraphValidationError::DuplicateNodeId { .. }));
        let has_orphan = errors
            .iter()
            .any(|e| matches!(e, GraphValidationError::OrphanNode { .. }));

        assert!(
            has_duplicate,
            "DuplicateNodeId must be collected; errors: {errors:?}"
        );
        assert!(
            has_orphan,
            "OrphanNode must be collected alongside DuplicateNodeId; errors: {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_overflow_take_edge_references_unknown_edge_returns_error() {
        // Rework edge with OverflowBehaviour::TakeEdge("no-such-edge").
        // The overflow target does not exist → UnknownOverflowEdge.
        let nodes = vec![make_node("a"), make_node("b")];
        let fwd = make_edge("e-fwd", "a", "b");
        let mut rework = make_rework_edge("e-rework", "b", "a", 2);
        if let Some(ref mut r) = rework.rework_edge {
            r.overflow_behaviour =
                OverflowBehaviour::TakeEdge(EdgeId::new("no-such-edge").expect("test edge id"));
        }
        let graph = make_graph(nodes, vec![fwd, rework]);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors.iter().any(|e| matches!(
                e,
                GraphValidationError::UnknownOverflowEdge { overflow_edge, .. }
                    if overflow_edge == &EdgeId::new("no-such-edge").unwrap()
            )),
            "expected UnknownOverflowEdge; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_overflow_take_edge_references_known_edge_returns_ok() {
        // OverflowBehaviour::TakeEdge("e-fwd") where "e-fwd" IS declared → valid.
        let nodes = vec![make_node("a"), make_node("b")];
        let fwd = make_edge("e-fwd", "a", "b");
        let mut rework = make_rework_edge("e-rework", "b", "a", 2);
        if let Some(ref mut r) = rework.rework_edge {
            r.overflow_behaviour =
                OverflowBehaviour::TakeEdge(EdgeId::new("e-fwd").expect("test edge id"));
        }
        let graph = make_graph(nodes, vec![fwd, rework]);

        assert!(
            validate_pipeline_graph(&graph).is_ok(),
            "TakeEdge referencing a declared edge must be valid"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_same_slot_name_in_inputs_and_outputs_is_valid() {
        // Pass-through node: reads and writes the same artifact name.
        // The check is per-list; a name appearing in both lists is permitted.
        let mut node = make_node("a");
        node.declared_inputs = vec!["code_diff".to_string()];
        node.declared_outputs = vec!["code_diff".to_string()];
        let nodes = vec![node, make_node("b")];
        let graph = make_graph(nodes, vec![make_edge("e1", "a", "b")]);

        assert!(
            validate_pipeline_graph(&graph).is_ok(),
            "same slot name in inputs and outputs must not be flagged as a duplicate"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_duplicate_slot_name_in_inputs_returns_error() {
        // Node declares the same input slot twice (intra-list duplicate) → DuplicateSlotName.
        let mut node = make_node("a");
        node.declared_inputs = vec!["artifact".to_string(), "artifact".to_string()];
        let nodes = vec![node, make_node("b")];
        let graph = make_graph(nodes, vec![make_edge("e1", "a", "b")]);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors.iter().any(|e| matches!(
                e,
                GraphValidationError::DuplicateSlotName { node, slot }
                    if node == &nid("a") && slot == "artifact"
            )),
            "expected DuplicateSlotName for 'artifact' on node 'a'; got {errors:?}"
        );
    }

    #[test]
    fn test_validate_pipeline_graph_duplicate_slot_name_in_outputs_returns_error() {
        // Node declares the same output slot twice (intra-list duplicate) → DuplicateSlotName.
        let mut node = make_node("a");
        node.declared_outputs = vec!["result".to_string(), "result".to_string()];
        let nodes = vec![node, make_node("b")];
        let graph = make_graph(nodes, vec![make_edge("e1", "a", "b")]);

        let errors = validate_pipeline_graph(&graph).unwrap_err();

        assert!(
            errors.iter().any(|e| matches!(
                e,
                GraphValidationError::DuplicateSlotName { node, slot }
                    if node == &nid("a") && slot == "result"
            )),
            "expected DuplicateSlotName for 'result' on node 'a'; got {errors:?}"
        );
    }
}

// ─── compute_eligible_nodes ──────────────────────────────────────────────────

mod compute_eligible_nodes_tests {
    use super::*;

    // Spec: a node is eligible iff its NodeStatus is Pending AND all upstream
    // nodes via non-rework forward edges have NodeStatus::Completed.
    // Gate status is NOT evaluated here.
    // Nodes absent from node_states default to Pending.

    #[test]
    fn test_compute_eligible_nodes_pending_source_node_is_eligible() {
        // A → B; empty state (both default to Pending).
        // A has no forward predecessors → A is eligible.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b")];
        let graph = make_graph(nodes, edges);
        let state = make_state();

        let eligible = compute_eligible_nodes(&state, &graph);

        assert!(
            eligible.contains(&nid("a")),
            "source node a must be eligible in empty state"
        );
    }

    #[test]
    fn test_compute_eligible_nodes_completed_predecessor_makes_successor_eligible() {
        // A → B; A is Completed → B's predecessor constraint is satisfied → B eligible.
        // A stub returning an empty Vec would fail this.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b")];
        let graph = make_graph(nodes, edges);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Completed));

        let eligible = compute_eligible_nodes(&state, &graph);

        assert!(
            eligible.contains(&nid("b")),
            "b must be eligible when its only predecessor a is Completed"
        );
    }

    #[test]
    fn test_compute_eligible_nodes_active_predecessor_prevents_eligibility() {
        // A → B; A is Active (not Completed) → B must NOT be eligible.
        // A stub returning all Pending nodes as eligible would fail this.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b")];
        let graph = make_graph(nodes, edges);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Active));

        let eligible = compute_eligible_nodes(&state, &graph);

        assert!(
            !eligible.contains(&nid("b")),
            "b must NOT be eligible when predecessor a is Active"
        );
    }

    #[test]
    fn test_compute_eligible_nodes_fan_in_one_incomplete_predecessor_blocks_target() {
        // A → C and B → C (fan-in); A is Completed, B is still Pending.
        // C requires BOTH predecessors to be Completed → C must not be eligible.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![make_edge("e1", "a", "c"), make_edge("e2", "b", "c")];
        let graph = make_graph(nodes, edges);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Completed));
        // b remains absent → Pending

        let eligible = compute_eligible_nodes(&state, &graph);

        assert!(
            !eligible.contains(&nid("c")),
            "c must NOT be eligible: predecessor b is not Completed"
        );
    }

    #[test]
    fn test_compute_eligible_nodes_active_node_not_returned() {
        // A node that is Active is not Pending → must never appear in eligible list.
        let nodes = vec![make_node("a")];
        let edges = vec![];
        let graph = make_graph(nodes, edges);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Active));

        let eligible = compute_eligible_nodes(&state, &graph);

        assert!(
            !eligible.contains(&nid("a")),
            "Active node a must NOT be in the eligible list"
        );
    }

    #[test]
    fn test_compute_eligible_nodes_rework_predecessor_does_not_block_eligibility() {
        // A →rework→ B (back-edge), B → C (forward).
        // A is Active. B's only predecessor is via a rework edge (excluded from
        // eligibility checks) → B has no forward predecessors → B is eligible.
        let nodes = vec![make_node("a"), make_node("b"), make_node("c")];
        let edges = vec![
            make_rework_edge("e-rework", "a", "b", 2),
            make_edge("e-fwd", "b", "c"),
        ];
        let graph = make_graph(nodes, edges);
        let mut state = make_state();
        state
            .node_states
            .insert(nid("a"), make_node_state(NodeStatus::Active));

        let eligible = compute_eligible_nodes(&state, &graph);

        assert!(
            eligible.contains(&nid("b")),
            "b must be eligible: rework predecessor a is not a forward predecessor"
        );
    }

    #[test]
    fn test_compute_eligible_nodes_empty_state_only_source_nodes_are_eligible() {
        // Two independent chains A → B and C → D; state is empty.
        // Source nodes A and C are eligible (no predecessors).
        // B and D have Pending predecessors → not eligible.
        let nodes = vec![
            make_node("a"),
            make_node("b"),
            make_node("c"),
            make_node("d"),
        ];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "c", "d")];
        let graph = make_graph(nodes, edges);
        let state = make_state();

        let eligible = compute_eligible_nodes(&state, &graph);

        assert!(
            eligible.contains(&nid("a")),
            "source a must be eligible in empty state"
        );
        assert!(
            eligible.contains(&nid("c")),
            "source c must be eligible in empty state"
        );
        assert!(
            !eligible.contains(&nid("b")),
            "b must NOT be eligible: predecessor a is Pending"
        );
        assert!(
            !eligible.contains(&nid("d")),
            "d must NOT be eligible: predecessor c is Pending"
        );
    }
}

// ─── topological_sort kill tests (surviving mutants) ─────────────────────────

mod topological_sort_kill_tests {
    use super::*;

    /// Kills line 700:44 mutant: `&&` → `||` in the forward-edge guard.
    ///
    /// When an edge references a source node that is not in the node list the
    /// guard prevents that edge from being added to the adjacency/in-degree
    /// maps.  With the `||` mutant, the unknown source is inserted into the
    /// adjacency map and the known target's in-degree is incremented, so the
    /// known target never reaches in-degree 0 and the sort incorrectly reports
    /// a cycle.
    #[test]
    fn test_topological_sort_edge_with_unknown_source_is_silently_excluded() {
        // "ghost" is not declared in nodes; the edge "ghost" → "a" must be
        // ignored so that A → B sorts cleanly.
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "ghost", "a")];

        let result = topological_sort(&nodes, &edges);

        assert!(
            result.is_ok(),
            "edge with unknown source must be silently excluded; got {result:?}"
        );
        let sorted = result.unwrap();
        assert_eq!(
            sorted.len(),
            2,
            "result must contain exactly the 2 known nodes"
        );
        assert!(sorted.contains(&nid("a")));
        assert!(sorted.contains(&nid("b")));
    }

    /// Companion to the above: unknown *target* is equally excluded.
    #[test]
    fn test_topological_sort_edge_with_unknown_target_is_silently_excluded() {
        let nodes = vec![make_node("a"), make_node("b")];
        let edges = vec![make_edge("e1", "a", "b"), make_edge("e2", "a", "phantom")];

        let result = topological_sort(&nodes, &edges);

        assert!(
            result.is_ok(),
            "edge with unknown target must be silently excluded; got {result:?}"
        );
        let sorted = result.unwrap();
        assert_eq!(sorted.len(), 2);
    }
}

// ─── parse_literal kill tests (surviving mutants) ────────────────────────────

mod parse_literal_kill_tests {
    /// Kills line 808:19 mutant: `raw == "true"` → `raw != "true"`.
    ///
    /// With the mutant, `parse_literal("true")` evaluates the branch condition
    /// as `"true" != "true"` = false and falls through to return `None`.
    #[test]
    fn test_parse_literal_true_returns_bool_true() {
        let result = super::parse_literal("true");
        assert_eq!(result, Some(serde_json::Value::Bool(true)));
    }

    /// Kills line 810:19 mutant: `raw == "false"` → `raw != "false"`.
    ///
    /// With the mutant, `parse_literal("false")` returns `None`.
    #[test]
    fn test_parse_literal_false_returns_bool_false() {
        let result = super::parse_literal("false");
        assert_eq!(result, Some(serde_json::Value::Bool(false)));
    }

    /// Kills line 802:30 mutant: `raw.starts_with('"') && raw.ends_with('"')` → `||`.
    ///
    /// A half-open double-quoted string (starts with `"` but does not end with
    /// it) is not a valid literal.  With the `||` mutant this would be parsed
    /// as a string by stripping one char from each end, producing a wrong value.
    #[test]
    fn test_parse_literal_unclosed_double_quote_returns_none() {
        let result = super::parse_literal("\"unclosed");
        assert_eq!(
            result, None,
            "half-open double-quoted literal must not parse"
        );
    }

    /// Kills line 803:35 mutant: `raw.starts_with('\'') && raw.ends_with('\'')` → `||`.
    ///
    /// Same invariant for single-quote delimiters.
    #[test]
    fn test_parse_literal_unclosed_single_quote_returns_none() {
        let result = super::parse_literal("'unclosed");
        assert_eq!(
            result, None,
            "half-open single-quoted literal must not parse"
        );
    }

    /// Verify that a properly closed double-quoted string parses correctly.
    /// This also confirms the inner content is returned (not empty string).
    #[test]
    fn test_parse_literal_closed_double_quoted_string_returns_inner_value() {
        let result = super::parse_literal("\"hello world\"");
        assert_eq!(
            result,
            Some(serde_json::Value::String("hello world".to_string()))
        );
    }

    /// Verify that a properly closed single-quoted string parses correctly.
    #[test]
    fn test_parse_literal_closed_single_quoted_string_returns_inner_value() {
        let result = super::parse_literal("'hello world'");
        assert_eq!(
            result,
            Some(serde_json::Value::String("hello world".to_string()))
        );
    }

    /// An unrecognised token (no quotes, not bool) must return None.
    #[test]
    fn test_parse_literal_unrecognised_token_returns_none() {
        let result = super::parse_literal("not_a_literal");
        assert_eq!(result, None);
    }

    /// FINDING-001 regression: a bare double-quote (len == 1) must return None,
    /// not panic with a slice-index-out-of-bounds.
    #[test]
    fn test_parse_literal_bare_double_quote_returns_none_not_panic() {
        let result = super::parse_literal("\"");
        assert_eq!(
            result, None,
            "bare double-quote must not panic and must return None"
        );
    }

    /// FINDING-001 regression: a bare single-quote (len == 1) must return None,
    /// not panic with a slice-index-out-of-bounds.
    #[test]
    fn test_parse_literal_bare_single_quote_returns_none_not_panic() {
        let result = super::parse_literal("'");
        assert_eq!(
            result, None,
            "bare single-quote must not panic and must return None"
        );
    }
}

// ─── Scalar accessor kill tests (surviving mutants) ──────────────────────────

mod scalar_accessor_kill_tests {
    use super::*;

    /// Kills line 504:9 mutants (×2): `SchemaVersion::as_str` returning `""` or `"xyzzy"`.
    #[test]
    fn test_schema_version_as_str_returns_the_current_version_string() {
        let v = SchemaVersion::current();
        assert_eq!(
            v.as_str(),
            SchemaVersion::CURRENT,
            "as_str must return the inner version string"
        );
        assert!(!v.as_str().is_empty(), "as_str must not be empty");
        assert_ne!(v.as_str(), "xyzzy", "as_str must not return a placeholder");
    }

    /// Kills line 523:9 mutant: `From<SchemaVersion> for String` returning `Default::default()` (`""`).
    #[test]
    fn test_schema_version_into_string_roundtrip_preserves_version() {
        let v = SchemaVersion::current();
        let s: String = v.into();
        assert_eq!(s, SchemaVersion::CURRENT);
        assert_ne!(
            s,
            String::default(),
            "conversion must not return the empty default"
        );
    }

    /// Kills line 87:9 mutant: `From<TimeoutSeconds> for Duration` returning `Default::default()` (0 secs).
    #[test]
    fn test_timeout_seconds_into_duration_preserves_seconds() {
        let t = TimeoutSeconds(42);
        let d: std::time::Duration = t.into();
        assert_eq!(
            d.as_secs(),
            42,
            "conversion must preserve the timeout value"
        );
        assert_ne!(
            d,
            std::time::Duration::default(),
            "must not collapse to zero"
        );
    }

    /// Kills line 69:9 mutants (×2): `NaturalLanguageCondition::as_str` returning `""` or `"xyzzy"`.
    #[test]
    fn test_natural_language_condition_as_str_returns_the_inner_string() {
        let cond = NaturalLanguageCondition::new("the spec requires X to hold").expect("non-empty");
        assert_eq!(
            cond.as_str(),
            "the spec requires X to hold",
            "as_str must return the inner description"
        );
        assert!(!cond.as_str().is_empty());
        assert_ne!(cond.as_str(), "xyzzy");
    }
}

// ─── Adversarial expression inputs (Tier 5 — no raw bytes, structured inputs) ─

mod adversarial_expression_tests {
    use super::*;

    fn empty_state() -> PipelineState {
        make_state()
    }

    /// No operator at all: must return false without panicking.
    #[test]
    fn test_expression_no_operator_returns_false_without_panic() {
        let expr = Expression::new("just_a_field_no_operator").expect("non-empty");
        assert!(!evaluate_deterministic_condition(&expr, &empty_state()));
    }

    /// Expression is all spaces (non-empty but no operator): must return false.
    #[test]
    fn test_expression_only_spaces_returns_false_without_panic() {
        let expr = Expression::new("   ").expect("non-empty whitespace");
        assert!(!evaluate_deterministic_condition(&expr, &empty_state()));
    }

    /// Seven-level dot path: navigate_json must return None gracefully, not panic.
    #[test]
    fn test_expression_deeply_nested_dot_path_returns_false_without_panic() {
        let expr = Expression::new("a.b.c.d.e.f.g == \"x\"").expect("valid expr string");
        assert!(!evaluate_deterministic_condition(&expr, &empty_state()));
    }

    /// Unicode characters in path segments: must return false without panicking.
    #[test]
    fn test_expression_unicode_path_segments_returns_false_without_panic() {
        let expr = Expression::new("ñoño.状態 == \"x\"").expect("valid expr string");
        assert!(!evaluate_deterministic_condition(&expr, &empty_state()));
    }

    /// Operator present but LHS path is empty (leading space before `==`):
    /// navigate_json must handle the empty-string segment without panicking.
    #[test]
    fn test_expression_empty_lhs_path_returns_false_without_panic() {
        let expr = Expression::new(" == \"value\"").expect("non-empty");
        assert!(!evaluate_deterministic_condition(&expr, &empty_state()));
    }

    /// Operator present but RHS literal is empty (trailing space after `==`):
    /// parse_literal must return None for an empty token without panicking.
    #[test]
    fn test_expression_empty_rhs_literal_returns_false_without_panic() {
        let expr = Expression::new("field == ").expect("non-empty");
        assert!(!evaluate_deterministic_condition(&expr, &empty_state()));
    }
}
