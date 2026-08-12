//! Adversarial test suite for `json_guard.rs` — the iterative JSON
//! nesting-depth guard used to prevent recursive-`serde_json`
//! stack-overflow `DoS` when comparing untrusted interface schemas.
//!
//! ## Phase: RED
//!
//! `exceeds_max_depth` is a `todo!()` stub. Every test that calls it is
//! expected to **compile** cleanly but **panic** at runtime until the
//! implementation lands.
//!
//! ## Why This Module Exists
//!
//! A Security Reviewer found that `crate::interfaces::compare_schemas` and
//! its helpers call `serde_json::Value::to_string()`/`==` — both recursive
//! over arbitrary `Value` nesting depth — on schema content sourced from an
//! untrusted domain-service response. A sufficiently deep adversarial value
//! (tens of thousands of nested levels) crashes the whole process with an
//! uncatchable `STATUS_STACK_OVERFLOW`. `exceeds_max_depth` is the guard that
//! must run *before* any such recursive operation touches untrusted content,
//! and it must itself be immune to the same attack — hence the emphasis
//! below on iterative construction of adversarially deep test fixtures
//! (recursion in a test helper would crash the test binary before ever
//! reaching the function under test).
//!
//! ## Assertions covered
//!
//! - Depth semantics: scalar/empty container = 0, `{"a": 1}` = 1, `{"a":
//!   {"b": 1}}` = 2 (per the design's doc comment on `exceeds_max_depth`).
//! - Boundary: depth exactly `max_depth` does not exceed; depth
//!   `max_depth + 1` exceeds.
//! - Non-recursive implementation: must survive (return correctly, not
//!   crash, not hang) on a value nested ~1,000,000 levels deep.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use std::mem::ManuallyDrop;

use proptest::prelude::*;
use serde_json::{Map, Value};

use super::{MAX_SCHEMA_COMPARISON_DEPTH, exceeds_max_depth};

// ─── Test helpers ────────────────────────────────────────────────────────────

/// Builds a JSON object nested `depth` levels deep via direct, iterative
/// `Map`/`Value` construction — a `for` loop, never recursion, and never the
/// `json!` macro applied to a variable expression (which round-trips
/// through `Serialize`/`to_value` and would itself recurse over the growing
/// value on every iteration, defeating the purpose of an iterative test
/// fixture builder). Depth 0 is a bare scalar; `{"a": <depth-1>}` is depth N
/// for N >= 1.
fn nested_object(depth: usize) -> Value {
    let mut value = Value::from(1);
    for _ in 0..depth {
        let mut map = Map::new();
        map.insert("a".to_string(), value);
        value = Value::Object(map);
    }
    value
}

/// Same as [`nested_object`] but nests via single-element JSON arrays
/// instead of single-key objects.
fn nested_array(depth: usize) -> Value {
    let mut value = Value::from(1);
    for _ in 0..depth {
        value = Value::Array(vec![value]);
    }
    value
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 1: Specification tests — depth semantics
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_exceeds_max_depth_null_scalar_has_depth_zero_not_exceeded() {
    assert!(!exceeds_max_depth(&Value::Null, 0));
}

#[test]
fn test_exceeds_max_depth_number_scalar_has_depth_zero_not_exceeded() {
    assert!(!exceeds_max_depth(&Value::from(42), 0));
}

#[test]
fn test_exceeds_max_depth_string_scalar_has_depth_zero_not_exceeded() {
    assert!(!exceeds_max_depth(&Value::from("hello"), 0));
}

#[test]
fn test_exceeds_max_depth_bool_scalar_has_depth_zero_not_exceeded() {
    assert!(!exceeds_max_depth(&Value::from(true), 0));
}

#[test]
fn test_exceeds_max_depth_empty_object_has_depth_zero_not_exceeded() {
    assert!(!exceeds_max_depth(&Value::Object(Map::new()), 0));
}

#[test]
fn test_exceeds_max_depth_empty_array_has_depth_zero_not_exceeded() {
    assert!(!exceeds_max_depth(&Value::Array(vec![]), 0));
}

#[test]
fn test_exceeds_max_depth_single_key_object_depth_one_not_exceeded_at_limit_one() {
    let value = nested_object(1);
    assert!(!exceeds_max_depth(&value, 1));
}

#[test]
fn test_exceeds_max_depth_single_key_object_depth_one_exceeded_at_limit_zero() {
    let value = nested_object(1);
    assert!(exceeds_max_depth(&value, 0));
}

#[test]
fn test_exceeds_max_depth_two_level_object_depth_two_not_exceeded_at_limit_two() {
    let value = nested_object(2);
    assert!(!exceeds_max_depth(&value, 2));
}

#[test]
fn test_exceeds_max_depth_two_level_object_depth_two_exceeded_at_limit_one() {
    let value = nested_object(2);
    assert!(exceeds_max_depth(&value, 1));
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 2: Adversarial / boundary / stub-killing tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_exceeds_max_depth_exact_boundary_at_configured_limit_not_exceeded() {
    let value = nested_object(MAX_SCHEMA_COMPARISON_DEPTH);
    assert!(
        !exceeds_max_depth(&value, MAX_SCHEMA_COMPARISON_DEPTH),
        "a value exactly at the configured max depth must not be reported as exceeding it"
    );
}

#[test]
fn test_exceeds_max_depth_one_level_beyond_configured_limit_exceeded() {
    let value = nested_object(MAX_SCHEMA_COMPARISON_DEPTH + 1);
    assert!(
        exceeds_max_depth(&value, MAX_SCHEMA_COMPARISON_DEPTH),
        "a value one level beyond the configured max depth must exceed it"
    );
}

#[test]
fn test_exceeds_max_depth_array_nesting_counted_same_as_object_nesting() {
    let value = nested_array(5);
    assert!(!exceeds_max_depth(&value, 5));
    assert!(exceeds_max_depth(&value, 4));
}

#[test]
fn test_exceeds_max_depth_mixed_object_and_array_nesting_counts_both_container_kinds() {
    // depth 0 (scalar) -> 1 (array) -> 2 (object) -> 3 (array) -> 4 (object)
    let mut value = Value::from(1);
    value = Value::Array(vec![value]); // depth 1
    let mut map = Map::new();
    map.insert("a".to_string(), value);
    value = Value::Object(map); // depth 2
    value = Value::Array(vec![value]); // depth 3
    let mut map2 = Map::new();
    map2.insert("b".to_string(), value);
    value = Value::Object(map2); // depth 4

    assert!(!exceeds_max_depth(&value, 4));
    assert!(exceeds_max_depth(&value, 3));
}

#[test]
fn test_exceeds_max_depth_object_depth_is_max_of_branches_not_sum() {
    let mut map = Map::new();
    map.insert("shallow".to_string(), Value::from(1)); // depth-0 branch
    map.insert("deep".to_string(), nested_object(2)); // depth-2 branch

    let value = Value::Object(map);

    // Overall depth is 1 (this object) + 2 (the "deep" branch) = 3, not
    // 1 (this object) + 0 (shallow) + 2 (deep) = a summed total of 4.
    assert!(!exceeds_max_depth(&value, 3));
    assert!(exceeds_max_depth(&value, 2));
}

#[test]
fn test_exceeds_max_depth_array_depth_is_max_of_elements_not_sum() {
    let value = Value::Array(vec![Value::from(1), nested_array(2), Value::from(4)]);

    // Overall depth is 1 (this array) + 2 (the nested_array(2) element) = 3,
    // not a sum across every element.
    assert!(!exceeds_max_depth(&value, 3));
    assert!(exceeds_max_depth(&value, 2));
}

#[test]
fn test_exceeds_max_depth_wide_object_with_many_keys_at_depth_one_not_exceeded_at_limit_one() {
    let mut map = Map::new();
    for i in 0..10_000 {
        map.insert(format!("key-{i}"), Value::from(i));
    }
    let value = Value::Object(map);

    assert!(
        !exceeds_max_depth(&value, 1),
        "a wide-but-shallow object (10,000 keys, all holding depth-0 values) must not be \
         reported as exceeding a depth-1 limit; this is a nesting-depth check, not a \
         node-count or breadth check"
    );
    assert!(exceeds_max_depth(&value, 0));
}

#[test]
fn test_exceeds_max_depth_finite_value_never_exceeds_usize_max_limit() {
    let value = nested_object(50);
    assert!(!exceeds_max_depth(&value, usize::MAX));
}

// --- Stub-killing ---

#[test]
fn test_exceeds_max_depth_cannot_hardcode_true_for_shallow_value() {
    assert!(
        !exceeds_max_depth(&Value::from("shallow"), 1000),
        "a stub that always returns true must fail this test"
    );
}

#[test]
fn test_exceeds_max_depth_cannot_hardcode_false_for_moderately_deep_value() {
    let value = nested_object(1000);
    assert!(
        exceeds_max_depth(&value, 1),
        "a stub that always returns false must fail this test"
    );
}

/// The core non-recursion property under test: a naive recursive
/// implementation of `exceeds_max_depth` would itself stack-overflow on
/// this input, crashing the entire test binary uncatchably — the exact
/// failure mode the guard exists to prevent. An iterative implementation
/// must return `true` promptly.
///
/// `value` is wrapped in `ManuallyDrop` and intentionally never dropped:
/// `serde_json::Value`'s own `Drop` implementation is *also* recursive, so
/// an ordinary drop of a million-level-deep value at the end of this test
/// (whether via normal return, or via unwinding out of a `todo!()` panic in
/// the current RED phase) would itself stack-overflow the test process — a
/// test-harness artifact wholly unrelated to whether `exceeds_max_depth` is
/// implemented correctly. Leaking this single fixture for the duration of
/// the test process is an acceptable, deliberate trade-off.
#[test]
fn test_exceeds_max_depth_survives_one_million_level_deep_object_without_crashing_or_hanging() {
    let value = ManuallyDrop::new(nested_object(1_000_000));

    let exceeded = exceeds_max_depth(&value, MAX_SCHEMA_COMPARISON_DEPTH);

    assert!(
        exceeded,
        "a million-level-deep value must be reported as exceeding the configured limit"
    );
}

#[test]
fn test_exceeds_max_depth_survives_one_million_level_deep_array_without_crashing_or_hanging() {
    let value = ManuallyDrop::new(nested_array(1_000_000));

    let exceeded = exceeds_max_depth(&value, MAX_SCHEMA_COMPARISON_DEPTH);

    assert!(
        exceeded,
        "a million-level-deep array must be reported as exceeding the configured limit"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 3: Property-based tests
// ═══════════════════════════════════════════════════════════════════════════

fn arbitrary_bounded_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        ".{0,8}".prop_map(Value::from),
    ];
    leaf.prop_recursive(6, 64, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            prop::collection::vec((".{1,5}", inner), 0..4)
                .prop_map(|entries| Value::Object(entries.into_iter().collect())),
        ]
    })
}

proptest! {
    /// `exceeds_max_depth` must never panic for arbitrary, boundedly-nested
    /// JSON values and arbitrary thresholds.
    #[test]
    fn test_exceeds_max_depth_never_panics_on_arbitrary_bounded_json(
        value in arbitrary_bounded_json(),
        max_depth in 0usize..20,
    ) {
        let _ = exceeds_max_depth(&value, max_depth);
    }

    /// A value built to exactly `depth` levels must not exceed a limit of
    /// exactly `depth`, but must exceed a limit of `depth - 1` (for
    /// depth > 0). This ties the function's output to the exact depth
    /// semantics across a wide range of generated depths, rather than just
    /// the few hand-picked cases in the Tier 1 tests above.
    #[test]
    fn test_exceeds_max_depth_matches_exact_constructed_depth_across_range(
        depth in 0usize..200,
    ) {
        let value = nested_object(depth);
        prop_assert!(!exceeds_max_depth(&value, depth));
        if depth > 0 {
            prop_assert!(exceeds_max_depth(&value, depth - 1));
        }
    }

    /// Monotonicity: `exceeds_max_depth` is non-increasing in `max_depth`
    /// for a fixed value — if it does not exceed a larger threshold, it
    /// cannot exceed any smaller threshold either.
    #[test]
    fn test_exceeds_max_depth_is_monotonic_non_increasing_in_max_depth(
        depth in 0usize..80,
        t1 in 0usize..100,
        t2 in 0usize..100,
    ) {
        let value = nested_object(depth);
        let (lo, hi) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };
        if !exceeds_max_depth(&value, lo) {
            prop_assert!(!exceeds_max_depth(&value, hi));
        }
    }
}
