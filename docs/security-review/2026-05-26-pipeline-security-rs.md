# Security Review: `pipeline/security.rs` — Constitutional Layer, Injection Detection, Scope Enforcement

**Date**: 2026-05-26
**Reviewer**: GitHub Copilot — Security Reviewer Mode
**Scope**: `crates/pipeline/src/security.rs`, `crates/pipeline/src/security_tests.rs`
**Spec References**: `docs/spec/security.md`, `docs/spec/constraints.md`, `docs/spec/assertions.md`, `docs/spec/interfaces/security.md`

---

## Summary

| Severity | Count |
|---|---|
| High | 1 |
| Medium | 5 |
| Low | 4 |
| Informational | 4 |

**Overall verdict**: **BLOCKED** — one High finding (H-001) and one Medium finding (M-004) must be addressed before this module is considered production-ready for a security enforcement role.

---

## Methodology

1. Read `docs/spec/security.md` (threat catalog), `docs/spec/constraints.md`, `docs/spec/assertions.md` (ASSERT-SEC-001…005), and `docs/spec/interfaces/security.md` to establish the intended security contract.
2. Read `crates/pipeline/src/security.rs` in full (five functions + all supporting types and helpers).
3. Read `crates/pipeline/src/security_tests.rs` in full to assess test coverage.
4. Ran `cargo audit` — results below.
5. Ran `cargo deny check` — results below.
6. Checked `Cargo.toml` (workspace) for dependency versions.
7. Checked `crates/pipeline/src/identifiers.rs` for `ArtifactPath` construction semantics.

---

## Dependency Audit Results

### `cargo audit`

Three advisories reported, all in the `cli → listener → queue-runtime → azure_core` transitive chain. **None affect `pipeline` directly.**

| Advisory | Crate | Severity | Via | `deny.toml` status |
|---|---|---|---|---|
| RUSTSEC-2024-0384 | `instant 0.1.13` | Unmaintained | `azure_core ← queue-runtime` | Explicitly ignored with rationale |
| RUSTSEC-2024-0436 | `paste 1.0.15` | Unmaintained | `azure_core ← queue-runtime` | Explicitly ignored with rationale |
| RUSTSEC-2026-0097 | `rand 0.7.3` | Unsound | `azure_core ← queue-runtime` | Explicitly ignored with rationale |

`sha2 = "0.10"` and `globset = "0.4"` have **no advisories**. Exit code 1 is expected when advisories are present even when ignored; `cargo deny check` exits 0 (see below).

### `cargo deny check`

Exit code: **0** (pass). No security, license, ban, or source violations. Warnings are benign configuration housekeeping (`MPL-2.0`/`OpenSSL` listed in allow-list but not present in the dependency tree; several `skip` entries are now single-version and can be removed from `deny.toml`). No action required for the security review.

### New Dependency Assessment

| Crate | Version | Maintainer | Assessment |
|---|---|---|---|
| `sha2` | `0.10` | RustCrypto | Well-maintained, widely audited, correct choice for SHA-256. No issues. |
| `globset` | `0.4` | BurntSushi | Widely used, actively maintained, correct choice for gitignore-style glob matching. No issues. |

---

## Findings

---

### FINDING-H-001 [HIGH] Injection detector bypassed by whitespace insertion, zero-width characters, or Unicode look-alikes

**Location**: `crates/pipeline/src/security.rs` — `detect_injection` function, lines ~490–550

**Spec Reference**: `docs/spec/security.md` §THREAT-001 mitigation 2; `docs/spec/interfaces/security.md` §detect_injection; ASSERT-SEC-004

**Description**:
`detect_injection` performs lowercase-folded substring matching against a fixed corpus of 20 phrases (5 per category). No whitespace normalization, Unicode normalization, or zero-width-character stripping is performed before matching. The entire corpus can be bypassed with trivial character manipulation:

| Bypass technique | Example | Result |
|---|---|---|
| Extra space | `"ignore  all  previous  instructions"` | `Clean` |
| Zero-width space (U+200B) | `"ignore​all previous instructions"` | `Clean` |
| Unicode homoglyphs | `"ɪɢɴᴏʀᴇ all previous instructions"` (mathematical small caps) | `Clean` |
| Newline injection | `"ignore all\nprevious instructions"` | `Clean` |
| Soft-hyphen (U+00AD) | `"ig­nore all previous instructions"` | `Clean` |

An attacker who knows the detection corpus (it is in version-controlled source) can craft an issue body that passes `detect_injection` while still being semantically interpreted by the LLM as an injection instruction.

**Impact**:
Injection content reaching the LLM without detection — the pipeline does not halt, no `cogworks:security-hold` label is applied, no audit event is emitted. The LLM receives the injection embedded in a user content block. The first-line defence from THREAT-001 mitigation 2 is neutralised. The constitutional layer (mitigation 1) and schema validation (mitigation 3) remain, but the spec relies on injection detection as a primary halt mechanism.

The spec acknowledges residual risk ("a sufficiently sophisticated injection might evade detection"), but bypassing the detector by inserting a single extra space is not what that caveat intends to cover.

**Test coverage gap**:
`security_tests.rs` tests UPPERCASE detection (`test_detect_injection_mixed_case_phrase_detected`) but contains no tests for whitespace variants, zero-width characters, or Unicode normalization. The proptest only verifies panic-freedom, not detection completeness.

**Remediation**:

1. Before matching, normalize the content to a canonical form:

   ```rust
   fn normalize_for_injection_scan(s: &str) -> String {
       // Strip zero-width and invisible Unicode code points, then collapse whitespace.
       s.chars()
           .filter(|c| !matches!(c, '\u{200B}'|'\u{200C}'|'\u{200D}'|'\u{FEFF}'|'\u{00AD}'))
           .collect::<String>()
           .split_whitespace()       // collapses all whitespace runs to single space
           .collect::<Vec<_>>()
           .join(" ")
           .to_lowercase()
   }
   ```

2. Apply this normalization to both `content` and each corpus phrase before the `contains` check.
3. Consider also applying Unicode NFKC normalization (`unicode-normalization` crate) to collapse homoglyphs to their ASCII equivalents before pattern matching.
4. Add adversarial test cases: space-padded phrases, zero-width-character-injected phrases, and newline-split phrases for each of the four `InjectionPattern` categories.

**Spec compliance**: PARTIAL — ASSERT-SEC-004 passes for canonical phrases; trivially mutated variants are not scanned.

---

### FINDING-M-001 [MEDIUM] Fail-open for invalid glob patterns silently drops protection

**Location**: `crates/pipeline/src/security.rs` — `build_glob_set` (~line 800); `is_protected` (~line 840); `validate_tool_scope` (~line 960)

**Spec Reference**: `docs/spec/security.md` §THREAT-015, §THREAT-017; `docs/spec/interfaces/security.md` §is_protected

**Description**:
`build_glob_set` silently skips invalid glob patterns and emits only `tracing::warn!`. This fail-open behaviour has different security impact depending on context:

| Callsite | Invalid pattern effect | Security posture |
|---|---|---|
| `is_protected` (protected-paths set) | Protection silently absent for that path | **Fail-open**: a misconfigured protected-path pattern leaves the file unprotected |
| `validate_scope` (allowed set) | Pattern absent from allowed set | Fail-closed: fewer allowed paths = more rejections (acceptable) |
| `validate_tool_scope` (prohibited set) | Prohibition silently absent | **Fail-open**: a misconfigured prohibited-pattern allows the operation |

For the protected-path and prohibited-pattern cases, a misconfigured pattern (e.g., a typo creating an invalid glob like `**.cogworks/**` instead of `**/.cogworks/**`) will silently fail to protect the intended path. No pipeline halt occurs; the only observable signal is a `tracing::warn!` event that may be missed in production.

The module documentation states: "Pattern validity MUST be enforced at configuration load time." This module provides no enforcement — it relies entirely on a caller contract that has no static verification.

**Impact**:
If a protected-path pattern is invalid due to a typo, an LLM-generated artifact that would have been blocked can modify a constitutional rules file, prompt template, or pipeline configuration — directly undermining THREAT-015 and THREAT-017.

**Remediation**:

1. **Short term**: Add a `validate_protected_paths(paths: &[ProtectedPath]) -> Result<(), Vec<(String, globset::Error)>>` function that validates all patterns eagerly. Call this during pipeline configuration load. Document the call requirement in `is_protected`'s doc comment as a precondition.
2. **Longer term**: Consider making `ProtectedPath::new` return `Result<ProtectedPath, globset::Error>` so invalid patterns cannot be constructed — enforcing validity at the type level.
3. For the prohibited-pattern case in `validate_tool_scope`, consider treating an invalid prohibited pattern as a hard error rather than a warning (different from the protected-path case where the fail-open was a deliberate DoS-prevention choice).

**Spec compliance**: PARTIAL — the documented mitigation ("enforce at config load time") is not implemented anywhere in the codebase visible in this review.

---

### FINDING-M-002 [MEDIUM] `validate_tool_scope` does not enforce `max_file_changes` for negative numeric values

**Location**: `crates/pipeline/src/security.rs` — `validate_tool_scope`, ~lines 970–990

**Spec Reference**: `docs/spec/interfaces/security.md` §validate_tool_scope; `docs/spec/security.md` §THREAT-019

**Description**:
The `max_file_changes` check in `validate_tool_scope` reads:

```rust
matches!(
    value,
    serde_json::Value::Number(n) if n.as_u64().is_some_and(|v| v > u64::from(limit))
)
```

`serde_json::Number::as_u64()` returns `None` for negative numbers (they are stored as `i64` in serde_json). When `as_u64()` is `None`, `is_some_and(...)` evaluates to `false`, so the condition is not triggered. The consequence: a tool call with `"count": -1` or `"limit": -9999` passes the `max_file_changes` check unconditionally.

**Impact**:
A malicious LLM or skill could specify `count: -1` to bypass the file-change limit and invoke a batch tool that modifies an unrestricted number of files. Whether the downstream tool then interprets `-1` as "unlimited" is implementation-dependent.

**Remediation**:
Replace the inner check with:

```rust
// Treat any non-positive number as a violation (negative counts are nonsensical
// for file-change limits; zero is only valid when the limit is >= 0).
let v = match value {
    serde_json::Value::Number(n) => n.as_i64().unwrap_or(i64::MIN),
    _ => continue,
};
if v < 0 || v as u64 > u64::from(limit) {
    // violation
}
```

Or more directly, reject any numeric value that is not a non-negative integer:

```rust
if let serde_json::Value::Number(n) = value {
    let count = n.as_u64().ok_or_else(|| ToolScopeViolation {
        tool: tool.clone(),
        parameter_name: key.clone(),
        violated_constraint: format!(
            "Parameter '{key}' must be a non-negative integer for file-change limit enforcement."
        ),
    })?;
    if scope.max_file_changes.is_some_and(|limit| count > u64::from(limit)) {
        return Err(ToolScopeViolation { ... });
    }
}
```

**Spec compliance**: FAIL — `max_file_changes` limit is not enforced for negative input values.

---

### FINDING-M-003 [MEDIUM] `max_new_files` not enforced by `validate_scope`; enforcement entirely caller-delegated without mechanism or test

**Location**: `crates/pipeline/src/security.rs` — `validate_scope` and `ApprovedScope` docstring (~lines 660–670)

**Spec Reference**: `docs/spec/interfaces/security.md` §validate_scope; `docs/spec/security.md` §THREAT-013

**Description**:
`ApprovedScope.max_new_files` is explicitly documented as not enforced by `validate_scope` ("It is the caller's responsibility to separate new files from modified files and enforce this limit before calling `validate_scope`"). The test suite has no test that exercises any caller enforcing `max_new_files`. No helper or guard is provided.

**Impact**:
A pipeline node that creates new files using `validate_scope` as its sole scope check will silently accept any number of new files regardless of the configured `max_new_files` value. Scope creep via new file creation (THREAT-013) is not prevented.

**Remediation**:
Either:
(a) Add a `new_artifacts` parameter to `validate_scope` alongside `artifacts` and enforce the limit internally (preferred — keeps the security contract self-contained), or
(b) Add a companion `validate_new_file_count(new_files: &[ArtifactPath], scope: &ApprovedScope) -> Result<(), Vec<ScopeViolation>>` function with a test that ensures callers can use it, and update the `validate_scope` doc comment to reference it explicitly.

Option (a) is preferred as it removes the unverified caller obligation.

**Spec compliance**: PARTIAL — the field exists and is documented; enforcement is absent.

---

### FINDING-M-004 [MEDIUM] `ArtifactPath` does not normalize path components; traversal variants bypass glob patterns

**Location**: `crates/pipeline/src/identifiers.rs` — `string_id!` macro, `ArtifactPath::new`; `crates/pipeline/src/security.rs` — `is_protected`, `validate_scope`, `validate_tool_scope`

**Spec Reference**: `docs/spec/interfaces/security.md` §is_protected, §validate_scope

**Description**:
`ArtifactPath::new` accepts any non-empty string, including paths with `./` prefixes, `..` components, or duplicate separators. No normalization is performed. Glob patterns in `protected_paths` and `artifact_patterns` are written to match canonical paths (e.g., `.cogworks/pipeline.toml`, `src/**`).

Illustrative mismatches under current implementation:

| Input path | Protection pattern | `is_protected` result | Expected |
|---|---|---|---|
| `./src/main.rs` | `src/**` | `false` (no match) | `true` (should match) |
| `./.cogworks/rules.md` | `**/.cogworks/**` | `false` (no match with `normalize_glob_pattern`) | `true` |

Note: `**/.cogworks/**` with `globset` matching against `./.cogworks/rules.md` may or may not match depending on how `globset` interprets the leading `./` — this should be tested but currently is not.

Additionally, `../` traversal paths (`../../.cogworks/rules.md`) could be submitted by a compromised LLM as artifact paths. While `**/.cogworks/**` may still match (because `**` matches across `../`), protection patterns without `**` (e.g., `.cogworks/rules.md` as an exact path pattern) would not match.

**Impact**:
A path like `./src/lib.rs` that should match the approved scope `src/**` might be rejected as unauthorized, causing false-positive scope violations. More critically, paths like `./.cogworks/rules.md` might bypass `is_protected` for operators who write patterns without careful consideration of the leading-dot case.

**Remediation**:

1. Normalize `ArtifactPath` values at construction time (strip leading `./`, reject `..` components, normalize duplicate slashes). A minimal implementation:

   ```rust
   pub fn new(value: impl Into<String>) -> Option<Self> {
       let v = value.into();
       if v.is_empty() { return None; }
       // Reject traversal components.
       if v.split('/').any(|seg| seg == "..") { return None; }
       // Strip leading ./
       let v = v.strip_prefix("./").map(String::from).unwrap_or(v);
       if v.is_empty() { return None; }
       Some(Self(v))
   }
   ```

2. Add `is_protected` tests for paths with `./` prefix and `..` components.

**Spec compliance**: UNSPECIFIED — `ArtifactPath` construction semantics are not documented beyond "non-empty string"; path normalization is not specified but is required for correct glob matching.

---

### FINDING-M-005 [MEDIUM] Approved-branch allowlist hardcoded to `"master"` / `"main"`; not configurable

**Location**: `crates/pipeline/src/security.rs` — `validate_constitutional_prompt`, ~lines 370–376

**Spec Reference**: `docs/spec/interfaces/security.md` §validate_constitutional_prompt ("will be configurable in a later PR"); `docs/spec/security.md` §THREAT-015 mitigation 2

**Description**:
The branch validation check is:

```rust
if branch_str != "master" && branch_str != "main" {
    return Err(ConstitutionalError::InvalidSourceBranch { ... });
}
```

No configuration parameter is accepted. A repository that uses `release`, `stable`, `trunk`, or any other default branch name cannot use CogWorks without modifying source code.

**Impact**:
Deployments on repositories with non-standard default branches will fail with `InvalidSourceBranch` on every LLM call. This is a denial-of-service against legitimate use. More subtly, the hardcoded list is a code-level policy — changing the policy requires a code change and deployment.

**Remediation**:
Add an `approved_branches: &[BranchName]` parameter to `validate_constitutional_prompt` (or load it from a config struct), replacing the hardcoded check:

```rust
if !approved_branches.iter().any(|b| b == &rules.source_branch) {
    return Err(ConstitutionalError::InvalidSourceBranch { branch: rules.source_branch.clone() });
}
```

Update callers to pass the configured approved branch list. Keep `"master"` and `"main"` as the default in any config struct that doesn't require explicit configuration.

**Spec compliance**: PARTIAL — the spec explicitly deferred configurability to a later PR; however this review notes it as a blocking medium issue since any deployment to a non-standard-branch repository will fail entirely.

---

### FINDING-L-001 [LOW] Hash comparison uses variable-time `String !=`; `subtle` crate already available in workspace

**Location**: `crates/pipeline/src/security.rs` — `validate_constitutional_prompt`, ~line 381

**Spec Reference**: `docs/spec/security.md` §THREAT-015

**Description**:

```rust
if computed != rules.source_hash {
    return Err(ConstitutionalError::HashMismatch { ... });
}
```

`String !=` in Rust is not guaranteed to be constant-time. The `subtle` crate (`subtle = "2"`, already declared as a workspace dependency and used in `crates/listener/`) provides constant-time comparison.

**Impact Assessment**:
The practical timing-oracle risk here is low. Both sides of the comparison are values the caller controls (`content` and `source_hash` are fields of `ConstitutionalRules` constructed by the caller). A true timing oracle requires the attacker to control one side while the other is secret; neither the `content` nor the `source_hash` is secret to an attacker who can submit a `ConstitutionalRules` struct. The comparison is an integrity check between two caller-supplied values, not a secret verification.

The risk is flagged as Low because: (a) the `subtle` crate is already in the workspace, (b) using constant-time comparison is best practice for any hash comparison, (c) a future refactor that makes the expected hash a system secret (e.g., pinned at pipeline build time) would retroactively make this a higher-severity issue.

**Remediation**:

```rust
use subtle::ConstantTimeEq;

// Compare as byte slices for constant-time equality.
if computed.as_bytes().ct_ne(rules.source_hash.as_bytes()).into() {
    return Err(ConstitutionalError::HashMismatch { ... });
}
```

---

### FINDING-L-002 [LOW] `InjectionDetectionResult::offending_text` reports corpus phrase, not original input span

**Location**: `crates/pipeline/src/security.rs` — `detect_injection`, ~line 540

**Spec Reference**: ASSERT-SEC-004 ("offending_text is non-empty"); `docs/spec/interfaces/security.md` §InjectionDetectionResult

**Description**:

```rust
return InjectionDetectionResult::InjectionDetected {
    source: source_label.to_string(),
    offending_text: (*phrase).to_string(),   // ← corpus phrase, not input text
    pattern: pattern.clone(),
};
```

`offending_text` is set to the matched corpus phrase (always lowercase), not to the actual text span from the input. The audit record for a detection of `"IGNORE ALL PREVIOUS INSTRUCTIONS"` will show `offending_text: "ignore all previous instructions"`.

**Impact**:
The audit record loses the original casing, surrounding context, and exact position of the injection attempt. Forensic investigation is hampered — the security analyst cannot determine whether the detection was a false positive, cannot see how the injection was phrased, and cannot improve the detection corpus without re-examining the original source content separately.

**Remediation**:
Capture the matching substring from the lowercased content's position in the original content:

```rust
let lower = content.to_lowercase();
if let Some(pos) = lower.find(phrase) {
    let span = &content[pos..pos + phrase.len()];
    return InjectionDetectionResult::InjectionDetected {
        source: source_label.to_string(),
        offending_text: span.to_string(),  // original casing from input
        pattern: pattern.clone(),
    };
}
```

(This also requires restructuring the inner `contains` → `find` loop, but is straightforward.)

---

### FINDING-L-003 [LOW] All `String`-typed tool parameters treated as file paths; non-path string parameters generate false scope violations

**Location**: `crates/pipeline/src/security.rs` — `validate_tool_scope`, ~lines 955–970

**Spec Reference**: `docs/spec/interfaces/security.md` §validate_tool_scope

**Description**:
`validate_tool_scope` applies glob-pattern matching to **every** string-valued parameter regardless of parameter name or semantic type:

```rust
if let serde_json::Value::String(path) = value {
    if prohibited_set.is_match(path.as_str()) { ... reject ... }
    if !allowed_set.is_match(path.as_str()) { ... reject ... }
}
```

A tool with a `"message"`, `"title"`, `"comment"`, or `"description"` string parameter would always be rejected (the value would not match `src/**` or similar file-path scope patterns), even for legitimate tool calls.

**Impact**:
If any pipeline tool in scope uses non-path string parameters, those tool calls will be incorrectly rejected by scope validation, causing false-positive halts. This creates a usability constraint: only tools whose every string parameter is a file path can be validated by this function.

**Remediation**:
Introduce a per-tool or per-parameter annotation in `ToolParams` (or in the tool schema) to distinguish path parameters from other string parameters. The scope check should only apply to parameters explicitly marked as file paths. A minimal approach: check only parameters whose names match a list of known path-parameter names (e.g., `"path"`, `"file"`, `"target"`, `"source"`, `"artifact"`).

---

### FINDING-L-004 [LOW] `validate_tool_scope` first-violation selection is non-deterministic due to `HashMap` iteration order

**Location**: `crates/pipeline/src/security.rs` — `validate_tool_scope`, `ToolParams.params: HashMap<String, serde_json::Value>`

**Spec Reference**: `docs/spec/interfaces/security.md` §validate_tool_scope

**Description**:
`validate_tool_scope` iterates `params.params` (a `HashMap`) and returns on the first violation. `HashMap` does not guarantee a stable iteration order. When multiple parameters violate constraints, the violation reported in the `Err` is non-deterministic across program runs (including across different builds due to hash-randomization).

**Impact**:
No security impact — the function correctly returns `Err` for any violating invocation. The affected quality is debuggability: the violation logged in the audit trail for a given input may differ between runs, making it harder to reproduce and investigate incidents.

**Remediation**:
Replace `HashMap<String, serde_json::Value>` in `ToolParams.params` with `IndexMap<String, serde_json::Value>` (from the `indexmap` crate, which preserves insertion order) or sort parameter keys before iterating:

```rust
let mut sorted_keys: Vec<&String> = params.params.keys().collect();
sorted_keys.sort();
for key in sorted_keys {
    let value = &params.params[key];
    // ... existing checks
}
```

---

## Informational Observations

### INFO-001: `cargo audit` advisories are already documented and justified in `deny.toml`

The three advisories (RUSTSEC-2024-0384, RUSTSEC-2024-0436, RUSTSEC-2026-0097) are all in the `listener → queue-runtime → azure_core` chain, have no upstream fix available, and are ignored in `deny.toml` with per-advisory rationale. No action required for this review. Recommend revisiting when `azure_core` is updated.

For RUSTSEC-2023-0071 (Marvin Attack in `rsa 0.9.x` via `github-bot-sdk`): the `deny.toml` rationale correctly notes that the `rsa` crate is used only for webhook signature verification (no decryption oracle is exposed). Accept.

### INFO-002: `cargo deny` configuration housekeeping

Four `unnecessary-skip` warnings and two `license-not-encountered` warnings. No security impact. Can be cleaned up in a maintenance PR.

### INFO-003: `sha2` and `globset` are appropriate, well-maintained choices

- `sha2 = "0.10"` (RustCrypto): industry-standard Rust SHA-2 implementation, audited by Trail of Bits, no known CVEs. Correct choice for constitutional rules integrity verification.
- `globset = "0.4"` (BurntSushi): widely used `.gitignore`-style glob matching library, actively maintained, no known CVEs. Correct choice for path-pattern enforcement.

### INFO-004: Module documentation claims "pure functions" but `build_glob_set` emits `tracing::warn!`

`tracing::warn!` is a side effect (structured log emission). The module doc comment states all functions are "pure — no I/O, no async." This is a documentation inaccuracy. `tracing` is not prohibited by `docs/spec/constraints.md` (which excludes only `tokio`, `reqwest`, `octocrab`, `std::fs`, `std::process`), so the implementation is not a constraint violation — but the module-level doc comment should be updated to say "no filesystem I/O, no network I/O, no async" rather than "pure."

---

## Security Assertions Compliance Matrix

| Assertion | Status | Finding |
|---|---|---|
| ASSERT-SEC-001: Non-approved branch rejected | ✅ PASS | — |
| ASSERT-SEC-002: Hash mismatch before rule-presence check | ✅ PASS | — |
| ASSERT-SEC-003: All missing rules listed | ✅ PASS | — |
| ASSERT-SEC-004: Injection detected with correct fields | ✅ PASS (canonical phrases) / ⚠️ PARTIAL (bypass vectors) | H-001 |
| ASSERT-SEC-005: Protected path overrides approved scope, no double-flagging | ✅ PASS | — |

## OWASP API Top 10 Compliance

| Category | Status | Finding(s) |
|---|---|---|
| A01 Broken Access Control | ⚠️ PARTIAL | M-002 (`max_file_changes` bypass for negative values), M-003 (`max_new_files` not enforced) |
| A03 Injection | ⚠️ PARTIAL | H-001 (injection detector bypassed by whitespace/Unicode normalization) |
| A04 Insecure Design | ⚠️ PARTIAL | M-001 (fail-open invalid patterns), M-004 (path normalization gap), M-005 (hardcoded branch list) |
| A09 Security Logging | ✅ PASS (for invalid patterns) | L-002 (audit record loses original offending text) |

---

## Remediation Priority

| Priority | Finding | Rationale |
|---|---|---|
| 1 | **H-001** | Trivially bypassable injection detector; defeats a primary THREAT-001 mitigation |
| 2 | **M-002** | Numeric bypass allows `max_file_changes` limit to be circumvented with a single negative value |
| 3 | **M-001** | Silent fail-open for invalid protected-path patterns; add config-load validation |
| 4 | **M-004** | `ArtifactPath` path normalization prevents `./`-prefixed paths from matching protections |
| 5 | **M-003** | `max_new_files` entirely caller-delegated; add enforcement or companion function |
| 6 | **M-005** | Hardcoded branch allowlist; add `approved_branches` parameter (deferred by spec, but notable) |
| 7 | **L-001** | Swap `String !=` for `subtle::ConstantTimeEq` (workspace dep already present) |
| 8 | **L-002** | Capture original input span in `offending_text` for audit forensics |
| 9 | **L-003** | Scope non-path string parameters out of file-path glob checks |
| 10 | **L-004** | Use `IndexMap` or pre-sort to make first-violation deterministic |
