# Testing Strategy

This document defines how CogWorks itself is tested. Not how CogWorks tests generated artifacts (that's a domain service operation), but how we verify CogWorks is correct.

---

## Testing Pyramid

```
            ┌─────────┐
            │  E2E    │   Few: Full pipeline against real GitHub + LLM
            │  Tests  │
           ─┼─────────┼─
          │  Integration  │  Moderate: Stage executors with mock services
          │    Tests      │
         ─┼───────────────┼─
        │    Unit Tests       │  Many: Business logic, pure input/output
        │                     │
        └─────────────────────┘
```

---

## Unit Tests (Business Logic)

Business logic is pure — no I/O, no mocks needed. Test with direct input/output assertions.

### Pipeline State Machine

- All valid stage transitions (stage N → stage N+1)
- Invalid transitions (skip a stage, go backwards)
- Human-gated vs. auto-proceed behavior
- Safety-critical gate override
- Processing lock detection (back off when another instance is active)
- Sub-work-item ordering within stages 5-7

### Classification Rules

- Safety override: LLM says safe + registry match → override to unsafe
- Safety no-override: LLM says safe + no registry match → remains safe
- LLM says unsafe → always unsafe regardless of registry
- Scope within threshold → proceed
- Scope exceeds threshold → escalation

### Dependency Graph

- Empty graph → valid (no sub-work-items, edge case)
- Linear chain (A → B → C) → valid order [A, B, C]
- Diamond (A → B, A → C, B → D, C → D) → valid order respecting all edges
- Simple cycle (A → B → A) → error identifying cycle
- Complex cycle (A → B → C → A) → error identifying cycle
- Disconnected components → valid order (independent items in stable order)
- Self-dependency (A → A) → error

### Review Aggregation

- All passes clean → proceed
- One blocking finding in quality → remediate
- One blocking finding in security → remediate
- Blocking in multiple passes → remediate (all findings collected)
- Warnings only → proceed (warnings preserved for PR comments)
- Mixed blocking + warnings → remediate (warnings still preserved)
- Remediation cycle count exceeded → escalate

### Budget Enforcement

- Cost within budget → approved
- Cost exactly at budget → approved (boundary condition)
- Cost exceeding budget → denied with cost report
- Cost report structure (per-stage, per-sub-work-item breakdown)

### Context Priority and Truncation

- Context fits in window → no truncation
- Context exceeds window → lowest priority items removed first
- Current SWI interface is never removed (highest priority)
- Empty context → valid (edge case)
- Single item exceeding window → item truncated with warning

### Label Parsing and Generation

- Round-trip: structured type → label string → structured type = identity
- All label patterns: stage, status, depends-on, order, processing, safety-critical
- Invalid label strings → parse error (not panic)

---

## Integration Tests (Stage Executors with Mocks)

Stage executors orchestrate calls between business logic and abstractions. Test them with mock implementations of abstraction traits.

### Mock Implementations

- **Mock LLM Provider**: Returns pre-configured responses; tracks calls for assertion; can simulate failures (API error, schema validation failure, rate limit)
- **Mock Issue Tracker**: In-memory issue store; returns configured issues; tracks all mutations (create, label, comment)
- **Mock PR Manager**: In-memory PR store; returns configured PRs; tracks all mutations
- **Mock Code Repository**: In-memory file store; returns configured file contents
- **Mock Domain Service Client**: Returns pre-configured structured responses (diagnostics with standard categories, simulation results, dependency graphs); pre-configured handshake responses (capabilities, domain, artifact types, interface types); can simulate unavailability, API version mismatch, partial capabilities, timeout, standardised error codes, and malformed responses
- **Mock Template Engine**: Returns pre-configured rendered strings; tracks variable usage
- **Mock Audit Store**: In-memory event log; allows assertions on recorded events
- **Mock Interface Registry Loader**: Returns pre-configured interface definitions; can simulate missing definitions, schema errors, or conflicts
- **Mock Constraint Validator**: Returns pre-configured constraint validation results; can simulate hard failures, warnings, and extraction errors

### Test Scenarios per Stage

**Task Classifier (Stage 1):**

- Happy path: Issue body → LLM classification → safety cross-validation → labels applied
- LLM returns invalid schema → retry with error → valid response → proceed
- Safety override triggered → safety-critical label applied
- Scope exceeded → escalation produced

**Specification Generator (Stage 2):**

- Happy path: Context assembled → LLM generates spec → validation passes → PR created
- Module reference validation fails → retry with error → passes on retry → PR created
- Constraint violation → retry with error → fails again → retry budget exceeded → escalation
- PR already exists (idempotency) → detected, no duplicate created

**Interface Generator (Stage 3):**

- Happy path: Spec read → context assembled → LLM generates interfaces → domain service validates → PR created
- Validation fails → retry with error → passes on retry → PR created
- Parse error in generated code → retry with error

**Work Planner (Stage 4):**

- Happy path: Decomposition → validation → sub-issues created
- Cycle detected → retry with error → acyclic plan → sub-issues created
- Too many sub-work-items → escalation
- Missing interface coverage → retry with error

**Code Generator (Stage 5):**

- Happy path: Generate → normalise → validate → review_rules → simulate → pass
- Validation failure → feedback → retry → pass
- Simulation failure (self-explanatory) → direct feedback → retry → pass
- Simulation failure (non-obvious) → LLM interpretation → feedback → retry → pass
- Retry budget exhausted → escalation
- Cost budget exceeded mid-generation → halt with report
- Domain service unavailable → early failure with diagnostic

**Review Executor (Stage 6):**

- Happy path: Four passes (1 deterministic + 3 LLM), all clean → proceed
- Blocking finding → feed back to code generator
- Multiple remediation cycles → finding resolved → proceed
- Remediation cycle limit exceeded → escalation

**Integration Manager (Stage 7):**

- Happy path: PR created with correct references and comments
- Non-blocking findings posted as inline comments
- PR already exists (idempotency) → detected

### Pipeline Executor (Full Stage Sequence)

- Fresh work item → progresses through all stages in sequence
- Re-processing after crash at each stage → resumes correctly
- Human-gated stage → stops and waits
- Safety-critical work item → forced human gates

---

## Integration Tests (Infrastructure)

Each infrastructure implementation tested against the real external system or a faithful simulation.

### GitHub Client

- Use a dedicated test repository on GitHub
- Test: create issue, add label, remove label, post comment, create branch, commit file, create PR, read PR status, list sub-issues
- Test rate limit handling: verify backoff when rate limit headers indicate low budget
- Test pagination: verify correct handling of paginated responses

### Anthropic LLM Provider

- Use mock HTTP server (e.g., `wiremock`) simulating Anthropic API
- Test: successful completion, rate limit response, server error, malformed response
- Test token counting accuracy against known inputs
- Test retry behavior with exponential backoff

### Extension API Client

- Use a mock domain service process (test harness that speaks the Extension API protocol)
- Test: health_check (handshake), validate, normalise, review_rules, simulate, dependency_graph, extract_interface_surface
- Test: Unix domain socket transport — connection, message exchange, disconnection
- Test: HTTP transport — connection, message exchange, error responses
- Test: Handshake response parsing — capabilities, artifact types, interface types, domain, API version discovered correctly
- Test: Handshake result caching — re-query on error, periodic refresh
- Test: timeout enforcement — domain service that never responds is terminated after configured timeout
- Test: malformed response handling — invalid JSON, schema-violating response, unexpected message type
- Test: API version negotiation — handshake returns incompatible version → graceful rejection
- Test: domain service crash mid-operation → CogWorks detects disconnection and reports failure
- Test: standardised error code handling — each error code mapped to correct retryability
- Test: diagnostic category handling — known and unknown categories parsed correctly

### Interface Registry Loader

- Use fixture `.cogworks/interfaces/` directory with known TOML files
- Test: valid definitions loaded and parsed correctly
- Test: malformed TOML → clear error with file name and line
- Test: schema-violating definition → clear error identifying the violation
- Test: conflicting parameter definitions across files → detected and reported
- Test: missing interfaces directory → empty registry (not error)
- Test: version format validation

---

## Extension API Conformance Test Suite

CogWorks publishes a conformance test suite that domain service authors run against their implementations. This is separate from CogWorks' own tests.

### Purpose

- Verify a domain service correctly implements the Extension API protocol
- Catch protocol mismatches before deployment
- Serve as living documentation of the API contract

### Coverage

- **Health check**: Returns correct version, capabilities list, and status
- **Validate**: Accepts artifact, returns structured diagnostics (with severity, location, message)
- **Normalise**: Accepts artifact, returns normalised artifact content or no-change indicator
- **Review rules**: Accepts artifact, returns domain-specific rule findings
- **Simulate**: Accepts test specification, returns pass/fail with structured output
- **Dependency graph**: Accepts artifact set, returns adjacency list
- **Extract interface surface**: Accepts artifact, returns public interface description
- **Error responses**: Correct error codes for invalid requests, unsupported capabilities, internal failures
- **Progress polling**: Long-running operations return operation ID; poll_progress returns correct status transitions
- **Graceful shutdown**: Domain service handles termination signals cleanly

### Distribution

- Published as a standalone binary or test harness alongside CogWorks releases
- Domain service authors integrate it into their CI pipeline

---

## End-to-End Tests

Full pipeline tests against real GitHub and real LLM API. These are expensive and slow — run sparingly (nightly, pre-release).

### Setup

- Dedicated test repository on GitHub with known structure
- Test issues with known expected outcomes
- LLM API key with cost budget for testing

### Scenarios

- **Happy path**: Create issue → apply trigger label → verify all stages complete → verify PRs created with correct content
- **Safety-critical path**: Create issue touching safety-critical module → verify human gates enforced
- **Escalation path**: Create issue with scope exceeding threshold → verify escalation comment posted
- **Re-trigger path**: Fail a sub-work-item intentionally → re-trigger → verify recovery

### Cost Control

- E2E tests have their own cost budget (separate from production)
- Tests that would exceed budget are skipped with a warning
- Use cheaper/faster LLM models for E2E testing where possible

---

## Test Fixtures

The test suite requires several fixture types:

- **Fixture domain projects**: Small, valid projects for each supported domain (e.g., Rust crate, KiCad schematic) with known validation results, simulation results, and interface surfaces. Stored in `tests/fixtures/domains/`.
- **Fixture interface registries**: Hand-crafted `.cogworks/interfaces/` directories with known valid, invalid, and conflicting TOML definitions. Stored in `tests/fixtures/interfaces/`.
- **GitHub API response fixtures**: Recorded or hand-crafted JSON responses for GitHub API calls. Stored in `tests/fixtures/github/`.
- **LLM response fixtures**: Recorded or hand-crafted LLM responses for each stage's output schema. Stored in `tests/fixtures/llm/`.
- **Extension API message fixtures**: Hand-crafted JSON request/response pairs for each Extension API method. Stored in `tests/fixtures/extension_api/`.
- **Configuration fixtures**: Various `.cogworks/config.toml` files exercising different configuration options (including domain service registration). Stored in `tests/fixtures/config/`.

---

## Coverage Requirements

| Layer | Coverage Target | Measured By |
|-------|----------------|-------------|
| Business logic | 100% line coverage | `cargo tarpaulin` or `grcov` |
| Stage executors | 90% line coverage | `cargo tarpaulin` or `grcov` |
| Infrastructure | 80% line coverage | `cargo tarpaulin` or `grcov` |
| E2E | Key scenarios covered | Manual checklist |

Mutation testing (per `.tech-decisions.yml` target of 70%) applied to business logic modules.
