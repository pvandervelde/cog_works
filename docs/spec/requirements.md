# Functional Requirements

This document defines the functional requirements for CogWorks. Every requirement has a unique ID that is referenced in `assertions.md` to establish traceability between requirements and testable behavioral assertions.

Requirements are organized by functional area. Each requirement states what the system **MUST** or **MUST NOT** do. Implementation constraints (how it must be done) are in `constraints.md`.

---

## REQ-PIPE: Pipeline Execution

### REQ-PIPE-001: Trigger on label

CogWorks MUST initiate the pipeline when the `cogworks:run` label is applied to a work item that has no current `cogworks:stage:*` label.

### REQ-PIPE-002: Stage label tracking

CogWorks MUST apply a `cogworks:stage:<name>` label to track the current pipeline stage of each work item. Only one stage label may be present at a time.

### REQ-PIPE-003: Stage advancement

CogWorks MUST advance to the next pipeline stage when the current stage's gate is satisfied (auto-proceed configuration, or human approval in human-gated configuration).

### REQ-PIPE-004: Idempotent operations

All pipeline operations MUST be idempotent. Re-invoking the step function for the same GitHub state MUST produce the same outcome or detect prior completion and take no action.

### REQ-PIPE-005: Audit comments

CogWorks MUST post a structured status comment on the work item issue when entering each stage, when a stage completes successfully, and when a failure occurs.

### REQ-PIPE-006: Stage gate enforcement

CogWorks MUST respect stage gate configuration (`auto-proceed` or `human-gated`). Safety-critical work items MUST override all code-producing stage gates to `human-gated` regardless of repository configuration.

### REQ-PIPE-007: Processing lock

CogWorks MUST apply a `cogworks:processing` label before processing a work item and MUST back off (exit without taking action) if that label is already present when a new invocation begins.

---

## REQ-CLASS: Task Classification

### REQ-CLASS-001: Classification schema

The Task Classifier MUST produce structured output conforming to the classification schema: task type (enum), affected modules (list), estimated scope, safety_affecting flag, and rationale.

### REQ-CLASS-002: Safety override

If any module in the `affected_modules` list is registered in the safety-critical module registry, the final classification MUST set `safety_affecting: true`, regardless of what the LLM produced.

### REQ-CLASS-003: Scope threshold

When the estimated scope exceeds the configured threshold, the Task Classifier MUST produce an escalation result and MUST NOT proceed to subsequent stages.

---

## REQ-ARCH: Architecture and Specification

### REQ-ARCH-001: Architectural context

The Specification Generator MUST include relevant ADRs, coding standards, and architectural constraints from the repository in its LLM context.

### REQ-ARCH-002: Specification format

The generated specification MUST be a structured Markdown document containing: affected modules, design decisions with rationale, dependency changes, risk assessment, and required ADRs.

### REQ-ARCH-003: Specification delivered as PR

The specification document MUST be delivered via a Pull Request targeting the appropriate branch of the target repository.

### REQ-ARCH-004: Specification gate

The pipeline MUST NOT advance past the architecture stage until the specification PR is merged or explicitly approved per the gate configuration.

### REQ-ARCH-005: Specification validation

The Specification Generator MUST validate that all referenced modules exist in the repository (or are explicitly marked new) and that proposed dependency changes do not violate the project's architectural constraints.

---

## REQ-IFACE: Interface Design

### REQ-IFACE-001: Context from approved specification

The Interface Generator MUST build its LLM context from the approved specification document, existing interface conventions in the repository, and any cross-domain interface registry entries relevant to the affected modules.

### REQ-IFACE-002: Structured interface output

The Interface Generator MUST instruct the LLM to produce complete, syntactically valid interface definition files in the target domain's artifact format. Partial or template-placeholder files MUST be rejected.

### REQ-IFACE-003: Domain service validation

Generated interface files MUST be validated via the domain service's `validate` method before a PR is created. Validation failures MUST be fed back to the LLM as structured diagnostics for correction.

### REQ-IFACE-004: Retry budget

Each interface generation attempt MUST be counted against a configurable retry budget (default: 5). Exceeding the retry budget MUST trigger escalation with a summary of all attempts and their failure diagnostics.

### REQ-IFACE-005: Delivered as a Pull Request

Validated interface files MUST be delivered via a Pull Request. The PR MUST reference the parent work item and the specification PR.

### REQ-IFACE-006: Interface design gate

The pipeline MUST NOT advance to the planning stage until the interface design PR is merged or explicitly approved per the stage gate configuration.

### REQ-IFACE-007: Cross-domain registry consistency

If the affected modules declare cross-domain interfaces in `.cogworks/interfaces/`, the Interface Generator MUST verify that generated interface definitions are consistent with the declared registry contracts. Inconsistencies MUST be reported before PR creation.

---

## REQ-PLAN: Work Planning

### REQ-PLAN-001: Planning schema

The Work Planner MUST produce structured output: a list of sub-work-items each with a title, description, file list, interface references, test specification, and dependency references.

### REQ-PLAN-002: Sub-work-item minimum

A plan MUST contain at least one sub-work-item. A plan with zero sub-work-items is invalid and MUST be fed back to the LLM.

### REQ-PLAN-003: Dependency graph validation

The Work Planner MUST compute a topological ordering of sub-work-items. Circular dependencies MUST be detected, and the specific cycle MUST be returned as structured feedback to the LLM for replanning.

### REQ-PLAN-004: Granularity limit

The Work Planner MUST enforce a configurable maximum number of sub-work-items per work item (default: 10). Exceeding the limit MUST trigger escalation.

### REQ-PLAN-005: Sub-work-item issues

Each sub-work-item MUST be created as a GitHub Issue linked to the parent work item, with all required labels applied.

### REQ-PLAN-006: Interface coverage

The Work Planner MUST verify that every interface from the Interface Design stage is covered by at least one sub-work-item.

---

## REQ-CODE: Code Generation

### REQ-CODE-001: Sequential processing with prior context

Sub-work-items MUST be processed sequentially in topological dependency order. Each sub-work-item MUST receive the implementation outputs of all prior completed sub-work-items as part of its context.

### REQ-CODE-002: Structured feedback loop

CogWorks MUST feed structured domain service diagnostics (artifact, location, severity, message) back to the LLM on failure. Simulation failures that are not self-explanatory MUST be interpreted by an LLM before being included in retry context.

### REQ-CODE-003: Retry budget

Each sub-work-item MUST have a configurable maximum retry count (default: 5). Exceeding the retry budget MUST trigger escalation with a summary of all attempts and their failure reasons.

### REQ-CODE-004: Cost budget

The pipeline MUST track accumulated LLM token cost and halt when the pipeline budget is exceeded. The halt MUST include a per-stage, per-sub-work-item cost report.

### REQ-CODE-005: Context truncation

Context assembly MUST apply deterministic priority-based truncation when the assembled package would exceed the model's context window. The current sub-work-item's interface definition MUST never be removed regardless of truncation pressure.

---

## REQ-SCEN: Scenario Validation

### REQ-SCEN-001: Human-authored scenarios

Scenario specifications MUST be authored and maintained by humans. CogWorks MUST NOT create or modify scenario specifications.

### REQ-SCEN-002: Holdout principle

Scenario specification files MUST NOT be included in any code generation context package, regardless of their relevance to affected modules.

### REQ-SCEN-003: Multiple trajectories

Each scenario MUST be executed for the configured number of independent trajectories (default: 10). Each trajectory MUST start with fresh state.

### REQ-SCEN-004: Satisfaction scoring

A satisfaction score MUST be computed as the fraction of trajectories that satisfy acceptance criteria. Any trajectory that triggers an explicit failure criterion MUST cause immediate failure of the entire validation, regardless of the overall score.

### REQ-SCEN-005: Threshold enforcement

Scenario validation MUST fail when the overall satisfaction score falls below the configured threshold (default: 0.95).

### REQ-SCEN-006: Below-threshold remediation

When scenario validation fails, the failing scenario identifiers, trajectory observations, and failure details MUST be fed back to the Code Generator as structured context for remediation.

### REQ-SCEN-007: Applicable scenario selection

CogWorks MUST select only scenarios whose declared interface coverage overlaps with the interfaces implemented by the current sub-work-item. Sub-work-items with no applicable scenarios MUST skip scenario validation (not fail it).

### REQ-SCEN-008: Scenario availability is optional

The absence of scenario specifications for a sub-work-item MUST NOT be treated as a failure. Scenario validation is skipped silently when no applicable scenarios exist.

### REQ-SCEN-009: Scenario audit trail

Scenario validation results MUST be recorded in the audit trail, including: overall satisfaction score, per-scenario scores, trajectory count, failure details, and any explicit failure criteria that were triggered.

---

## REQ-REVIEW: Review Gate

### REQ-REVIEW-001: Review pass schema

Each LLM review pass MUST produce structured output: overall pass/fail, per-criterion findings, each with file reference, line number (where applicable), severity (blocking/warning/informational), and explanation.

### REQ-REVIEW-002: Four review passes

The review gate MUST execute four passes in order:

1. Deterministic cross-domain constraint validation (no LLM, no tokens)
2. Code quality LLM review (coding standards, idioms, error handling, naming, documentation)
3. Architecture compliance LLM review (matches spec, respects boundaries, no unplanned dependencies)
4. Security LLM review (input validation, auth boundaries, unsafe code, vulnerability patterns)

### REQ-REVIEW-003: Independent pass prompts

Each of the three LLM review passes MUST use a separate, focused prompt. Review passes MUST NOT be combined into a single LLM call.

### REQ-REVIEW-004: Blocking vs non-blocking aggregation

Any blocking finding in any review pass MUST prevent PR creation and trigger the remediation loop. Non-blocking findings (warning, informational) MUST be collected and posted as PR review comments.

### REQ-REVIEW-005: Remediation loop

Blocking findings MUST be fed back to the Code Generator as structured context for remediation. If blocking findings persist after the configured maximum remediation cycles (default: 3), escalation MUST be triggered.

### REQ-REVIEW-006: Safety-critical human approval

PRs for safety-critical work items MUST require explicit human approval before merge. CogWorks MUST NOT take any action that would merge a PR.

---

## REQ-INT: Integration (PR Creation)

### REQ-INT-001: Non-blocking findings as inline comments

Non-blocking review findings MUST be posted as inline review comments on the PR at the relevant file and line number.

### REQ-INT-002: PR traceability references

Every sub-work-item PR created by CogWorks MUST include references to: the sub-work-item issue, the parent work item, the specification PR, and the interface design PR.

---

## REQ-AUDIT: Audit Trail

### REQ-AUDIT-001: LLM call recording

Every LLM call MUST be recorded in the audit trail with: model name, input token count, output token count, latency, stage, and sub-work-item identifier (if applicable).

### REQ-AUDIT-002: State transition recording

Every pipeline state transition (stage entry, stage completion, gate evaluation) MUST be recorded in the audit trail.

### REQ-AUDIT-003: Failure reporting

When a stage fails, CogWorks MUST post a structured failure report as a GitHub issue comment and apply the `cogworks:stage:failed` label.

---

## REQ-BOUND: System Boundaries

### REQ-BOUND-001: No code execution within CogWorks

CogWorks MUST NOT execute generated code or LLM output within its own process. Code execution is delegated to domain services.

### REQ-BOUND-002: No PR merging

CogWorks MUST NOT merge, approve, close, or request changes on any Pull Request. PR lifecycle decisions belong to humans.

---

## REQ-DTU: Digital Twin Utility

### REQ-DTU-001: Twin specification format

Digital Twin specifications MUST be structured documents (TOML/YAML) describing the external dependency being modelled, the expected behavioral contracts, and the fidelity requirements.

### REQ-DTU-002: Twin lifecycle management

Twins MUST be programmatically startable and stoppable. CogWorks MUST be able to start a twin before scenario execution and stop it cleanly after.

### REQ-DTU-003: Twin state isolation

Each trajectory execution MUST start with a fresh twin state. Twin state MUST NOT persist between trajectories.

### REQ-DTU-004: Twin conformance testing

Each Digital Twin MUST include a conformance test suite that validates its behavior against the specification.

### REQ-DTU-005: Twin provisioning for scenarios

When a scenario specification requires a Digital Twin, the twin MUST be started before the first trajectory and stopped after the last trajectory.

---

## REQ-EXT: Extension API (Domain Services)

### REQ-EXT-001: Health check before invocation

CogWorks MUST perform a health check on a domain service before each pipeline run that requires it. Unhealthy services MUST be detected before any domain service method is called.

### REQ-EXT-002: Unsupported method handling

A domain service method invocation that the service does not support MUST return a clear error identifying the unsupported method. This MUST be treated as a non-retryable error.

### REQ-EXT-003: Response schema validation

All domain service responses MUST be validated against the Extension API JSON Schema before being used. Responses that do not conform MUST be rejected.

### REQ-EXT-004: Operation timeouts

Every Extension API method call MUST have a configurable timeout. Operations exceeding the timeout MUST be terminated and reported as failures (default: 10 minutes for `simulate`, 5 minutes for all other methods).

### REQ-EXT-005: Transport abstraction

The domain service communication layer MUST be transport-agnostic. Unix domain sockets are the default transport; HTTP/gRPC MUST be supported as an alternative.

### REQ-EXT-006: Long-running operation handling

The Extension API baseline is synchronous request-response with configurable timeouts. The protocol design MUST NOT preclude adding progress polling (via operation IDs) or streaming transport in a future API version. When progress polling is added, domain services that support it MUST declare the capability in their handshake response.

### REQ-EXT-007: Service availability policies

Primary domain service unavailability MUST halt the pipeline with a clear diagnostic. Secondary domain service unavailability (services that would only participate in cross-domain validation) MUST produce a warning and allow the pipeline to continue with that validation skipped.

### REQ-EXT-008: Structured diagnostics

All domain service diagnostic output MUST be structured data: artifact identifier, location (domain-specific JSON object), severity (`blocking`/`warning`/`informational`), category (from the standardised diagnostic category set), and message. CogWorks MUST NOT parse free-form text from domain services.

### REQ-EXT-009: API version compatibility

Domain services declaring an Extension API version incompatible with CogWorks MUST be rejected during health check. The rejection MUST identify the version mismatch clearly.

---

## REQ-XDOM: Cross-Domain Interface Registry

### REQ-XDOM-001: Human-authored only

Interface definitions in `.cogworks/interfaces/` MUST be authored and maintained by humans. CogWorks MUST NOT create or modify interface definitions autonomously. CogWorks MAY suggest additions as recommendations for human review.

### REQ-XDOM-002: Schema conformance

All interface definition files MUST conform to the published interface definition JSON Schema. Non-conformant files MUST be rejected with a clear error identifying the specific violation.

### REQ-XDOM-003: Computed constraints

Derived constraints (such as total bus load computed from message parameters) MUST be evaluated deterministically by the constraint validator. Computed constraint formulas MUST NOT be stored in the registry.

### REQ-XDOM-004: Version mismatch detection

The Interface Registry Manager MUST detect and report mismatches between the interface version declared in the registry and the interface version a domain service declares compatibility with.

### REQ-XDOM-005: Pre-pipeline validation

The interface registry MUST be validated on every pipeline run, before any stage executes. Registry validation failures MUST prevent any pipeline stage from running.

---

## REQ-XVAL: Cross-Domain Constraint Validation

### REQ-XVAL-001: Deterministic-first ordering

Cross-domain constraint validation MUST execute before any LLM review pass in the review gate. It MUST NOT consume LLM tokens.

### REQ-XVAL-002: Interface extraction

The constraint validator MUST use the domain service's `extract_interfaces` method to obtain actual interface values from generated artifacts before comparing against registry contracts.

### REQ-XVAL-003: Severity levels

Hard constraint violations (values outside declared min/max bounds) MUST produce blocking findings. Nominal deviations (within bounds but outside the declared nominal range) MUST produce warnings.

### REQ-XVAL-004: Single-service operation

Cross-domain constraint validation MUST be able to validate a single domain's artifacts against registry contracts without requiring other participating domains' services to be running.

### REQ-XVAL-005: Architecture-stage validation

Cross-domain constraint validation MUST also run during the architecture stage to catch violations before implementation begins.
