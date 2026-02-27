# Behavioral Assertions

This document defines testable behavioral assertions for CogWorks. Each assertion follows a Given/When/Then structure and maps to specific requirements from the spec. These assertions guide the interface designer on error types, inform the planner on test coverage, and give the coder clear implementation targets.

---

## Pipeline State Machine

### ASSERT-PSM-001: Fresh work item enters intake

- **Given**: A work item with `cogworks:run` label and no `cogworks:node:*` label
- **When**: The step function processes this work item
- **Then**: The Task Classifier is invoked, and `cogworks:node:intake` label is applied
- **Traces to**: REQ-PIPE-001, REQ-PIPE-002

### ASSERT-PSM-002: Completed node activates downstream nodes

- **Given**: A work item at node N whose outgoing edge conditions are satisfied
- **When**: The step function processes this work item
- **Then**: All downstream nodes whose edge conditions are satisfied and inputs are available are activated; `cogworks:node:<downstream>` labels are applied
- **Traces to**: REQ-PIPE-003, REQ-EDGE-001

### ASSERT-PSM-003: Human-gated node waits for approval

- **Given**: A work item at node N with `human-gated` configuration, and the gate has not been approved
- **When**: The step function processes this work item
- **Then**: The step function exits without advancing; `cogworks:awaiting-review` label is present
- **Traces to**: REQ-PIPE-006

### ASSERT-PSM-004: Safety-critical work item forces human gates

- **Given**: A work item classified as safety-affecting, with auto-proceed configured for node N (a code-producing node)
- **When**: The step function evaluates the gate for node N
- **Then**: The gate behaves as human-gated regardless of configuration
- **Traces to**: REQ-PIPE-006, REQ-CLASS-002

### ASSERT-PSM-005: Processing lock prevents concurrent processing

- **Given**: A work item with `cogworks:processing` label already applied
- **When**: A second CLI invocation attempts to process this work item
- **Then**: The second invocation backs off without taking action
- **Traces to**: REQ-PIPE-007

### ASSERT-PSM-006: Failed node reports and halts

- **Given**: A node that fails (unrecoverable error, budget exceeded, or max escalation)
- **When**: The failure is reported
- **Then**: `cogworks:node:failed` label is applied, a structured failure report is posted as an issue comment, and the step function exits
- **Traces to**: REQ-PIPE-005, REQ-AUDIT-003

### ASSERT-PSM-007: Status update posted on node entry

- **Given**: A work item where a new node is activated
- **When**: The new node begins
- **Then**: A status comment is posted on the work item issue with the node name and summary
- **Traces to**: REQ-PIPE-005

### ASSERT-PSM-008: Default linear pipeline used when no configuration exists

- **Given**: A repository with no `.cogworks/pipeline.toml` file
- **When**: The pipeline executor loads the pipeline configuration
- **Then**: The default 7-node linear pipeline (Intake → Architecture → Interface Design → Planning → Code Generation → Review → Integration) is used
- **Traces to**: REQ-GRAPH-003, REQ-PIPE-003

### ASSERT-PSM-009: Pipeline resumes from failed node on re-trigger

- **Given**: A pipeline run that failed at node N with pipeline state persisted to GitHub
- **When**: The pipeline is re-triggered (label re-applied or CLI invocation)
- **Then**: The pipeline reconstructs state from GitHub and resumes from node N without re-executing completed nodes
- **Traces to**: REQ-PIPE-009, REQ-EXEC-002

### ASSERT-PSM-010: Pipeline cancellation terminates active nodes

- **Given**: A pipeline running with node N active
- **When**: The `cogworks:cancel` label is applied (or `/cogworks cancel` command issued)
- **Then**: Active node execution is terminated, current state is written to GitHub, a summary comment is posted, and the working directory is cleaned up
- **Traces to**: REQ-EXEC-005

### ASSERT-PSM-011: Duplicate pipeline prevention

- **Given**: A pipeline already running for work item #42
- **When**: A new trigger event arrives for work item #42
- **Then**: The new trigger is rejected with a comment explaining the conflict (or queued, per configuration)
- **Traces to**: REQ-PIPE-008

---

## Task Classification

### ASSERT-CLASS-001: LLM classification produces valid structured output

- **Given**: An issue body and list of repository modules
- **When**: The classification LLM call completes
- **Then**: The output matches the classification schema (type, affected_modules, estimated_scope, safety_affecting, rationale)
- **Traces to**: REQ-CLASS-001

### ASSERT-CLASS-002: Safety override applies when registry matches

- **Given**: An LLM classification that says `safety_affecting: false`, but one affected module is in the safety-critical registry
- **When**: The classification is cross-validated
- **Then**: The final classification has `safety_affecting: true`
- **Traces to**: REQ-CLASS-002

### ASSERT-CLASS-003: Safety override does not apply when no registry match

- **Given**: An LLM classification that says `safety_affecting: false`, and no affected modules are in the safety-critical registry
- **When**: The classification is cross-validated
- **Then**: The final classification has `safety_affecting: false`
- **Traces to**: REQ-CLASS-002

### ASSERT-CLASS-004: Scope exceeding threshold triggers escalation

- **Given**: An LLM classification with estimated scope exceeding the configured threshold
- **When**: The scope is evaluated
- **Then**: An escalation result is produced (not a proceed result)
- **Traces to**: REQ-CLASS-003

---

## Architecture (Specification)

### ASSERT-ARCH-001: Valid spec references pass validation

- **Given**: A generated specification that references only modules that exist in the repository (or are explicitly marked new)
- **When**: Deterministic validation runs
- **Then**: Validation passes
- **Traces to**: REQ-ARCH-005

### ASSERT-ARCH-002: Invalid module references fail validation

- **Given**: A generated specification that references a module that does not exist and is not marked new
- **When**: Deterministic validation runs
- **Then**: Validation fails with a specific error identifying the missing module
- **Traces to**: REQ-ARCH-005

### ASSERT-ARCH-003: Constraint-violating dependency changes fail validation

- **Given**: A generated specification that proposes a dependency change violating the project's architectural constraints
- **When**: Deterministic validation runs
- **Then**: Validation fails with a specific error identifying the violated constraint
- **Traces to**: REQ-ARCH-005

### ASSERT-ARCH-004: Validation failure triggers LLM retry

- **Given**: A specification that fails validation
- **When**: The validation error is processed
- **Then**: The LLM is re-invoked with the specific error messages appended to context
- **And**: Retry count is incremented
- **Traces to**: REQ-ARCH-005, REQ-CODE-003

---

## Planning

### ASSERT-PLAN-001: Acyclic dependencies produce valid topological order

- **Given**: A set of sub-work-items with acyclic dependencies
- **When**: Dependency ordering is computed
- **Then**: A valid topological ordering is produced where every sub-work-item appears after all its dependencies
- **Traces to**: REQ-PLAN-003

### ASSERT-PLAN-002: Cyclic dependencies are rejected

- **Given**: A set of sub-work-items with a circular dependency (A → B → C → A)
- **When**: Dependency ordering is attempted
- **Then**: An error is returned identifying the cycle
- **And**: The error is fed back to the LLM for replanning
- **Traces to**: REQ-PLAN-003

### ASSERT-PLAN-003: Granularity limit enforced

- **Given**: A plan with more sub-work-items than the configured maximum
- **When**: The plan is validated
- **Then**: An escalation result is produced (not proceed)
- **Traces to**: REQ-PLAN-004

### ASSERT-PLAN-004: Interface coverage verified

- **Given**: A plan where some interface from the Interface Design node is not covered by any sub-work-item
- **When**: The plan is validated
- **Then**: Validation fails with a specific error identifying the uncovered interface
- **Traces to**: REQ-PLAN-006

### ASSERT-PLAN-005: All interfaces covered

- **Given**: A plan where every interface from the Interface Design node is covered by at least one sub-work-item
- **When**: Interface coverage is checked
- **Then**: Validation passes
- **Traces to**: REQ-PLAN-006

---

## Code Generation

### ASSERT-CODE-001: Validation failure feeds structured errors back

- **Given**: Generated artifacts that fail domain service validation
- **When**: The domain service returns structured diagnostics
- **Then**: The diagnostics (artifact, location, severity, message) are appended to context and the LLM is re-invoked
- **Traces to**: REQ-CODE-002

### ASSERT-CODE-002: Simulation failure with self-explanatory error feeds directly

- **Given**: Generated artifacts that pass validation but fail simulation, where the failure type is in the "self-explanatory" heuristic set
- **When**: The simulation result is analyzed
- **Then**: The failure output is fed directly to the LLM without additional interpretation
- **Traces to**: REQ-CODE-002

### ASSERT-CODE-003: Simulation failure with non-obvious error triggers LLM interpretation

- **Given**: Generated artifacts that fail simulation, where the failure type is NOT in the "self-explanatory" heuristic set
- **When**: The simulation result is analyzed
- **Then**: An LLM is invoked to interpret the failure, and the interpretation is included in the retry context
- **Traces to**: REQ-CODE-002

### ASSERT-CODE-004: Retry budget exhaustion triggers escalation

- **Given**: A sub-work-item that has failed its maximum number of retries
- **When**: The retry budget check runs
- **Then**: Escalation is triggered with a summary of all attempts and their failure reasons
- **Traces to**: REQ-CODE-003

### ASSERT-CODE-005: Cost budget exhaustion halts pipeline

- **Given**: A pipeline where accumulated token cost plus the next call's estimated cost exceeds the budget
- **When**: The budget check runs before the LLM call
- **Then**: The call is refused, and the pipeline halts with a cost report
- **And**: The cost report includes per-node and per-sub-work-item breakdown
- **Traces to**: REQ-CODE-004

### ASSERT-CODE-006: Context truncation follows priority order

- **Given**: A context package that exceeds the model's context window
- **When**: Truncation is applied
- **Then**: Items are removed starting from the lowest priority tier; higher-priority items are retained
- **And**: The current sub-work-item's interface definition is never removed
- **Traces to**: REQ-CODE-005

### ASSERT-CODE-007: Sub-work-item receives prior outputs

- **Given**: Sub-work-item N that depends on sub-work-items A and B (both complete)
- **When**: Context is assembled for sub-work-item N
- **Then**: The outputs (code) of sub-work-items A and B are included in the context package
- **Traces to**: REQ-CODE-001

---

## Scenario Validation

### ASSERT-SCEN-001: Applicable scenarios are selected

- **Given**: A sub-work-item that implements interfaces I1 and I2, where scenarios S1, S2, and S3 cover I1, and scenario S4 covers I2
- **When**: Scenario validation begins
- **Then**: Scenarios S1, S2, S3, and S4 are selected for execution; other scenarios are not run
- **Traces to**: REQ-SCEN-007

### ASSERT-SCEN-002: Scenario specifications excluded from code generation context

- **Given**: Context assembly for code generation node
- **When**: The context package is built
- **Then**: No scenario specification files are included, even if they are relevant to the affected modules
- **Traces to**: REQ-SCEN-002 (holdout principle)

### ASSERT-SCEN-003: Multiple trajectories executed per scenario

- **Given**: Scenario S with configuration for 10 trajectories
- **When**: Scenario validation executes scenario S
- **Then**: Exactly 10 independent trajectory executions occur, each with fresh state
- **Traces to**: REQ-SCEN-003

### ASSERT-SCEN-004: Satisfaction score computed correctly

- **Given**: Scenario S with 10 trajectories where 9 satisfy acceptance criteria and 1 does not
- **When**: Satisfaction scoring occurs
- **Then**: The satisfaction score for scenario S is 0.9
- **Traces to**: REQ-SCEN-004

### ASSERT-SCEN-005: Threshold enforcement

- **Given**: Overall satisfaction score of 0.96 with threshold configured at 0.95
- **When**: Scenario validation decision is made
- **Then**: Validation passes
- **Traces to**: REQ-SCEN-004

### ASSERT-SCEN-006: Below-threshold score triggers remediation

- **Given**: Overall satisfaction score of 0.92 with threshold configured at 0.95
- **When**: Scenario validation decision is made
- **Then**: Validation fails, and failing scenario details are fed back to Code Generator
- **Traces to**: REQ-SCEN-006

### ASSERT-SCEN-007: Explicit failure criterion overrides score

- **Given**: Overall satisfaction score of 0.98 (above threshold), but one trajectory triggered an explicit failure criterion
- **When**: Scenario validation decision is made
- **Then**: Validation fails immediately
- **Traces to**: REQ-SCEN-004

### ASSERT-SCEN-008: No applicable scenarios skips validation

- **Given**: A sub-work-item with no applicable scenarios
- **When**: Scenario validation node is reached
- **Then**: Validation is skipped (not failed), and the pipeline proceeds to review gate
- **Traces to**: REQ-SCEN-007

### ASSERT-SCEN-009: Twin provisioning for scenarios

- **Given**: Scenario S requires Digital Twin T, and T is registered
- **When**: Scenario S is executed
- **Then**: Twin T is started before execution and stopped after execution
- **Traces to**: REQ-DTU-005

### ASSERT-SCEN-010: Scenario results in audit trail

- **Given**: Scenario validation completes
- **When**: Audit trail is written
- **Then**: Overall satisfaction score, per-scenario scores, trajectory count, and any failure details are included
- **Traces to**: REQ-SCEN-009

---

## Review Gate

### ASSERT-REVIEW-001: Blocking finding prevents PR creation

- **Given**: A review pass that produces at least one `blocking` finding
- **When**: Review results are aggregated
- **Then**: The aggregate result is `remediate` (not `proceed`)
- **And**: PR creation does not occur
- **Traces to**: REQ-REVIEW-004

### ASSERT-REVIEW-002: Non-blocking findings are preserved

- **Given**: Review passes with only `warning` and `informational` findings
- **When**: Review results are aggregated
- **Then**: The aggregate result is `proceed`
- **And**: All non-blocking findings are collected for PR comments
- **Traces to**: REQ-REVIEW-004

### ASSERT-REVIEW-003: All four review passes execute

- **Given**: Generated artifacts ready for review
- **When**: The review gate runs
- **Then**: Cross-domain constraint validation, code quality, architecture compliance, and security reviews all execute
- **And**: Results from all four are included in the aggregate
- **Traces to**: REQ-REVIEW-002, REQ-XVAL-001

### ASSERT-REVIEW-004: Blocking finding feeds back to code generation

- **Given**: A blocking review finding
- **When**: The review result is processed
- **Then**: The finding (file, line, severity, explanation) is fed back to the Code Generator for remediation
- **Traces to**: REQ-REVIEW-005

### ASSERT-REVIEW-005: Unresolved finding triggers escalation

- **Given**: A blocking finding that persists after the maximum number of remediation cycles
- **When**: The remediation cycle count is checked
- **Then**: Escalation is triggered
- **Traces to**: REQ-REVIEW-005

### ASSERT-REVIEW-006: Safety-critical PR requires human approval

- **Given**: A sub-work-item PR for a safety-affecting work item
- **When**: The PR is created
- **Then**: The system does NOT auto-approve; human approval is required before merge
- **Traces to**: REQ-REVIEW-006

---

## Idempotency

### ASSERT-IDEM-001: Duplicate PR creation is prevented

- **Given**: A specification PR already exists for work item #42
- **When**: The step function re-processes work item #42 at the architecture node
- **Then**: No duplicate PR is created; the existing PR is detected and used
- **Traces to**: REQ-PIPE-004

### ASSERT-IDEM-002: Duplicate sub-issue creation is prevented

- **Given**: Sub-work-item issues already exist for work item #42
- **When**: The step function re-processes work item #42 at the planning node
- **Then**: No duplicate issues are created; the existing issues are detected and used
- **Traces to**: REQ-PIPE-004

### ASSERT-IDEM-003: Label transitions are safe to repeat

- **Given**: Work item #42 already has `cogworks:node:architecture` label
- **When**: The step function attempts to set `cogworks:node:architecture` again
- **Then**: No error occurs; the operation is a no-op
- **Traces to**: REQ-PIPE-004

---

## Integration

### ASSERT-INT-001: PR references work item and sub-work-item

- **Given**: A sub-work-item PR being created
- **When**: The PR description is generated
- **Then**: It includes references to the sub-work-item issue, parent work item, specification PR, and interface PR
- **Traces to**: REQ-INT-002

### ASSERT-INT-002: Non-blocking findings appear as PR comments

- **Given**: A review gate that passed with warning and informational findings
- **When**: The PR is created
- **Then**: Each non-blocking finding is posted as an inline review comment at the relevant file and line
- **Traces to**: REQ-REVIEW-004, REQ-INT-001

### ASSERT-INT-003: Code PRs are never merged by CogWorks

- **Given**: A sub-work-item PR that has been created and approved
- **When**: CogWorks processes the work item
- **Then**: CogWorks does NOT merge the PR
- **Traces to**: REQ-BOUND-002

---

## Schema Validation

### ASSERT-SCHEMA-001: Invalid LLM output triggers retry

- **Given**: An LLM response that does not match the expected output schema
- **When**: Schema validation runs
- **Then**: The response is rejected, the validation error is appended to the prompt context, and the LLM is re-invoked
- **Traces to**: Design philosophy (Structured I/O at every boundary)

### ASSERT-SCHEMA-002: Valid LLM output proceeds

- **Given**: An LLM response that matches the expected output schema
- **When**: Schema validation runs
- **Then**: The validated, typed data is returned to the caller
- **Traces to**: Design philosophy (Structured I/O at every boundary)

---

## Cross-Domain Constraint Validation

### ASSERT-XVAL-001: Hard constraint violation blocks review

- **Given**: Generated artifacts where an extracted value (e.g., CAN bus load 0.55) exceeds the interface contract's maximum (0.50)
- **When**: Cross-domain constraint validation runs
- **Then**: A blocking finding is produced with the interface ID, parameter, expected range, actual value, and violating domain
- **Traces to**: REQ-XVAL-003

### ASSERT-XVAL-002: Nominal deviation produces warning

- **Given**: Generated artifacts where an extracted value deviates from nominal but stays within min/max bounds
- **When**: Cross-domain constraint validation runs
- **Then**: A warning finding is produced (not blocking)
- **Traces to**: REQ-XVAL-003

### ASSERT-XVAL-003: Constraint validation runs before LLM reviews

- **Given**: Generated artifacts ready for review
- **When**: The review gate begins
- **Then**: Cross-domain constraint validation executes first (deterministic, no tokens), before any LLM review pass
- **Traces to**: REQ-XVAL-001

### ASSERT-XVAL-004: Architecture node checks cross-domain constraints

- **Given**: A proposed architecture that would push CAN bus load over the declared maximum
- **When**: Architecture node validation runs
- **Then**: The constraint violation is caught before implementation begins
- **Traces to**: REQ-XVAL-005

### ASSERT-XVAL-005: Validation works with single domain service

- **Given**: Only the firmware domain service is registered, but the interface registry defines contracts between firmware and electrical
- **When**: Cross-domain constraint validation runs for firmware artifacts
- **Then**: Firmware artifacts are validated against the registry contracts without requiring the electrical domain service
- **Traces to**: REQ-XVAL-004

### ASSERT-XVAL-006: Computed constraints evaluated correctly

- **Given**: An interface contract defining max bus load of 0.50, and generated artifacts containing three CAN messages with known sizes and cycle times
- **When**: The constraint validator computes total bus load from message parameters
- **Then**: The computed bus load is compared against the max_bus_load contract parameter
- **Traces to**: REQ-XDOM-003

---

## Interface Registry

### ASSERT-XDOM-001: Registry validates on every pipeline run

- **Given**: A pipeline run starts
- **When**: Registry validation runs (before any node)
- **Then**: All interface definitions are checked against schema, cross-references validated, conflicts detected
- **Traces to**: REQ-XDOM-005

### ASSERT-XDOM-002: Invalid interface definition halts pipeline

- **Given**: An interface definition that doesn't conform to the schema
- **When**: Registry validation runs
- **Then**: Validation fails with a clear error identifying the malformed definition
- **And**: No pipeline nodes execute
- **Traces to**: REQ-XDOM-005

### ASSERT-XDOM-003: Version mismatch detected

- **Given**: A domain service declaring compatibility with interface v2, but the registry has v3
- **When**: Registry validation runs
- **Then**: A blocking finding is produced identifying the version mismatch
- **Traces to**: REQ-XDOM-004

---

## Extension API

### ASSERT-EXT-001: Domain service unavailability for primary domain halts pipeline

- **Given**: The primary domain service (covering the artifacts being generated) is registered but unavailable
- **When**: CogWorks attempts to invoke a domain service method
- **Then**: The pipeline halts with a clear error identifying the unavailable service
- **Traces to**: REQ-EXT-007

### ASSERT-EXT-002: Domain service unavailability for secondary domain continues with warning

- **Given**: A secondary domain service (would only participate in cross-domain validation) is registered but unavailable
- **When**: CogWorks processes a sub-work-item
- **Then**: The pipeline continues, but cross-domain validation for that domain is skipped
- **And**: A warning is reported in the PR and audit trail
- **Traces to**: REQ-EXT-007

### ASSERT-EXT-003: API version incompatibility rejected

- **Given**: A domain service implementing API v1, but CogWorks requires v2
- **When**: Health check / handshake occurs
- **Then**: The domain service is rejected with a clear version mismatch error
- **Traces to**: REQ-EXT-009

### ASSERT-EXT-004: Unsupported method produces clear error

- **Given**: A domain service that supports `validate` and `extract_interfaces` but not `normalise`
- **When**: CogWorks invokes `normalise` on that service
- **Then**: A clear error is returned indicating the method is not supported
- **And**: This is treated as a non-retryable error
- **Traces to**: REQ-EXT-002

---

## Context Pack System

### ASSERT-CPACK-001: Matched pack is always loaded

- **Given**: A work item whose classification labels match Context Pack P's trigger definition
- **When**: Context Pack selection runs at the Architecture node
- **Then**: Pack P is loaded (domain knowledge, safe patterns, anti-patterns, required artefacts)
- **And**: The loaded pack is recorded in the audit trail
- **Traces to**: REQ-CPACK-001, REQ-CPACK-005, REQ-CPACK-007

### ASSERT-CPACK-002: Unmatched pack is not loaded

- **Given**: A work item whose classification does not match Context Pack P's trigger definition
- **When**: Context Pack selection runs
- **Then**: Pack P is not loaded
- **And**: Pack P's content does not appear in any subsequent LLM context
- **Traces to**: REQ-CPACK-001

### ASSERT-CPACK-003: Multiple packs loaded simultaneously

- **Given**: A work item whose classification matches packs P1 and P2
- **When**: Context Pack selection runs
- **Then**: Both P1 and P2 are loaded and their combined content is incorporated into context assembly
- **Traces to**: REQ-CPACK-003

### ASSERT-CPACK-004: Conflicting guidance uses the more restrictive rule

- **Given**: Pack P1 sets satisfaction threshold at 0.97 and pack P2 sets it at 0.99 for overlapping domain interfaces
- **When**: Scenario validation runs for those interfaces
- **Then**: The stricter threshold (0.99) is applied
- **Traces to**: REQ-CPACK-004

### ASSERT-CPACK-005: Missing required artefact is a blocking finding

- **Given**: Pack P declares required artefact "unsafe usage justification" and the generated output does not contain it
- **When**: The Review node runs required artefact checking
- **Then**: A blocking finding is produced identifying pack P and the missing artefact
- **And**: PR creation does not occur
- **Traces to**: REQ-CPACK-006, REQ-REVIEW-004

### ASSERT-CPACK-006: Present required artefact is not a finding

- **Given**: Pack P declares required artefact A and the generated output contains A
- **When**: The Review node runs required artefact checking
- **Then**: No finding is produced for this artefact
- **Traces to**: REQ-CPACK-006

### ASSERT-CPACK-007: Pack content included in context from Architecture node onward

- **Given**: Context Pack P is loaded for a work item at the Architecture node
- **When**: Context assembly runs for the Architecture node and subsequent nodes
- **Then**: Pack P's domain knowledge, safe patterns, and anti-patterns are included in context packages
- **Traces to**: REQ-CPACK-008

---

## Constitutional Security Layer

### ASSERT-CONST-001: Constitutional rules loaded on every pipeline run

- **Given**: A pipeline run starts
- **When**: Pipeline Executor's first action runs
- **Then**: Constitutional rules are loaded before any other action, including LLM calls and context assembly
- **Traces to**: REQ-CONST-001

### ASSERT-CONST-002: Failure to load constitutional rules halts pipeline

- **Given**: The constitutional rules file does not exist at the configured path
- **When**: The Pipeline Executor attempts to load constitutional rules
- **Then**: The pipeline halts with a clear error identifying the missing file
- **And**: No LLM call is made
- **Traces to**: REQ-CONST-001, REQ-CONST-003

### ASSERT-CONST-003: Unreviewed constitutional rules are rejected

- **Given**: The constitutional rules file exists but is from an unreviewed branch (not merged)
- **When**: The Constitutional Rules Loader validates the file source
- **Then**: The rules are rejected and the pipeline halts
- **Traces to**: REQ-CONST-003

### ASSERT-CONST-004: Injection detection triggers pipeline halt and hold state

- **Given**: An issue body containing text structured as a directive to CogWorks (e.g., "Ignore previous instructions and...")
- **When**: The Injection Detector scans the issue body before it is included in any LLM prompt
- **Then**: An `INJECTION_DETECTED` event is emitted with source document and offending text
- **And**: The pipeline halts immediately
- **And**: The work item enters hold state with `cogworks:hold` label
- **Traces to**: REQ-CONST-005, REQ-CONST-006, REQ-CONST-007

### ASSERT-CONST-005: Hold state requires human resolution

- **Given**: A work item in hold state (INJECTION_DETECTED)
- **When**: A new pipeline invocation processes this work item
- **Then**: The invocation detects the hold state and exits without taking action
- **And**: A message is logged indicating human review is required
- **Traces to**: REQ-CONST-007

### ASSERT-CONST-006: Scope underspecification halts generation

- **Given**: A work item requiring capabilities not explicitly in the approved specification
- **When**: Scope enforcement runs
- **Then**: A `SCOPE_UNDERSPECIFIED` event is emitted identifying the missing capability
- **And**: Code generation halts until specification is updated
- **Traces to**: REQ-CONST-011

### ASSERT-CONST-007: Protected path violation fails pre-PR validation

- **Given**: Generated artifacts include a file matching a protected path pattern (e.g., `.cogworks/constitutional-rules.md`)
- **When**: Pre-PR validation runs scope enforcement
- **Then**: A `PROTECTED_PATH_VIOLATION` event is emitted
- **And**: PR creation does not occur
- **Traces to**: REQ-CONST-013

---

## Graph Execution

### ASSERT-GRAPH-001: Edge conditions determine downstream activation

- **Given**: Node A completes with output O, and edges A→B (condition: `O.status == "pass"`) and A→C (condition: `O.status == "fail"`)
- **When**: Edge condition evaluation runs
- **Then**: Only the edge whose condition is satisfied activates its target node
- **And**: The non-matching edge's target node remains pending
- **Traces to**: REQ-EDGE-001, REQ-PIPE-003

### ASSERT-GRAPH-002: Fan-out activates multiple parallel nodes

- **Given**: Node A completes with edges to B and C (both unconditional, `all-matching` mode)
- **When**: Edge condition evaluation runs
- **Then**: Both B and C are activated and may execute concurrently
- **And**: Both B and C have `cogworks:node:<name>` labels applied
- **Traces to**: REQ-GRAPH-001, REQ-EXEC-006

### ASSERT-GRAPH-003: Fan-in waits for all upstream nodes

- **Given**: Node D declares inputs from nodes B and C; node B has completed but C has not
- **When**: The step function evaluates D's readiness
- **Then**: Node D remains pending (not activated)
- **And**: When C later completes, the next step function invocation activates D
- **Traces to**: REQ-EXEC-006

### ASSERT-GRAPH-004: Rework edge tracks traversal count

- **Given**: A rework edge from Review→CodeGen with max_traversals: 3, and 2 prior traversals
- **When**: Review fails and the rework edge condition is true
- **Then**: CodeGen is re-activated (traversal count becomes 3)
- **And**: On the next Review failure, the rework edge is NOT taken (max reached) and the overflow action (escalate) is triggered
- **Traces to**: REQ-EDGE-003

### ASSERT-GRAPH-005: Cycle without termination condition is rejected at configuration load

- **Given**: A pipeline configuration with node A→B→A (cycle) but no `max_traversals` on any edge in the cycle
- **When**: The Pipeline Configuration Manager validates the graph
- **Then**: Validation fails with an error identifying the unterminated cycle
- **And**: The pipeline does not start
- **Traces to**: REQ-GRAPH-001, REQ-GRAPH-004

### ASSERT-GRAPH-006: LLM-evaluated edge condition falls back on LLM failure

- **Given**: An LLM-evaluated edge condition with fallback `edge_taken: false`
- **When**: The LLM call fails (timeout, API error, etc.)
- **Then**: The fallback is applied (edge not taken)
- **And**: The fallback application is recorded in the audit trail
- **Traces to**: REQ-EDGE-001

### ASSERT-GRAPH-007: Parallel execution respects max concurrent LLM calls

- **Given**: Nodes B, C, D, and E are all eligible for concurrent execution (all are LLM nodes), and max concurrent LLM calls is configured as 3
- **When**: The Graph Execution Engine schedules node execution
- **Then**: At most 3 nodes execute LLM calls simultaneously; the 4th waits until one completes
- **Traces to**: REQ-EXEC-006

### ASSERT-GRAPH-008: Parallel cost budget is atomic

- **Given**: Nodes B and C executing in parallel, pipeline cost budget remaining is 100 tokens, B estimates 80 tokens, C estimates 80 tokens
- **When**: Both nodes attempt to acquire budget simultaneously
- **Then**: Only one node's LLM call is approved; the other is denied (budget exceeded)
- **And**: The denied node receives a budget exceeded error
- **Traces to**: REQ-EXEC-006, REQ-CODE-004

### ASSERT-GRAPH-009: Pipeline state written to GitHub at each node boundary

- **Given**: Node A completes successfully
- **When**: The pipeline executor processes node A's completion
- **Then**: The full pipeline state (active/completed/pending/failed nodes, traversal counts, cumulative cost) is written as a JSON document to a GitHub comment
- **Traces to**: REQ-EXEC-002

### ASSERT-GRAPH-010: Spawning node is non-blocking

- **Given**: A spawning node S in the pipeline graph with a downstream edge to node T
- **When**: Spawning node S completes (whether issues were created or not)
- **Then**: Downstream node T is activated; spawning node completion does not depend on created issue outcomes
- **Traces to**: REQ-NODE-004

### ASSERT-GRAPH-011: Pipeline configuration selects named pipeline from classification

- **Given**: Pipeline configuration defines pipelines named "standard" and "hotfix", and intake classification outputs `pipeline: "hotfix"`
- **When**: The Pipeline Configuration Manager selects the pipeline
- **Then**: The "hotfix" pipeline graph is used for the remainder of the run
- **Traces to**: REQ-GRAPH-004

### ASSERT-GRAPH-012: Partial parallel failure continues other nodes

- **Given**: Nodes B and C executing in parallel; node B fails with `abort_siblings_on_failure: false`
- **When**: Node B reports failure
- **Then**: Node C continues executing; node B enters `failed` state
- **And**: Downstream nodes that depend only on C can still proceed
- **Traces to**: REQ-EXEC-006

### ASSERT-GRAPH-013: Abort-siblings parallel failure stops sibling nodes

- **Given**: Nodes B and C executing in parallel; node B fails with `abort_siblings_on_failure: true`
- **When**: Node B reports failure
- **Then**: Node C is terminated (in-progress LLM calls allowed to complete)
- **And**: The pipeline enters an error state for the failed sub-graph
- **Traces to**: REQ-EXEC-006
