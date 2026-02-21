# Domain Vocabulary

This document defines the precise meaning of every domain concept in CogWorks. These definitions establish the language that all subsequent design, implementation, and documentation must use consistently.

---

## Pipeline Concepts

### Pipeline

The complete SDLC sequence from task intake to PR creation for a single work item.

- Identified by: The parent work item's GitHub Issue number
- Contains: A fixed sequence of stages (1-7)
- State: Represented entirely by GitHub labels on the parent work item
- Lifespan: From `cogworks:run` label application to `cogworks:stage:complete` or `cogworks:stage:failed`

### Stage

A discrete phase of the pipeline that produces a specific artifact.

- Identified by: Stage number (1-7) and name
- Contains: One or more actions (deterministic or LLM-assisted)
- State: Represented by `cogworks:stage:<name>` label on parent work item
- Constraint: Stages execute sequentially; a stage cannot begin until the prior stage's gate is passed

### Stage Gate

A configurable decision point between stages where the pipeline pauses for approval.

- Configuration: `auto-proceed` (continue immediately) or `human-gated` (wait for explicit approval)
- Scope: Configurable per-stage, per-repository
- Override: Safety-critical work items force `human-gated` for all code-producing stages regardless of configuration
- Represented by: `cogworks:awaiting-review` label when waiting

### Step Function

A single CLI invocation that reads GitHub state, determines the next action, executes it, and writes results back.

- Stateless: No carried state between invocations
- Idempotent: Re-invoking for the same state produces the same result (or detects prior completion)
- Atomic: Each invocation performs one logical action

---

## Work Item Concepts

### Work Item

A GitHub Issue that represents a unit of work to be implemented by CogWorks.

- Identified by: GitHub Issue number
- Contains: Title, description, optional structured fields (affected components, priority, type)
- Trigger: Pipeline starts when `cogworks:run` label is applied
- State labels: `cogworks:stage:<stage-name>`, `cogworks:safety-critical`, `cogworks:processing`, `cogworks:awaiting-review`

### Sub-Work-Item

A GitHub Issue created by CogWorks during the Planning stage, representing one implementation task within a larger work item.

- Identified by: GitHub Issue number
- Created by: The Planning stage (Stage 4)
- Contains: Title, description, file list, interface references, test specification, dependency references
- State labels: `cogworks:sub-work-item`, `cogworks:status:<status>`, `cogworks:depends-on:<issue>`, `cogworks:order:<n>`
- Constraint: Maximum configurable count per work item (default: 10)
- Constraint: Must represent logical changes, not individual files
- Ordering: Topologically sorted by declared dependencies

### Classification

The result of analyzing a work item to determine its type, scope, and safety impact.

- Contains: Task type (enum), affected modules (list), estimated scope, safety-affecting flag, rationale
- Task types: new feature, bug fix, refactor, configuration change, documentation, dependency update
- Determines: Which pipeline stages execute, which constraints apply, which gates are enforced

### Safety Classification

A determination of whether a work item touches safety-critical code paths.

- Determined by: Cross-referencing affected modules against the safety-critical module registry
- Override rule: If *any* affected module is in the registry, the work item is safety-affecting regardless of LLM classification
- Consequence: Forces human-gated transitions for all code-producing stages

### Scope Estimate

A rough measure of work item size used to decide whether human confirmation is needed.

- Contains: Estimated file count, estimated interface count, complexity rating
- Threshold: Configurable; exceeding triggers escalation

---

## Validation Concepts

### Scenario

A structured specification describing an end-to-end behavior or user story to validate against generated code.

- Contains: Natural-language description, preconditions, action sequence, acceptance criteria, optional failure criteria
- Format: Structured (TOML/YAML with schema)
- Stored: Separate from codebase (`.cogworks/scenarios/`), excluded from code generation context (holdout principle)
- Applicability: Each scenario declares which modules/interfaces it covers
- Purpose: Probabilistic validation across realistic situations, preventing overfitting to deterministic tests

### Trajectory

A single execution run of a scenario against generated code.

- Contains: Observed outputs, timing information, captured logs/metrics, satisfaction determination
- Multiple trajectories: Each scenario runs multiple times (configurable, default 10) to capture non-deterministic behavior
- Evaluation: Each trajectory is evaluated against scenario acceptance criteria

### Satisfaction Score

The fraction of scenario trajectories that satisfy acceptance criteria.

- Range: 0.0 to 1.0
- Threshold: Configurable minimum (default 0.95) required to pass scenario validation
- Failure criteria override: A single trajectory triggering an explicit failure criterion fails the entire validation regardless of score
- Reported: Per-scenario and overall, included in PR description and audit trail

### Digital Twin

A high-fidelity behavioral clone of an external dependency (API, hardware, network protocol) built and maintained by CogWorks.

- Purpose: Enable high-volume integration testing without rate limits, cost, or safety concerns of testing against real services
- Built via: Standard CogWorks pipeline (twins are work items)
- Contains: Conformance test suite validating twin behavior against specification
- Lifecycle: Versioned, maintained, updated when real dependency changes
- Used by: Scenario validation stage provisions twins when scenarios reference external dependencies
- Properties: Programmatically startable/stoppable, stateless between runs, supports failure injection

### Pyramid Summary Levels

Multi-level summaries of modules enabling efficient context assembly.

- **Level 1 (one-line)**: Module name + single-sentence purpose (10-20 tokens)
- **Level 2 (paragraph)**: Purpose, public interface summary, key dependencies, constraints (100-300 tokens)
- **Level 3 (full interface detail)**: Complete public interfaces with type signatures and documentation
- **Level 4 (source code)**: Full file contents (implicit level, used for files being directly modified)
- Cached: Stored in `.cogworks/summaries/`, regenerated when source changes
- Usage: Context Assembler selects appropriate level based on dependency distance

---

## Artifact Concepts

### Specification Document

A Markdown document produced by the Architecture stage describing what will be built and why.

- Contains: Affected modules, design decisions with rationale, dependency changes, risk assessment, required ADRs
- Delivered via: Pull Request (referencing the work item)
- Validated: All referenced modules must exist (or be marked new); dependency changes must not violate constraints

### Interface Definition

Source code files containing type signatures, trait definitions, and function signatures produced by the Interface Design stage.

- Language: Target language (Rust initially), not pseudocode
- Delivered via: Pull Request (referencing work item and spec PR)
- Validated: Must parse and type-check via language service

### Sub-Work-Item Plan

The set of sub-work-items produced by the Planning stage, with their dependency graph.

- Contains: One GitHub Issue per sub-work-item, each with file list, interface references, test specification, and dependency links
- Validated: Topological sort must succeed (no cycles), all interfaces must be covered, granularity limits respected
- Delivered via: GitHub Issues (linked to parent work item)

### Implementation Output

The code and tests produced by the Code Generation stage for a single sub-work-item.

- Contains: Source files (new or modified), test files, all passing deterministic checks
- Delivered via: Pull Request (referencing sub-work-item and parent work item)
- Validated: Compilation, type checking, formatting, linting, test execution (all via language service)

### Review Result

The structured output of the Review Gate for a single sub-work-item.

- Contains: Three separate review pass results (quality, architecture compliance, security)
- Per-pass: Overall pass/fail, per-criterion pass/fail, file/line references, severity, explanation
- Severity levels: Blocking, warning, informational
- Aggregation rule: Any blocking finding in any pass prevents PR creation

---

## Context and LLM Concepts

### Context Package

The assembled set of files, documentation, and constraints provided as input to an LLM call.

- Contents vary by stage but may include: specification, interface definitions, prior SWI outputs, ADRs, coding standards, architectural constraints, relevant source code
- Constraint: Must fit within target model's context window
- Truncation: Deterministic priority-based strategy when content exceeds window

### Context Priority Order

The deterministic ranking used to select context when the full package exceeds the model's window.

1. Current sub-work-item's interface definition (highest priority)
2. Directly depended-upon sub-work-item outputs
3. Architectural constraints
4. Coding standards
5. Remaining context by import-graph proximity (lowest priority)

### Prompt Template

A version-controlled Markdown file with variable placeholders that defines the instructions given to an LLM at a specific stage.

- Format: Markdown with `{{variable}}` placeholders
- Stored: In repository (version-controlled)
- Constraint: Never hardcoded in source code
- Contract: Each template declares its required variables and expected output schema

### Output Schema

A JSON Schema definition that specifies the structure of an LLM's response for a given stage.

- Purpose: Enables deterministic validation of LLM output before the pipeline proceeds
- Stored: In repository (version-controlled)
- Enforcement: Invalid outputs trigger automatic retry with validation error appended to context

---

## Infrastructure Concepts

### Domain Service

An external process providing domain-specific tooling capabilities. Domain services run as separate binaries from CogWorks and communicate through the Extension API.

- Capabilities: Validation, normalisation, rule review, simulation/testing, dependency validation, interface extraction, dependency graph computation
- Outputs: Always structured data in a common diagnostic format (pass/fail, artifact, location, severity, message)
- Execution: Separate process communicating over Unix domain socket (default) or HTTP/gRPC
- CogWorks is domain-ignorant: it does not interpret results beyond the structured output format
- Initial implementation: Rust domain service (software/firmware domain), shipped as a separate binary alongside CogWorks
- Extensibility: Any team can build a domain service (KiCad, FreeCAD, etc.) by implementing the Extension API

### Working Copy

A temporary local clone of the target repository used for domain service operations.

- Created: Shallow clone to a temporary directory when toolchain operations are needed
- Branch convention: `cogworks/<work-item-number>/<stage-slug>`
- Lifecycle: Created and destroyed within a single CLI invocation
- Not used for: Lightweight file reads (those use GitHub API)
- Management: Domain services are responsible for creating and managing working copies. CogWorks provides shared libraries that domain services can use for clone management and other common operations.

### Escalation

Transfer of control from CogWorks to a human reviewer when the system cannot resolve an issue within its budget.

- Triggers: Retry budget exceeded, cost budget exceeded, scope threshold exceeded, unresolvable review findings
- Mechanism: Issue comment with structured failure report + `cogworks:stage:failed` label
- Contains: All attempts, all failures, accumulated context at point of failure

### Cost Budget

A configurable limit on total LLM tokens consumed per pipeline run.

- Scope: Per-pipeline (across all stages and sub-work-items)
- Tracking: Accumulated in-memory during processing, written to GitHub as audit artifact on completion
- Enforcement: Pipeline halts immediately when budget exceeded
- Reporting: Per-stage and per-sub-work-item breakdown

### Audit Trail

A complete record of every decision, LLM call, validation result, and state transition in a pipeline run.

- Contents: LLM calls (model, input hash, output, tokens, latency), validation results, state transitions, total cost
- Storage: GitHub issue comments or linked artifacts
- Purpose: ISO 9001 traceability, systematic improvement, debugging

---

## GitHub State Concepts

### Processing Lock

A lightweight concurrency control using the `cogworks:processing` label.

- Applied: Before a CLI invocation starts processing a work item
- Checked: If already present, the invocation backs off (another instance is working on it)
- Removed: After the invocation completes its action
- Limitation: Race condition window between check and set; acceptable for expected concurrency levels

### Branch Convention

The naming pattern for git branches created by CogWorks.

- Pattern: `cogworks/<work-item-number>/<slug>`
- Slugs: `spec` (architecture), `interfaces` (interface design), `swi-<n>` (sub-work-item implementation)

---

## Cross-Domain Concepts

### Interface Registry

A version-controlled repository of cross-domain interface definitions, stored in `.cogworks/interfaces/`.

- Contains: Structured definitions of interfaces that span two or more domains
- Format: TOML files conforming to a published JSON Schema
- Authorship: MUST be authored and maintained by humans; CogWorks MUST NOT create or modify definitions autonomously
- Validation: Registry is validated deterministically on every pipeline run, before any pipeline stage executes
- Purpose: Provide a single source of truth for inter-domain contracts

### Interface Definition

A structured specification of a cross-domain interface contract.

- Identified by: Unique human-readable ID (e.g., `SWD-IF-CAN-01`)
- Contains: Interface type, participating domains, contract parameters, ownership declarations, version number
- Interface types: `bus_protocol`, `power_rail`, `mechanical_mounting`, `thermal_interface`, `connector`, `signal`, etc.
- Versioned: Version incremented on contract change; domain services declare compatible versions

### Interface Contract

The set of constraints that all participating domains must respect for a given cross-domain interface.

- Expressed as: Named parameters with values and tolerances
- Parameter types: Numeric with tolerance, numeric exact, enumerated, boolean, reference, structured (nested)
- Ownership: Each parameter has an owning domain (defines the constraint) and complying domains (must respect it)
- Enforcement: Validated deterministically by the constraint validator; no LLM involved

### Contract Parameter

A single constraint within an interface contract.

- Numeric with tolerance: Value with min/max bounds (e.g., `voltage: { nominal: 24.0, min: 21.6, max: 26.4, unit: "V" }`)
- Numeric exact: Value without tolerance (e.g., `baud_rate: { value: 500000, unit: "bps" }`)
- Enumerated: Value from a defined set (e.g., `connector_type: { value: "JST-PH-4", allowed: ["JST-PH-4", "JST-PH-6"] }`)
- Boolean: A flag (e.g., `termination_required: true`)
- Reference: Pointer to a detailed specification document
- Structured: Nested object for complex contracts (e.g., CAN message definitions)
- Computed constraints: Derived values (e.g., total bus load) are validated by deterministic checks in the constraint validator, not expressed as formulas in the registry

---

## Extension API Concepts

### Extension API

The protocol by which external domain services register with and are invoked by CogWorks.

- Transport: Unix domain sockets (default, for co-located services) or HTTP/gRPC (for remote services)
- Message format: JSON conforming to published JSON Schemas
- Versioned: CogWorks and domain services declare compatible API versions; incompatible versions rejected during handshake
- Schemas: Published in CogWorks repository (`schemas/extension-api/`)

### Domain Service Registration

The configuration by which a domain service declares its identity and capabilities to CogWorks.

- Declared in: `.cogworks/config.toml` under `[[domain_services]]`
- Contains: Service name, domain covered, socket/URL, supported capabilities, artifact types handled, interface types the service can validate against
- Multiple services: Multiple domain services may be registered simultaneously
- Selection: CogWorks routes operations to the appropriate service based on artifact types and domains

### Service Capability

A method that a domain service implements from the Extension API.

- Methods: `validate`, `normalise`, `review_rules`, `simulate`, `validate_deps`, `extract_interfaces`, `dependency_graph`
- Optional: Not all domain services need all methods (e.g., a service may support `validate` and `extract_interfaces` but not `normalise`)
- Discovery: CogWorks queries capabilities during health check handshake

### Domain Service Health Check

A mechanism for CogWorks to verify that a domain service is available and responsive.

- Includes: API version negotiation, capability discovery
- Timing: Checked before invoking any domain service method
- Failure handling: Primary domain service unavailable → pipeline halts; secondary domain service unavailable → pipeline continues with warning that cross-domain validation was skipped

### Progress Polling

The mechanism by which CogWorks monitors long-running domain service operations.

- Current design: Request/response with polling endpoint for progress updates
- Domain services may expose a progress endpoint alongside synchronous method invocation
- Future: Streaming responses may be added as an alternative transport; current design does not preclude this
- Example: `cogworks/42/spec`, `cogworks/42/interfaces`, `cogworks/42/swi-3`
- Cleanup: Branches deleted after PR merge
