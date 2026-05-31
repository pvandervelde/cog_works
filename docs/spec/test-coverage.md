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
