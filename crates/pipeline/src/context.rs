//! Context assembly, context pack selection, and classification result types.
//!
//! This module provides:
//!
//! - [`ClassificationResult`] / [`TaskType`] — the Intake node's classification
//!   of the work item. Drives context pack selection and safety gating throughout
//!   the rest of the pipeline run.
//! - [`ContextPriority`] / [`ContextItem`] / [`ContextPackage`] — the data
//!   structures representing assembled context for an LLM node invocation.
//! - [`ContextPackTrigger`] / [`ContextPack`] / [`MergedGuidance`] /
//!   [`LoadedContextPacks`] — the Context Pack type hierarchy.
//! - [`ContextAssemblyRequest`] — parameters for one context assembly call.
//! - [`select_context_packs`], [`merge_pack_guidance`], [`assemble_context`],
//!   [`apply_priority_truncation`], [`enforce_scenario_holdout`] — context
//!   assembly functions.
//!
//! ## Scenario Holdout Constraint
//!
//! [`enforce_scenario_holdout`] **must** be called before any context package
//! is used for code generation. Scenario files must never appear in a code
//! generation context (see `docs/spec/constraints.md` §Module Boundaries).
//! `assemble_context` calls this automatically; direct callers of
//! `apply_priority_truncation` must also call it first.
//!
//! ## Architectural Layer
//!
//! **Business logic.** No infrastructure dependencies.
//! `assemble_context` is `async` because it queries [`SummaryCache`], which is
//! an async port trait defined in `pipeline` and implemented in infrastructure.
//!
//! ## Specification
//!
//! See `docs/spec/interfaces/context.md` for the full contract, merge semantics,
//! and assembly algorithm.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    domain_services::InterfaceDefinition,
    graph::NodeType,
    identifiers::{ArtifactPath, ContextPackId},
    knowledge::{SummaryCache, SummaryLevel},
    types::{SatisfactionScore, TokenCount},
};

// ─── Classification result ───────────────────────────────────────────────────

/// The category of work the pipeline has been asked to perform, as determined
/// by the Intake node's classification step.
///
/// See `docs/spec/interfaces/context.md` §TaskType.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskType {
    /// Implementing new functionality.
    Feature,
    /// Correcting defective behaviour.
    BugFix,
    /// Updating or adding documentation only.
    Documentation,
    /// Restructuring code without changing observable behaviour.
    Refactoring,
    /// Changing configuration files or build settings.
    Configuration,
    /// Adding or updating tests without changing production code.
    Testing,
    /// Addressing a security vulnerability or hardening.
    Security,
    /// Could not be determined with sufficient confidence.
    Unknown,
}

// ---------------------------------------------------------------------------

/// The Intake node's classification of the work item.
///
/// All downstream pipeline nodes consume this value to customise their
/// behaviour — context pack selection, safety gating, scope checks.
///
/// Note: the processing functions that operate on `ClassificationResult`
/// (`apply_safety_override`, `check_scope_threshold`) are defined in
/// `classification.rs` (PR 7).
///
/// See `docs/spec/interfaces/context.md` §ClassificationResult.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// What category of work this is.
    pub task_type: TaskType,

    /// `true` if any affected module belongs to the safety-critical registry.
    ///
    /// Safety-affecting tasks require human approval before any PR is merged.
    /// This field may be overridden to `true` by `apply_safety_override` in
    /// `classification.rs` when the registry match happens post-classification.
    pub safety_affecting: bool,

    /// Magnitude estimate of the change (1 = trivial, 10 = very large).
    ///
    /// Used by `check_scope_threshold` (PR 7) to detect over-scoped tasks
    /// that should be split into smaller work items before proceeding.
    pub estimated_scope: u32,

    /// Repo-relative paths of modules the change is expected to touch.
    pub affected_modules: Vec<ArtifactPath>,
}

// ─── Context priority ────────────────────────────────────────────────────────

/// Priority rank for a context item, ordinal from highest to lowest.
///
/// Items with a lower discriminant value have higher priority and are retained
/// first when the token budget is exhausted. `Ord` is derived by declaration
/// order (smallest = most important).
///
/// See `docs/spec/interfaces/context.md` §ContextPriority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ContextPriority {
    /// Interface definitions for the artifact(s) the current node operates on.
    /// Most important — always included if it fits at all.
    CurrentInterfaceDefinition = 0,
    /// Output artifacts produced by nodes that the current node depends on.
    DirectDependencyOutput = 1,
    /// Constraints and principles from architectural decision records and
    /// constraint documents.
    ArchitecturalConstraints = 2,
    /// Domain knowledge provided by matched Context Packs.
    ContextPackKnowledge = 3,
    /// Coding standards, style guides, and linting rules.
    CodingStandards = 4,
    /// Summaries of modules transitively (but not directly) depended on.
    TransitiveDependency = 5,
}

// ─── Context item and package ─────────────────────────────────────────────────

/// A single unit of knowledge included in a node's context window.
///
/// See `docs/spec/interfaces/context.md` §ContextItem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    /// The verbatim text delivered to the LLM at this position in the context.
    pub content: String,

    /// The pyramid granularity level at which this entry was included.
    ///
    /// Used for observability: logging which levels were selected indicates
    /// how tight the token budget is.
    pub summary_level: SummaryLevel,

    /// Priority tier used for truncation ordering.
    pub priority: ContextPriority,

    /// Pre-computed approximate token count for `content`.
    ///
    /// The context assembler uses this for budget arithmetic. Must match the
    /// LLM provider's tokenisation (approximate counts are acceptable for
    /// budget enforcement; precision is not guaranteed).
    pub token_count: TokenCount,

    /// The artifact this item was derived from; `None` for synthesised items.
    ///
    /// `None` examples: merged pack guidance text, sub-work-item description.
    /// `enforce_scenario_holdout` filters by this field.
    pub source_path: Option<ArtifactPath>,
}

// ---------------------------------------------------------------------------

/// The complete assembled context for a single LLM node invocation.
///
/// Produced by [`assemble_context`] and [`apply_priority_truncation`].
///
/// See `docs/spec/interfaces/context.md` §ContextPackage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackage {
    /// Context items, ordered high-priority first.
    ///
    /// Within the same priority tier, items are ordered alphabetically by
    /// `source_path` for reproducibility.
    pub items: Vec<ContextItem>,

    /// Sum of `item.token_count` across all retained items.
    pub total_token_count: TokenCount,

    /// `true` if one or more items were dropped to fit within the token budget,
    /// or if a `SummaryCache` lookup failed during assembly.
    pub truncation_applied: bool,
}

// ─── Context pack types ───────────────────────────────────────────────────────

/// The conditions under which a Context Pack is selected for a pipeline run.
///
/// Pack selection uses OR semantics across fields: the pack is selected if **any**
/// matching criterion evaluates to `true`.
///
/// See `docs/spec/interfaces/context.md` §ContextPackTrigger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPackTrigger {
    /// Glob patterns matched against GitHub label strings on the work item.
    ///
    /// The pack fires if any pattern matches any label. Evaluated before
    /// `component_tag_patterns` and `requires_safety_critical`.
    pub label_patterns: Vec<String>,

    /// Glob patterns matched against each path in
    /// [`ClassificationResult::affected_modules`].
    ///
    /// The pack fires if any pattern matches any affected module path.
    pub component_tag_patterns: Vec<String>,

    /// If `true`, this pack is only selected when
    /// [`ClassificationResult::safety_affecting`] is `true`.
    pub requires_safety_critical: bool,
}

// ---------------------------------------------------------------------------

/// A domain-specific knowledge bundle loaded from `.cogworks/context-packs/`.
///
/// Each Context Pack provides domain knowledge text, coding pattern guidance,
/// artifact requirements, and optionally a tighter scenario threshold for its
/// domain. Multiple packs may be active for a single run; their guidance is
/// merged by [`merge_pack_guidance`].
///
/// See `docs/spec/interfaces/context.md` §ContextPack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPack {
    /// The pack identifier, matching its directory name in
    /// `.cogworks/context-packs/`.
    pub id: ContextPackId,

    /// The conditions under which this pack is selected.
    pub trigger: ContextPackTrigger,

    /// Domain knowledge text included at the [`ContextPriority::ContextPackKnowledge`]
    /// tier. Verbatim text; not subject to pyramid truncation.
    pub domain_knowledge: String,

    /// Code or design patterns that are correct for this domain.
    ///
    /// Merged into [`MergedGuidance::safe_patterns`] with union semantics.
    pub safe_patterns: Vec<String>,

    /// Code or design patterns that must be avoided in this domain.
    ///
    /// Merged into [`MergedGuidance::anti_patterns`] with union semantics.
    pub anti_patterns: Vec<String>,

    /// Artifacts that must be included in the context regardless of priority
    /// truncation.
    ///
    /// Merged into [`MergedGuidance::required_artifacts`] with
    /// deduplication-by-path union semantics.
    pub required_artifacts: Vec<ArtifactPath>,

    /// Per-pack scenario satisfaction threshold override.
    ///
    /// When multiple packs are active, the strictest (lowest) threshold wins.
    /// `None` defers to the pipeline-level default.
    pub scenario_threshold_override: Option<SatisfactionScore>,
}

// ---------------------------------------------------------------------------

/// Merged Context Pack guidance produced by [`merge_pack_guidance`].
///
/// `safe_patterns` and `anti_patterns` are union-merged across all active packs.
/// `required_artifacts` are union-merged with deduplication by path.
///
/// See `docs/spec/interfaces/context.md` §MergedGuidance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MergedGuidance {
    /// Union of `safe_patterns` from all matched packs.
    pub safe_patterns: Vec<String>,
    /// Union of `anti_patterns` from all matched packs.
    pub anti_patterns: Vec<String>,
    /// Deduplicated union of `required_artifacts` from all matched packs.
    pub required_artifacts: Vec<ArtifactPath>,
}

// ---------------------------------------------------------------------------

/// Output of context pack selection and guidance merging for one pipeline run.
///
/// Passed to [`assemble_context`] to inject pack knowledge and required artifacts.
///
/// See `docs/spec/interfaces/context.md` §LoadedContextPacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedContextPacks {
    /// All packs selected for this run. May be empty.
    pub matched_packs: Vec<ContextPack>,

    /// Pre-merged guidance from all matched packs.
    pub merged_guidance: MergedGuidance,

    /// Strictest scenario threshold across matched packs.
    ///
    /// If no pack overrides the threshold, this should be set to the
    /// pipeline-level default by the caller of [`merge_pack_guidance`].
    pub strictest_threshold: SatisfactionScore,
}

// ─── Context assembly request ─────────────────────────────────────────────────

/// All parameters needed to assemble a [`ContextPackage`] for one node invocation.
///
/// Constructed by node orchestration code in `nodes` and passed to
/// [`assemble_context`].
///
/// See `docs/spec/interfaces/context.md` §ContextAssemblyRequest.
#[derive(Debug, Clone)]
pub struct ContextAssemblyRequest {
    /// The type of node being executed, used to select appropriate detail levels.
    ///
    /// LLM nodes typically need finer-grained summaries; deterministic nodes
    /// may not need summaries at all.
    pub node_type: NodeType,

    /// Natural-language description of the sub-task being worked on.
    ///
    /// Included verbatim as user context within the assembled package.
    pub sub_work_item_description: String,

    /// Repo-relative paths of modules the node is expected to interact with.
    ///
    /// Per-module summaries are fetched from `SummaryCache` for each of these.
    pub affected_modules: Vec<ArtifactPath>,

    /// Repo-relative directories containing scenario specification files.
    ///
    /// Items derived from these paths are stripped by [`enforce_scenario_holdout`]
    /// before assembly. This enforces the hard holdout constraint.
    pub scenario_holdout_dirs: Vec<PathBuf>,

    /// Root of the pipeline's working directory checkout.
    ///
    /// Used to resolve relative paths when fetching summaries from the cache.
    pub pipeline_working_dir: PathBuf,
}

// ─── Context assembly functions ───────────────────────────────────────────────

/// Returns the IDs of all Context Packs whose triggers match `classification`.
///
/// A pack is selected if any of the following is true:
/// - Any `trigger.component_tag_patterns` glob matches any path in
///   `classification.affected_modules`.
/// - `trigger.requires_safety_critical` is `true` and
///   `classification.safety_affecting` is `true`.
///
/// Label patterns are not evaluated here (no label context available at
/// classification time). Callers with label context can apply a second filter.
///
/// Returns an empty `Vec` if no packs match. This is valid; the node proceeds
/// with no pack guidance.
///
/// **Infallible. Pure.**
///
/// See `docs/spec/interfaces/context.md` §select_context_packs.
pub fn select_context_packs(
    _classification: &ClassificationResult,
    _available: &[ContextPack],
) -> Vec<ContextPackId> {
    todo!("See docs/spec/interfaces/context.md §select_context_packs")
}

// ---------------------------------------------------------------------------

/// Merges the guidance fields from all selected Context Packs into a single
/// [`MergedGuidance`].
///
/// Returns an empty [`MergedGuidance`] for an empty `packs` slice.
///
/// **Merge semantics**:
/// - `safe_patterns` — union of all packs' safe patterns.
/// - `anti_patterns` — union of all packs' anti-patterns.
/// - `required_artifacts` — union with deduplication by path.
///
/// **Infallible. Pure.**
///
/// See `docs/spec/interfaces/context.md` §merge_pack_guidance.
pub fn merge_pack_guidance(_packs: &[ContextPack]) -> MergedGuidance {
    todo!("See docs/spec/interfaces/context.md §merge_pack_guidance")
}

// ---------------------------------------------------------------------------

/// Assembles a [`ContextPackage`] for one LLM node invocation.
///
/// Fetches summaries for required and affected artifacts from `summaries`,
/// combines them with interface definitions and pack guidance, enforces the
/// scenario holdout constraint, and truncates to the token budget.
///
/// # Arguments
///
/// - `req` — describes the node type, affected modules, and holdout directories.
/// - `summaries` — the pyramid summary cache; queried for each artifact path.
/// - `packs` — pre-loaded and merged Context Pack guidance and required artifacts.
/// - `interface_entries` — current interface definitions from the registry.
/// - `token_budget` — the maximum total token count for the returned package.
///
/// # Algorithm (in order)
///
/// 1. Convert `packs.merged_guidance.required_artifacts` + `req.affected_modules`
///    into `ContextItem` values, selecting the finest summary level that fits.
/// 2. Add `interface_entries` as `CurrentInterfaceDefinition` items.
/// 3. Add merged pack guidance as a `ContextPackKnowledge` item.
/// 4. Call `enforce_scenario_holdout` with `req.scenario_holdout_dirs`.
/// 5. Call `apply_priority_truncation` with `token_budget`.
///
/// # Error policy
///
/// If `SummaryCache::get_summary` returns an error for an artifact, that
/// artifact is **skipped** (not returned as an error). The resulting package
/// has `truncation_applied = true` to signal incomplete data.
///
/// See `docs/spec/interfaces/context.md` §assemble_context.
pub async fn assemble_context(
    _req: &ContextAssemblyRequest,
    _summaries: &dyn SummaryCache,
    _packs: &LoadedContextPacks,
    _interface_entries: &[InterfaceDefinition],
    _token_budget: TokenCount,
) -> ContextPackage {
    todo!("See docs/spec/interfaces/context.md §assemble_context")
}

// ---------------------------------------------------------------------------

/// Sorts context items by priority (highest first) and greedily fills the
/// token budget, returning a [`ContextPackage`].
///
/// Items at the same priority tier are ordered alphabetically by `source_path`
/// for reproducibility.
///
/// An item is included in full or excluded entirely. If a single item exceeds
/// the budget, it is **still included** (required artifacts must never be
/// silently dropped). `truncation_applied` is set `true` whenever any item
/// is dropped **or** when budget overflow occurs.
///
/// **Infallible. Pure.**
///
/// See `docs/spec/interfaces/context.md` §apply_priority_truncation.
pub fn apply_priority_truncation(_items: Vec<ContextItem>, _budget: TokenCount) -> ContextPackage {
    todo!("See docs/spec/interfaces/context.md §apply_priority_truncation")
}

// ---------------------------------------------------------------------------

/// Removes any context item whose `source_path` is rooted under one of the
/// holdout directories.
///
/// This is a **hard constraint**: scenario specifications must never be present
/// in code generation context (see `docs/spec/constraints.md` §Module Boundaries).
///
/// Items with `source_path == None` are never removed.
///
/// **Infallible. Pure.**
///
/// See `docs/spec/interfaces/context.md` §enforce_scenario_holdout.
pub fn enforce_scenario_holdout(
    _items: Vec<ContextItem>,
    _holdout_dirs: &[PathBuf],
) -> Vec<ContextItem> {
    todo!("See docs/spec/interfaces/context.md §enforce_scenario_holdout")
}
