//! Iterative depth guard for JSON values used in cross-domain schema
//! comparison.
//!
//! [`crate::interfaces::compare_schemas`] and
//! [`crate::interfaces::missing_interface_finding`] call
//! `serde_json::Value::to_string()` and `serde_json::Value::eq` on schema
//! content sourced from an untrusted domain-service response (the extracted
//! interface's `schema` field). Both of those `serde_json` operations are
//! implemented recursively over the `Value` tree; an adversarial or corrupted
//! domain-service response containing a sufficiently deep chain of nested
//! objects/arrays can therefore exhaust the call stack and trigger an
//! uncatchable `STATUS_STACK_OVERFLOW` — killing the entire `CogWorks`
//! orchestrator process, not just the current pipeline run (Rust cannot
//! `catch_unwind` a stack overflow).
//!
//! [`exceeds_max_depth`] is the guard that must be checked *before* any
//! recursive `serde_json` operation touches an untrusted schema value. It is
//! implemented iteratively (an explicit heap-allocated stack, not recursion)
//! specifically so that the guard itself cannot be defeated by the same
//! adversarial input it exists to protect against.
//!
//! ## Security Hardening
//!
//! This module addresses a HIGH-severity `DoS` finding raised by the Security
//! Reviewer against `crate::interfaces::compare_schemas` (and the
//! `field_mismatch`/`whole_schema_mismatch`/`missing_interface_finding`
//! helpers it and `validate_single_contract` rely on): a ~10,000-level-deep
//! nested `Value` in an `extracted: &InterfaceMap` schema crashes the process
//! before any `catch_unwind` boundary can intervene.

use serde_json::Value;

/// Maximum JSON nesting depth permitted when comparing interface schemas.
/// Chosen generously above any realistic legitimate schema depth while
/// staying well clear of stack-overflow territory once bounded values are
/// passed to `serde_json`'s recursive Display/PartialEq impls.
pub const MAX_SCHEMA_COMPARISON_DEPTH: usize = 64;

/// Returns `true` if `value`'s nesting exceeds `max_depth`.
///
/// ## Depth Semantics
///
/// - A scalar, string, bool, null, empty object, or empty array has depth 0.
/// - `{"a": 1}` has depth 1.
/// - `{"a": {"b": 1}}` has depth 2.
/// - Depth is the length of the longest chain of nested containers (objects
///   or arrays); sibling branches do not add to each other's depth — a
///   wide-but-shallow object with many keys, all holding depth-0 values,
///   still has depth 1.
///
/// Returns `true` iff the actual depth of `value` is strictly greater than
/// `max_depth`. A `value` whose depth is exactly `max_depth` does **not**
/// exceed it.
///
/// ## Non-Recursive Implementation Requirement
///
/// This function must be implemented iteratively, using an explicit
/// heap-allocated stack (not language-level recursion), so that it cannot
/// itself stack-overflow on the same adversarially deep input it exists to
/// guard against. It must terminate and return correctly (never panic, never
/// loop forever) even for a `value` nested millions of levels deep.
///
/// ## Short-Circuiting
///
/// Traversal order is a plain LIFO stack pop (no guaranteed visitation
/// order), which is sufficient because only the *maximum* level reached is
/// meaningful. As soon as a popped `(node, level)` pair has `level >
/// max_depth`, the function returns `true` immediately: a non-leaf node at
/// that level would only push children at even greater levels, and a leaf
/// node at that level already witnesses the exceeded depth. This bounds
/// work on adversarial single-branch chains (e.g. a million-deep object) to
/// roughly `max_depth` steps rather than the full input size.
///
/// # See also
///
/// `crate::interfaces::compare_schemas`,
/// `crate::interfaces::missing_interface_finding`
#[must_use]
pub fn exceeds_max_depth(value: &Value, max_depth: usize) -> bool {
    let mut stack: Vec<(&Value, usize)> = vec![(value, 0)];

    while let Some((current, level)) = stack.pop() {
        if level > max_depth {
            return true;
        }

        match current {
            Value::Object(map) if !map.is_empty() => {
                stack.extend(map.values().map(|child| (child, level + 1)));
            }
            Value::Array(items) if !items.is_empty() => {
                stack.extend(items.iter().map(|child| (child, level + 1)));
            }
            _ => {}
        }
    }

    false
}

#[cfg(test)]
#[path = "json_guard_tests.rs"]
mod tests;
