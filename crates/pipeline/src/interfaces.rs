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
    _contracts: &[InterfaceDefinition],
    _extracted: &InterfaceMap,
) -> Vec<ConstraintFinding> {
    todo!("See docs/spec/interfaces/pipeline-execution.md §validate_cross_domain_constraints")
}
