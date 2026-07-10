# Test Coverage Record

This document maps specification assertions and behavioral contracts to the test
cases that exercise them. Updated as each module's test suite is written.

---

## Module: `pipeline/security.rs`

**Test file**: `crates/pipeline/src/security_tests.rs`
**Criticality**: Security-critical — every LLM call in the pipeline flows through these functions.
**Tiers**: 1 (specification) + 2 (adversarial) + 3 (property / fuzz)

### Specification Tests (Tier 1 — from assertions.md and security.md)

| Assertion | Test |
|-----------|------|
| ASSERT-SEC-001: non-approved branch rejected with `InvalidSourceBranch` | `test_validate_constitutional_prompt_feature_branch_returns_invalid_source_branch` |
| ASSERT-SEC-002: hash mismatch rejected before rule-presence check | `test_validate_constitutional_prompt_hash_mismatch_returns_hash_mismatch`, `test_validate_constitutional_prompt_hash_checked_before_rules` |
| ASSERT-SEC-003: missing rules rejected with complete missing list | `test_validate_constitutional_prompt_one_missing_rule_returns_missing_rules`, `test_validate_constitutional_prompt_all_rules_missing_returns_all_five` |
| ASSERT-SEC-004: injection detected → `InjectionDetected` with correct fields | `test_detect_injection_instruction_injection_phrase_detected`, `test_detect_injection_source_label_preserved_in_result`, `test_detect_injection_offending_text_captured` |
| ASSERT-SEC-005: protected path overrides approved scope (no double-flagging) | `test_validate_scope_protected_path_returns_protected_path_violation`, `test_validate_scope_protected_not_also_flagged_unauthorized` |

### Adversarial Tests (Tier 2)

#### `validate_constitutional_prompt` (9 tests)

| Scenario | Test |
|----------|------|
| Valid rules, "master" branch → Ok; assembled_system_prompt correct | `test_validate_constitutional_prompt_valid_master_branch_returns_validated_prompt` |
| Valid rules, "main" branch → Ok | `test_validate_constitutional_prompt_valid_main_branch_returns_validated_prompt` |
| Feature branch → `InvalidSourceBranch` | `test_validate_constitutional_prompt_feature_branch_returns_invalid_source_branch` |
| Tampered content → `HashMismatch` | `test_validate_constitutional_prompt_hash_mismatch_returns_hash_mismatch` |
| One missing rule → `MissingRules` listing exactly that rule | `test_validate_constitutional_prompt_one_missing_rule_returns_missing_rules` |
| All rules absent → `MissingRules` with all 5 variants | `test_validate_constitutional_prompt_all_rules_missing_returns_all_five` |
| Bad hash AND missing rules → `HashMismatch` (hash checked first) | `test_validate_constitutional_prompt_hash_checked_before_rules` |
| user_content preserved verbatim | `test_validate_constitutional_prompt_user_content_preserved` |
| Empty system_prompt assembles correctly | `test_validate_constitutional_prompt_empty_system_prompt_valid_assembly` |
| rules() accessor returns original rules | `test_validate_constitutional_prompt_rules_accessor_returns_original_rules` |
| Custom approved branch accepted | `test_validate_constitutional_prompt_custom_approved_branch_accepted` |
| Branch not in custom list rejected | `test_validate_constitutional_prompt_branch_not_in_custom_list_rejected` |
| Empty approved-branch list rejects all branches | `test_validate_constitutional_prompt_empty_approved_list_rejects_all` |

#### `detect_injection` (10 tests)

| Scenario | Test |
|----------|------|
| Empty string → `Clean` | `test_detect_injection_empty_string_returns_clean` |
| Benign text → `Clean` | `test_detect_injection_benign_text_returns_clean` |
| InstructionInjection phrase → `InjectionDetected { InstructionInjection }` | `test_detect_injection_instruction_injection_phrase_detected` |
| PersonaOverride phrase → `InjectionDetected { PersonaOverride }` | `test_detect_injection_persona_override_phrase_detected` |
| BehavioralModification phrase → `InjectionDetected { BehavioralModification }` | `test_detect_injection_behavioral_modification_phrase_detected` |
| SystemPromptExtractionAttempt phrase → correct pattern | `test_detect_injection_system_prompt_extraction_phrase_detected` |
| Both InstructionInjection and PersonaOverride → InstructionInjection wins | `test_detect_injection_precedence_instruction_wins_over_persona` |
| source_label preserved in result | `test_detect_injection_source_label_preserved_in_result` |
| UPPERCASE phrase → case-insensitive detection | `test_detect_injection_mixed_case_phrase_detected` |
| offending_text non-empty and contains trigger | `test_detect_injection_offending_text_captured` |

#### `is_protected` (7 tests)

| Scenario | Test |
|----------|------|
| Empty protected list → false | `test_is_protected_empty_protected_list_returns_false` |
| Exact filename pattern matches | `test_is_protected_exact_filename_pattern_matches` |
| Double-star pattern matches nested path | `test_is_protected_double_star_pattern_matches_nested` |
| Anchored pattern matches root path | `test_is_protected_anchored_pattern_matches_root` |
| Anchored pattern does NOT match nested path | `test_is_protected_anchored_pattern_does_not_match_nested` |
| Invalid glob pattern → false (no panic) | `test_is_protected_invalid_pattern_returns_false_not_panic` |
| Non-matching pattern → false | `test_is_protected_no_match_returns_false` |

#### `validate_scope` (10 tests)

| Scenario | Test |
|----------|------|
| Empty artifact_patterns → `ScopeUnderspecified` | `test_validate_scope_empty_artifact_patterns_returns_scope_underspecified` |
| Protected path → `ProtectedPathViolation` | `test_validate_scope_protected_path_returns_protected_path_violation` |
| Artifact not in allowed patterns → `UnauthorizedCapability` | `test_validate_scope_artifact_matching_no_allowed_returns_unauthorized` |
| Artifact in allowed patterns → Ok | `test_validate_scope_artifact_matching_allowed_returns_ok` |
| 3 artifacts (1 protected, 1 unauthorized, 1 ok) → 2 violations | `test_validate_scope_collects_all_violations_for_multiple_artifacts` |
| Protected artifact NOT double-flagged as unauthorized | `test_validate_scope_protected_not_also_flagged_unauthorized` |
| max_files exceeded → UnauthorizedCapability violation | `test_validate_scope_max_files_exceeded_adds_violation` |
| No artifacts, valid scope → Ok | `test_validate_scope_empty_artifacts_returns_ok` |
| ProtectedPathViolation.artifact_path = Some(artifact) | `test_validate_scope_violation_artifact_path_is_set` |
| ScopeUnderspecified.artifact_path = None | `test_validate_scope_underspecified_artifact_path_is_none` |
| max_new_files exceeded → `UnauthorizedCapability` violation | `test_validate_scope_max_new_files_exceeded_produces_violation` |
| max_new_files at limit → Ok | `test_validate_scope_max_new_files_at_limit_is_ok` |
| max_new_files = 0, any new file → violation | `test_validate_scope_max_new_files_zero_any_new_file_is_violation` |

#### `validate_tool_scope` (8 tests)

| Scenario | Test |
|----------|------|
| Empty params → Ok | `test_validate_tool_scope_empty_params_returns_ok` |
| String param matches allowed → Ok | `test_validate_tool_scope_string_param_matching_allowed_returns_ok` |
| String param matches prohibited → Err | `test_validate_tool_scope_string_param_matching_prohibited_returns_violation` |
| Matches both allowed and prohibited → prohibited wins → Err | `test_validate_tool_scope_string_param_matching_both_allowed_and_prohibited_prohibited_wins` |
| Matches neither → Err (unauthorized) | `test_validate_tool_scope_string_param_matching_neither_returns_unauthorized` |
| count param within max_file_changes → Ok | `test_validate_tool_scope_count_param_within_limit_returns_ok` |
| count param exceeds max_file_changes → Err | `test_validate_tool_scope_count_param_exceeds_limit_returns_violation` |
| Two violating params → exactly one violation (short-circuit) | `test_validate_tool_scope_returns_on_first_violation_only` |
| `count` param with value `-1` → `ToolScopeViolation` | `test_validate_tool_scope_negative_count_rejected` |
| `limit` param with value `-1` → `ToolScopeViolation` | `test_validate_tool_scope_negative_limit_rejected` |
| `count` param with value `0` → Ok | `test_validate_tool_scope_zero_count_accepted` |

### Property Tests (Tier 3 — proptest fuzz)

| Invariant | Test |
|-----------|------|
| `detect_injection` never panics on arbitrary input | `test_detect_injection_never_panics_proptest` |
| `validate_scope` never panics on arbitrary paths and patterns | `test_validate_scope_never_panics_proptest` |
| `validate_tool_scope` never panics on arbitrary param values and patterns | `test_validate_tool_scope_never_panics_proptest` |

### Regression / Remediation Tests (from security review)

#### H-001 — Injection bypass via whitespace and invisible characters

| Scenario | Test |
|----------|------|
| Double space between words does not bypass detection | `test_detect_injection_double_space_bypass_blocked` |
| Zero-width space (U+200B) does not bypass detection | `test_detect_injection_zero_width_space_bypass_blocked` |
| Newline between words does not bypass detection | `test_detect_injection_newline_bypass_blocked` |
| Soft hyphen (U+00AD) within word does not bypass detection | `test_detect_injection_soft_hyphen_bypass_blocked` |

#### M-004 — ArtifactPath normalization

| Scenario | Test |
|----------|------|
| `./src/main.rs` normalizes to same value as `src/main.rs` | `test_artifact_path_strips_leading_dot_slash` |
| Path with `..` component returns `None` | `test_artifact_path_rejects_traversal` |
| `./`-prefixed path matches protection pattern | `test_is_protected_dot_slash_prefix_matches_protection` |
| `./`-prefixed artifact matches approved scope | `test_validate_scope_dot_slash_artifact_matches_approved` |

#### Additional mutant-killing tests (from d4bcef8)

See commit `d4bcef8` — 4 additional tests added to improve mutation coverage beyond 95% to 100%.

#### `validate_protected_paths` (4 tests)

| Scenario | Test |
|----------|------|
| Empty list → Ok | `test_validate_protected_paths_empty_list_returns_ok` |
| Single invalid pattern → `Err` with that pattern | `test_validate_protected_paths_invalid_pattern_detected` |
| Multiple invalid patterns → all returned together | `test_validate_protected_paths_multiple_invalid_patterns_all_returned` |
| All valid patterns → Ok | `test_validate_protected_paths_valid_patterns_pass` |

#### L-002 — offending_text alignment for normalization-only matches (2 tests)

| Scenario | Test |
|----------|------|
| Zero-width-space bypass: `offending_text` contains the original U+200B character | `test_detect_injection_zero_width_space_offending_text_contains_original_span` |
| Double-space bypass: `offending_text` contains the original double space | `test_detect_injection_double_space_offending_text_contains_original_span` |
| Mid-content phrase: `offending_text` does not include leading prefix text | `test_detect_injection_mid_content_offending_text_excludes_prefix` |

### Gaps / Known Limitations

- `validate_constitutional_prompt` performance test (< 5 ms for 10 KB rules) is not covered —
  performance testing requires a benchmark harness (criterion), not unit tests.
- `detect_injection` corpus is based on representative phrases from the spec; exhaustive phrase
  coverage is the QA Engineer's responsibility via mutation testing.
- Concurrent invocation behaviour is not tested here — requires integration tests.
- `validate_protected_paths` wiring into `PipelineConfigurationLoader` is not tested here — blocked
  on Task 16.0 (`PipelineConfigurationLoader` implementation).

---

## Module: `pipeline/execution.rs` — Task 3.0

**Test file**: `crates/pipeline/src/execution_tests.rs`
**Criticality**: domain-logic — mutation target 85%
**Tiers**: 4 (mutation)

### Functions in scope for Task 3.0

- `check_fan_in_ready`
- `evaluate_edge_condition`
- `increment_rework_counter`

### Adversarial Tests (Tier 2)

#### `check_fan_in_ready` (5 tests)

| Scenario | Test |
|----------|------|
| No incoming forward edges → true (no predecessors) | `test_check_fan_in_ready_no_predecessors_returns_true` |
| All predecessors Completed → true | `test_check_fan_in_ready_all_completed_returns_true` |
| One predecessor not Completed → false | `test_check_fan_in_ready_one_incomplete_returns_false` |
| All predecessors Pending → false | `test_check_fan_in_ready_all_pending_returns_false` |
| Rework edges excluded from predecessor check → true | `test_check_fan_in_ready_rework_edges_ignored` |

#### `evaluate_edge_condition` (13 tests)

| Scenario | Test |
|----------|------|
| Deterministic true → `(true, record)` | `test_evaluate_edge_condition_deterministic_true_produces_record` |
| Deterministic false → `(false, record)` | `test_evaluate_edge_condition_deterministic_false_produces_record` |
| LlmEvaluated present in map → map value returned | `test_evaluate_edge_condition_llm_evaluated_present_returns_value` |
| LlmEvaluated absent from map → false (conservative fallback) | `test_evaluate_edge_condition_llm_evaluated_absent_returns_false` |
| Composite And, all true → true | `test_evaluate_edge_condition_composite_and_all_true_returns_true` |
| Composite And, one false → false | `test_evaluate_edge_condition_composite_and_one_false_returns_false` |
| Composite Or, one true → true | `test_evaluate_edge_condition_composite_or_one_true_returns_true` |
| Composite Or, all false → false | `test_evaluate_edge_condition_composite_or_all_false_returns_false` |
| Composite Not, wrapping true → false | `test_evaluate_edge_condition_composite_not_inverts_true_to_false` |
| Composite Not, wrapping false → true | `test_evaluate_edge_condition_composite_not_inverts_false_to_true` |
| `record.input_snapshot` captures pipeline state | `test_evaluate_edge_condition_record_contains_input_snapshot` |
| `record.edge_id` matches the supplied edge_id | `test_evaluate_edge_condition_record_contains_edge_id` |
| `record.timestamp` matches `evaluated_at` | `test_evaluate_edge_condition_record_timestamp_matches_evaluated_at` |

#### `increment_rework_counter` (6 tests)

| Scenario | Test |
|----------|------|
| First traversal → Ok(1) | `test_increment_rework_counter_first_traversal_returns_one` |
| Second traversal → Ok(2) (accumulates) | `test_increment_rework_counter_increments_existing_count` |
| Traversal count equals max_traversals → Ok (at-limit is allowed) | `test_increment_rework_counter_at_limit_returns_ok` |
| Traversal count exceeds max_traversals → Err | `test_increment_rework_counter_over_limit_returns_err` |
| `TerminationConditionReached.edge_id` / `.current_traversals` / `.max_traversals` correct | `test_increment_rework_counter_err_contains_correct_fields` |
| `rework_edge_traversals` in state updated after call | `test_increment_rework_counter_mutates_state` |

### Tier 4 — Mutation Testing

**Run**: `cargo mutants --package pipeline --file crates/pipeline/src/execution.rs --timeout 60`
**Report**: `docs/spec/mutation-report-task3.0-4ad9cc7.json`
**Date**: 2026-06-05

| Module / Function | Viable Mutants | Caught | Missed | Score |
|-------------------|---------------|--------|--------|-------|
| `check_fan_in_ready` | 5 | 5 | 0 | **100%** |
| `evaluate_edge_condition` | 1 | 1 | 0 | **100%** (2 unviable: `EdgeEvaluationRecord` non-Default) |
| `increment_rework_counter` | 6 | 6 | 0 | **100%** |
| `determine_next_actions` _(todo stub, out of scope)_ | 1 | 0 | 1 | N/A |
| `topological_sort_sub_work_items` _(todo stub, out of scope)_ | 1 | 0 | 1 | N/A |
| **Target functions total** | **12** | **12** | **0** | **100%** |
| File total (all functions) | 14 | 12 | 2 | 85.7% |

**Target met**: ✅ 100% kill rate on the three task-3.0 functions (domain-logic minimum: 85%).

**Surviving mutants** (out of scope — cannot be killed without implementing the stubs):

- `execution.rs:384`: `replace determine_next_actions -> Vec<NextAction> with vec![]`
  — Base is `todo!()` (panics). No tests call this function. Blocked on Task N (implementation).
- `execution.rs:657`: `replace topological_sort_sub_work_items -> Result<Vec<SubWorkItemId>, DependencyError> with Ok(vec![])`
  — Base is `todo!()` (panics). No tests call this function. Blocked on its implementation task.

**New kill tests added**: 0 — all 3 target functions achieved 100% kill rate with the existing 24-test suite.

### Gaps / Known Limitations

- `determine_next_actions` mutation survivor will be killed when that function is implemented (not Task 3.0).
- `topological_sort_sub_work_items` mutation survivor will be killed when that function is implemented.
- No fuzz targets for `check_fan_in_ready` / `evaluate_edge_condition` / `increment_rework_counter` —
  these are pure functions over typed structs (not raw byte parsers); fuzz coverage is deferred.
- No Kani proofs — functions are domain-logic tier (Tier 4 only); Tier 6 reserved for safety-critical paths.

---

## Module: `pipeline/budget.rs` + `pipeline/execution.rs` — Task 5.0

**Test files**: `crates/pipeline/src/budget_tests.rs`, `crates/pipeline/src/execution_tests.rs`
**Criticality**: domain-logic — mutation target 70%
**Tiers**: 4 (mutation)

### Functions in scope for Task 5.0

- `acquire_budget` (`budget.rs`)
- `topological_sort_sub_work_items` (`execution.rs`)

### Mutation Audit Report (Tier 4) — post-kill

**Scope**: `crates/pipeline/src/budget.rs` + `crates/pipeline/src/execution.rs`
**Tool**: cargo-mutants 25.0.0

| File | Caught | Missed (pre-kill) | Missed (post-kill) | Score |
|------|--------|-------------------|--------------------|-------|
| `budget.rs` | 5 (of file total) | 0 | 0 | 100% |
| `execution.rs` | 47 (of file total) | 2 | 0 | 100% |
| **Combined totals** | **52** | **2** | **0** | **100%** |
| Unviable | 18 | — | — | — |

**Target met**: ✅ 100% kill rate (domain-logic minimum: 70%).

### Surviving Mutants Found and Killed

#### Survivor 1 — `execution.rs:992:42 replace > with <`

- **Mutation**: `*deg > 0` → `*deg < 0` in in-degree filter when collecting cycle members
- **Why it survived**: in_degree values are always ≥ 0; `< 0` returns an empty list. Tests only
  checked that `CyclicDependency` variant was returned, not the `cycle` field contents.
- **Kill test**: `test_topological_sort_sub_work_items_two_node_cycle_reports_cycle_members`
- **Resolution**: ✅ killed

#### Survivor 2 — `execution.rs:992:42 replace > with ==`

- **Mutation**: `*deg > 0` → `*deg == 0` — picks zero-degree nodes (resolved nodes) instead of
  stuck nodes with remaining in-degree, producing the wrong set in the cycle field.
- **Why it survived**: same as above — `cycle` field not asserted.
- **Kill test**: `test_topological_sort_sub_work_items_three_node_cycle_reports_all_cycle_members`
- **Resolution**: ✅ killed

### New Kill Tests Added: 2

| Test | File | Kills |
|------|------|-------|
| `test_topological_sort_sub_work_items_two_node_cycle_reports_cycle_members` | `execution_tests.rs` | Survivor 1 |
| `test_topological_sort_sub_work_items_three_node_cycle_reports_all_cycle_members` | `execution_tests.rs` | Survivor 2 |

### Gaps / Known Limitations

- No fuzz targets — `acquire_budget` and `topological_sort_sub_work_items` are typed-struct
  functions, not raw-byte parsers; fuzz coverage is deferred to Tier 5 scope expansion.
- No Kani proofs — domain-logic tier (Tier 4 only).

---

## Module: `pipeline/context.rs` — Task #43

**Test file**: `crates/pipeline/src/context_tests.rs`
**Criticality**: Domain-logic with a **hard safety constraint** on `enforce_scenario_holdout` (ASSERT-SCEN-002).
**Tiers**: 1 (specification) + 2 (adversarial) + 3 (property-based)
**Status**: RED — all five functions have `todo!()` stubs; tests compile and will fail until implementation is written.

### Functions in scope

1. `select_context_packs` — glob trigger matching with OR semantics
2. `merge_pack_guidance` — union-merge with required-artifact deduplication
3. `enforce_scenario_holdout` — **HARD SAFETY CONSTRAINT** path-prefix filter
4. `apply_priority_truncation` — priority sort, greedy fill, single-item overflow
5. `assemble_context` — orchestration of all steps above (async)

### Specification Tests (Tier 1)

| Assertion | Test |
|-----------|------|
| ASSERT-SCEN-002: scenario files excluded from code-gen context | `test_enforce_scenario_holdout_item_rooted_under_holdout_dir_is_removed`, `test_assemble_context_scenario_holdout_enforced_excludes_scenario_files` |
| ASSERT-CODE-006: truncation by priority, current interface never removed first | `test_apply_priority_truncation_higher_priority_item_always_included_before_lower`, `test_apply_priority_truncation_single_item_exceeding_budget_still_included` |
| ASSERT-CODE-007: dependency outputs included | `test_assemble_context_cache_hit_for_affected_module_produces_context_item`, `test_assemble_context_required_artifacts_from_packs_included_in_context` |

### Adversarial Tests (Tier 2)

#### `select_context_packs` (11 tests)

| Scenario | Test |
|----------|------|
| Label pattern glob match → pack selected | `test_select_context_packs_matching_label_pattern_returns_pack_id` |
| Component tag pattern glob match → pack selected | `test_select_context_packs_matching_component_tag_pattern_returns_pack_id` |
| `requires_safety_critical=true`, `safety_affecting=true` → selected | `test_select_context_packs_safety_critical_pack_selected_when_safety_affecting_true` |
| No trigger match → empty vec | `test_select_context_packs_no_trigger_match_returns_empty_vec` |
| OR semantics: label matches, component doesn't → still selected | `test_select_context_packs_or_semantics_label_match_when_component_doesnt` |
| `requires_safety_critical=true`, `safety_affecting=false` → not selected | `test_select_context_packs_requires_safety_critical_not_selected_when_not_safety_affecting` |
| Empty `available` slice → empty | `test_select_context_packs_empty_available_returns_empty_vec` |
| Multiple packs match → all returned | `test_select_context_packs_multiple_matching_packs_all_ids_returned` |
| Empty labels slice doesn't block component match | `test_select_context_packs_empty_labels_slice_still_matches_component_tag` |
| `**` glob matches deeply nested path | `test_select_context_packs_glob_double_star_matches_deeply_nested_path` |
| All-empty trigger fields never fire | `test_select_context_packs_pack_with_all_empty_trigger_fields_never_matches` |
| Multiple trigger fields match same pack → appears once | `test_select_context_packs_same_pack_appears_at_most_once_when_multiple_fields_match` |

#### `merge_pack_guidance` (7 tests)

| Scenario | Test |
|----------|------|
| Empty slice → empty `MergedGuidance` | `test_merge_pack_guidance_empty_slice_returns_empty_merged_guidance` |
| Single pack → its fields verbatim | `test_merge_pack_guidance_single_pack_returns_all_its_fields_verbatim` |
| Two packs: safe_patterns union | `test_merge_pack_guidance_two_packs_safe_patterns_union_merged` |
| Two packs: anti_patterns union | `test_merge_pack_guidance_two_packs_anti_patterns_union_merged` |
| Duplicate required artifact → deduplicated to one | `test_merge_pack_guidance_duplicate_required_artifact_path_deduplicated` |
| Distinct required artifacts → all present | `test_merge_pack_guidance_distinct_required_artifacts_all_present` |
| Three packs with shared + unique artifacts → deduped correctly | `test_merge_pack_guidance_three_packs_shared_artifact_deduped_unique_artifacts_present` |

#### `enforce_scenario_holdout` (8 tests — HARD SAFETY CONSTRAINT)

| Scenario | Test |
|----------|------|
| Item under holdout dir → removed | `test_enforce_scenario_holdout_item_rooted_under_holdout_dir_is_removed` |
| Item outside all holdout dirs → kept | `test_enforce_scenario_holdout_item_outside_all_holdout_dirs_is_kept` |
| `source_path=None` → never removed | `test_enforce_scenario_holdout_source_path_none_item_is_never_removed` |
| Empty holdout dirs → nothing removed | `test_enforce_scenario_holdout_empty_holdout_dirs_removes_nothing` |
| Empty item list → empty result | `test_enforce_scenario_holdout_empty_item_list_returns_empty` |
| Deeply nested path under holdout → removed | `test_enforce_scenario_holdout_deeply_nested_path_under_holdout_dir_is_removed` |
| Multiple holdout dirs → removes from all | `test_enforce_scenario_holdout_multiple_holdout_dirs_removes_from_all` |
| Sibling directory (e.g. `spec/scenarios-alt`) → NOT removed | `test_enforce_scenario_holdout_sibling_directory_not_removed` |
| Mixed None + path items → only holdout paths filtered | `test_enforce_scenario_holdout_mixed_none_and_path_items_only_holdout_paths_filtered` |

#### `apply_priority_truncation` (10 tests)

| Scenario | Test |
|----------|------|
| All items fit budget → all included, `truncation_applied=false` | `test_apply_priority_truncation_all_items_fit_budget_all_included_no_truncation` |
| Priority sort: `CurrentInterfaceDefinition` < `CodingStandards` < `TransitiveDependency` | `test_apply_priority_truncation_items_sorted_by_priority_highest_first` |
| Budget exceeded → lowest priority dropped, `truncation_applied=true` | `test_apply_priority_truncation_lowest_priority_item_dropped_when_budget_exceeded` |
| Single item > budget → still included, `truncation_applied=true` | `test_apply_priority_truncation_single_item_exceeding_budget_still_included` |
| Empty input → empty package, `truncation_applied=false` | `test_apply_priority_truncation_empty_input_returns_empty_package_no_truncation` |
| `total_token_count` = sum of included items | `test_apply_priority_truncation_total_token_count_equals_sum_of_included_items` |
| `total_token_count` excludes dropped items | `test_apply_priority_truncation_total_token_count_excludes_dropped_items` |
| Same-priority tier: alphabetical by source_path | `test_apply_priority_truncation_same_priority_items_sorted_alphabetically_by_source_path` |
| Exactly at budget → all included, no truncation | `test_apply_priority_truncation_exactly_at_budget_includes_all_no_truncation` |
| One token over budget → last item dropped | `test_apply_priority_truncation_one_token_over_budget_drops_last_item` |
| Greedy fill: partial lower tier included | `test_apply_priority_truncation_greedy_fill_includes_partial_lower_priority_tier` |
| High priority beats low with same token size | `test_apply_priority_truncation_higher_priority_item_always_included_before_lower` |

#### `assemble_context` (9 async tests)

| Scenario | Test |
|----------|------|
| Cache hit → item in context | `test_assemble_context_cache_hit_for_affected_module_produces_context_item` |
| Interface entries → `CurrentInterfaceDefinition` items | `test_assemble_context_interface_entries_included_as_current_interface_def_priority` |
| Non-empty pack guidance → `ContextPackKnowledge` item | `test_assemble_context_non_empty_pack_guidance_included_as_context_pack_knowledge` |
| Cache error → `assembly_errors` non-empty, `truncation_applied=true` | `test_assemble_context_cache_error_records_assembly_error_and_sets_truncation` |
| One error, one hit → error recorded + good item present | `test_assemble_context_cache_error_on_one_artifact_does_not_fail_whole_assembly` |
| Scenario holdout enforced — scenario file excluded (ASSERT-SCEN-002) | `test_assemble_context_scenario_holdout_enforced_excludes_scenario_files` |
| Required artifact from pack → appears in context | `test_assemble_context_required_artifacts_from_packs_included_in_context` |
| Same path in affected_modules AND required_artifacts → one item only | `test_assemble_context_same_path_in_affected_modules_and_required_artifacts_produces_one_item` |
| Three cache errors → three entries in `assembly_errors` | `test_assemble_context_multiple_cache_errors_all_recorded_in_assembly_errors` |

### Property Tests (Tier 3 — proptest)

| Invariant | Test |
|-----------|------|
| `select_context_packs` result length ≤ available packs | `test_select_context_packs_result_length_never_exceeds_available_packs` |
| `select_context_packs` result contains no duplicate IDs | `test_select_context_packs_result_contains_no_duplicate_pack_ids` |
| `merge_pack_guidance` required_artifacts never contains duplicates | `test_merge_pack_guidance_required_artifacts_never_contains_duplicates` |
| `merge_pack_guidance` all patterns from all packs present | `test_merge_pack_guidance_all_patterns_from_all_packs_present` |
| `enforce_scenario_holdout` None-source items always preserved | `test_enforce_scenario_holdout_none_source_path_items_always_preserved` |
| `enforce_scenario_holdout` holdout-path items always removed | `test_enforce_scenario_holdout_holdout_path_items_always_removed` |
| `enforce_scenario_holdout` non-holdout path items always preserved | `test_enforce_scenario_holdout_non_holdout_path_items_always_preserved` |
| `apply_priority_truncation` total = sum of retained items | `test_apply_priority_truncation_total_count_equals_sum_of_retained_items` |
| `apply_priority_truncation` never exceeds budget (no overflow) | `test_apply_priority_truncation_total_never_exceeds_budget_without_overflow` |
| `apply_priority_truncation` output always in priority order | `test_apply_priority_truncation_output_always_ordered_by_priority` |
| `assemble_context` scenario files never in output (ASSERT-SCEN-002 exhaustive) | `test_assemble_context_scenario_files_never_appear_in_output` |

### Spec Gaps / Known Limitations

- **`enforce_scenario_holdout` prefix semantics ambiguity**: the spec says "prefix match on path
  string" but also "rooted under any holdout_dir". A raw string prefix of `spec/scenarios` would
  also match `spec/scenarios-alt/foo.md` (a sibling directory). The tests assume **directory-prefix
  semantics** (holdout matches paths starting with `{dir}/` or equal to `dir`). The architect
  should clarify this in `docs/spec/interfaces/context.md`.
- `assemble_context` cache-miss (`Ok(None)`) behaviour is not explicitly specified — the spec only
  specifies `Err` → skip + record. The tests assume `Ok(None)` is silently skipped without an error.
  This should be confirmed in the spec.
- No test for `assemble_context` with `NodeType::Deterministic` — the impl may vary summary level
  by node type; tests currently use `NodeType::Llm` only.
- Mutation coverage audit is blocked on implementation (cannot run `cargo mutants` against `todo!()`).

