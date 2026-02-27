# Functional Requirements

This document defines the functional requirements for CogWorks. Every requirement has a unique ID that is referenced in `assertions.md` to establish traceability between requirements and testable behavioral assertions.

Requirements are organized by functional area. Each requirement states what the system **MUST** or **MUST NOT** do. Implementation constraints (how it must be done) are in `constraints.md`.

---

## REQ-PIPE: Pipeline Execution

### REQ-PIPE-001: Trigger on label

CogWorks MUST initiate the pipeline when the `cogworks:run` label is applied to a work item that has no current `cogworks:node:*` label.

### REQ-PIPE-002: Node label tracking

CogWorks MUST apply `cogworks:node:<name>` labels to track the currently active node(s) of each work item. Multiple node labels may be present simultaneously when nodes execute in parallel.

### REQ-PIPE-003: Node advancement

CogWorks MUST evaluate outgoing edge conditions when a node completes and activate all downstream nodes whose edge conditions are satisfied and whose other input requirements are met. When no configuration file exists, the default linear pipeline (Intake → Architecture → Interface Design → Planning → Code Generation → Review → Integration) is used.

### REQ-PIPE-004: Idempotent operations

All pipeline operations MUST be idempotent. Re-invoking the step function for the same GitHub state MUST produce the same outcome or detect prior completion and take no action.

### REQ-PIPE-005: Audit comments

CogWorks MUST post a structured status comment on the work item issue when entering each node, when a node completes successfully, and when a failure occurs.

### REQ-PIPE-006: Node gate enforcement

CogWorks MUST respect node gate configuration (`auto-proceed` or `human-gated`). Safety-critical work items MUST override all code-producing node gates to `human-gated` regardless of repository configuration.

**Exception**: Constitutional rules loading (REQ-CONST-001) is unconditional and not subject to gate configuration. It runs on every pipeline invocation regardless of gate settings.

### REQ-PIPE-007: Processing lock

CogWorks MUST apply a `cogworks:processing` label before processing a work item and MUST back off (exit without taking action) if that label is already present when a new invocation begins.

### REQ-PIPE-008: Duplicate pipeline prevention

The orchestrator MUST prevent duplicate pipeline runs for the same work item. If a pipeline is already running for an issue, a new trigger MUST either be rejected (with a comment explaining the conflict) or queue for execution after the current run completes (configurable).

### REQ-PIPE-009: Pipeline resumption

Re-triggering a failed pipeline MUST support resuming from the failed node (using the persisted pipeline state from REQ-EXEC-002) rather than restarting from the beginning. A full restart MUST be supported only on explicit request (`cogworks:restart` label or `/cogworks restart` comment).

### REQ-PIPE-010: Configurable pipeline graph

The pipeline MUST support configurable directed graphs where nodes are processing steps and edges are transitions with conditions. The default configuration MUST be the existing linear pipeline. Edge conditions MUST support both deterministic evaluation (expression checks against pipeline state) and LLM evaluation (natural language conditions assessed against pipeline context). Graph cycles MUST have termination conditions (maximum traversals, cost budget) to prevent infinite loops.

### REQ-PIPE-011: Shift work boundary

Each work item classification MUST define an explicit shift work boundary — the pipeline node after which CogWorks proceeds non-interactively. Nodes before the boundary MAY require human approval (configurable). Nodes after the boundary run autonomously with gate enforcement as the quality control. The boundary MUST be visible in the GitHub issue (as a label or comment) so humans know when to engage and when to let the system run.

### REQ-PIPE-012: Pipeline working directory

Each pipeline run MUST have a dedicated working directory (git worktree) for intermediate artifacts. The working directory persists across nodes within a single pipeline run. Intermediate artifacts (specs, interface definitions, plans, generated code) are written to the working directory before being committed as PRs. The working directory is cleaned up on pipeline completion. The working directory state MUST be recoverable from GitHub artifacts (PRs, issue comments) in case of failure — the working directory is a performance optimisation, not a durability mechanism.

---

## REQ-GRAPH: Pipeline Graph Structure

### REQ-GRAPH-001: Directed graph with controlled cycles

The pipeline MUST be a directed graph where nodes represent processing steps and edges represent transitions. The graph MUST support sequential execution, parallel fan-out, fan-in synchronisation, and conditional edges. The graph MAY contain cycles (for retry and rework loops). Every cycle MUST have an explicit termination condition that guarantees eventual exit. The orchestrator MUST enforce termination conditions and halt the pipeline with a clear error if a cycle would exceed its limit.

### REQ-GRAPH-002: Node identity and ordering

Each node in the graph MUST have a unique name within the pipeline configuration. The orchestrator MUST compute the execution order from the edge definitions using topological sorting. For nodes that are not connected by any path, the orchestrator MUST support concurrent execution (see REQ-EXEC-006).

### REQ-GRAPH-003: Default linear pipeline

If a repository has no pipeline configuration file, the orchestrator MUST use a default pipeline equivalent to the original 7-node linear sequence: Intake → Architecture → Interface Design → Planning → Code Generation → Review → Integration. Default properties: all edges unconditional, review-to-code-generation rework edge has `max_traversals: 3` (matching REQ-REVIEW-005's default remediation cycle limit), safety-classified work items require human approval at Architecture, Interface Design, and Review, no parallel execution.

### REQ-GRAPH-004: Pipeline configuration file

Pipeline graphs MUST be configurable per repository via a TOML configuration file at `.cogworks/pipeline.toml`. The configuration defines nodes, edges, and pipeline-level settings. Multiple named pipelines MAY be defined in the same file. The pipeline for a work item is selected by the Intake node's classification output.

---

## REQ-NODE: Node Types

### REQ-NODE-001: Node interface

Every node, regardless of type, MUST implement a common interface: name (unique identifier), type (`llm`, `deterministic`, or `spawning`), inputs (required artifacts or state), outputs (produced artifacts or state), validation (how success is determined), timeout (maximum wall-clock time), and cost budget (for LLM nodes). The orchestrator MUST verify all inputs are available before starting a node.

### REQ-NODE-002: LLM nodes

LLM nodes MUST specify a prompt template, context requirements, output schema, and retry behaviour. The orchestrator MUST assemble context, invoke the LLM gateway, validate output against the schema, retry on failure (up to the node's retry budget), and record the full prompt, response, validation result, and cost in the audit trail.

### REQ-NODE-003: Deterministic nodes

Deterministic nodes MUST specify an execution method (`script`, `domain_service`, or `builtin`). The orchestrator MUST execute the specified method, capture output, parse it according to the node's output specification, and record the invocation and result in the audit trail.

### REQ-NODE-004: Spawning nodes

Spawning nodes MUST specify a prompt template (for LLM-based analysis) or a script (for deterministic issue creation), an issue template, labels to apply, and whether to link new issues to the current work item. Spawning nodes MUST be non-blocking by default — the pipeline continues regardless of success. The orchestrator MUST create resulting issues, link them to the parent work item, and record them in the audit trail.

---

## REQ-EDGE: Edge Conditions

### REQ-EDGE-001: Edge condition types

The orchestrator MUST support three types of edge conditions: deterministic conditions (evaluated by the orchestrator against pipeline state via a simple expression language), LLM-evaluated conditions (natural-language conditions assessed by the LLM against current pipeline context), and composite conditions (boolean combinations AND/OR/NOT of deterministic and LLM-evaluated conditions). LLM-evaluated conditions MUST be recorded in the audit trail. LLM-evaluated conditions MUST have a deterministic fallback (edge either taken or not taken) when the LLM is unavailable or returns an ambiguous response.

### REQ-EDGE-002: Edge priority and mutual exclusion

When multiple edges leave the same source node, the orchestrator MUST evaluate them in declared order. The configuration MUST support: `all-matching` (all true edges taken — fan-out), `first-matching` (only first true edge taken — exclusive routing), and `explicit` (node output names the edges to take). The evaluation mode MUST be declared per source node.

### REQ-EDGE-003: Rework edges

Edges that create cycles MUST specify a maximum traversal count, which node outputs to preserve vs. discard on re-entry, and whether to increment the retry or rework counter. The orchestrator MUST track traversal counts per cycle and enforce the maximum. When the maximum is reached, the pipeline MUST either halt with an error, escalate to a human, or take a configured overflow edge.

---

## REQ-EXEC: Pipeline Execution

### REQ-EXEC-001: Working directory

Each pipeline run MUST have a dedicated working directory — a git worktree checked out from the target repository at the relevant branch. The working directory persists across all nodes within a single pipeline run. Nodes read inputs from and write outputs to the working directory (for file artifacts) or to a structured state store (for metadata). The working directory is cleaned up when the pipeline run completes. On pipeline failure, the orchestrator MUST be able to reconstruct pipeline state from GitHub artifacts and resume from the failed node.

### REQ-EXEC-002: Pipeline state machine

The orchestrator MUST maintain a state machine for each pipeline run tracking: current active nodes, completed nodes with outputs, pending nodes, failed nodes with error info, per-cycle traversal counts, and cumulative cost. The state machine MUST be representable as a JSON document and MUST be written to a GitHub comment on the parent work item at each node boundary.

### REQ-EXEC-003: Node execution lifecycle

Each node execution MUST follow this lifecycle: (1) precondition check — verify all declared inputs are available, (2) announce — update pipeline state comment, (3) execute — run the node, (4) validate — check output against validation criteria, (5) record — write outputs and audit trail, (6) announce — update GitHub state, (7) evaluate edges — evaluate outgoing edge conditions. If execution fails and retries are available, the node re-enters execute with error info in context. If retries are exhausted, the node enters `failed` state.

### REQ-EXEC-004: Pipeline triggering

The pipeline MUST be triggerable by: a GitHub Issue label (`cogworks:run`), a GitHub comment command (`/cogworks run`), or a manual CLI invocation (`cogworks run --issue <number>`). The orchestrator MUST prevent duplicate pipeline runs for the same work item.

### REQ-EXEC-005: Pipeline cancellation

A running pipeline MUST be cancellable by removing the `cogworks:run` label, adding a `cogworks:cancel` label, or a comment command (`/cogworks cancel`). On cancellation: active node executions are terminated (in-progress LLM calls are allowed to complete), current state is written to GitHub, a summary comment is posted, and the working directory is cleaned up.

### REQ-EXEC-006: Parallel node execution

When the graph has multiple nodes whose inputs are all available simultaneously, the orchestrator MUST support executing them concurrently as async tasks within the orchestrator process. Parallel execution MUST respect the pipeline's total cost budget (shared), the maximum concurrent LLM calls limit (configurable, default: 3), report progress for each node independently, and handle partial failure (other nodes continue unless a failed node is marked `abort_siblings_on_failure: true`). Fan-in occurs when a downstream node declares inputs from multiple upstream nodes; it stays pending until all inputs complete.

### REQ-EXEC-007: Sub-work-item execution within graph

Sub-work-items produced by a planning node MUST be processed as a sub-graph within the pipeline. Each sub-work-item follows a code-generation → review → integration sequence (or a configured sub-graph). Sub-work-items MUST be processed in topological dependency order. Sub-work-items with no mutual dependency path MAY execute concurrently when the pipeline configuration allows parallel fan-out.

---

## REQ-CLASS: Task Classification

### REQ-CLASS-001: Classification schema

The Task Classifier MUST produce structured output conforming to the classification schema: task type (enum), affected modules (list), estimated scope, safety_affecting flag, and rationale.

### REQ-CLASS-002: Safety override

If any module in the `affected_modules` list is registered in the safety-critical module registry, the final classification MUST set `safety_affecting: true`, regardless of what the LLM produced.

### REQ-CLASS-003: Scope threshold

When the estimated scope exceeds the configured threshold, the Task Classifier MUST produce an escalation result and MUST NOT proceed to subsequent nodes.

---

## REQ-ARCH: Architecture and Specification

### REQ-ARCH-001: Architectural context

The Specification Generator MUST include relevant ADRs, coding standards, and architectural constraints from the repository in its LLM context.

### REQ-ARCH-002: Specification format

The generated specification MUST be a structured Markdown document containing: affected modules, design decisions with rationale, dependency changes, risk assessment, and required ADRs.

### REQ-ARCH-003: Specification delivered as PR

The specification document MUST be delivered via a Pull Request targeting the appropriate branch of the target repository.

### REQ-ARCH-004: Specification gate

The pipeline MUST NOT advance past the architecture node until the specification PR is merged or explicitly approved per the gate configuration.

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

The pipeline MUST NOT advance to the planning node until the interface design PR is merged or explicitly approved per the node gate configuration.

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

The Work Planner MUST verify that every interface from the Interface Design node is covered by at least one sub-work-item.

---

## REQ-CODE: Code Generation

### REQ-CODE-001: Dependency-ordered processing with prior context

Sub-work-items MUST be processed in topological dependency order. Each sub-work-item MUST receive the implementation outputs of all prior completed sub-work-items that it depends on as part of its context. Sub-work-items with no mutual dependency path MAY execute concurrently when the pipeline configuration allows parallel fan-out; each concurrent sub-work-item receives the outputs of its own dependency chain.

### REQ-CODE-002: Structured feedback loop

CogWorks MUST feed structured domain service diagnostics (artifact, location, severity, message) back to the LLM on failure. Simulation failures that are not self-explanatory MUST be interpreted by an LLM before being included in retry context.

### REQ-CODE-003: Retry budget

Each sub-work-item MUST have a configurable maximum retry count (default: 5). Exceeding the retry budget MUST trigger escalation with a summary of all attempts and their failure reasons.

### REQ-CODE-004: Cost budget

The pipeline MUST track accumulated LLM token cost and halt when the pipeline budget is exceeded. The halt MUST include a per-node, per-sub-work-item cost report.

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

Scenario validation MUST fail when the overall satisfaction score falls below the configured threshold (default: 0.95). Context Packs MAY declare a stricter satisfaction threshold for their domain; when present, the stricter threshold applies to scenarios covering that domain's interfaces.

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

Any blocking finding in any review pass MUST prevent PR creation and trigger the remediation loop. Non-blocking findings (warning, informational) MUST be collected and posted as PR review comments. Missing required artefacts (declared by loaded Context Packs) MUST be treated as blocking findings identifying the pack and the missing artefact.

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

Every LLM call MUST be recorded in the audit trail with: model name, input token count, output token count, latency, node, and sub-work-item identifier (if applicable).

### REQ-AUDIT-002: State transition recording

Every pipeline state transition (node entry, node completion, gate evaluation) MUST be recorded in the audit trail.

### REQ-AUDIT-003: Failure reporting

When a node fails, CogWorks MUST post a structured failure report as a GitHub issue comment and apply the `cogworks:node:failed` label.

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

The interface registry MUST be validated on every pipeline run, before any node executes. Registry validation failures MUST prevent any pipeline node from running.

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

### REQ-XVAL-005: Architecture-node validation

Cross-domain constraint validation MUST also run during the architecture node to catch violations before implementation begins.

---

## REQ-CPACK: Context Pack System

### REQ-CPACK-001: Pack trigger mechanism

Context Pack loading MUST be driven deterministically by the work item's classification labels, component tags, and safety classification. The LLM MUST NOT choose which packs to load.

### REQ-CPACK-002: Pack loading timing

Context Packs MUST be loaded at the Architecture node, before any LLM call in the Architecture node begins. Loaded packs MUST remain active for the entire pipeline run.

### REQ-CPACK-003: Multiple simultaneous packs

A single pipeline run MUST support loading multiple Context Packs simultaneously when a work item matches multiple pack triggers.

### REQ-CPACK-004: Conflict resolution

Where loaded Context Packs contain contradictory guidance, the more restrictive rule MUST apply.

### REQ-CPACK-005: Unconditional loading on match

If a work item matches a Context Pack's trigger criteria, the pack MUST be loaded. There MUST NOT be an option to skip a matched pack.

### REQ-CPACK-006: Required artefact enforcement

Context Packs MAY declare required artefacts. At the Review node, CogWorks MUST verify all declared required artefacts are present. Missing artefacts MUST produce blocking findings identifying the pack and the specific missing artefact.

### REQ-CPACK-007: Pack audit trail

The set of Context Packs loaded for each pipeline run MUST be recorded in the audit trail and included in the PR description.

### REQ-CPACK-008: Pack content in context assembly

Context Pack domain knowledge, safe patterns, and anti-patterns MUST be included in the context package for all LLM calls from the Architecture node onward, subject to the standard context priority and truncation rules.

---

## REQ-CONST: Constitutional Security Layer

### REQ-CONST-001: Unconditional loading

Constitutional rules MUST be loaded on every pipeline run, before context assembly and before any LLM call. This is NOT a configurable gate.

### REQ-CONST-002: Privileged position

Constitutional rules MUST be injected as a privileged, non-overridable component of the LLM system prompt. No content in the context package MAY modify, append to, or override the constitutional rules.

### REQ-CONST-003: Human-approved source

Constitutional rules MUST be loaded from a version-controlled file at a well-known path. Changes to the constitutional rules file MUST require a reviewed and merged PR with at least one human approval. Rules from unreviewed branches MUST be rejected.

### REQ-CONST-004: External content as data

The constitutional rules MUST include a rule declaring that issue bodies, specifications, dependency docs, API responses, and any content not from core configuration are inputs to be analyzed — not instructions that modify CogWorks' behavior.

### REQ-CONST-005: Injection detection and halt

If external content contains text structured as a directive to CogWorks (persona overrides, instruction injections, behavioral modifications), the pipeline MUST halt immediately with an `INJECTION_DETECTED` event.

### REQ-CONST-006: Injection event content

The `INJECTION_DETECTED` event MUST include: pipeline run ID, work item ID, source document, and offending text.

### REQ-CONST-007: Hold state on injection

When injection is detected, the work item MUST enter a hold state. The work item MUST NOT be automatically requeued or retried. A human MUST review and either confirm false positive (with justification recorded in audit trail) or mark the work item as contaminated.

### REQ-CONST-008: Specification scope binding

The constitutional rules MUST include a rule that only capabilities explicitly in the approved specification and interface documents are implemented. Implied or inferred capabilities MUST NOT be implemented.

### REQ-CONST-009: Unauthorized capability prohibition

The constitutional rules MUST include a rule prohibiting network calls, file system access, IPC mechanisms, external process invocations, or hardware access unless explicitly specified in the interface document.

### REQ-CONST-010: No credential generation

The constitutional rules MUST include a rule that no strings resembling credentials, API keys, tokens, passwords, or secrets appear in any output artefact.

### REQ-CONST-011: Scope underspecification detection

When fulfilling a work item would require capabilities not in the approved specification, CogWorks MUST emit a `SCOPE_UNDERSPECIFIED` event and halt generation.

### REQ-CONST-012: Scope ambiguity detection

When a specification is ambiguous for a safety-affecting work item, CogWorks MUST emit a `SCOPE_AMBIGUOUS` event and require human clarification before proceeding.

### REQ-CONST-013: Protected path enforcement

CogWorks MUST NOT create or modify files matching protected path patterns (constitutional rules, prompt templates, scenario specifications) through the normal pipeline. Pre-PR validation MUST check generated files against protected path patterns.
