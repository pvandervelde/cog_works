//! Shared value types for the CogWorks pipeline domain.
//!
//! Unlike the newtype identifiers in [`crate::identifiers`], these types carry
//! meaningful values with invariants (e.g. scores are in `[0.0, 1.0]`, token
//! counts are non-negative integers) and participate in domain computations.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/shared-types.md` §Value Types for the full contract.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ArtifactPath;

// ---------------------------------------------------------------------------
// Token and cost types
// ---------------------------------------------------------------------------

/// Number of tokens consumed or budgeted in an LLM API call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TokenCount(u64);

impl TokenCount {
    /// Creates a [`TokenCount`] from a raw integer.
    pub fn new(count: u64) -> Self {
        Self(count)
    }

    /// Returns the underlying integer value.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Returns `true` if this count is zero.
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for TokenCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Add for TokenCount {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for TokenCount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

// ---------------------------------------------------------------------------

/// Monetary cost of LLM token usage, expressed in US dollars.
///
/// Used for per-call, per-node, and per-pipeline cost tracking. Arithmetic
/// operations are provided; callers are responsible for rounding to suitable
/// display precision.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TokenCost(f64);

impl TokenCost {
    /// Creates a [`TokenCost`] from a raw float value (USD).
    ///
    /// Returns `None` if `value` is negative, infinite, or NaN.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && value >= 0.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Creates a [`TokenCost`] of exactly zero.
    pub fn zero() -> Self {
        Self(0.0)
    }

    /// Returns the underlying `f64` value (USD).
    pub fn as_f64(self) -> f64 {
        self.0
    }

    /// Returns `true` if this cost is zero.
    pub fn is_zero(self) -> bool {
        self.0 == 0.0
    }
}

impl std::fmt::Display for TokenCost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "${:.6}", self.0)
    }
}

impl std::ops::Add for TokenCost {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for TokenCost {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

// ---------------------------------------------------------------------------

/// Maximum token cost permitted for a pipeline run, a node, or a parallel
/// budget window.
///
/// See `docs/spec/constraints.md` §Pipeline Graph — cost budget is shared
/// across parallel nodes.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CostBudget(f64);

impl CostBudget {
    /// Creates a [`CostBudget`] cap (USD).
    ///
    /// Returns `None` if `limit` is not strictly positive, infinite, or NaN.
    #[must_use]
    pub fn new(limit: f64) -> Option<Self> {
        if limit.is_finite() && limit > 0.0 {
            Some(Self(limit))
        } else {
            None
        }
    }

    /// Returns the budget limit as a `f64` (USD).
    pub fn as_f64(self) -> f64 {
        self.0
    }

    /// Returns `true` if `accumulated` equals or exceeds this budget.
    pub fn is_exceeded_by(self, accumulated: TokenCost) -> bool {
        accumulated.as_f64() >= self.0
    }
}

impl std::fmt::Display for CostBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "${:.6}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Score types
// ---------------------------------------------------------------------------

/// A scenario satisfaction score in the range `[0.0, 1.0]`.
///
/// Computed by `compute_satisfaction` from trajectory results. Compared against
/// the configured threshold to determine scenario pass/fail.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SatisfactionScore(f64);

impl SatisfactionScore {
    /// Creates a [`SatisfactionScore`], returning `None` if `value` is outside
    /// the valid range `[0.0, 1.0]`.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the score as an `f64` in `[0.0, 1.0]`.
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for SatisfactionScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

// ---------------------------------------------------------------------------

/// An alignment verification score in the range `[0.0, 1.0]`.
///
/// Used for both deterministic and LLM-semantic alignment checks. A blocking
/// finding always fails the check regardless of this score.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct AlignmentScore(f64);

impl AlignmentScore {
    /// Creates an [`AlignmentScore`], returning `None` if `value` is outside
    /// the valid range `[0.0, 1.0]`.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the score as an `f64` in `[0.0, 1.0]`.
    pub fn as_f64(self) -> f64 {
        self.0
    }
}

impl std::fmt::Display for AlignmentScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Severity level for a [`Diagnostic`] finding.
///
/// Used consistently across domain service responses, alignment findings, and
/// review results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// Finding that blocks progression; the relevant check fails.
    Blocking,
    /// Finding that should be addressed but does not block progression.
    Warning,
    /// Contextual information with no impact on progression.
    Informational,
}

// ---------------------------------------------------------------------------

/// Diagnostic category tag.
///
/// The standardised set is defined in `docs/spec/constraints.md` §Extension API.
/// Domain services may emit custom categories; consumers treat unknown categories
/// as [`DiagnosticSeverity::Informational`].
///
/// Examples: `"syntax_error"`, `"type_error"`, `"constraint_violation"`,
/// `"interface_mismatch"`, `"dependency_error"`, `"style_violation"`,
/// `"safety_concern"`, `"performance_concern"`, `"test_failure"`, `"completeness"`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiagnosticCategory(String);

impl DiagnosticCategory {
    /// Creates a [`DiagnosticCategory`] from a category string.
    ///
    /// Returns `None` if the string is empty.
    pub fn new(category: impl Into<String>) -> Option<Self> {
        let c = category.into();
        if c.is_empty() {
            None
        } else {
            Some(Self(c))
        }
    }

    /// Returns the category tag as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DiagnosticCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------

/// A structured diagnostic finding produced by a domain service, alignment
/// checker, or review pass.
///
/// Findings are accumulated and compared against severity thresholds to
/// determine whether a pipeline step may proceed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Artefact path the finding relates to (relative to the repository root).
    ///
    /// `None` for findings that are not file-specific (e.g. missing dependency).
    pub artifact: Option<ArtifactPath>,

    /// Human-readable location within the artefact (e.g. `"line 42, column 5"`).
    ///
    /// `None` when the finding applies to the whole artefact.
    pub location: Option<String>,

    /// Severity of this finding.
    pub severity: DiagnosticSeverity,

    /// Category tag (use standardised values where possible).
    pub category: DiagnosticCategory,

    /// Human-readable description of the finding.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Versioning
// ---------------------------------------------------------------------------

/// Semantic version of the Extension API protocol.
///
/// Additive changes bump `minor`; breaking changes bump `major`.
/// CogWorks and domain services negotiate compatibility during the handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ApiVersion {
    /// Major version — bumped on breaking changes.
    pub major: u32,
    /// Minor version — bumped on additive changes.
    pub minor: u32,
}

impl ApiVersion {
    /// Creates a new [`ApiVersion`].
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Returns `true` if `other` is compatible with `self`.
    ///
    /// Compatibility requires the same major version and `other.minor >= self.minor`.
    pub fn is_compatible_with(self, other: ApiVersion) -> bool {
        self.major == other.major && other.minor >= self.minor
    }
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

/// A UTC wall-clock timestamp.
///
/// Wraps [`chrono::DateTime<Utc>`] so callers never depend on `chrono` types
/// directly; the underlying representation can change without affecting the
/// domain API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Returns the current UTC time as a [`Timestamp`].
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Creates a [`Timestamp`] from a [`DateTime<Utc>`].
    pub fn from_utc(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }

    /// Returns the underlying [`DateTime<Utc>`].
    pub fn as_datetime(self) -> DateTime<Utc> {
        self.0
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}
