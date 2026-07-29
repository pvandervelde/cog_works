//! Adversarial test suite for `interfaces.rs` — `validate_cross_domain_constraints`.
//!
//! ## Phase: RED
//!
//! `validate_cross_domain_constraints` is a `todo!()` stub. Every test that
//! calls it is expected to **compile** cleanly but **panic** at runtime until
//! the implementation lands.
//!
//! ## Assertions covered
//!
//! - ASSERT-XDOM-001 (cross-reference clause only): every mismatch between
//!   registry contracts and extracted interfaces must be detected — the
//!   schema-validation clause belongs to a separate trait, out of scope here.
//!
//! ## Spec assumptions / gaps
//!
//! - "Compare schemas field-by-field" (algorithm step 1) is read literally as:
//!   enumerate the **top-level** keys of the contract's JSON object schema and
//!   compare each key's value (by structural equality, regardless of nesting
//!   depth) against the extracted schema's value for that key. A missing
//!   top-level key in the extracted schema is treated the same as a missing
//!   interface (`actual_value == "<not present>"`). Extra top-level keys present
//!   only in the extracted schema are not reported (mirrors the "new interfaces
//!   permitted" rule for whole interfaces). This is the most literal reading of
//!   the algorithm text; if the eventual implementation instead recurses into
//!   nested objects to produce dotted `parameter_name` paths, only the tests
//!   that pin an exact `parameter_name` string for a *nested* mismatch would
//!   need revisiting — flat top-level tests are unaffected either way.
//! - Non-object schema values (scalars, arrays, null): the spec does not define
//!   a "field" for a non-object JSON value. Tests for this case assert only:
//!   (a) no panic, and (b) equal schemas produce zero findings while unequal
//!   schemas produce at least one finding — without pinning the exact
//!   `parameter_name` used for a whole-schema mismatch.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use serde_json::json;

use super::{ConstraintFinding, validate_cross_domain_constraints};
use crate::{
    domain_services::{InterfaceDefinition, InterfaceMap},
    identifiers::{DomainServiceName, InterfaceId},
    types::{ApiVersion, DiagnosticSeverity},
};

// ─── Test helpers ────────────────────────────────────────────────────────────

fn iid(s: &str) -> InterfaceId {
    InterfaceId::new(s).expect("test interface id must not be empty")
}

fn domain(s: &str) -> DomainServiceName {
    DomainServiceName::new(s).expect("test domain name must not be empty")
}

fn definition(id: &str, owning_domain: &str, schema: serde_json::Value) -> InterfaceDefinition {
    InterfaceDefinition {
        id: iid(id),
        domain: domain(owning_domain),
        schema,
        artifact_types: vec![],
        version: ApiVersion::new(1, 0),
    }
}

fn map_of(entries: Vec<InterfaceDefinition>) -> InterfaceMap {
    InterfaceMap { entries }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 1: Specification tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_validate_cross_domain_constraints_empty_contracts_and_extracted_returns_empty_vec() {
    let findings = validate_cross_domain_constraints(&[], &map_of(vec![]));
    assert!(findings.is_empty());
}

#[test]
fn test_validate_cross_domain_constraints_matching_contract_and_extracted_returns_empty_vec() {
    let schema = json!({"type": "string", "maxLength": 64});
    let contracts = vec![definition("iface-a", "rust", schema.clone())];
    let extracted = map_of(vec![definition("iface-a", "kicad", schema)]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(
        findings.is_empty(),
        "identical schema for matching id must produce full conformance, got {findings:?}"
    );
}

#[test]
fn test_validate_cross_domain_constraints_missing_interface_in_extracted_emits_blocking_finding() {
    let contracts = vec![definition("iface-missing", "rust", json!({"type": "string"}))];
    let extracted = map_of(vec![]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, DiagnosticSeverity::Blocking);
    assert_eq!(findings[0].interface_id, iid("iface-missing"));
}

#[test]
fn test_validate_cross_domain_constraints_missing_interface_actual_value_is_not_present_marker() {
    let contracts = vec![definition("iface-missing", "rust", json!({"type": "string"}))];
    let extracted = map_of(vec![]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert_eq!(findings[0].actual_value, "<not present>");
}

#[test]
fn test_validate_cross_domain_constraints_missing_interface_expected_value_reflects_contract_schema() {
    let contracts = vec![definition(
        "iface-missing",
        "rust",
        json!({"type": "string", "distinctive_marker": "xyz123"}),
    )];
    let extracted = map_of(vec![]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(
        findings[0].expected_value.contains("xyz123"),
        "expected_value must reflect the contract schema, got '{}'",
        findings[0].expected_value
    );
}

#[test]
fn test_validate_cross_domain_constraints_field_value_mismatch_emits_blocking_finding() {
    let contracts = vec![definition("iface-a", "rust", json!({"type": "string"}))];
    let extracted = map_of(vec![definition("iface-a", "kicad", json!({"type": "integer"}))]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(!findings.is_empty(), "a field-value mismatch must be reported");
    assert!(findings.iter().all(|f| f.severity == DiagnosticSeverity::Blocking));
}

#[test]
fn test_validate_cross_domain_constraints_field_mismatch_parameter_name_identifies_the_field() {
    let contracts = vec![definition("iface-a", "rust", json!({"type": "string"}))];
    let extracted = map_of(vec![definition("iface-a", "kicad", json!({"type": "integer"}))]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(
        findings.iter().any(|f| f.parameter_name == "type"),
        "expected a finding naming the mismatched 'type' field, got {findings:?}"
    );
}

#[test]
fn test_validate_cross_domain_constraints_extra_extracted_interface_with_no_contract_not_reported() {
    let schema = json!({"type": "string"});
    let contracts = vec![definition("iface-a", "rust", schema.clone())];
    let extracted = map_of(vec![
        definition("iface-a", "kicad", schema),
        definition("iface-brand-new", "kicad", json!({"type": "object"})),
    ]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(
        findings.is_empty(),
        "new extracted interfaces without a registry counterpart must not be reported, got {findings:?}"
    );
}

#[test]
fn test_validate_cross_domain_constraints_owning_and_violating_domain_populated() {
    let contracts = vec![definition("iface-a", "rust-service", json!({"type": "string"}))];
    let extracted = map_of(vec![definition(
        "iface-a",
        "kicad-service",
        json!({"type": "integer"}),
    )]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(!findings.is_empty());
    assert_eq!(findings[0].owning_domain, domain("rust-service"));
    assert_eq!(findings[0].violating_domain, domain("kicad-service"));
}

/// Completeness: every contract must be validated, not just the first.
#[test]
fn test_validate_cross_domain_constraints_multiple_contracts_each_validated_independently() {
    let contracts = vec![
        definition("iface-a", "rust", json!({"type": "string"})),
        definition("iface-b", "rust", json!({"type": "boolean"})),
    ];
    let extracted = map_of(vec![
        definition("iface-a", "kicad", json!({"type": "integer"})), // mismatch
        definition("iface-b", "kicad", json!({"type": "boolean"})), // conforms
    ]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(
        findings.iter().any(|f| f.interface_id == iid("iface-a")),
        "iface-a mismatch must be reported: {findings:?}"
    );
    assert!(
        !findings.iter().any(|f| f.interface_id == iid("iface-b")),
        "iface-b conforms and must not be reported: {findings:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 2: Adversarial / boundary / stub-killing tests
// ═══════════════════════════════════════════════════════════════════════════

/// Stub-killing: three missing interfaces must produce exactly three findings,
/// not one (hardcoded single finding) and not zero (hardcoded empty vec).
#[test]
fn test_validate_cross_domain_constraints_all_contracts_missing_produces_one_finding_each() {
    let contracts = vec![
        definition("iface-1", "rust", json!({"type": "string"})),
        definition("iface-2", "rust", json!({"type": "integer"})),
        definition("iface-3", "rust", json!({"type": "boolean"})),
    ];
    let extracted = map_of(vec![]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert_eq!(findings.len(), 3);
    for f in &findings {
        assert_eq!(f.actual_value, "<not present>");
    }
}

/// Multiple mismatched top-level fields in the same interface must each
/// produce their own finding (not merged into a single generic finding).
#[test]
fn test_validate_cross_domain_constraints_multiple_field_mismatches_produce_multiple_findings() {
    let contracts = vec![definition(
        "iface-a",
        "rust",
        json!({"type": "string", "format": "uuid", "maxLength": 36}),
    )];
    let extracted = map_of(vec![definition(
        "iface-a",
        "kicad",
        json!({"type": "integer", "format": "int32", "maxLength": 36}),
    )]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    let mismatched_fields: std::collections::HashSet<&str> =
        findings.iter().map(|f| f.parameter_name.as_str()).collect();
    assert!(mismatched_fields.contains("type"));
    assert!(mismatched_fields.contains("format"));
    assert!(
        !mismatched_fields.contains("maxLength"),
        "maxLength is identical in both schemas and must not be reported"
    );
}

#[test]
fn test_validate_cross_domain_constraints_severity_is_always_blocking_for_missing_and_mismatched() {
    let contracts = vec![
        definition("iface-missing", "rust", json!({"type": "string"})),
        definition("iface-mismatch", "rust", json!({"type": "string"})),
    ];
    let extracted = map_of(vec![definition(
        "iface-mismatch",
        "kicad",
        json!({"type": "integer"}),
    )]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(!findings.is_empty());
    assert!(
        findings.iter().all(|f| f.severity == DiagnosticSeverity::Blocking),
        "per doc comment, structural incompatibilities are always Blocking: {findings:?}"
    );
}

/// Duplicate `InterfaceId`s in the registry: the algorithm iterates
/// `contracts[i]` independently ("for each contracts[i]"), so each occurrence
/// is validated on its own — two identical duplicate contract entries against
/// a mismatched extracted entry must yield two findings, not one deduplicated
/// finding.
#[test]
fn test_validate_cross_domain_constraints_duplicate_interface_ids_in_contracts_each_produce_own_finding() {
    let dup = definition("iface-dup", "rust", json!({"type": "string"}));
    let contracts = vec![dup.clone(), dup];
    let extracted = map_of(vec![definition("iface-dup", "kicad", json!({"type": "integer"}))]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    let count_for_dup = findings
        .iter()
        .filter(|f| f.interface_id == iid("iface-dup"))
        .count();
    assert_eq!(
        count_for_dup, 2,
        "each duplicate contract occurrence must be validated independently, got {findings:?}"
    );
}

#[test]
fn test_validate_cross_domain_constraints_extracted_interface_id_absent_from_contracts_ignored_even_with_other_violations()
 {
    let contracts = vec![definition("iface-known", "rust", json!({"type": "string"}))];
    let extracted = map_of(vec![
        definition("iface-known", "kicad", json!({"type": "integer"})), // mismatch, reported
        definition("iface-unregistered", "kicad", json!({"type": "object"})), // not reported
    ]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(findings.iter().any(|f| f.interface_id == iid("iface-known")));
    assert!(!findings.iter().any(|f| f.interface_id == iid("iface-unregistered")));
}

/// Findings must reference contracts in the same relative order they were
/// supplied (no unexplained reordering).
#[test]
fn test_validate_cross_domain_constraints_result_order_matches_contracts_order() {
    let contracts = vec![
        definition("iface-first", "rust", json!({"type": "string"})),
        definition("iface-second", "rust", json!({"type": "boolean"})),
    ];
    let extracted = map_of(vec![]); // both missing

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].interface_id, iid("iface-first"));
    assert_eq!(findings[1].interface_id, iid("iface-second"));
}

#[test]
fn test_validate_cross_domain_constraints_non_object_schema_scalar_equal_values_returns_empty() {
    let contracts = vec![definition("iface-a", "rust", json!("just-a-string-schema"))];
    let extracted = map_of(vec![definition("iface-a", "kicad", json!("just-a-string-schema"))]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(findings.is_empty(), "identical non-object schemas must conform, got {findings:?}");
}

#[test]
fn test_validate_cross_domain_constraints_non_object_schema_scalar_mismatched_values_returns_nonempty() {
    let contracts = vec![definition("iface-a", "rust", json!("schema-v1"))];
    let extracted = map_of(vec![definition("iface-a", "kicad", json!("schema-v2"))]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(
        !findings.is_empty(),
        "mismatched non-object scalar schemas must produce at least one finding"
    );
}

#[test]
fn test_validate_cross_domain_constraints_schema_is_json_array_does_not_panic() {
    let contracts = vec![definition("iface-a", "rust", json!(["a", "b", "c"]))];
    let extracted = map_of(vec![definition("iface-a", "kicad", json!(["a", "b", "different"]))]);

    let _findings = validate_cross_domain_constraints(&contracts, &extracted);
}

#[test]
fn test_validate_cross_domain_constraints_schema_is_json_null_does_not_panic() {
    let contracts = vec![definition("iface-a", "rust", serde_json::Value::Null)];
    let extracted = map_of(vec![definition("iface-a", "kicad", serde_json::Value::Null)]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(findings.is_empty(), "identical null schemas must conform");
}

#[test]
fn test_validate_cross_domain_constraints_empty_extracted_with_nonempty_contracts_reports_all_missing() {
    let contracts = vec![
        definition("iface-a", "rust", json!({"type": "string"})),
        definition("iface-b", "rust", json!({"type": "integer"})),
    ];
    let extracted = map_of(vec![]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.actual_value == "<not present>"));
}

#[test]
fn test_validate_cross_domain_constraints_nested_json_object_mismatch_does_not_panic() {
    let contracts = vec![definition(
        "iface-a",
        "rust",
        json!({"properties": {"nested_field": {"type": "string", "minLength": 1}}}),
    )];
    let extracted = map_of(vec![definition(
        "iface-a",
        "kicad",
        json!({"properties": {"nested_field": {"type": "integer", "minLength": 1}}}),
    )]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(
        !findings.is_empty(),
        "a nested structural mismatch must surface as at least one finding"
    );
}

#[test]
fn test_validate_cross_domain_constraints_nested_json_object_identical_returns_empty() {
    let schema = json!({"properties": {"nested_field": {"type": "string", "minLength": 1}}});
    let contracts = vec![definition("iface-a", "rust", schema.clone())];
    let extracted = map_of(vec![definition("iface-a", "kicad", schema)]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(findings.is_empty(), "identical nested schemas must conform, got {findings:?}");
}

/// Stub-killing: a stub that always returns `vec![]` must fail when real
/// violations exist.
#[test]
fn test_validate_cross_domain_constraints_cannot_hardcode_empty_vec_when_violations_exist() {
    let contracts = vec![definition("iface-a", "rust", json!({"type": "string"}))];
    let extracted = map_of(vec![]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert!(!findings.is_empty(), "a stub returning vec![] unconditionally must fail this test");
}

/// Stub-killing: a stub that always returns exactly one finding must fail when
/// multiple independent violations exist.
#[test]
fn test_validate_cross_domain_constraints_cannot_hardcode_single_finding_when_multiple_violations_exist() {
    let contracts = vec![
        definition("iface-a", "rust", json!({"type": "string"})),
        definition("iface-b", "rust", json!({"type": "boolean"})),
        definition("iface-c", "rust", json!({"type": "number"})),
    ];
    let extracted = map_of(vec![]);

    let findings = validate_cross_domain_constraints(&contracts, &extracted);

    assert_eq!(findings.len(), 3, "a stub returning exactly one finding must fail this test");
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 3: Property-based tests
// ═══════════════════════════════════════════════════════════════════════════

fn arbitrary_json_scalar() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::from),
        any::<i64>().prop_map(serde_json::Value::from),
        ".{0,12}".prop_map(serde_json::Value::from),
    ]
}

fn arbitrary_interface_id() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,10}"
}

proptest! {
    /// `validate_cross_domain_constraints` must never panic for arbitrary
    /// combinations of contracts and extracted interfaces, including scalar
    /// (non-object) schemas.
    #[test]
    fn test_validate_cross_domain_constraints_never_panics_on_arbitrary_inputs(
        contract_ids in proptest::collection::vec(arbitrary_interface_id(), 0..5),
        contract_schemas in proptest::collection::vec(arbitrary_json_scalar(), 0..5),
        extracted_ids in proptest::collection::vec(arbitrary_interface_id(), 0..5),
        extracted_schemas in proptest::collection::vec(arbitrary_json_scalar(), 0..5),
    ) {
        let contracts: Vec<InterfaceDefinition> = contract_ids
            .iter()
            .zip(contract_schemas.iter())
            .filter_map(|(id, schema)| {
                InterfaceId::new(id.clone()).map(|id| definition(id.as_str(), "rust", schema.clone()))
            })
            .collect();
        let extracted_entries: Vec<InterfaceDefinition> = extracted_ids
            .iter()
            .zip(extracted_schemas.iter())
            .filter_map(|(id, schema)| {
                InterfaceId::new(id.clone()).map(|id| definition(id.as_str(), "kicad", schema.clone()))
            })
            .collect();
        let extracted = map_of(extracted_entries);

        let _findings = validate_cross_domain_constraints(&contracts, &extracted);
    }

    /// Full conformance: when `extracted` contains an exact copy of every
    /// contract (same id, same schema), the result must always be empty.
    #[test]
    fn test_validate_cross_domain_constraints_full_conformance_yields_empty(
        ids in proptest::collection::vec(arbitrary_interface_id(), 0..5),
    ) {
        // De-duplicate ids to keep this test focused on the conformance
        // invariant rather than duplicate-id semantics (covered separately).
        let unique_ids: std::collections::BTreeSet<String> = ids.into_iter().collect();
        let contracts: Vec<InterfaceDefinition> = unique_ids
            .iter()
            .map(|id| definition(id, "rust", json!({"type": "string", "id_marker": id})))
            .collect();
        let extracted = map_of(
            unique_ids
                .iter()
                .map(|id| definition(id, "kicad", json!({"type": "string", "id_marker": id})))
                .collect(),
        );

        let findings = validate_cross_domain_constraints(&contracts, &extracted);

        prop_assert!(findings.is_empty(), "exact copies of every contract must conform, got {findings:?}");
    }

    /// Completeness: when `extracted` is entirely empty, every contract must
    /// produce exactly one "missing" finding — the result length must equal
    /// the number of (deduplicated) contracts supplied.
    #[test]
    fn test_validate_cross_domain_constraints_missing_all_yields_one_finding_per_contract(
        ids in proptest::collection::vec(arbitrary_interface_id(), 0..6),
    ) {
        let unique_ids: std::collections::BTreeSet<String> = ids.into_iter().collect();
        let contracts: Vec<InterfaceDefinition> = unique_ids
            .iter()
            .map(|id| definition(id, "rust", json!({"type": "string"})))
            .collect();
        let extracted = map_of(vec![]);

        let findings = validate_cross_domain_constraints(&contracts, &extracted);

        prop_assert_eq!(findings.len(), contracts.len());
        for f in &findings {
            prop_assert_eq!(&f.actual_value, "<not present>");
        }
    }

    /// Soundness: every finding's `interface_id` must correspond to some
    /// contract that was actually supplied — the function must never fabricate
    /// an interface_id that wasn't in `contracts`.
    #[test]
    fn test_validate_cross_domain_constraints_findings_interface_ids_are_subset_of_contract_ids(
        ids in proptest::collection::vec(arbitrary_interface_id(), 0..5),
        extra_extracted_id in arbitrary_interface_id(),
    ) {
        let unique_ids: std::collections::BTreeSet<String> = ids.into_iter().collect();
        let contracts: Vec<InterfaceDefinition> = unique_ids
            .iter()
            .map(|id| definition(id, "rust", json!({"type": "string"})))
            .collect();
        let contract_id_set: std::collections::BTreeSet<InterfaceId> =
            contracts.iter().map(|c| c.id.clone()).collect();

        // Add one extracted-only entry that has no registry counterpart.
        let mut extracted_entries = contracts.clone();
        for entry in &mut extracted_entries {
            entry.schema = json!({"type": "integer"}); // force mismatches
        }
        if let Some(extra_id) = InterfaceId::new(extra_extracted_id) {
            extracted_entries.push(definition(extra_id.as_str(), "kicad", json!({"type": "object"})));
        }
        let extracted = map_of(extracted_entries);

        let findings = validate_cross_domain_constraints(&contracts, &extracted);

        for f in &findings {
            prop_assert!(
                contract_id_set.contains(&f.interface_id),
                "finding referenced an interface_id not present in contracts: {:?}",
                f.interface_id
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Contract test — ConstraintFinding shape sanity
// ═══════════════════════════════════════════════════════════════════════════

/// `ConstraintFinding` fields must be independently constructible and
/// readable exactly as declared in the interface contract (all `pub`, no
/// hidden invariants enforced by a constructor). This guards against a future
/// refactor accidentally hiding or renaming a field relied upon above.
#[test]
fn test_constraint_finding_all_fields_are_publicly_constructible_and_readable() {
    let f = ConstraintFinding {
        interface_id: iid("iface-shape-test"),
        parameter_name: "some_field".to_string(),
        expected_value: "expected".to_string(),
        actual_value: "actual".to_string(),
        owning_domain: domain("owner"),
        violating_domain: domain("violator"),
        severity: DiagnosticSeverity::Blocking,
    };

    assert_eq!(f.interface_id, iid("iface-shape-test"));
    assert_eq!(f.parameter_name, "some_field");
    assert_eq!(f.expected_value, "expected");
    assert_eq!(f.actual_value, "actual");
    assert_eq!(f.owning_domain, domain("owner"));
    assert_eq!(f.violating_domain, domain("violator"));
    assert_eq!(f.severity, DiagnosticSeverity::Blocking);
}
