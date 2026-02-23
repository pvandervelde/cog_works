# Implementation Constraints

These constraints MUST be enforced during implementation. They inform the interface designer on type-level decisions and give the coder hard rules.

---

## Type System

- **Branded/newtype identifiers**: All domain identifiers must use distinct types that cannot be accidentally interchanged. `WorkItemId`, `SubWorkItemId`, `PipelineStageLabel`, `BranchName`, etc. must not be bare strings or integers.
- **Result-based error handling**: All domain operations must return `Result<T, E>`. Business errors are values in the `Err` variant, not panics or exceptions.
- **Exhaustive error types**: Each component's error type must be an enum covering all failure modes. No catch-all `Other(String)` variants in domain error types. Infrastructure error types may have a catch-all for truly unexpected failures.
- **No `unwrap()` in production code**: All `Option` and `Result` values must be explicitly handled. `unwrap()`, `expect()` are allowed only in tests.
- **Structured data over strings**: All data that crosses a component boundary must be a typed struct or enum, not a serialized string. Parsing happens at the boundary (infrastructure layer), not in business logic.

---

## Module Boundaries

- **Business logic is pure**: Business logic modules must not import I/O crates (`tokio`, `reqwest`, `octocrab`, `std::fs`, `std::process`). They operate on data passed in as arguments.
- **Abstractions are traits**: External system interfaces are defined as Rust traits. Business logic depends on these traits via generics or trait objects.
- **Infrastructure implements traits**: Each infrastructure module implements one or more abstraction traits. Only infrastructure modules import external crates.
- **No circular dependencies**: Module dependency graph must be a DAG. Business logic → abstractions ← infrastructure. Abstractions never depend on business logic or infrastructure.
- **Stage executors are orchestration, not business logic**: Stage executors coordinate calls between business logic, abstractions, and other stage executors. They contain sequencing logic, not domain rules.
- **Scenario holdout enforcement**: The Context Assembler MUST enforce that scenario specifications are never included in code generation context packages. This is a hard constraint, not advisory. The configuration must explicitly list scenario directories to exclude.
- **Domain services are external processes**: CogWorks MUST NOT contain domain-specific code. All domain operations (validate, normalise, review_rules, simulate, validate_deps, extract_interfaces, dependency_graph) are delegated to external domain services via the Extension API.
- **No built-in privileged path**: The Rust domain service MUST use the Extension API like any other domain service. If the API is insufficient for the Rust domain service, the API must be improved, not bypassed.
- **Interface registry is human-authored**: CogWorks MUST NOT create or modify interface definitions autonomously. It MAY suggest additions as recommendations for human review.
- **Domain services do not communicate directly**: CogWorks mediates all cross-domain interactions. Domain services do not invoke or depend on each other.

---

## Error Handling

- **Expected errors are `Result` values**: Validation failures, budget exceeded, scope threshold exceeded, review blocking findings — these are all `Err` variants, not panics.
- **Retryable vs. non-retryable**: Error types must distinguish between retryable failures (API timeout, rate limit) and non-retryable failures (budget exceeded, invalid configuration). The LLM Gateway and GitHub Client must expose this distinction.
- **Error context**: All error types must include sufficient context for debugging — what operation was attempted, what input was provided, what went wrong. Use structured error types, not just strings.
- **Error propagation**: Errors flow upward through the call stack via `Result`. Each layer may enrich the error with additional context (what it was trying to do when the error occurred).
- **Escalation is an error kind**: When the system cannot resolve an issue within its budget, it produces an escalation result — a structured report of what was tried and what failed. This is a value, not an exception.

---

## Testing

- **Business logic: 100% unit test coverage**: Every function in business logic modules must have tests covering happy paths, error cases, and edge conditions. No mocks needed — pure input/output.
- **Abstraction traits: contract tests**: Each trait must have a test suite that any implementation must pass. Written as generic test functions parameterized by the trait.
- **Infrastructure: integration tests**: Each infrastructure implementation must be tested against the real external system (or a faithful simulation). Use testcontainers, mock HTTP servers, or temporary git repos as appropriate.
- **Stage executors: integration tests with mocks**: Test the orchestration logic by providing mock implementations of abstractions. Verify the correct sequence of calls and state transitions.
- **Test naming**: `test_<function>_<scenario>_<expected>` (per `.tech-decisions.yml`).
- **No test pollution**: Tests must not depend on external state. Each test sets up its own context and tears it down.
- **Scenarios test generated code, not CogWorks**: Scenario validation tests the code generated by CogWorks. CogWorks' own correctness is tested through unit/integration/E2E tests as defined in testing.md.
- **Pyramid summary accuracy**: Summaries must be regenerated when source changes. Staleness check must be deterministic (file hash comparison). Stale summaries must never be used for context assembly decisions.
- **Extension API conformance tests**: A published conformance test suite must exist that any domain service can run to verify it correctly implements the Extension API. The Rust domain service must pass this suite.
- **Cross-domain constraint validation tests**: The constraint validator must have tests for all contract parameter types (numeric with tolerance, exact, enumerated, boolean, reference, computed).

---

## Performance

- **Pipeline step function < 60s wall time**: A single CLI invocation (one step function execution) should complete within 60 seconds, excluding LLM API latency and domain service operation latency. If a step takes longer, it's likely doing too much.
- **LLM latency is external**: The system must not add significant overhead on top of LLM response time. Context assembly, schema validation, and result processing should each take < 1 second.
- **GitHub API efficiency**: Minimize API calls per invocation. Batch reads where possible (e.g., read all labels in one call, not one per label). Cache within a single invocation (not across invocations — stateless design).
- **Domain service operations are external**: Domain service latency (validation, simulation, etc.) is external to CogWorks. CogWorks must support progress polling for long-running operations.
- **Subprocess timeouts (domain service side)**: Domain services are responsible for their own subprocess timeouts. CogWorks enforces an overall operation timeout per domain service method call (configurable, default: 10 minutes for simulate, 5 minutes for validate/normalise/review_rules).

---

## Security

- **Minimum-privilege GitHub token**: The token must have only the permissions needed: issues (read/write), pull requests (read/write), contents (read/write), labels (read/write). No admin access.
- **No secrets in context packages**: LLM API keys, GitHub tokens, and any other credentials must never appear in context packages, audit trails, or generated code.
- **No secrets in generated code**: Code generation must use placeholder values for secrets, with documentation on what needs to be configured.
- **Domain service isolation**: Domain services run as separate processes. CogWorks does not pass secrets to domain services. Domain services receive only the Extension API context (work item info, stage, repository path, relevant interface contracts).
- **Prompt injection awareness**: Issue bodies and repository content are untrusted input. The system must validate all LLM outputs against schemas — never execute LLM output as code or commands within the CogWorks process itself.
- **Rate limit respect**: The system must respect GitHub API rate limits (5000/hr for authenticated requests). Track remaining budget from response headers, back off proactively.
- **Extension API authentication**: For Unix domain sockets, file system permissions provide access control. For HTTP/gRPC transport, authentication mechanism is to be determined but the design must not preclude adding authentication later (e.g., bearer tokens, mutual TLS).

---

## Configuration

- **Configuration file**: `.cogworks/config.toml` in the target repository. Loaded once per CLI invocation.
- **Mandatory fields**: At minimum: a reference to a domain service registration file and at least one LLM model selection.
- **Domain service registration file**: Domain services are declared in `.cogworks/services.toml` (overridable via `COGWORKS_DOMAIN_SERVICES_CONFIG`), not in `config.toml`. Each service entry (under `[[services]]`) specifies name, transport, and connection endpoint (socket path or URL). Service capabilities, artifact types, interface types, and domain are discovered dynamically via the handshake (health check) — they are NOT statically configured. This keeps the config minimal and ensures the config can't drift from what the service actually provides.
- **Interface registry configuration**: `[interfaces]` section specifying registry directory and startup validation flag.
- **Constraint validation configuration**: `[constraint_validation]` section specifying enabled flag and missing-service behavior.
- **Sensible defaults**: Every configurable value must have a sensible default. A minimal configuration file should be sufficient to run the pipeline.
- **Validation at load time**: Configuration must be fully validated when loaded. Invalid configuration produces a clear error message and halts the pipeline before any work begins.
- **No environment-variable-driven behavior in business logic**: Environment variables are read in the infrastructure/configuration layer only. Business logic receives typed configuration values, never raw strings from the environment.

---

## Extension API

- **API version compatibility**: CogWorks declares supported API versions. Domain services declare their implemented version. Incompatible versions are rejected during health check handshake.
- **JSON Schema enforcement**: All Extension API messages must conform to published JSON Schemas in `schemas/extension-api/`. Schema changes follow semantic versioning (additive = minor, breaking = major).
- **Transport flexibility**: Default transport is Unix domain sockets. HTTP/gRPC support is configurable per domain service. The protocol layer must be transport-agnostic so new transports can be added without changing business logic.
- **Progress polling**: The v1 Extension API baseline is synchronous request-response with configurable timeouts. The protocol design must accommodate adding progress polling (via operation IDs) or streaming transport in a future API version without breaking existing domain services. Domain services are NOT required to implement progress polling in v1.
- **Domain services own their lifecycle**: CogWorks does not start, stop, or manage domain service processes. Services are started independently (systemd, Docker, manually). CogWorks checks health before invocation.
- **Graceful degradation**: Primary domain service unavailable = halt. Secondary domain service unavailable = continue with warning. This distinction is based on whether the service covers the domain of the artifacts being generated.
- **Standardised diagnostic categories**: Domain service diagnostics must use the standardised diagnostic category set (`syntax_error`, `type_error`, `constraint_violation`, `interface_mismatch`, `dependency_error`, `style_violation`, `safety_concern`, `performance_concern`, `test_failure`, `completeness`). Domain services may use additional domain-specific categories; consumers treat unknown categories as informational.
- **Standardised error codes**: Service-level errors (cannot process request) must use the standardised error code set (`tool_not_found`, `tool_failed`, `invalid_request`, `unsupported_method`, `api_version_mismatch`, `timeout`, `artifact_not_found`, `internal_error`). Each code has a defined recoverability (retryable or non-retryable). Consumers use the `recoverable` field to decide retry strategy.

---

## Observability

- **Structured logging**: All log output must be structured (JSON). Each log entry must include: `pipeline_id` (work item number), `stage`, `sub_work_item` (if applicable), `action`, `result`, `duration_ms`.
- **Audit trail completeness**: Every LLM call, validation result, state transition, and cost event must appear in the audit trail. The trail must be sufficient to reconstruct the full decision history.
- **Cost visibility**: Token usage and cost must be tracked per-call, per-stage, and per-pipeline. The final cost report must be posted as a comment on the work item.

---

## Code Style (Rust-Specific)

- **Follow Rust conventions**: `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants (per `.tech-decisions.yml`).
- **Max function length: 50 lines** (per `.tech-decisions.yml`).
- **Max file length: 500 lines** (per `.tech-decisions.yml`).
- **Max cyclomatic complexity: 10** (per `.tech-decisions.yml`).
- **`clippy` clean**: Code must pass `clippy` with no warnings.
- **`rustfmt` formatted**: Code must be formatted with `rustfmt`.
- **Public API documentation**: All public types, functions, and traits must have `///` doc comments including purpose, parameters, return values, error conditions, and examples where non-trivial.
