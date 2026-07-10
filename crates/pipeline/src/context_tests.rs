//! Adversarial test suite for `context.rs` — context assembly functions.
//!
//! Functions under test:
//! - [`select_context_packs`] — glob-based trigger matching with OR semantics
//! - [`merge_pack_guidance`] — union-merge with required-artifact deduplication
//! - [`enforce_scenario_holdout`] — **HARD SAFETY CONSTRAINT** (ASSERT-SCEN-002)
//! - [`apply_priority_truncation`] — priority sort, greedy fill, overflow (ASSERT-CODE-006)
//! - [`assemble_context`] — orchestration of all steps above
//!
//! ## Tier map
//!
//! | Function | Tier 1 (spec) | Tier 2 (adversarial) | Tier 3 (proptest) |
//! |---|---|---|---|
//! | `select_context_packs` | 4 | 7 | 2 |
//! | `merge_pack_guidance` | 4 | 3 | 2 |
//! | `enforce_scenario_holdout` | 4 | 4 | 3 |
//! | `apply_priority_truncation` | 5 | 7 | 3 |
//! | `assemble_context` | 3 | 6 | 1 |
//!
//! All five functions have `todo!()` stubs. Every test below compiles (GREEN)
//! but panics at runtime (RED) until the implementation is written.
//!
//! ## Spec gap noted
//!
//! `enforce_scenario_holdout` says "prefix match on path string" but also
//! "rooted under any holdout_dir". These are ambiguous for sibling directories:
//! "spec/scenarios" is a raw-string prefix of "spec/scenarios-alt/foo.md".
//! The tests assume **directory-prefix semantics** ("spec/scenarios-alt" is NOT
//! rooted under "spec/scenarios"). See `test_enforce_scenario_holdout_sibling_directory_not_removed`.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use proptest::prelude::*;

use super::{
    apply_priority_truncation, assemble_context, enforce_scenario_holdout, merge_pack_guidance,
    select_context_packs, ClassificationResult, ContextAssemblyRequest, ContextItem,
    ContextPack, ContextPackTrigger, ContextPriority, HoldoutFilteredItems,
    LoadedContextPacks, MergedGuidance, TaskType,
};
use crate::{
    domain_services::InterfaceDefinition,
    graph::NodeType,
    identifiers::{ArtifactPath, CommitSha, ContextPackId, InterfaceId},
    knowledge::{CacheError, PyramidSummary, SummaryCache, SummaryLevel},
    types::{ApiVersion, SatisfactionScore, TokenCount},
    DomainServiceName,
};

// ─── Test helpers ─────────────────────────────────────────────────────────────

fn artifact(s: &str) -> ArtifactPath {
    ArtifactPath::new(s).expect("test artifact path must be valid")
}

fn pack_id(s: &str) -> ContextPackId {
    ContextPackId::new(s).expect("test pack id must be non-empty")
}

fn commit(s: &str) -> CommitSha {
    CommitSha::new(s).expect("test commit sha must be non-empty")
}

fn satisfaction(v: f64) -> SatisfactionScore {
    SatisfactionScore::new(v).expect("test satisfaction score must be in [0,1]")
}

fn tokens(n: u64) -> TokenCount {
    TokenCount::new(n)
}

fn make_classification(safety_affecting: bool, modules: &[&str]) -> ClassificationResult {
    ClassificationResult {
        task_type: TaskType::Feature,
        safety_affecting,
        estimated_scope: 3,
        affected_modules: modules.iter().map(|s| artifact(s)).collect(),
    }
}

fn make_trigger(
    label_patterns: &[&str],
    component_patterns: &[&str],
    requires_safety_critical: bool,
) -> ContextPackTrigger {
    ContextPackTrigger {
        label_patterns: label_patterns.iter().map(|s| s.to_string()).collect(),
        component_tag_patterns: component_patterns.iter().map(|s| s.to_string()).collect(),
        requires_safety_critical,
    }
}

fn make_pack(
    id: &str,
    label_patterns: &[&str],
    component_patterns: &[&str],
    requires_safety_critical: bool,
) -> ContextPack {
    ContextPack {
        id: pack_id(id),
        trigger: make_trigger(label_patterns, component_patterns, requires_safety_critical),
        domain_knowledge: format!("domain knowledge for {id}"),
        safe_patterns: vec![format!("{id}-safe")],
        anti_patterns: vec![format!("{id}-anti")],
        required_artifacts: vec![],
        scenario_threshold_override: None,
    }
}

/// Constructs a [`ContextItem`] with the given fields.
///
/// Uses `SummaryLevel::Paragraph` for all items; tests that care about level
/// should build the item manually.
fn make_item(
    content: &str,
    priority: ContextPriority,
    token_count: u64,
    source_path: Option<&str>,
) -> ContextItem {
    ContextItem {
        content: content.to_string(),
        summary_level: SummaryLevel::Paragraph,
        priority,
        token_count: tokens(token_count),
        source_path: source_path.map(artifact),
    }
}

fn make_pyramid_summary(path: &str, content: &str, token_count: u64) -> PyramidSummary {
    PyramidSummary {
        path: artifact(path),
        level: SummaryLevel::Paragraph,
        content: content.to_string(),
        commit_sha: commit("a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"),
        token_count: tokens(token_count),
    }
}

fn make_interface_def(id: &str) -> InterfaceDefinition {
    InterfaceDefinition {
        id: InterfaceId::new(id).expect("test interface id must be non-empty"),
        domain: DomainServiceName::new("test-domain").expect("test domain name"),
        schema: serde_json::json!({"type": "object"}),
        artifact_types: vec!["*.rs".to_string()],
        version: ApiVersion::new(1, 0),
    }
}

/// Builds [`LoadedContextPacks`] by manually setting merged guidance, avoiding
/// any call to the under-test `merge_pack_guidance` function.
fn empty_loaded_packs() -> LoadedContextPacks {
    LoadedContextPacks {
        matched_packs: vec![],
        merged_guidance: MergedGuidance::default(),
        strictest_threshold: satisfaction(0.8),
    }
}

/// Builds [`LoadedContextPacks`] with specific required artifacts but no matched packs.
fn loaded_packs_with_artifacts(required_artifacts: Vec<ArtifactPath>) -> LoadedContextPacks {
    LoadedContextPacks {
        matched_packs: vec![],
        merged_guidance: MergedGuidance {
            safe_patterns: vec![],
            anti_patterns: vec![],
            required_artifacts,
        },
        strictest_threshold: satisfaction(0.8),
    }
}

/// Builds [`LoadedContextPacks`] from packs, manually constructing the guidance
/// without calling `merge_pack_guidance` (which is a `todo!()`).
fn loaded_packs_from(packs: Vec<ContextPack>) -> LoadedContextPacks {
    let safe_patterns: Vec<String> = packs
        .iter()
        .flat_map(|p| p.safe_patterns.iter().cloned())
        .collect();
    let anti_patterns: Vec<String> = packs
        .iter()
        .flat_map(|p| p.anti_patterns.iter().cloned())
        .collect();
    let mut required_artifacts: Vec<ArtifactPath> = packs
        .iter()
        .flat_map(|p| p.required_artifacts.iter().cloned())
        .collect();
    required_artifacts.sort();
    required_artifacts.dedup();
    LoadedContextPacks {
        matched_packs: packs,
        merged_guidance: MergedGuidance {
            safe_patterns,
            anti_patterns,
            required_artifacts,
        },
        strictest_threshold: satisfaction(0.8),
    }
}

fn make_request(
    node_type: NodeType,
    affected_modules: &[&str],
    holdout_dirs: &[&str],
) -> ContextAssemblyRequest {
    ContextAssemblyRequest {
        node_type,
        sub_work_item_description: "Implement the context assembly functions.".to_string(),
        affected_modules: affected_modules.iter().map(|s| artifact(s)).collect(),
        scenario_holdout_dirs: holdout_dirs.iter().map(|s| artifact(s)).collect(),
        pipeline_working_dir: PathBuf::from("/repo"),
    }
}

// ─── Mock SummaryCache ────────────────────────────────────────────────────────

enum MockResult {
    Hit(PyramidSummary),
    Error { message: String },
}

struct MockSummaryCache {
    results: HashMap<(String, SummaryLevel), MockResult>,
}

impl MockSummaryCache {
    fn new() -> Self {
        Self {
            results: HashMap::new(),
        }
    }

    fn with_hit(mut self, path: &str, level: SummaryLevel, summary: PyramidSummary) -> Self {
        self.results
            .insert((path.to_string(), level), MockResult::Hit(summary));
        self
    }

    fn with_error(mut self, path: &str, level: SummaryLevel, message: &str) -> Self {
        self.results.insert(
            (path.to_string(), level),
            MockResult::Error {
                message: message.to_string(),
            },
        );
        self
    }
}

#[async_trait]
impl SummaryCache for MockSummaryCache {
    async fn get_summary(
        &self,
        path: &ArtifactPath,
        level: SummaryLevel,
    ) -> Result<Option<PyramidSummary>, CacheError> {
        match self.results.get(&(path.as_str().to_string(), level)) {
            Some(MockResult::Hit(s)) => Ok(Some(s.clone())),
            None => Ok(None),
            Some(MockResult::Error { message }) => Err(CacheError::Unavailable {
                message: message.clone(),
            }),
        }
    }

    async fn is_stale(
        &self,
        _path: &ArtifactPath,
        _current_sha: &CommitSha,
    ) -> Result<bool, CacheError> {
        Ok(false)
    }

    async fn invalidate(&self, _path: &ArtifactPath) -> Result<(), CacheError> {
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// select_context_packs
// ═══════════════════════════════════════════════════════════════════════════

// ─── Tier 1: Specification Tests ──────────────────────────────────────────────

#[test]
fn test_select_context_packs_matching_label_pattern_returns_pack_id() {
    let classification = make_classification(false, &[]);
    let labels = vec!["security-review".to_string()];
    let packs = vec![make_pack("security", &["security-*"], &[], false)];

    let result = select_context_packs(&classification, &labels, &packs);

    assert_eq!(result, vec![pack_id("security")]);
}

#[test]
fn test_select_context_packs_matching_component_tag_pattern_returns_pack_id() {
    let classification = make_classification(false, &["crates/pipeline/src/security.rs"]);
    let packs = vec![make_pack("pipeline", &[], &["crates/pipeline/**"], false)];

    let result = select_context_packs(&classification, &[], &packs);

    assert_eq!(result, vec![pack_id("pipeline")]);
}

#[test]
fn test_select_context_packs_safety_critical_pack_selected_when_safety_affecting_true() {
    let classification = make_classification(true, &[]);
    let packs = vec![make_pack("safety", &[], &[], true)];

    let result = select_context_packs(&classification, &[], &packs);

    assert_eq!(result, vec![pack_id("safety")]);
}

#[test]
fn test_select_context_packs_no_trigger_match_returns_empty_vec() {
    let classification = make_classification(false, &["src/main.rs"]);
    let labels = vec!["documentation".to_string()];
    let packs = vec![make_pack("security", &["security-*"], &["crates/security/**"], false)];

    let result = select_context_packs(&classification, &labels, &packs);

    assert!(result.is_empty());
}

// ─── Tier 2: Adversarial Tests ────────────────────────────────────────────────

#[test]
fn test_select_context_packs_or_semantics_label_match_when_component_doesnt() {
    // A pack fires if ANY criterion matches. Label matches → selected even though
    // affected_modules has no component_tag match.
    let classification = make_classification(false, &["src/unrelated.rs"]);
    let labels = vec!["my-security-label".to_string()];
    let packs = vec![make_pack("sec", &["*security*"], &["crates/security/**"], false)];

    let result = select_context_packs(&classification, &labels, &packs);

    assert_eq!(result, vec![pack_id("sec")]);
}

#[test]
fn test_select_context_packs_requires_safety_critical_not_selected_when_not_safety_affecting() {
    // requires_safety_critical=true but classification.safety_affecting=false → no match.
    let classification = make_classification(false, &[]);
    let packs = vec![make_pack("safety", &[], &[], true)];

    let result = select_context_packs(&classification, &[], &packs);

    assert!(result.is_empty());
}

#[test]
fn test_select_context_packs_empty_available_returns_empty_vec() {
    let classification = make_classification(true, &["src/lib.rs"]);
    let labels = vec!["feature".to_string()];

    let result = select_context_packs(&classification, &labels, &[]);

    assert!(result.is_empty());
}

#[test]
fn test_select_context_packs_multiple_matching_packs_all_ids_returned() {
    let classification = make_classification(false, &["crates/pipeline/src/lib.rs"]);
    let labels = vec!["rust".to_string()];
    let packs = vec![
        make_pack("rust-pack", &["rust"], &[], false),
        make_pack("pipeline-pack", &[], &["crates/pipeline/**"], false),
        make_pack("unrelated", &["java"], &["crates/java/**"], false),
    ];

    let mut result = select_context_packs(&classification, &labels, &packs);
    result.sort();

    assert_eq!(result.len(), 2);
    assert!(result.contains(&pack_id("rust-pack")));
    assert!(result.contains(&pack_id("pipeline-pack")));
    assert!(!result.contains(&pack_id("unrelated")));
}

#[test]
fn test_select_context_packs_empty_labels_slice_still_matches_component_tag() {
    let classification = make_classification(false, &["crates/pipeline/src/lib.rs"]);
    let packs = vec![make_pack("pipeline", &[], &["crates/pipeline/**"], false)];

    let result = select_context_packs(&classification, &[], &packs);

    assert_eq!(result, vec![pack_id("pipeline")]);
}

#[test]
fn test_select_context_packs_glob_double_star_matches_deeply_nested_path() {
    let classification =
        make_classification(false, &["crates/pipeline/src/deep/nested/module.rs"]);
    let packs = vec![make_pack("all-crates", &[], &["crates/**"], false)];

    let result = select_context_packs(&classification, &[], &packs);

    assert_eq!(result, vec![pack_id("all-crates")]);
}

#[test]
fn test_select_context_packs_pack_with_all_empty_trigger_fields_never_matches() {
    // A pack whose trigger has no patterns and requires_safety_critical=false
    // can never fire for any input.
    let classification = make_classification(false, &["src/lib.rs"]);
    let labels = vec!["some-label".to_string()];
    let packs = vec![ContextPack {
        id: pack_id("empty-trigger"),
        trigger: make_trigger(&[], &[], false),
        domain_knowledge: "nothing".to_string(),
        safe_patterns: vec![],
        anti_patterns: vec![],
        required_artifacts: vec![],
        scenario_threshold_override: None,
    }];

    let result = select_context_packs(&classification, &labels, &packs);

    assert!(result.is_empty());
}

#[test]
fn test_select_context_packs_same_pack_appears_at_most_once_when_multiple_fields_match() {
    // A pack that matches via BOTH label AND component tag must appear only once.
    let classification = make_classification(true, &["src/lib.rs"]);
    let labels = vec!["label-match".to_string()];
    let packs = vec![make_pack("triple-match", &["label-match"], &["src/**"], true)];

    let result = select_context_packs(&classification, &labels, &packs);

    assert_eq!(result.len(), 1, "same pack must not be duplicated");
    assert_eq!(result[0], pack_id("triple-match"));
}

// ─── Tier 3: Property-Based Tests ─────────────────────────────────────────────

proptest! {
    #[test]
    fn test_select_context_packs_result_length_never_exceeds_available_packs(
        safety in any::<bool>(),
        has_label_match in any::<bool>(),
    ) {
        let classification = make_classification(safety, &[]);
        let labels: Vec<String> = if has_label_match {
            vec!["match-me".to_string()]
        } else {
            vec![]
        };
        let packs = vec![
            make_pack("a", &["match-me"], &[], false),
            make_pack("b", &[], &[], false),
            make_pack("c", &[], &[], true),
        ];

        let result = select_context_packs(&classification, &labels, &packs);

        prop_assert!(result.len() <= packs.len());
    }

    #[test]
    fn test_select_context_packs_result_contains_no_duplicate_pack_ids(
        safety in any::<bool>(),
    ) {
        let classification = make_classification(safety, &["src/lib.rs"]);
        let labels = vec!["label-a".to_string()];
        // This pack matches all three trigger fields when safety=true.
        let packs = vec![make_pack("dup", &["label-a"], &["src/**"], safety)];

        let result = select_context_packs(&classification, &labels, &packs);

        let mut sorted = result.clone();
        sorted.sort();
        sorted.dedup();
        prop_assert_eq!(
            result.len(),
            sorted.len(),
            "duplicate pack IDs must not appear in result"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// merge_pack_guidance
// ═══════════════════════════════════════════════════════════════════════════

// ─── Tier 1: Specification Tests ──────────────────────────────────────────────

#[test]
fn test_merge_pack_guidance_empty_slice_returns_empty_merged_guidance() {
    let result = merge_pack_guidance(&[]);

    assert!(result.safe_patterns.is_empty());
    assert!(result.anti_patterns.is_empty());
    assert!(result.required_artifacts.is_empty());
}

#[test]
fn test_merge_pack_guidance_single_pack_returns_all_its_fields_verbatim() {
    let mut pack = make_pack("single", &[], &[], false);
    pack.safe_patterns = vec!["use-async-await".to_string()];
    pack.anti_patterns = vec!["avoid-unwrap".to_string()];
    pack.required_artifacts = vec![artifact("docs/spec/architecture.md")];

    let result = merge_pack_guidance(&[pack]);

    assert_eq!(result.safe_patterns, vec!["use-async-await"]);
    assert_eq!(result.anti_patterns, vec!["avoid-unwrap"]);
    assert_eq!(
        result.required_artifacts,
        vec![artifact("docs/spec/architecture.md")]
    );
}

#[test]
fn test_merge_pack_guidance_two_packs_safe_patterns_union_merged() {
    let mut pack_a = make_pack("a", &[], &[], false);
    pack_a.safe_patterns = vec!["pattern-a".to_string()];
    pack_a.anti_patterns = vec![];
    pack_a.required_artifacts = vec![];

    let mut pack_b = make_pack("b", &[], &[], false);
    pack_b.safe_patterns = vec!["pattern-b".to_string()];
    pack_b.anti_patterns = vec![];
    pack_b.required_artifacts = vec![];

    let result = merge_pack_guidance(&[pack_a, pack_b]);

    assert!(result.safe_patterns.contains(&"pattern-a".to_string()));
    assert!(result.safe_patterns.contains(&"pattern-b".to_string()));
}

#[test]
fn test_merge_pack_guidance_two_packs_anti_patterns_union_merged() {
    let mut pack_a = make_pack("a", &[], &[], false);
    pack_a.anti_patterns = vec!["anti-a".to_string()];
    pack_a.safe_patterns = vec![];
    pack_a.required_artifacts = vec![];

    let mut pack_b = make_pack("b", &[], &[], false);
    pack_b.anti_patterns = vec!["anti-b".to_string()];
    pack_b.safe_patterns = vec![];
    pack_b.required_artifacts = vec![];

    let result = merge_pack_guidance(&[pack_a, pack_b]);

    assert!(result.anti_patterns.contains(&"anti-a".to_string()));
    assert!(result.anti_patterns.contains(&"anti-b".to_string()));
}

// ─── Tier 2: Adversarial Tests ────────────────────────────────────────────────

#[test]
fn test_merge_pack_guidance_duplicate_required_artifact_path_deduplicated() {
    let dup_path = artifact("docs/architecture.md");

    let mut pack_a = make_pack("a", &[], &[], false);
    pack_a.required_artifacts = vec![dup_path.clone()];
    pack_a.safe_patterns = vec![];
    pack_a.anti_patterns = vec![];

    let mut pack_b = make_pack("b", &[], &[], false);
    pack_b.required_artifacts = vec![dup_path.clone()];
    pack_b.safe_patterns = vec![];
    pack_b.anti_patterns = vec![];

    let result = merge_pack_guidance(&[pack_a, pack_b]);

    let count = result
        .required_artifacts
        .iter()
        .filter(|a| **a == dup_path)
        .count();
    assert_eq!(
        count, 1,
        "duplicate artifact path must appear exactly once in merged guidance"
    );
}

#[test]
fn test_merge_pack_guidance_distinct_required_artifacts_all_present() {
    let path_a = artifact("docs/a.md");
    let path_b = artifact("docs/b.md");

    let mut pack_a = make_pack("a", &[], &[], false);
    pack_a.required_artifacts = vec![path_a.clone()];
    pack_a.safe_patterns = vec![];
    pack_a.anti_patterns = vec![];

    let mut pack_b = make_pack("b", &[], &[], false);
    pack_b.required_artifacts = vec![path_b.clone()];
    pack_b.safe_patterns = vec![];
    pack_b.anti_patterns = vec![];

    let result = merge_pack_guidance(&[pack_a, pack_b]);

    assert!(result.required_artifacts.contains(&path_a));
    assert!(result.required_artifacts.contains(&path_b));
}

#[test]
fn test_merge_pack_guidance_three_packs_shared_artifact_deduped_unique_artifacts_present() {
    let shared = artifact("shared/types.rs");
    let unique_b = artifact("b/specific.rs");
    let unique_c = artifact("c/specific.rs");

    let mut pack_a = make_pack("a", &[], &[], false);
    pack_a.required_artifacts = vec![shared.clone()];
    pack_a.safe_patterns = vec![];
    pack_a.anti_patterns = vec![];

    let mut pack_b = make_pack("b", &[], &[], false);
    pack_b.required_artifacts = vec![shared.clone(), unique_b.clone()];
    pack_b.safe_patterns = vec![];
    pack_b.anti_patterns = vec![];

    let mut pack_c = make_pack("c", &[], &[], false);
    pack_c.required_artifacts = vec![shared.clone(), unique_c.clone()];
    pack_c.safe_patterns = vec![];
    pack_c.anti_patterns = vec![];

    let result = merge_pack_guidance(&[pack_a, pack_b, pack_c]);

    let shared_count = result
        .required_artifacts
        .iter()
        .filter(|a| **a == shared)
        .count();
    assert_eq!(shared_count, 1, "shared artifact must appear exactly once");
    assert!(result.required_artifacts.contains(&unique_b));
    assert!(result.required_artifacts.contains(&unique_c));
    assert_eq!(result.required_artifacts.len(), 3);
}

// ─── Tier 3: Property-Based Tests ─────────────────────────────────────────────

proptest! {
    #[test]
    fn test_merge_pack_guidance_required_artifacts_never_contains_duplicates(
        num_packs in 1usize..6,
    ) {
        // All packs contribute the same artifact → merged result must have it exactly once.
        let shared_path = artifact("shared/types.rs");
        let packs: Vec<ContextPack> = (0..num_packs)
            .map(|i| {
                let mut p = make_pack(&format!("pack-{i}"), &[], &[], false);
                p.required_artifacts = vec![shared_path.clone()];
                p.safe_patterns = vec![];
                p.anti_patterns = vec![];
                p
            })
            .collect();

        let result = merge_pack_guidance(&packs);

        let mut sorted = result.required_artifacts.clone();
        sorted.sort();
        let dup_count = sorted.windows(2).filter(|w| w[0] == w[1]).count();
        prop_assert_eq!(dup_count, 0, "merged artifacts must never contain duplicates");
    }

    #[test]
    fn test_merge_pack_guidance_all_patterns_from_all_packs_present(
        num_packs in 1usize..5,
    ) {
        let packs: Vec<ContextPack> = (0..num_packs)
            .map(|i| {
                let mut p = make_pack(&format!("pack-{i}"), &[], &[], false);
                p.safe_patterns = vec![format!("safe-{i}")];
                p.anti_patterns = vec![format!("anti-{i}")];
                p.required_artifacts = vec![];
                p
            })
            .collect();

        let result = merge_pack_guidance(&packs);

        for i in 0..num_packs {
            prop_assert!(
                result.safe_patterns.contains(&format!("safe-{i}")),
                "safe pattern safe-{i} must be in merged result"
            );
            prop_assert!(
                result.anti_patterns.contains(&format!("anti-{i}")),
                "anti pattern anti-{i} must be in merged result"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// enforce_scenario_holdout   — HARD SAFETY CONSTRAINT (ASSERT-SCEN-002)
// ═══════════════════════════════════════════════════════════════════════════

// ─── Tier 1: Specification Tests (must all hold in any correct implementation) ─

#[test]
fn test_enforce_scenario_holdout_item_rooted_under_holdout_dir_is_removed() {
    let items = vec![make_item(
        "scenario content",
        ContextPriority::TransitiveDependency,
        50,
        Some("spec/scenarios/test-01.md"),
    )];
    let holdout_dirs = vec![artifact("spec/scenarios")];

    let result = enforce_scenario_holdout(items, &holdout_dirs);

    assert!(
        result.into_inner().is_empty(),
        "ASSERT-SCEN-002: scenario item rooted under holdout dir must be removed"
    );
}

#[test]
fn test_enforce_scenario_holdout_item_outside_all_holdout_dirs_is_kept() {
    let items = vec![make_item(
        "architecture content",
        ContextPriority::ArchitecturalConstraints,
        100,
        Some("docs/spec/architecture.md"),
    )];
    let holdout_dirs = vec![artifact("spec/scenarios")];

    let result = enforce_scenario_holdout(items, &holdout_dirs);

    assert_eq!(
        result.into_inner().len(),
        1,
        "non-scenario item outside holdout dirs must be retained"
    );
}

#[test]
fn test_enforce_scenario_holdout_source_path_none_item_is_never_removed() {
    // ASSERT-SCEN-002: synthesised items (pack guidance, sub-work-item descriptions)
    // with source_path=None must never be removed, regardless of holdout dirs.
    let items = vec![make_item(
        "merged pack guidance",
        ContextPriority::ContextPackKnowledge,
        200,
        None,
    )];
    let holdout_dirs = vec![
        artifact("spec/scenarios"),
        artifact("tests/scenarios"),
    ];

    let result = enforce_scenario_holdout(items, &holdout_dirs);

    assert_eq!(
        result.into_inner().len(),
        1,
        "None-source items must never be removed by holdout filter"
    );
}

#[test]
fn test_enforce_scenario_holdout_empty_holdout_dirs_removes_nothing() {
    let items = vec![
        make_item(
            "item-a",
            ContextPriority::CodingStandards,
            50,
            Some("spec/scenarios/foo.md"),
        ),
        make_item("item-b", ContextPriority::CodingStandards, 50, Some("src/lib.rs")),
    ];

    let result = enforce_scenario_holdout(items, &[]);

    assert_eq!(
        result.into_inner().len(),
        2,
        "empty holdout dirs list must remove nothing"
    );
}

// ─── Tier 2: Adversarial Tests ────────────────────────────────────────────────

#[test]
fn test_enforce_scenario_holdout_empty_item_list_returns_empty() {
    let result = enforce_scenario_holdout(vec![], &[artifact("spec/scenarios")]);

    assert!(result.into_inner().is_empty());
}

#[test]
fn test_enforce_scenario_holdout_deeply_nested_path_under_holdout_dir_is_removed() {
    let items = vec![make_item(
        "deeply nested scenario",
        ContextPriority::TransitiveDependency,
        30,
        Some("spec/scenarios/feature/auth/login-test.md"),
    )];
    let holdout_dirs = vec![artifact("spec/scenarios")];

    let result = enforce_scenario_holdout(items, &holdout_dirs);

    assert!(
        result.into_inner().is_empty(),
        "deeply nested path under holdout dir must be removed"
    );
}

#[test]
fn test_enforce_scenario_holdout_multiple_holdout_dirs_removes_from_all() {
    let items = vec![
        make_item(
            "scenario-1",
            ContextPriority::TransitiveDependency,
            30,
            Some("spec/scenarios/s1.md"),
        ),
        make_item(
            "scenario-2",
            ContextPriority::TransitiveDependency,
            30,
            Some("tests/scenarios/s2.md"),
        ),
        make_item(
            "keep-me",
            ContextPriority::ArchitecturalConstraints,
            50,
            Some("docs/architecture.md"),
        ),
    ];
    let holdout_dirs = vec![artifact("spec/scenarios"), artifact("tests/scenarios")];

    let result = enforce_scenario_holdout(items, &holdout_dirs);
    let inner = result.into_inner();

    assert_eq!(inner.len(), 1, "only the non-scenario item must survive");
    assert_eq!(inner[0].content, "keep-me");
}

#[test]
fn test_enforce_scenario_holdout_sibling_directory_not_removed() {
    // "spec/scenarios-alt" is a SIBLING of "spec/scenarios", not rooted under it.
    // Directory-prefix semantics: "spec/scenarios" does NOT match
    // "spec/scenarios-alt/test.md" because there is no separator after "scenarios".
    //
    // Spec gap: the spec says "prefix match on path string" which is ambiguous.
    // This test encodes the SAFER interpretation: rooted-under = directory prefix.
    // See module-level doc for the spec gap report.
    let items = vec![make_item(
        "alt scenario",
        ContextPriority::TransitiveDependency,
        30,
        Some("spec/scenarios-alt/test.md"),
    )];
    let holdout_dirs = vec![artifact("spec/scenarios")];

    let result = enforce_scenario_holdout(items, &holdout_dirs);

    assert_eq!(
        result.into_inner().len(),
        1,
        "spec/scenarios-alt is a sibling of spec/scenarios — must NOT be removed"
    );
}

#[test]
fn test_enforce_scenario_holdout_mixed_none_and_path_items_only_holdout_paths_filtered() {
    let items = vec![
        make_item("synthesised", ContextPriority::ContextPackKnowledge, 100, None),
        make_item(
            "scenario",
            ContextPriority::TransitiveDependency,
            50,
            Some("spec/scenarios/foo.md"),
        ),
        make_item(
            "code",
            ContextPriority::CurrentInterfaceDefinition,
            200,
            Some("src/lib.rs"),
        ),
    ];
    let holdout_dirs = vec![artifact("spec/scenarios")];

    let result = enforce_scenario_holdout(items, &holdout_dirs);
    let inner = result.into_inner();

    assert_eq!(inner.len(), 2, "only the scenario item must be removed");
    assert!(
        inner.iter().all(|i| i.content != "scenario"),
        "scenario item must not appear in filtered result"
    );
}

// ─── Tier 3: Property-Based Tests ─────────────────────────────────────────────

proptest! {
    /// None-source items are NEVER removed, regardless of holdout dirs or count.
    #[test]
    fn test_enforce_scenario_holdout_none_source_path_items_always_preserved(
        num_items in 1usize..10,
    ) {
        let items: Vec<ContextItem> = (0..num_items)
            .map(|i| make_item(
                &format!("synthesised-{i}"),
                ContextPriority::ContextPackKnowledge,
                50,
                None,
            ))
            .collect();
        let holdout_dirs = vec![
            artifact("spec/scenarios"),
            artifact("tests/scenarios"),
            artifact("features"),
        ];

        let result = enforce_scenario_holdout(items, &holdout_dirs);

        prop_assert_eq!(
            result.into_inner().len(),
            num_items,
            "all None-source items must survive holdout filtering"
        );
    }

    /// Items whose source_path starts with a holdout dir prefix are ALWAYS removed.
    #[test]
    fn test_enforce_scenario_holdout_holdout_path_items_always_removed(
        num_holdout in 1usize..6,
    ) {
        let holdout_dir = artifact("spec/scenarios");
        let items: Vec<ContextItem> = (0..num_holdout)
            .map(|i| make_item(
                &format!("scenario-{i}"),
                ContextPriority::TransitiveDependency,
                20,
                Some(&format!("spec/scenarios/file-{i}.md")),
            ))
            .collect();

        let result = enforce_scenario_holdout(items, &[holdout_dir]);

        prop_assert_eq!(
            result.into_inner().len(),
            0,
            "all scenario items must be removed by holdout filter"
        );
    }

    /// Non-holdout items with source paths outside all holdout dirs always survive.
    #[test]
    fn test_enforce_scenario_holdout_non_holdout_path_items_always_preserved(
        num_items in 1usize..8,
    ) {
        let items: Vec<ContextItem> = (0..num_items)
            .map(|i| make_item(
                &format!("code-{i}"),
                ContextPriority::CodingStandards,
                30,
                Some(&format!("src/module_{i}.rs")),
            ))
            .collect();
        let holdout_dirs = vec![artifact("spec/scenarios")];

        let result = enforce_scenario_holdout(items, &holdout_dirs);

        prop_assert_eq!(
            result.into_inner().len(),
            num_items,
            "all non-holdout path items must survive"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// apply_priority_truncation
// ═══════════════════════════════════════════════════════════════════════════

// ─── Tier 1: Specification Tests ──────────────────────────────────────────────

#[test]
fn test_apply_priority_truncation_all_items_fit_budget_all_included_no_truncation() {
    let items = HoldoutFilteredItems(vec![
        make_item("high-prio", ContextPriority::CurrentInterfaceDefinition, 100, None),
        make_item("low-prio", ContextPriority::TransitiveDependency, 50, None),
    ]);

    let result = apply_priority_truncation(items, tokens(200));

    assert_eq!(result.items.len(), 2);
    assert!(!result.truncation_applied);
}

#[test]
fn test_apply_priority_truncation_items_sorted_by_priority_highest_first() {
    let items = HoldoutFilteredItems(vec![
        make_item("transitive", ContextPriority::TransitiveDependency, 10, None),
        make_item("coding-std", ContextPriority::CodingStandards, 10, None),
        make_item("interface", ContextPriority::CurrentInterfaceDefinition, 10, None),
    ]);

    let result = apply_priority_truncation(items, tokens(1000));

    assert_eq!(result.items[0].content, "interface");
    assert_eq!(result.items[1].content, "coding-std");
    assert_eq!(result.items[2].content, "transitive");
}

#[test]
fn test_apply_priority_truncation_lowest_priority_item_dropped_when_budget_exceeded() {
    // Budget: 150. Both items are 100 tokens. High-prio fits; low-prio dropped.
    let items = HoldoutFilteredItems(vec![
        make_item("low-prio", ContextPriority::TransitiveDependency, 100, None),
        make_item("high-prio", ContextPriority::CurrentInterfaceDefinition, 100, None),
    ]);

    let result = apply_priority_truncation(items, tokens(150));

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].content, "high-prio");
    assert!(result.truncation_applied);
}

#[test]
fn test_apply_priority_truncation_single_item_exceeding_budget_still_included() {
    // ASSERT-CODE-006 / required-artifact overflow: a single item that exceeds
    // the entire budget must still be included (never silently dropped).
    let items = HoldoutFilteredItems(vec![make_item(
        "huge-artifact",
        ContextPriority::CurrentInterfaceDefinition,
        10_000,
        Some("src/huge.rs"),
    )]);

    let result = apply_priority_truncation(items, tokens(100));

    assert_eq!(result.items.len(), 1);
    assert!(
        result.truncation_applied,
        "budget overflow must set truncation_applied=true"
    );
}

#[test]
fn test_apply_priority_truncation_empty_input_returns_empty_package_no_truncation() {
    let result = apply_priority_truncation(HoldoutFilteredItems(vec![]), tokens(1000));

    assert!(result.items.is_empty());
    assert_eq!(result.total_token_count, tokens(0));
    assert!(!result.truncation_applied);
    assert!(result.assembly_errors.is_empty());
}

// ─── Tier 2: Adversarial Tests ────────────────────────────────────────────────

#[test]
fn test_apply_priority_truncation_total_token_count_equals_sum_of_included_items() {
    let items = HoldoutFilteredItems(vec![
        make_item("a", ContextPriority::CurrentInterfaceDefinition, 100, None),
        make_item("b", ContextPriority::CodingStandards, 50, None),
        make_item("c", ContextPriority::TransitiveDependency, 200, None),
    ]);

    let result = apply_priority_truncation(items, tokens(1000));

    assert_eq!(result.total_token_count, tokens(350));
}

#[test]
fn test_apply_priority_truncation_total_token_count_excludes_dropped_items() {
    let items = HoldoutFilteredItems(vec![
        make_item("kept", ContextPriority::CurrentInterfaceDefinition, 80, None),
        make_item("dropped", ContextPriority::TransitiveDependency, 80, None),
    ]);

    let result = apply_priority_truncation(items, tokens(100));

    assert_eq!(
        result.total_token_count,
        tokens(80),
        "total_token_count must only count retained items"
    );
}

#[test]
fn test_apply_priority_truncation_same_priority_items_sorted_alphabetically_by_source_path() {
    let items = HoldoutFilteredItems(vec![
        make_item("c-content", ContextPriority::CodingStandards, 10, Some("src/z_module.rs")),
        make_item("a-content", ContextPriority::CodingStandards, 10, Some("src/a_module.rs")),
        make_item("b-content", ContextPriority::CodingStandards, 10, Some("src/m_module.rs")),
    ]);

    let result = apply_priority_truncation(items, tokens(1000));

    assert_eq!(result.items[0].content, "a-content");
    assert_eq!(result.items[1].content, "b-content");
    assert_eq!(result.items[2].content, "c-content");
}

#[test]
fn test_apply_priority_truncation_exactly_at_budget_includes_all_no_truncation() {
    let items = HoldoutFilteredItems(vec![
        make_item("a", ContextPriority::CurrentInterfaceDefinition, 50, None),
        make_item("b", ContextPriority::CodingStandards, 50, None),
    ]);

    let result = apply_priority_truncation(items, tokens(100));

    assert_eq!(result.items.len(), 2);
    assert!(!result.truncation_applied);
}

#[test]
fn test_apply_priority_truncation_one_token_over_budget_drops_last_item() {
    let items = HoldoutFilteredItems(vec![
        make_item("a", ContextPriority::CurrentInterfaceDefinition, 50, None),
        // 50 + 52 = 102 > budget 101 → second item dropped.
        make_item("b", ContextPriority::CodingStandards, 52, None),
    ]);

    let result = apply_priority_truncation(items, tokens(101));

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].content, "a");
    assert!(result.truncation_applied);
}

#[test]
fn test_apply_priority_truncation_greedy_fill_includes_partial_lower_priority_tier() {
    // Budget: 250. Interface=100, two CodingStandards=100 each (alphabetical: a < b).
    // Greedy fill: interface(100) + std-a(100) = 200 ≤ 250; std-b(100) → 300 > 250 → dropped.
    let items = HoldoutFilteredItems(vec![
        make_item(
            "interface",
            ContextPriority::CurrentInterfaceDefinition,
            100,
            Some("src/interface.rs"),
        ),
        make_item("std-a", ContextPriority::CodingStandards, 100, Some("src/a_std.rs")),
        make_item("std-b", ContextPriority::CodingStandards, 100, Some("src/b_std.rs")),
    ]);

    let result = apply_priority_truncation(items, tokens(250));

    assert_eq!(result.items.len(), 2);
    assert!(result.items.iter().any(|i| i.content == "interface"));
    assert!(result.items.iter().any(|i| i.content == "std-a"));
    assert!(result.truncation_applied);
}

#[test]
fn test_apply_priority_truncation_higher_priority_item_always_included_before_lower() {
    // With a budget that fits only one item, the CurrentInterfaceDefinition must
    // win over CodingStandards.
    let items = HoldoutFilteredItems(vec![
        make_item("std", ContextPriority::CodingStandards, 90, None),
        make_item("iface", ContextPriority::CurrentInterfaceDefinition, 90, None),
    ]);

    let result = apply_priority_truncation(items, tokens(100));

    assert!(
        result.items.iter().any(|i| i.content == "iface"),
        "CurrentInterfaceDefinition must be included before CodingStandards"
    );
    assert!(
        !result.items.iter().any(|i| i.content == "std"),
        "CodingStandards must be dropped when budget forces a choice"
    );
}

// ─── Tier 3: Property-Based Tests ─────────────────────────────────────────────

proptest! {
    #[test]
    fn test_apply_priority_truncation_total_count_equals_sum_of_retained_items(
        item_count in 1usize..8,
        budget_tokens in 50u64..2000,
    ) {
        let items: Vec<ContextItem> = (0..item_count)
            .map(|i| make_item(
                &format!("item-{i}"),
                ContextPriority::CodingStandards,
                50,
                None,
            ))
            .collect();
        let filtered = HoldoutFilteredItems(items);

        let result = apply_priority_truncation(filtered, tokens(budget_tokens));

        let expected_total: u64 = result.items.iter().map(|i| i.token_count.as_u64()).sum();
        prop_assert_eq!(
            result.total_token_count.as_u64(),
            expected_total,
            "total_token_count must equal the sum of retained item token counts"
        );
    }

    /// With 50-token items, the total must never exceed the budget unless there
    /// is a single-item overflow (item > budget). Budget ≥ 50 ensures no overflow.
    #[test]
    fn test_apply_priority_truncation_total_never_exceeds_budget_without_overflow(
        item_count in 1usize..6,
        budget_tokens in 50u64..500,
    ) {
        // Each item is 50 tokens → no single item can overflow any budget ≥ 50.
        let items: Vec<ContextItem> = (0..item_count)
            .map(|i| make_item(&format!("item-{i}"), ContextPriority::CodingStandards, 50, None))
            .collect();
        let filtered = HoldoutFilteredItems(items);

        let result = apply_priority_truncation(filtered, tokens(budget_tokens));

        prop_assert!(
            result.total_token_count.as_u64() <= budget_tokens,
            "total tokens {} must not exceed budget {}",
            result.total_token_count.as_u64(),
            budget_tokens
        );
    }

    /// Output items must always be in non-decreasing priority discriminant order
    /// (lower discriminant = higher priority = appears first).
    #[test]
    fn test_apply_priority_truncation_output_always_ordered_by_priority(
        num_items in 2usize..7,
    ) {
        let priorities = [
            ContextPriority::TransitiveDependency,
            ContextPriority::CodingStandards,
            ContextPriority::CurrentInterfaceDefinition,
            ContextPriority::ArchitecturalConstraints,
        ];
        let items: Vec<ContextItem> = (0..num_items)
            .map(|i| make_item(
                &format!("item-{i}"),
                priorities[i % priorities.len()],
                10,
                None,
            ))
            .collect();
        let filtered = HoldoutFilteredItems(items);

        let result = apply_priority_truncation(filtered, tokens(10_000));

        let is_sorted = result.items.windows(2).all(|w| w[0].priority <= w[1].priority);
        prop_assert!(is_sorted, "items must be ordered by priority (ascending discriminant)");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// assemble_context
// ═══════════════════════════════════════════════════════════════════════════

// ─── Tier 1: Specification Tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_assemble_context_cache_hit_for_affected_module_produces_context_item() {
    let path = "src/lib.rs";
    let summary = make_pyramid_summary(path, "pub fn main() {}", 20);
    let cache = MockSummaryCache::new().with_hit(path, SummaryLevel::Paragraph, summary);

    let req = make_request(NodeType::Llm, &[path], &[]);
    let packs = empty_loaded_packs();

    let result = assemble_context(&req, &cache, &packs, &[], tokens(10_000)).await;

    assert!(result.assembly_errors.is_empty());
    assert!(result.items.iter().any(|i| {
        i.source_path.as_ref().map(|p| p.as_str()) == Some(path)
    }));
}

#[tokio::test]
async fn test_assemble_context_interface_entries_included_as_current_interface_def_priority() {
    let cache = MockSummaryCache::new();
    let req = make_request(NodeType::Llm, &[], &[]);
    let packs = empty_loaded_packs();
    let iface = make_interface_def("iface-001");

    let result = assemble_context(&req, &cache, &packs, &[iface], tokens(10_000)).await;

    assert!(
        result.items.iter().any(|i| i.priority == ContextPriority::CurrentInterfaceDefinition),
        "interface entries must appear as CurrentInterfaceDefinition items"
    );
}

#[tokio::test]
async fn test_assemble_context_non_empty_pack_guidance_included_as_context_pack_knowledge() {
    let mut pack = make_pack("my-pack", &[], &[], false);
    pack.domain_knowledge = "Use async/await everywhere.".to_string();
    pack.safe_patterns = vec!["async-await".to_string()];
    pack.required_artifacts = vec![];
    pack.anti_patterns = vec![];

    let cache = MockSummaryCache::new();
    let req = make_request(NodeType::Llm, &[], &[]);
    let packs = loaded_packs_from(vec![pack]);

    let result = assemble_context(&req, &cache, &packs, &[], tokens(10_000)).await;

    assert!(
        result.items.iter().any(|i| i.priority == ContextPriority::ContextPackKnowledge),
        "non-empty pack guidance must produce a ContextPackKnowledge item"
    );
}

// ─── Tier 2: Adversarial Tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_assemble_context_cache_error_records_assembly_error_and_sets_truncation() {
    let path = "src/broken.rs";
    let cache = MockSummaryCache::new()
        .with_error(path, SummaryLevel::Paragraph, "cache backend down");

    let req = make_request(NodeType::Llm, &[path], &[]);
    let packs = empty_loaded_packs();

    let result = assemble_context(&req, &cache, &packs, &[], tokens(10_000)).await;

    assert!(
        !result.assembly_errors.is_empty(),
        "cache error must be recorded in assembly_errors"
    );
    assert!(
        result.truncation_applied,
        "truncation_applied must be true when a cache error occurs"
    );
}

#[tokio::test]
async fn test_assemble_context_cache_error_on_one_artifact_does_not_fail_whole_assembly() {
    let good_path = "src/good.rs";
    let bad_path = "src/bad.rs";
    let summary = make_pyramid_summary(good_path, "pub struct Good;", 15);
    let cache = MockSummaryCache::new()
        .with_hit(good_path, SummaryLevel::Paragraph, summary)
        .with_error(bad_path, SummaryLevel::Paragraph, "not reachable");

    let req = make_request(NodeType::Llm, &[good_path, bad_path], &[]);
    let packs = empty_loaded_packs();

    let result = assemble_context(&req, &cache, &packs, &[], tokens(10_000)).await;

    assert_eq!(result.assembly_errors.len(), 1, "exactly one error for one failing artifact");
    assert!(
        result.items.iter().any(|i| {
            i.source_path.as_ref().map(|p| p.as_str()) == Some(good_path)
        }),
        "successful artifact must still appear in context despite another failing"
    );
}

#[tokio::test]
async fn test_assemble_context_scenario_holdout_enforced_excludes_scenario_files() {
    // ASSERT-SCEN-002: scenario files must NEVER appear in assembled code-gen context.
    let scenario_path = "spec/scenarios/test-01.md";
    let code_path = "src/lib.rs";
    let scenario_summary =
        make_pyramid_summary(scenario_path, "Given a user logs in...", 30);
    let code_summary = make_pyramid_summary(code_path, "pub fn lib() {}", 20);
    let cache = MockSummaryCache::new()
        .with_hit(scenario_path, SummaryLevel::Paragraph, scenario_summary)
        .with_hit(code_path, SummaryLevel::Paragraph, code_summary);

    let req = make_request(
        NodeType::Llm,
        &[scenario_path, code_path],
        &["spec/scenarios"],
    );
    let packs = empty_loaded_packs();

    let result = assemble_context(&req, &cache, &packs, &[], tokens(10_000)).await;

    assert!(
        result.items.iter().all(|i| {
            i.source_path.as_ref().map(|p| p.as_str()) != Some(scenario_path)
        }),
        "ASSERT-SCEN-002 VIOLATION: scenario file must not appear in assembled context"
    );
}

#[tokio::test]
async fn test_assemble_context_required_artifacts_from_packs_included_in_context() {
    let req_path = "docs/spec/architecture.md";
    let req_summary = make_pyramid_summary(req_path, "Architecture overview", 40);
    let cache =
        MockSummaryCache::new().with_hit(req_path, SummaryLevel::Paragraph, req_summary);

    let req = make_request(NodeType::Llm, &[], &[]);
    let packs = loaded_packs_with_artifacts(vec![artifact(req_path)]);

    let result = assemble_context(&req, &cache, &packs, &[], tokens(10_000)).await;

    assert!(
        result.items.iter().any(|i| {
            i.source_path.as_ref().map(|p| p.as_str()) == Some(req_path)
        }),
        "required artifact from pack must appear in assembled context"
    );
}

#[tokio::test]
async fn test_assemble_context_same_path_in_affected_modules_and_required_artifacts_produces_one_item()
{
    // ASSERT-CODE-007 corner case: the same path in both req.affected_modules
    // and packs.required_artifacts must not produce duplicate context items.
    let shared_path = "src/shared.rs";
    let summary = make_pyramid_summary(shared_path, "pub struct Shared;", 20);
    let cache =
        MockSummaryCache::new().with_hit(shared_path, SummaryLevel::Paragraph, summary);

    let req = make_request(NodeType::Llm, &[shared_path], &[]);
    let packs = loaded_packs_with_artifacts(vec![artifact(shared_path)]);

    let result = assemble_context(&req, &cache, &packs, &[], tokens(10_000)).await;

    let matching_count = result
        .items
        .iter()
        .filter(|i| i.source_path.as_ref().map(|p| p.as_str()) == Some(shared_path))
        .count();
    assert_eq!(
        matching_count, 1,
        "path appearing in both affected_modules and required_artifacts must produce one item"
    );
}

#[tokio::test]
async fn test_assemble_context_multiple_cache_errors_all_recorded_in_assembly_errors() {
    let paths = ["src/a.rs", "src/b.rs", "src/c.rs"];

    let cache = paths.iter().fold(MockSummaryCache::new(), |c, p| {
        c.with_error(p, SummaryLevel::Paragraph, "backend down")
    });

    let req = make_request(NodeType::Llm, &paths, &[]);
    let packs = empty_loaded_packs();

    let result = assemble_context(&req, &cache, &packs, &[], tokens(10_000)).await;

    assert_eq!(
        result.assembly_errors.len(),
        3,
        "each failing artifact must produce exactly one entry in assembly_errors"
    );
}

// ─── Tier 3: Property-Based Tests ─────────────────────────────────────────────

proptest! {
    /// ASSERT-SCEN-002 exhaustive: for any number of scenario files, NONE of them
    /// must appear in the assembled context when holdout is active.
    #[test]
    fn test_assemble_context_scenario_files_never_appear_in_output(
        num_scenarios in 1usize..5,
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime for proptest");

        rt.block_on(async {
            let scenario_paths: Vec<String> = (0..num_scenarios)
                .map(|i| format!("spec/scenarios/s{i}.md"))
                .collect();

            let cache = scenario_paths.iter().fold(MockSummaryCache::new(), |c, p| {
                let summary = make_pyramid_summary(p, &format!("Given scenario {p}"), 30);
                c.with_hit(p, SummaryLevel::Paragraph, summary)
            });

            let path_refs: Vec<&str> = scenario_paths.iter().map(String::as_str).collect();
            let req = make_request(NodeType::Llm, &path_refs, &["spec/scenarios"]);
            let packs = empty_loaded_packs();

            let result = assemble_context(&req, &cache, &packs, &[], tokens(100_000)).await;

            for item in &result.items {
                if let Some(path) = &item.source_path {
                    let is_scenario = path.as_str().starts_with("spec/scenarios/");
                    assert!(
                        !is_scenario,
                        "ASSERT-SCEN-002 VIOLATION: scenario file '{}' must not appear in assembled context",
                        path.as_str()
                    );
                }
            }
        });
    }
}
