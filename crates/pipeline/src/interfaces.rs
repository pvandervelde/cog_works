//! Cross-domain interface constraint validation.
//!
//! CogWorks maintains a human-authored interface registry (loaded via
//! [`crate::InterfaceRegistryLoader`]) that declares the contracts between
//! domain services. After the Code Generation node produces new artifacts, the
//! domain service's `extract_interfaces` operation extracts the actual
//! interfaces from those artifacts. This module's
//! [`validate_cross_domain_constraints`] function compares the two sets to
//! detect any cross-domain contract violations before the review gate runs.
//!
//! ## Why Cross-Domain Validation Matters
//!
//! A change to a Rust hardware-abstraction crate may inadvertently break the
//! interface contract expected by a FreeCAD simulation service, or vice versa.
//! Catching this early — before human review — prevents late-stage defects
//! that cross architectural boundaries.
//!
//! ## Pure Business Logic
//!
//! No I/O. [`validate_cross_domain_constraints`] compares the two slices of
//! definitions and returns findings; all data is passed in as arguments.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/pipeline-execution.md` §Cross-Domain Validation
//! for the full contract, matching algorithm, and examples.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    domain_services::{InterfaceDefinition, InterfaceMap},
    identifiers::{DomainServiceName, InterfaceId},
    types::DiagnosticSeverity,
};

// ─── Constraint finding ───────────────────────────────────────────────────────

/// A single cross-domain interface constraint violation.
///
/// Produced by [`validate_cross_domain_constraints`] when an interface
/// definition in the registry does not match what the domain service's
/// extraction tool found in the generated artifacts.
///
/// ## Usage
///
/// An empty `Vec<ConstraintFinding>` from
/// [`validate_cross_domain_constraints`] means all interfaces conform. Any
/// finding with [`DiagnosticSeverity::Blocking`] severity must block the review
/// gate from proceeding.
///
/// See `docs/spec/interfaces/pipeline-execution.md` §ConstraintFinding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintFinding {
    /// The interface definition that was violated.
    pub interface_id: InterfaceId,

    /// The parameter or field within the interface where the mismatch occurred.
    pub parameter_name: String,

    /// The value declared in the authoritative registry definition.
    pub expected_value: String,

    /// The value found in the domain service's extracted interface.
    pub actual_value: String,

    /// The domain service that owns (authored) the authoritative definition.
    pub owning_domain: DomainServiceName,

    /// The domain service whose extracted interface violates the contract.
    pub violating_domain: DomainServiceName,

    /// Severity of this violation.
    ///
    /// - [`DiagnosticSeverity::Blocking`]: the contract is structurally
    ///   incompatible (e.g. wrong type, missing required field). Must be
    ///   remediated before the review gate passes.
    /// - [`DiagnosticSeverity::Warning`]: a potential compatibility concern
    ///   (e.g. a deprecated field still present). Does not block the gate.
    /// - [`DiagnosticSeverity::Informational`]: informational alignment note.
    pub severity: DiagnosticSeverity,
}

// ─── Pure business logic function ────────────────────────────────────────────

/// Validates a set of extracted interface definitions against the
/// human-authored registry contracts.
///
/// Compares each interface definition in `contracts` (from the registry) with
/// the corresponding entry in `extracted` (from the domain service's
/// extraction tool). Returns a `Vec<ConstraintFinding>` describing every
/// mismatch found. An empty vec means full conformance.
///
/// ## Matching Algorithm
///
/// 1. For each `InterfaceDefinition` in `contracts`:
///    - Find the matching entry in `extracted.entries` by [`crate::InterfaceId`].
///    - If absent: emit a `Blocking` finding where `expected_value` is the
///      contract schema and `actual_value` is `"<not present>"`.
///    - If found: compare schemas field-by-field. Each field mismatch is one finding.
/// 2. Extra definitions in `extracted` that have no corresponding entry in
///    `contracts` are **not** reported — new interfaces are allowed. Only
///    missing or modified contracts are violations.
///
/// ## Parameters
///
/// - `contracts` — Interface definitions from the authoritative human-authored
///   registry (loaded by [`crate::InterfaceRegistryLoader::load_definitions`]).
/// - `extracted` — Interface definitions extracted from the generated artifacts
///   by [`crate::DomainServiceClient::extract_interfaces`].
///
/// ## Return Value
///
/// An empty `Vec` means all extracted interfaces conform to the registry
/// contracts. Any [`DiagnosticSeverity::Blocking`] findings must prevent the
/// review gate from proceeding.
///
/// # See also
///
/// `docs/spec/interfaces/pipeline-execution.md §validate_cross_domain_constraints`
#[must_use]
pub fn validate_cross_domain_constraints(
    contracts: &[InterfaceDefinition],
    extracted: &InterfaceMap,
) -> Vec<ConstraintFinding> {
    contracts
        .iter()
        .flat_map(|contract| validate_single_contract(contract, extracted))
        .collect()
}

/// Marker used as `parameter_name` when the whole interface is missing from
/// `extracted`, or when a non-object schema mismatches as a whole (there is
/// no single "field" to name in either case).
const MISSING_INTERFACE_MARKER: &str = "<interface>";

/// Marker used as `parameter_name` when a non-object (scalar/array/null)
/// schema mismatches as a whole. See the module doc comment and
/// `interfaces_tests.rs` for why non-object schemas have no defined "field".
const WHOLE_SCHEMA_MARKER: &str = "<schema>";

/// Sentinel `actual_value` for an interface or field that is absent from
/// `extracted`.
const NOT_PRESENT_MARKER: &str = "<not present>";

/// Validates one registry contract against `extracted`, independently of any
/// other contract (duplicate `InterfaceId`s in `contracts` are therefore each
/// validated on their own, per the algorithm's "for each contracts[i]"
/// wording).
fn validate_single_contract(
    contract: &InterfaceDefinition,
    extracted: &InterfaceMap,
) -> Vec<ConstraintFinding> {
    let Some(found) = extracted.entries.iter().find(|e| e.id == contract.id) else {
        return vec![missing_interface_finding(contract)];
    };
    compare_schemas(contract, found)
}

/// Builds the `Blocking` finding emitted when `contract`'s `InterfaceId` has
/// no matching entry in `extracted`.
fn missing_interface_finding(contract: &InterfaceDefinition) -> ConstraintFinding {
    ConstraintFinding {
        interface_id: contract.id.clone(),
        parameter_name: MISSING_INTERFACE_MARKER.to_string(),
        expected_value: contract.schema.to_string(),
        actual_value: NOT_PRESENT_MARKER.to_string(),
        owning_domain: contract.domain.clone(),
        violating_domain: contract.domain.clone(),
        severity: DiagnosticSeverity::Blocking,
    }
}

/// Compares `contract.schema` against `found.schema`. For a JSON object,
/// enumerates the contract's top-level keys and compares each one's value
/// (a missing top-level key in `found.schema` is reported the same way as a
/// missing interface). For any other JSON value kind (scalar, array, null),
/// compares the whole value and emits at most one finding.
fn compare_schemas(
    contract: &InterfaceDefinition,
    found: &InterfaceDefinition,
) -> Vec<ConstraintFinding> {
    match &contract.schema {
        Value::Object(fields) => fields
            .iter()
            .filter_map(|(key, expected)| field_mismatch(contract, found, key, expected))
            .collect(),
        expected if *expected == found.schema => vec![],
        expected => vec![whole_schema_mismatch(contract, found, expected)],
    }
}

/// Returns a finding when the top-level field `key` differs between
/// `contract.schema` and `found.schema` (or is absent from `found.schema`);
/// `None` when the field matches.
fn field_mismatch(
    contract: &InterfaceDefinition,
    found: &InterfaceDefinition,
    key: &str,
    expected: &Value,
) -> Option<ConstraintFinding> {
    let actual = found.schema.get(key);
    if actual == Some(expected) {
        return None;
    }
    Some(ConstraintFinding {
        interface_id: contract.id.clone(),
        parameter_name: key.to_string(),
        expected_value: expected.to_string(),
        actual_value: actual.map_or_else(|| NOT_PRESENT_MARKER.to_string(), Value::to_string),
        owning_domain: contract.domain.clone(),
        violating_domain: found.domain.clone(),
        severity: DiagnosticSeverity::Blocking,
    })
}

/// Builds the finding emitted when a non-object `contract.schema` differs
/// from `found.schema` as a whole.
fn whole_schema_mismatch(
    contract: &InterfaceDefinition,
    found: &InterfaceDefinition,
    expected: &Value,
) -> ConstraintFinding {
    ConstraintFinding {
        interface_id: contract.id.clone(),
        parameter_name: WHOLE_SCHEMA_MARKER.to_string(),
        expected_value: expected.to_string(),
        actual_value: found.schema.to_string(),
        owning_domain: contract.domain.clone(),
        violating_domain: found.domain.clone(),
        severity: DiagnosticSeverity::Blocking,
    }
}

#[cfg(test)]
#[path = "interfaces_tests.rs"]
mod tests;
